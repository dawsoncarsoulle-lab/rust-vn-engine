use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    if env::var_os("CARGO_FEATURE_VIDEO").is_none()
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32")
    {
        return;
    }
    // This search path must precede transitive system-library paths (e.g.
    // ALSA). Otherwise an installed FFmpeg with a different ABI can win even
    // though ffmpeg-sys generated its bindings from our audited headers.
    let prefix = PathBuf::from(
        env::var_os("FFMPEG_DIR")
            .expect("Video builds require FFMPEG_DIR from tools/build-ffmpeg-lgpl.sh"),
    );
    assert!(prefix.is_absolute(), "FFMPEG_DIR must be absolute");
    assert!(
        prefix.join("sources/COPYING.LGPLv2.1").is_file(),
        "The FFmpeg runtime must include its redistribution license and sources"
    );
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("lib").display()
    );
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib");
    }
}
