//! Transfer the audited shared decoder AND its corresponding source to an
//! exported game. Never fall back to an arbitrary system FFmpeg installation.
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
const LIBS_LINUX: [&str; 5] = [
    "libavcodec.so.63",
    "libavformat.so.63",
    "libavutil.so.61",
    "libswscale.so.10",
    "libswresample.so.7",
];
const LIBS_WINDOWS: [&str; 5] = [
    "avcodec-63.dll",
    "avformat-63.dll",
    "avutil-61.dll",
    "swscale-10.dll",
    "swresample-7.dll",
];
const SOURCES: [&str; 6] = [
    "ffmpeg-9.0.2.tar.xz",
    "ffmpeg-9.0.2.tar.xz.asc",
    "COPYING.LGPLv2.1",
    "config.log",
    "SHA256SUMS",
    "LIBRARY-SHA256SUMS",
];
const WINDOWS_NOTICES: [&str; 3] = [
    "GCC-RUNTIME-NOTICES.txt",
    "MINGW-NOTICES.txt",
    "COPYING.GPLv3",
];
fn verify_manifest(root: &Path, manifest: &Path, names: &[&str]) -> Result<()> {
    use std::io::Read;
    let entries = fs::read_to_string(manifest)?;
    anyhow::ensure!(
        entries.len() <= 16 * 1024,
        "Video integrity manifest exceeds its limit"
    );
    let mut seen = std::collections::BTreeSet::new();
    for line in entries.lines() {
        let (expected, name) = line
            .split_once("  ")
            .context("Invalid video integrity manifest entry")?;
        anyhow::ensure!(
            names.contains(&name) && seen.insert(name),
            "Unknown or repeated video integrity entry: {name}"
        );
        anyhow::ensure!(
            expected.len() == 64 && expected.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Invalid video integrity digest"
        );
        let mut file = fs::File::open(bounded_file(root, name, 100 * 1024 * 1024)?)?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        anyhow::ensure!(
            format!("{:x}", digest.finalize()).eq_ignore_ascii_case(expected),
            "Video distribution integrity check failed for {name}"
        );
    }
    anyhow::ensure!(
        seen.len() == names.len(),
        "Video integrity manifest is incomplete"
    );
    Ok(())
}
fn bounded_file(root: &Path, name: &str, limit: u64) -> Result<PathBuf> {
    let root = root.canonicalize()?;
    let file = root
        .join(name)
        .canonicalize()
        .with_context(|| format!("Required video distribution file '{name}' is missing"))?;
    anyhow::ensure!(
        file.starts_with(&root),
        "Video distribution symlink escapes its directory: {name}"
    );
    let data = fs::metadata(&file)?;
    anyhow::ensure!(
        data.is_file() && data.len() > 0 && data.len() <= limit,
        "Video distribution file is empty or exceeds its size limit: {name}"
    );
    Ok(file)
}
pub fn copy_from(
    libraries: &Path,
    sources: &Path,
    destination: &Path,
    windows: bool,
) -> Result<()> {
    let config = fs::read_to_string(bounded_file(sources, "config.log", 16 * 1024 * 1024)?)?;
    let flags: Vec<_> = config
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect();
    for required in [
        "--disable-gpl",
        "--disable-nonfree",
        "--disable-version3",
        "--enable-shared",
        "--disable-static",
        "--disable-autodetect",
    ] {
        anyhow::ensure!(
            flags.contains(&required),
            "Video decoder build is not the audited LGPL shared configuration: missing {required}"
        );
    }
    for forbidden in ["--enable-gpl", "--enable-nonfree", "--enable-version3"] {
        anyhow::ensure!(
            !flags.contains(&forbidden),
            "Video decoder configuration includes forbidden component {forbidden}"
        );
    }
    let libs = if windows { &LIBS_WINDOWS } else { &LIBS_LINUX };
    // Validate every input before starting to write the destination.
    let binaries: Vec<_> = libs
        .iter()
        .map(|name| bounded_file(libraries, name, 64 * 1024 * 1024))
        .collect::<Result<_>>()?;
    let mut source_names = SOURCES.to_vec();
    if windows {
        source_names.extend(WINDOWS_NOTICES);
    }
    let original: Vec<_> = source_names
        .iter()
        .map(|name| bounded_file(sources, name, 100 * 1024 * 1024))
        .collect::<Result<_>>()?;
    let mut hashed_sources = SOURCES[..4].to_vec();
    if windows {
        hashed_sources.extend(WINDOWS_NOTICES);
    }
    verify_manifest(
        sources,
        &bounded_file(sources, "SHA256SUMS", 16 * 1024)?,
        &hashed_sources,
    )?;
    verify_manifest(
        libraries,
        &bounded_file(sources, "LIBRARY-SHA256SUMS", 16 * 1024)?,
        libs,
    )?;
    let output = if windows {
        destination.to_owned()
    } else {
        destination.join("lib")
    };
    let notices = destination.join("rust-vn-notices/ffmpeg");
    fs::create_dir_all(&output)?;
    fs::create_dir_all(&notices)?;
    for (name, path) in libs.iter().zip(binaries) {
        fs::copy(path, output.join(name))?;
    }
    for (name, path) in source_names.iter().zip(original) {
        fs::copy(path, notices.join(name))?;
    }
    fs::write(
        notices.join("build-ffmpeg-lgpl.sh"),
        include_str!("../../tools/build-ffmpeg-lgpl.sh"),
    )?;
    fs::write(notices.join("README.txt"),"FFmpeg 9.0.2, dynamically linked, LGPL 2.1 or later.\nBuilt without GPL, nonfree or version3 FFmpeg components. VP8/Vorbis playback only.\nThe original corresponding source, license and complete configure log are included.\nTo rebuild, run: bash build-ffmpeg-lgpl.sh linux (or windows with MinGW installed).\nNo FFmpeg code has been modified. The same build script is included.\nYou may replace the shared libraries with ABI-compatible builds; do not remove these notices.\nThe Windows build also contains MinGW runtime code and libgcc under their separate notices, including GPLv3 with the GCC Runtime Library Exception 3.1; this exception does not relicense the game as GPL.\nFFmpeg is an independent project: https://ffmpeg.org/\n")?;
    Ok(())
}
pub fn copy_for_runtime(runtime: &Path, destination: &Path) -> Result<()> {
    let root = runtime
        .parent()
        .context("Video runtime has no parent directory")?;
    let windows = cfg!(windows);
    let libraries = if windows {
        root.to_owned()
    } else {
        root.join("lib")
    };
    // The editor and an exported game have different notice layouts. Both
    // carry the same verified decoder and corresponding source; neither needs
    // the developer's FFMPEG_DIR at runtime.
    for sources in [
        root.join("rust-vn-notices/ffmpeg"),
        root.join("video-notices"),
    ] {
        if sources.is_dir() {
            return copy_from(&libraries, &sources, destination, windows);
        }
    }
    let prefix=std::env::var_os("FFMPEG_DIR").map(PathBuf::from).context("Video export needs the packaged LGPL runtime and sources, or FFMPEG_DIR for an audited development build")?;
    anyhow::ensure!(prefix.is_absolute(), "FFMPEG_DIR must be absolute");
    copy_from(
        &prefix.join(if windows { "bin" } else { "lib" }),
        &prefix.join("sources"),
        destination,
        windows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path, windows: bool) {
        let libs = if windows { &LIBS_WINDOWS } else { &LIBS_LINUX };
        let libraries = if windows {
            root.to_owned()
        } else {
            root.join("lib")
        };
        let sources = root.join("video-notices");
        fs::create_dir_all(&libraries).unwrap();
        fs::create_dir_all(&sources).unwrap();
        let mut manifest = String::new();
        for name in libs {
            let bytes = format!("test decoder {name}");
            fs::write(libraries.join(name), &bytes).unwrap();
            manifest.push_str(&format!("{:x}  {name}\n", Sha256::digest(bytes.as_bytes())));
        }
        fs::write(sources.join("LIBRARY-SHA256SUMS"), manifest).unwrap();
        let mut names = SOURCES[..4].to_vec();
        if windows {
            names.extend(WINDOWS_NOTICES);
        }
        let mut manifest = String::new();
        for name in names {
            let bytes = if name == "config.log" {
                "# ./configure --disable-gpl --disable-nonfree --disable-version3 --enable-shared --disable-static --disable-autodetect\n".into()
            } else {
                format!("test source {name}")
            };
            fs::write(sources.join(name), &bytes).unwrap();
            manifest.push_str(&format!("{:x}  {name}\n", Sha256::digest(bytes.as_bytes())));
        }
        fs::write(sources.join("SHA256SUMS"), manifest).unwrap();
    }
    #[test]
    fn editor_bundle_exports_video_without_a_development_prefix() {
        let root = std::env::temp_dir().join(format!(
            "rvn-video-editor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fixture(&root, cfg!(windows));
        let out = root.join("export");
        copy_for_runtime(
            &root.join(if cfg!(windows) {
                "rvn_bevy.exe"
            } else {
                "rvn_bevy"
            }),
            &out,
        )
        .unwrap();
        assert!(out
            .join("rust-vn-notices/ffmpeg/ffmpeg-9.0.2.tar.xz")
            .is_file());
        let libraries = if cfg!(windows) {
            out.clone()
        } else {
            out.join("lib")
        };
        for name in if cfg!(windows) {
            LIBS_WINDOWS
        } else {
            LIBS_LINUX
        } {
            assert!(libraries.join(name).is_file());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_runtime_notices_are_required_and_integrity_checked_before_export() {
        let root = std::env::temp_dir().join(format!(
            "rvn-video-windows-notices-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fixture(&root, true);
        let sources = root.join("video-notices");
        let out = root.join("export");
        fs::write(sources.join("MINGW-NOTICES.txt"), "changed notice").unwrap();
        assert!(copy_from(&root, &sources, &out, true).is_err());
        assert!(!out.exists());
        fixture(&root, true);
        copy_from(&root, &sources, &out, true).unwrap();
        for name in WINDOWS_NOTICES {
            assert!(out.join("rust-vn-notices/ffmpeg").join(name).is_file());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn integrity_rejects_changed_libraries_unknown_paths_and_incomplete_manifests() {
        let root = std::env::temp_dir().join(format!(
            "rvn-video-integrity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("clip.dll"), b"audited library").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"audited library"));
        let manifest = root.join("manifest");
        fs::write(&manifest, format!("{digest}  clip.dll\n")).unwrap();
        verify_manifest(&root, &manifest, &["clip.dll"]).unwrap();
        fs::write(root.join("clip.dll"), b"changed").unwrap();
        assert!(verify_manifest(&root, &manifest, &["clip.dll"]).is_err());
        fs::write(&manifest, format!("{digest}  ../outside.dll\n")).unwrap();
        assert!(verify_manifest(&root, &manifest, &["clip.dll"]).is_err());
        fs::write(&manifest, "").unwrap();
        assert!(verify_manifest(&root, &manifest, &["clip.dll"]).is_err());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn incomplete_or_gpl_distribution_is_rejected_before_creating_an_export() {
        let root = std::env::temp_dir().join(format!(
            "rvn-video-dist-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("sources")).unwrap();
        fs::write(
            root.join("sources/config.log"),
            "# ./configure --enable-gpl\n",
        )
        .unwrap();
        let out = root.join("output");
        assert!(copy_from(&root, &root.join("sources"), &out, false).is_err());
        assert!(!out.exists());
        fs::write(root.join("sources/config.log"),"# ./configure --disable-gpl --disable-nonfree --disable-version3 --enable-shared --disable-static --disable-autodetect\n").unwrap();
        assert!(copy_from(&root, &root.join("sources"), &out, false).is_err());
        assert!(!out.exists());
        let _ = fs::remove_dir_all(root);
    }
}
