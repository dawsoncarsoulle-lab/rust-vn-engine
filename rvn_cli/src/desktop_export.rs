//! Build beside the previous export, then publish with a recoverable
//! directory backup. No old export is recursively deleted by this transaction.
use anyhow::{ensure, Context, Result};
use std::{fs, path::{Component, Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};

#[derive(Clone, Copy, Debug)]
pub(crate) enum OutputRoot { Desktop, Web }
impl OutputRoot {
    fn directory(self) -> &'static str { match self { Self::Desktop => "dist", Self::Web => "dist-web" } }
    fn label(self) -> &'static str { match self { Self::Desktop => "Desktop", Self::Web => "Web" } }
    fn stage_prefix(self) -> &'static str { match self { Self::Desktop => ".rvn-desktop-stage-", Self::Web => ".rvn-web-stage-" } }
    fn backup_prefix(self) -> &'static str { match self { Self::Desktop => ".rvn-desktop-backup-", Self::Web => ".rvn-web-backup-" } }
}

#[derive(Debug)]
pub(crate) struct Location {
    requested_project: PathBuf,
    root: PathBuf,
    parent: PathBuf,
    target: PathBuf,
    output_root: OutputRoot,
}

pub(crate) struct Stage {
    location: Location,
    path: PathBuf,
    owned: bool,
}

#[derive(Debug)]
pub(crate) struct Published {
    pub directory: PathBuf,
    pub backup: Option<PathBuf>,
}

fn nonce() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!("{}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed))
}

fn existing_directory(path: &Path) -> Result<bool> {
    match crate::desktop_paths::validate_entry(path) {
        Ok(metadata) => {
            ensure!(metadata.is_dir(), "Desktop output is not a directory: '{}'", path.display());
            Ok(true)
        }
        Err(error) if error.downcast_ref::<std::io::Error>().is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) => Ok(false),
        Err(error) => Err(error),
    }
}

fn check_ancestors(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "Desktop output boundary must be absolute");
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        existing_directory(ancestor)?;
    }
    Ok(())
}

impl Location {
    /// Read-only, including the path supplied by the caller before canonical
    /// resolution. A junction in `project/dist` must never redirect publication.
    pub(crate) fn preflight(project: &Path, name: &str) -> Result<Self> {
        Self::preflight_at(project, name, OutputRoot::Desktop)
    }

    pub(crate) fn preflight_at(project: &Path, name: &str, output_root: OutputRoot) -> Result<Self> {
        ensure!(matches!(Path::new(name).components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)])
            && !name.is_empty() && name.chars().all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-')),
            "Invalid Desktop export directory name");
        let requested_project = if project.is_absolute() { project.to_path_buf() } else { std::env::current_dir()?.join(project) };
        check_ancestors(&requested_project)?;
        ensure!(existing_directory(&requested_project)?, "Desktop project does not exist");
        let root = requested_project.canonicalize()?;
        let parent = root.join(output_root.directory());
        let target = parent.join(name);
        let location = Self { requested_project, root, parent, target, output_root };
        location.check()?;
        Ok(location)
    }

    fn check(&self) -> Result<()> {
        check_ancestors(&self.requested_project)?;
        ensure!(self.requested_project.canonicalize()? == self.root, "Desktop project location changed during build");
        check_ancestors(&self.target)?;
        if existing_directory(&self.parent)? {
            ensure!(self.parent.canonicalize()?.parent() == Some(self.root.as_path()), "Desktop output parent escaped its project");
        }
        if existing_directory(&self.target)? {
            ensure!(self.target.canonicalize()?.parent() == Some(self.parent.as_path()), "Desktop export escaped its output directory");
        }
        Ok(())
    }

    fn check_child(&self, path: &Path) -> Result<()> {
        self.check()?;
        ensure!(path.is_absolute() && path.parent() == Some(self.parent.as_path()) && path != self.target,
            "Desktop staging/backup path escaped its owned boundary");
        if existing_directory(path)? {
            let canonical = path.canonicalize()?;
            ensure!(canonical.parent() == Some(self.parent.as_path()) && canonical.starts_with(&self.root),
                "Desktop staging/backup path escaped its project");
        }
        Ok(())
    }
}

