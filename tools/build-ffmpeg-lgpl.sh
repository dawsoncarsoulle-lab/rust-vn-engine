#!/usr/bin/env bash
# Reproducible, dynamically linked, decoder-only WebM runtime. No GPL/nonfree
# libraries or automatically detected system codecs enter the distribution.
set -euo pipefail
version=9.0.2
platform=${1:-linux}
task_script=$(realpath "$0")
case "$platform" in
  linux) cross=() ;;
  windows) cross=(--target-os=mingw32 --arch=x86_64 --enable-cross-compile --cross-prefix=x86_64-w64-mingw32- --extra-ldflags=-static-libgcc) ;;
  *) printf 'Expected linux or windows\n' >&2; exit 2 ;;
esac
# FFmpeg's generated config.sh does not quote its prefix. Avoid producing
# silently broken .pc files when the project itself lives in a spaced path.
prefix="${RVN_FFMPEG_PREFIX:-${XDG_CACHE_HOME:-$HOME/.cache}/rust-vn/ffmpeg-$platform-$version}"
if [[ "$prefix" == *[[:space:]]* || "$prefix" != /* ]]; then
  printf 'RVN_FFMPEG_PREFIX must be an absolute path without whitespace\n' >&2; exit 2
fi
sources="$prefix/sources"
mkdir -p "$sources"
# MinGW links its startup/runtime code, and this build uses static libgcc.
# Preserve their independent notices and GCC Runtime Library Exception too.
runtime_notices=()
if [[ "$platform" == windows ]]; then
  gcc_notice="${RVN_MINGW_GCC_NOTICE:-/usr/share/doc/gcc-mingw-w64-x86-64-win32-runtime/copyright}"
  mingw_notice="${RVN_MINGW_NOTICE:-/usr/share/doc/mingw-w64-common/copyright}"
  gpl_notice="${RVN_GPL3_NOTICE:-/usr/share/common-licenses/GPL-3}"
  for notice in "$gcc_notice" "$mingw_notice" "$gpl_notice"; do
    if [[ ! -s "$notice" ]]; then printf 'Required MinGW license is missing: %s\n' "$notice" >&2; exit 2; fi
  done
  cp "$gcc_notice" "$sources/GCC-RUNTIME-NOTICES.txt"
  cp "$mingw_notice" "$sources/MINGW-NOTICES.txt"
  cp "$gpl_notice" "$sources/COPYING.GPLv3"
  runtime_notices=(GCC-RUNTIME-NOTICES.txt MINGW-NOTICES.txt COPYING.GPLv3)
fi
archive="ffmpeg-$version.tar.xz"
for file in "$archive" "$archive.asc"; do
  if [[ ! -f "$sources/$file" ]]; then
    curl --fail --location --proto '=https' --tlsv1.2 "https://ffmpeg.org/releases/$file" -o "$sources/$file"
  fi
done
keyring=$(mktemp -d /tmp/rvn-ffmpeg-keyring-XXXXXXXX)
curl --fail --location --proto '=https' --tlsv1.2 https://ffmpeg.org/ffmpeg-devel.asc -o "$keyring/signing-key.asc"
gpg --batch --homedir "$keyring" --import "$keyring/signing-key.asc"
gpg --batch --homedir "$keyring" --with-colons --fingerprint | awk -F: '$1=="fpr" {print $10}' | grep -qx FCF986EA15E6E293A5644F10B4322F04D67658D8
gpg --batch --homedir "$keyring" --verify "$sources/$archive.asc" "$sources/$archive"
mkdir -p "$prefix/build"
if [[ ! -d "$prefix/build/ffmpeg-$version" ]]; then
  tar -xJf "$sources/$archive" -C "$prefix/build"
fi
cd "$prefix/build/ffmpeg-$version"
./configure --prefix="$prefix" "${cross[@]}" \
  --disable-autodetect --disable-everything --disable-network \
  --disable-gpl --disable-nonfree --disable-version3 \
  --enable-shared --disable-static --disable-programs --disable-doc \
  --disable-debug --disable-x86asm \
  --enable-avcodec --enable-avformat --enable-avutil \
  --disable-avdevice --disable-avfilter \
  --enable-swscale --enable-swresample \
  --enable-protocol=file --enable-demuxer=matroska \
  --enable-decoder=vp8,vorbis --enable-parser=vp8,vorbis
# Keep the complete configure output and original source alongside the binary.
cp ffbuild/config.log "$sources/config.log"
cp COPYING.LGPLv2.1 "$sources/COPYING.LGPLv2.1"
cp "$task_script" "$sources/build-ffmpeg-lgpl.sh"
make -j "${RVN_BUILD_JOBS:-4}"
make install
(cd "$sources"; sha256sum "$archive" "$archive.asc" COPYING.LGPLv2.1 config.log "${runtime_notices[@]}") > "$sources/SHA256SUMS"
case "$platform" in
  linux) (cd "$prefix/lib"; sha256sum libavcodec.so.63 libavformat.so.63 libavutil.so.61 libswscale.so.10 libswresample.so.7) > "$sources/LIBRARY-SHA256SUMS" ;;
  windows) (cd "$prefix/bin"; sha256sum avcodec-63.dll avformat-63.dll avutil-61.dll swscale-10.dll swresample-7.dll) > "$sources/LIBRARY-SHA256SUMS" ;;
esac
printf '\nFFmpeg LGPL shared runtime: %s\n' "$prefix"
