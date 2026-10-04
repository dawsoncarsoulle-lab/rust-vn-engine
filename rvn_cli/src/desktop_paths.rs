//! Preflight every Desktop copy input before writing the output. Runtime paths
//! may be flexible, but a portable bundle cannot copy outside its own data tree.
use anyhow::{bail, ensure, Context, Result};
use std::fs;
use std::path::{Component, Path, PathBuf};

const PRIVATE: &[&str] = &[
    "saves",
    "exports",
    "dist",
    "dist-web",
    "target",
    ".git",
    ".editor-history",
    ".rvn-backups",
    ".rvn-authoring.json",
    ".rvn-authoring.lock",
];

fn relative(raw: &str, label: &str) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for part in Path::new(raw).components() {
        match part {
            Component::Normal(name) => result.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("Desktop {label} must be project-relative without absolute/parent components: '{raw}'")
            }
        }
    }
    ensure!(
        !result.as_os_str().is_empty(),
        "Desktop {label} cannot copy/use the project root: '{raw}'"
    );
    Ok(result)
}

fn starts_with(path: &Path, prefix: &Path) -> bool {
    if cfg!(windows) {
        let left: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect();
        let right: Vec<_> = prefix
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect();
        left.starts_with(&right)
    } else {
        path.starts_with(prefix)
    }
}

fn private(path: &Path, is_saves: bool) -> bool {
    PRIVATE
        .iter()
        .filter(|name| !(is_saves && **name == "saves"))
        .any(|name| starts_with(path, Path::new(name)))
}

pub(crate) fn validate(project: &Path, cfg: &crate::ProjectConfig) -> Result<()> {
    let root = fs::canonicalize(project).context("unable to resolve Desktop project")?;
    validate_entry(&root)?;
    let main = relative(&cfg.project.main_script, "main_script")?;
    let assets = relative(&cfg.paths.assets, "assets")?;
    let locales = relative(&cfg.paths.locales, "locales")?;
    let theme = relative(&cfg.paths.theme, "theme")?;
    let saves = relative(&cfg.paths.saves, "saves")?;
    ensure!(
        !private(&saves, true),
        "Desktop saves overlaps private exports/cache: '{}'",
        saves.display()
    );

    // Required type, optional existence and whether an entire tree is copied.
    let mut inputs = vec![
        (PathBuf::from("rvn.toml"), true, false),
        (main, true, false),
        (assets, true, true),
        (locales, true, true),
        (theme, false, false),
    ];
    for name in ["CREDITS.md", "LICENSE", "LICENSE.md", "LICENSE.txt"] {
        inputs.push((PathBuf::from(name), false, false));
    }
    // Inspect the manifest only after its path itself has passed link checks.
    check_path(&root, Path::new("rvn.toml"), true, false)?;
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(root.join("rvn.toml"))?)?;
    let configured = manifest
        .get("paths")
        .and_then(|p| p.get("menus"))
        .and_then(toml::Value::as_str);
    if let Some(name) = configured {
        inputs.push((relative(name, "menus")?, true, false));
    } else {
        inputs.push((PathBuf::from("menus.rvnui"), false, false));
    }
    for (path, required, directory) in inputs {
        ensure!(
            !private(&path, false),
            "Desktop copy input overlaps private saves/output/cache: '{}'",
            path.display()
        );
        ensure!(
            !starts_with(&path, &saves) && !(directory && starts_with(&saves, &path)),
            "Desktop copy input overlaps configured saves '{}': '{}'",
            saves.display(),
            path.display()
        );
        if check_path(&root, &path, required, directory)? && directory {
            check_tree(&root.join(&path))?;
        }
    }
    // Do not enumerate personal progress: only its existing ancestors/type are
    // relevant, and the exported saves directory will be newly created empty.
    check_path(&root, &saves, false, true)?;
    Ok(())
}

/// Web copies whole script-parent directories and, for a root script, an
/// optional scripts directory. Validate these additional trees and saves
/// exclusions before runtime preparation and again before publication.
pub(crate) fn validate_web_inputs(project: &Path, cfg: &crate::ProjectConfig) -> Result<()> {
    validate(project, cfg)?;
    let root = project.canonicalize()?;
    let main = relative(&cfg.project.main_script, "main_script")?;
    let saves = relative(&cfg.paths.saves, "saves")?;
    let directory = main.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("scripts"));
    ensure!(!private(directory, false) && !starts_with(directory, &saves) && !starts_with(&saves, directory),
        "Web script copy overlaps private saves/output/cache: '{}'", directory.display());
    if check_path(&root, directory, false, true)? {
        check_tree(&root.join(directory))?;
    }
    Ok(())
}

