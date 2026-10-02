//! Select the exact shared object from the prefix that supplies the headers.
use std::path::Path;

pub fn versioned_library(directory: &Path, library: &str) -> Result<String, String> {
    if library.is_empty() || !library.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("Invalid library name".into());
    }
    let root = directory.canonicalize().map_err(|error| error.to_string())?;
    let target = root.join(format!("lib{library}.so")).canonicalize()
        .map_err(|error| error.to_string())?;
    if !target.is_file() || target.parent() != Some(root.as_path()) {
        return Err("Shared library escapes its configured directory".into());
    }
    let name = target.file_name().and_then(|name| name.to_str())
        .ok_or("Non-Unicode shared library filename")?;
    let version = name.strip_prefix(&format!("lib{library}.so."))
        .ok_or("The prefix must provide versioned shared objects")?;
    if version.split('.').any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit())) {
        return Err("Invalid shared library version".into());
    }
    Ok(name.into())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("rvn-media-link-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn link(&self, target: &str) {
            fs::write(self.0.join(target), []).unwrap();
            symlink(target, self.0.join("libavcodec.so")).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
    }

    #[test]
    fn uses_the_prefix_version_not_an_unversioned_system_name() {
        let fixture = Fixture::new();
        fixture.link("libavcodec.so.63.1.102");
        assert_eq!(versioned_library(&fixture.0, "avcodec").unwrap(), "libavcodec.so.63.1.102");
    }
    #[test]
    fn rejects_unversioned_objects() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("libavcodec.so"), []).unwrap();
        assert!(versioned_library(&fixture.0, "avcodec").is_err());
    }
    #[test]
    fn rejects_wrong_or_malformed_versions() {
        for name in ["libavformat.so.63", "libavcodec.so.63..1", "libavcodec.so.63-debug"] {
            let fixture = Fixture::new();
            fixture.link(name);
            assert!(versioned_library(&fixture.0, "avcodec").is_err());
        }
    }
    #[test]
    fn rejects_prefix_escape() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        fs::write(outside.0.join("libavcodec.so.63"), []).unwrap();
        symlink(outside.0.join("libavcodec.so.63"), fixture.0.join("libavcodec.so")).unwrap();
        assert!(versioned_library(&fixture.0, "avcodec").is_err());
    }
    #[test]
    fn rejects_missing_libraries_and_path_names() {
        let fixture = Fixture::new();
        assert!(versioned_library(&fixture.0, "avcodec").is_err());
        assert!(versioned_library(&fixture.0, "../avcodec").is_err());
    }
}