impl Stage {
    pub(crate) fn create(location: Location) -> Result<Self> {
        location.check()?;
        if !existing_directory(&location.parent)? {
            match fs::create_dir(&location.parent) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        location.check()?;
        for _ in 0..8 {
            let path = location.parent.join(format!("{}{}", location.output_root.stage_prefix(), nonce()));
            location.check_child(&path)?;
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { location, path, owned: true }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        anyhow::bail!("Unable to allocate an exclusive Desktop staging directory")
    }

    pub(crate) fn path(&self) -> &Path { &self.path }

    /// Clean only the exact, exclusively-created stage. Refuse a redirected
    /// ancestor or child instead of recursively deleting through a link.
    pub(crate) fn cleanup(&mut self) -> Result<()> {
        if !self.owned { return Ok(()); }
        self.location.check_child(&self.path)?;
        ensure!(self.path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(self.location.output_root.stage_prefix())), "Unrecognized export stage ownership");
        if existing_directory(&self.path)? {
            crate::desktop_paths::check_tree(&self.path)?;
            fs::remove_dir_all(&self.path).with_context(|| format!("Unable to clean owned Desktop stage '{}'", self.path.display()))?;
        }
        self.owned = false;
        Ok(())
    }

    // The hook models an actual publication failure after the previous export
    // has moved, so tests traverse rollback rather than an unrelated copy API.
    pub(crate) fn publish_with_hook(&mut self, before_replace: impl FnOnce(&Path) -> Result<()>) -> Result<Published> {
        self.location.check_child(&self.path)?;
        ensure!(self.owned && existing_directory(&self.path)?, "Desktop stage is absent or not owned");
        crate::desktop_paths::check_tree(&self.path)?;
        let backup = if existing_directory(&self.location.target)? {
            let backup = self.location.parent.join(format!("{}{}-{}", self.location.output_root.backup_prefix(), self.location.target.file_name().unwrap().to_string_lossy(), nonce()));
            self.location.check_child(&backup)?;
            ensure!(!existing_directory(&backup)?, "Desktop backup path already exists");
            fs::rename(&self.location.target, &backup).with_context(|| format!("Unable to preserve previous Desktop export '{}'", self.location.target.display()))?;
            Some(backup)
        } else { None };
        let result: Result<()> = (|| {
            before_replace(&self.path)?;
            self.location.check_child(&self.path)?;
            ensure!(!existing_directory(&self.location.target)?, "Desktop export appeared during publication; it was not replaced");
            fs::rename(&self.path, &self.location.target).context("Unable to publish completed Desktop export")?;
            Ok(())
        })();
        if let Err(error) = result {
            if let Some(backup) = &backup {
                let rollback: Result<()> = (|| {
                    self.location.check_child(backup)?;
                    ensure!(!existing_directory(&self.location.target)?, "Another writer created the Desktop target; rollback will not overwrite it");
                    fs::rename(backup, &self.location.target).context("Unable to restore previous Desktop export")?;
                    Ok(())
                })();
                if let Err(rollback) = rollback {
                    return Err(anyhow::anyhow!("{error:#}\nPrevious export retained at '{}'; rollback suspended: {rollback:#}", backup.display()));
                }
                return Err(anyhow::anyhow!("{error:#}\nThe previous {} export was restored.", self.location.output_root.label()));
            }
            return Err(error);
        }
        self.owned = false;
        Ok(Published { directory: self.location.target.clone(), backup })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fixture { owned: PathBuf, project: PathBuf, runtime: PathBuf, junction: Option<PathBuf> }
    impl Fixture {
        fn new(previous: bool) -> Self {
            let owned = std::env::temp_dir().join(format!("rvn-desktop-public-{}", nonce()));
            fs::create_dir(&owned).unwrap();
            let project = owned.join("project");
            fs::create_dir(&project).unwrap();
            let runtime = owned.join("runtime.exe");
            fs::write(&runtime, b"isolated runtime fixture, never launched").unwrap();
            let fixture = Self { owned, project, runtime, junction: None };
            fixture.write("rvn.toml", "[project]\nmain_script='main.rvn'\nstart_label='start'\n");
            fixture.write("main.rvn", "label start\n\"Original story — 雨\"\nreturn\n");
            fixture.write("assets/picture.txt", b"original media");
            fixture.write("locales/fr.toml", b"title='Bonjour'\n");
            fixture.write("saves/slot1.sav", b"personal progress");
            if previous {
                fs::create_dir_all(fixture.target()).unwrap();
                fs::write(fixture.target().join("old.exe"), b"previous executable").unwrap();
                fs::create_dir_all(fixture.target().join("data/saves")).unwrap();
                fs::write(fixture.target().join("data/saves/old.sav"), b"previous exported progress").unwrap();
            }
            fixture
        }
        fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
            let path = self.project.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fn target(&self) -> PathBuf { self.project.join("dist").join(format!("Safe-{}", crate::detect_platform().unwrap())) }
        fn config(&self) -> crate::ProjectConfig { crate::load_project_config(&self.project).unwrap() }
        fn stage_names(&self) -> Vec<PathBuf> {
            fs::read_dir(self.project.join("dist")).unwrap().map(|entry| entry.unwrap().path())
                .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with(".rvn-desktop-stage-")).collect()
        }
        fn link_dist(&mut self, outside: &Path) {
            let link = self.project.join("dist");
            assert!(!link.exists());
            assert!(outside.canonicalize().unwrap().starts_with(self.owned.canonicalize().unwrap()));
            #[cfg(unix)]
            std::os::unix::fs::symlink(outside, &link).unwrap();
            #[cfg(windows)] {
                use std::os::windows::process::CommandExt;
                let quoted = |path: &Path| {
                    let path = path.to_str().unwrap();
                    assert!(!path.contains(['"', '%', '\r', '\n']));
                    format!("\"{path}\"")
                };
                let result = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"])
                    .raw_arg(quoted(&link)).raw_arg(quoted(outside)).creation_flags(0x08000000).output().unwrap();
                assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            }
            self.junction = Some(link);
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let Ok(owned) = self.owned.canonicalize() else { return; };
            let Ok(temporary) = std::env::temp_dir().canonicalize() else { return; };
            if owned.parent() != Some(temporary.as_path()) || !owned.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("rvn-desktop-public-")) { return; }
            if let Some(link) = &self.junction {
                // Remove only the owned link object, never its target tree.
                #[cfg(windows)] { if fs::remove_dir(link).is_err() { return; } }
                #[cfg(unix)] { if fs::remove_file(link).is_err() { return; } }
            }
            if crate::desktop_paths::validate_entry(&owned).is_err() || crate::desktop_paths::check_tree(&owned).is_err() { return; }
            let _ = fs::remove_dir_all(owned);
        }
    }
    fn snapshot(path: &Path, exclude_dist: bool) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, path: &Path, exclude_dist: bool, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                let relative = path.strip_prefix(root).unwrap();
                if exclude_dist && relative.starts_with("dist") { continue; }
                let metadata = crate::desktop_paths::validate_entry(&path).unwrap();
                if metadata.is_dir() { walk(root, &path, exclude_dist, files); }
                else { files.insert(relative.to_path_buf(), fs::read(path).unwrap()); }
            }
        }
        let mut files = BTreeMap::new();
        walk(path, path, exclude_dist, &mut files);
        files
    }

    #[test]
    fn public_build_preflight_and_runtime_failure_leave_previous_exports_inputs_and_saves_exact() {
        let fixture = Fixture::new(true);
        fixture.write("rvn.toml", "[project]\nmain_script='main.rvn'\n[paths]\nassets='.'\n");
        let before = snapshot(&fixture.project, false);
        let error = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || panic!("runtime must follow preflight"), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("Desktop assets cannot copy/use the project root"));
        assert_eq!(snapshot(&fixture.project, false), before);
        let fixture = Fixture::new(true);
        let before = snapshot(&fixture.project, false);
        let error = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || anyhow::bail!("runtime preparation failed"), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("runtime preparation failed"));
        assert_eq!(snapshot(&fixture.project, false), before);
        assert!(fixture.stage_names().is_empty());
    }

    #[test]
    fn public_build_refuses_an_external_dist_junction_before_runtime_or_output_mutation() {
        let mut fixture = Fixture::new(false);
        let outside = fixture.owned.join("outside");
        fs::create_dir(&outside).unwrap();
        let external_export = outside.join(fixture.target().file_name().unwrap());
        fs::create_dir(&external_export).unwrap();
        fs::write(external_export.join("keep.exe"), b"outside previous export").unwrap();
        let before = snapshot(&outside, false);
        fixture.link_dist(&outside);
        let inputs = snapshot(&fixture.project, true);
        let error = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || panic!("redirect must be refused before runtime"), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("symlink/reparse"), "{error:#}");
        assert_eq!(snapshot(&outside, false), before);
        assert_eq!(snapshot(&fixture.project, true), inputs);
    }

    #[cfg(not(feature = "video"))]
    #[test]
    fn public_build_rolls_back_a_real_previous_directory_when_publication_fails() {
        let fixture = Fixture::new(true);
        let before = snapshot(&fixture.project, false);
        let error = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime.clone()), |stage| {
            assert!(!fixture.target().exists(), "previous directory must already be in the rollback backup");
            assert_eq!(fs::read(stage.join(crate::desktop_executable_name("Safe", std::env::consts::OS))).unwrap(), fs::read(&fixture.runtime).unwrap());
            anyhow::bail!("publication failure after preservation")
        }).unwrap_err();
        assert!(format!("{error:#}").contains("previous Desktop export was restored"), "{error:#}");
        assert_eq!(snapshot(&fixture.project, false), before);
        assert!(fixture.stage_names().is_empty());
    }

    #[cfg(not(feature = "video"))]
    #[test]
    fn public_build_preserves_both_backup_and_competing_output_if_rollback_cannot_replace_it() {
        let fixture = Fixture::new(true);
        let original_export = snapshot(&fixture.target(), false);
        let original_inputs = snapshot(&fixture.project, true);
        let error = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime.clone()), |_| {
            fs::create_dir(fixture.target())?;
            fs::write(fixture.target().join("third-party.txt"), b"competing output")?;
            Ok(())
        }).unwrap_err();
        assert!(format!("{error:#}").contains("rollback suspended"), "{error:#}");
        assert_eq!(fs::read(fixture.target().join("third-party.txt")).unwrap(), b"competing output");
        let backups: Vec<_> = fs::read_dir(fixture.project.join("dist")).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.file_name().unwrap().to_string_lossy().starts_with(".rvn-desktop-backup-")).collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(snapshot(&backups[0], false), original_export);
        assert_eq!(snapshot(&fixture.project, true), original_inputs);
        assert!(fixture.stage_names().is_empty());
    }

    #[cfg(not(feature = "video"))]
    #[test]
    fn public_build_publishes_nested_source_bytes_and_retains_previous_export_for_manual_rollback() {
        let fixture = Fixture::new(true);
        let main = "use \"chapter.rvn\"\r\nlabel start\r\njump chapter\r\n";
        let chapter = "label chapter\r\n\"Été — 雨\"\r\nreturn\r\n";
        fixture.write("rvn.toml", "[project]\nmain_script='scripts/main.rvn'\nstart_label='start'\n");
        fixture.write("scripts/main.rvn", main);
        fixture.write("scripts/chapter.rvn", chapter);
        let original_inputs = snapshot(&fixture.project, true);
        let original_export = snapshot(&fixture.target(), false);
        let published = crate::build_desktop_project_with_runtime(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime.clone()), |_| Ok(())).unwrap();
        assert_eq!(published.directory, fixture.target().canonicalize().unwrap());
        assert_eq!(snapshot(&published.backup.unwrap(), false), original_export);
        assert_eq!(fs::read(published.directory.join("data/scripts/main.rvn")).unwrap(), main.as_bytes());
        assert_eq!(fs::read(published.directory.join("data/scripts/chapter.rvn")).unwrap(), chapter.as_bytes());
        assert_eq!(rvn_parser::parse_file_with_uses(published.directory.join("data/scripts/main.rvn")).unwrap(), rvn_parser::parse_file_with_uses(fixture.project.join("scripts/main.rvn")).unwrap());
        assert_eq!(fs::read_dir(published.directory.join("data/saves")).unwrap().count(), 0);
        assert_eq!(fs::read(published.directory.join(crate::desktop_executable_name("Safe", std::env::consts::OS))).unwrap(), fs::read(&fixture.runtime).unwrap());
        assert_eq!(snapshot(&fixture.project, true), original_inputs);
        assert!(fixture.stage_names().is_empty());
    }
}