fn check_path(root: &Path, relative: &Path, required: bool, directory: bool) -> Result<bool> {
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        match validate_entry(&path) {
            Ok(_) => {}
            Err(error)
                if !required
                    && error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(false)
            }
            Err(error) => return Err(error),
        }
    }
    let actual = fs::canonicalize(&path)?;
    ensure!(
        actual.starts_with(root),
        "Desktop copy input escapes project: '{}'",
        path.display()
    );
    let metadata = fs::metadata(&path)?;
    ensure!(
        if directory {
            metadata.is_dir()
        } else {
            metadata.is_file()
        },
        "Desktop {} is not a regular {}",
        path.display(),
        if directory { "directory" } else { "file" }
    );
    Ok(true)
}

pub(crate) fn check_tree(directory: &Path) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = validate_entry(&path)?;
        if metadata.is_dir() {
            check_tree(&path)?;
        } else {
            ensure!(
                metadata.is_file(),
                "Desktop input is not a regular file: '{}'",
                path.display()
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_entry(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("unable to inspect Desktop input '{}'", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "Desktop input symlink/reparse redirect is refused: '{}'",
        path.display()
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            if let Some(tag) = reparse_tag(path)? {
                // CLOUD_0..F are local storage-provider placeholders, not pathname
                // redirections. Copying their bytes produces ordinary bundled files.
                // SDK winnt.h explicitly reserves bits12..15 for these variants.
                ensure!(
                    cloud_storage_tag(tag),
                    "Desktop input symlink/reparse tag0x{tag:08X} is refused: '{}'",
                    path.display()
                );
            }
        }
    }
    Ok(metadata)
}

#[cfg(windows)]
fn cloud_storage_tag(tag: u32) -> bool {
    tag & !0x0000_F000 == 0x9000_001A
}

#[cfg(windows)]
fn reparse_tag(path: &Path) -> Result<Option<u32>> {
    use std::ffi::c_void;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    #[repr(C)]
    struct AttributeTagInfo {
        attributes: u32,
        tag: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandleEx(
            handle: *mut c_void,
            class: i32,
            info: *mut c_void,
            size: u32,
        ) -> i32;
    }
    // Metadata handle only; do not follow the reparse object. BACKUP_SEMANTICS
    // permits directories and OPEN_REPARSE_POINT permits inspecting links.
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(7)
        .custom_flags(0x0220_0000)
        .open(path)
        .with_context(|| format!("unable to inspect reparse tag '{}'", path.display()))?;
    let mut info = AttributeTagInfo {
        attributes: 0,
        tag: 0,
    };
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            9,
            (&mut info as *mut AttributeTagInfo).cast(),
            std::mem::size_of::<AttributeTagInfo>() as u32,
        )
    };
    ensure!(
        ok != 0,
        "unable to inspect Desktop reparse tag '{}': {}",
        path.display(),
        std::io::Error::last_os_error()
    );
    // Hydration can turn a CLOUD file into an ordinary file between the
    // directory metadata and this no-follow handle; use actual handle flags.
    Ok((info.attributes & 0x400 != 0).then_some(info.tag))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn metadata_handle_inspection_and_sdk_cloud_tags_preserve_regular_files_without_allowing_redirects(
    ) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rvn-desktop-tag-{}-{stamp}.rvn",
            std::process::id()
        ));
        let bytes = "label start\r\n\"Été\"\r\n".as_bytes();
        fs::write(&path, bytes).unwrap();
        assert_eq!(reparse_tag(&path).unwrap(), None);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(validate_entry(&path).unwrap().is_file());
        // These values are the documented Windows SDK CLOUD0..F range,
        // distinguished from mount points, symlinks and unknown tags.
        for tag in [0x9000_001A, 0x9000_101A, 0x9000_F01A] {
            assert!(cloud_storage_tag(tag));
        }
        for tag in [0, 0xA000_0003, 0xA000_000C, 0x8000_001B, 0x9001_001A] {
            assert!(!cloud_storage_tag(tag));
        }
        fs::remove_file(path).unwrap();
    }
}
