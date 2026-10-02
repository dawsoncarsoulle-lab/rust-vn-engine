fn main() {
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
    if std::env::var_os("CARGO_FEATURE_VIDEO").is_none()
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32")
    {
        return;
    }
    let prefix = std::path::PathBuf::from(
        std::env::var_os("FFMPEG_DIR").expect("Video builds need the audited shared FFmpeg prefix"),
    );
    assert!(
        prefix.is_absolute() && prefix.join("sources/COPYING.LGPLv2.1").is_file(),
        "Video builds require the separately packaged LGPL runtime and sources"
    );
    println!(
        "cargo:rustc-link-search=native={}",
        prefix.join("lib").display()
    );
}
