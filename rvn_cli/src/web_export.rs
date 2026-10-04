//! Prepare a Web export beside its predecessor; publish only a complete stage.
//! Runtime discovery/build is injected at the real preparation boundary for
//! tests. Tests copy actual files and never run Cargo, bindgen or a browser.
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::{Path, PathBuf}};

pub(crate) enum Runtime { Bundled(PathBuf), Wasm(PathBuf) }
#[derive(Debug, PartialEq, Eq)]
struct Identity { bytes: u64, digest: [u8; 32] }
type Snapshot = BTreeMap<PathBuf, Identity>;

fn validate_ancestors(path: &Path) -> Result<()> {
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    for ancestor in absolute.ancestors().collect::<Vec<_>>().into_iter().rev() {
        crate::desktop_paths::validate_entry(ancestor)?;
    }
    Ok(())
}

fn identity(path: &Path) -> Result<Identity> {
    let before = crate::desktop_paths::validate_entry(path)?;
    ensure!(before.is_file(), "Web runtime input is not a regular file: '{}'", path.display());
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();let mut bytes = 0_u64;let mut buffer = [0_u8; 65536];
    loop { let count = file.read(&mut buffer)?;if count == 0 { break; }digest.update(&buffer[..count]);bytes += count as u64; }
    let after = crate::desktop_paths::validate_entry(path)?;
    ensure!(after.is_file() && before.len() == bytes && after.len() == bytes && before.modified()? == after.modified()?,
        "Web runtime input changed while reading: '{}'", path.display());
    Ok(Identity { bytes, digest: digest.finalize().into() })
}

fn validate_wasm(path: &Path) -> Result<()> {
    let metadata = crate::desktop_paths::validate_entry(path)?;
    ensure!(metadata.is_file() && metadata.len() >= 8, "Web runtime Wasm is not a regular module: '{}'", path.display());
    let mut header = [0_u8; 8];fs::File::open(path)?.read_exact(&mut header)?;
    ensure!(&header == b"\0asm\x01\0\0\0", "Invalid WebAssembly header: '{}'", path.display());
    Ok(())
}

fn validate_generated(directory: &Path) -> Result<()> {
    validate_ancestors(directory)?;
    ensure!(crate::desktop_paths::validate_entry(directory)?.is_dir(), "Web runtime output is not a regular directory");
    let javascript = directory.join("game.js");
    let metadata = crate::desktop_paths::validate_entry(&javascript)?;
    ensure!(metadata.is_file() && metadata.len() > 0, "Web runtime game.js is empty or not regular");
    validate_wasm(&directory.join("game_bg.wasm"))?;
    crate::desktop_paths::check_tree(directory)
}

impl Runtime {
    fn snapshot(&self) -> Result<Snapshot> {
        match self {
            Self::Wasm(path) => {
                validate_ancestors(path)?;validate_wasm(path)?;
                Ok(BTreeMap::from([(PathBuf::new(), identity(path)?)]))
            }
            Self::Bundled(root) => {
                validate_generated(root)?;
                fn walk(root: &Path, directory: &Path, out: &mut Snapshot) -> Result<()> {
                    for entry in fs::read_dir(directory)? {
                        let path = entry?.path();let metadata = crate::desktop_paths::validate_entry(&path)?;
                        if metadata.is_dir() { walk(root, &path, out)?; }
                        else { out.insert(path.strip_prefix(root)?.to_path_buf(), identity(&path)?); }
                    }
                    Ok(())
                }
                let mut snapshot = Snapshot::new();walk(root, root, &mut snapshot)?;Ok(snapshot)
            }
        }
    }
    fn copy_to(&self, destination: &Path) -> Result<()> {
        match self {
            Self::Bundled(root) => crate::copy_dir_all(root, destination),
            Self::Wasm(path) => { println!("Génération JS/WASM...");crate::run_wasm_bindgen(path, destination) }
        }
    }
    fn verify_copy(&self, expected: &Snapshot, destination: &Path) -> Result<()> {
        validate_generated(destination)?;
        if matches!(self, Self::Bundled(_)) {
            for (relative, original) in expected {
                ensure!(&identity(&destination.join(relative))? == original,
                    "Copied Web runtime differs from its prepared bytes: '{}'", relative.display());
            }
        }
        Ok(())
    }
}

fn unchanged_manifest(project: &Path, expected: &[u8], phase: &str) -> Result<()> {
    crate::desktop_paths::validate_entry(&project.join("rvn.toml"))?;
    ensure!(fs::read(project.join("rvn.toml"))? == expected,
        "Project configuration changed during Web {phase}; the previous export was retained");
    Ok(())
}

pub(crate) fn build(
    project: &Path, cfg: &crate::ProjectConfig, name: &str,
    prepare_runtime: impl FnOnce() -> Result<Runtime>,
    before_replace: impl FnOnce(&Path) -> Result<()>,
) -> Result<crate::desktop_export::Published> {
    let location = crate::desktop_export::Location::preflight_at(project, name, crate::desktop_export::OutputRoot::Web)?;
    crate::desktop_paths::validate_web_inputs(project, cfg)?;
    let manifest = fs::read(project.join("rvn.toml"))?;
    println!("Build web runtime...");
    let runtime = prepare_runtime()?;
    let runtime_before = runtime.snapshot()?;
    unchanged_manifest(project, &manifest, "runtime preparation")?;
    crate::desktop_paths::validate_web_inputs(project, cfg)?;
    // Preparation failures above have not created output or moved the old Web
    // export. Every write below belongs to the exclusive, recoverable stage.
    let mut stage = crate::desktop_export::Stage::create(location)?;
    let result = (|| {
        runtime.copy_to(stage.path())?;
        runtime.verify_copy(&runtime_before, stage.path())?;
        println!("Copie fichiers...");
        crate::copy_web_project_files(project, stage.path(), cfg)?;
        println!("Génération index.html...");
        crate::write_web_index(stage.path(), name)?;
        runtime.verify_copy(&runtime_before, stage.path())?;
        crate::desktop_paths::validate_web_inputs(project, cfg)?;
        unchanged_manifest(project, &manifest, "staging")?;
        ensure!(runtime.snapshot()? == runtime_before, "Web runtime changed during staging; the previous export was retained");
        stage.publish_with_hook(|path| {
            before_replace(path)?;
            // This final guard runs after preservation of the old directory;
            // failure traverses the existing transaction's actual rollback.
            crate::desktop_paths::validate_web_inputs(project, cfg)?;
            unchanged_manifest(project, &manifest, "publication")?;
            ensure!(runtime.snapshot()? == runtime_before, "Web runtime changed during publication; the previous export was retained");
            runtime.verify_copy(&runtime_before, path)
        })
    })();
    match result {
        Ok(published) => Ok(published),
        Err(error) => {
            if let Err(cleanup) = stage.cleanup() {
                return Err(anyhow::anyhow!("{error:#}\nOwned Web stage retained at '{}': {cleanup:#}", stage.path().display()));
            }
            Err(error)
        }
    }
}

#[cfg(test)]mod tests {
    use super::*;
    use std::{sync::atomic::{AtomicU64, Ordering}, time::{SystemTime, UNIX_EPOCH}};
    fn nonce() -> String {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        format!("{}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed))
    }
    struct Fixture { owned: PathBuf, project: PathBuf, runtime: PathBuf, junction: Option<PathBuf> }
    impl Fixture {
        fn new(previous: bool) -> Self {
            let owned = std::env::temp_dir().join(format!("rvn-web-transaction-{}", nonce()));fs::create_dir(&owned).unwrap();
            let project = owned.join("project");let runtime = owned.join("runtime");fs::create_dir(&project).unwrap();fs::create_dir(&runtime).unwrap();
            let fixture = Self { owned, project, runtime, junction: None };
            fixture.write("rvn.toml", b"[project]\nmain_script='main.rvn'\nstart_label='start'\n");
            fixture.write("main.rvn", "label start\r\n\"Original story — 雨\"\r\nreturn\r\n");
            fixture.write("assets/picture.txt", b"original picture");fixture.write("locales/fr.toml", b"title='Bonjour'\n");fixture.write("saves/slot.sav", b"personal progress");
            fs::write(fixture.runtime.join("game.js"), b"export default async function init() {}\n").unwrap();
            fs::write(fixture.runtime.join("game_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
            fs::create_dir(fixture.runtime.join("snippets")).unwrap();fs::write(fixture.runtime.join("snippets/helper.js"), b"export const name='local';\n").unwrap();
            if previous { fs::create_dir_all(fixture.target().join("data/saves")).unwrap();fs::write(fixture.target().join("old.html"), b"previous index").unwrap();fs::write(fixture.target().join("data/saves/old.sav"), b"previous progress").unwrap(); }
            fixture
        }
        fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) { let path = self.project.join(relative);fs::create_dir_all(path.parent().unwrap()).unwrap();fs::write(path, bytes).unwrap(); }
        fn target(&self) -> PathBuf { self.project.join("dist-web/Safe") }
        fn config(&self) -> crate::ProjectConfig { crate::load_project_config(&self.project).unwrap() }
        fn runtime(&self) -> Runtime { Runtime::Bundled(self.runtime.clone()) }
        fn stages(&self) -> Vec<PathBuf> {
            let parent = self.project.join("dist-web");if !parent.is_dir() { return Vec::new(); }
            fs::read_dir(parent).unwrap().map(|entry|entry.unwrap().path()).filter(|path|path.file_name().unwrap().to_string_lossy().starts_with(".rvn-web-stage-")).collect()
        }
        fn link_dist(&mut self, outside: &Path) {
            let link = self.project.join("dist-web");assert!(!link.exists());assert!(outside.canonicalize().unwrap().starts_with(self.owned.canonicalize().unwrap()));
            #[cfg(unix)] std::os::unix::fs::symlink(outside, &link).unwrap();
            #[cfg(windows)] {
                use std::os::windows::process::CommandExt;
                let quoted = |path: &Path| { let path = path.to_str().unwrap();assert!(!path.contains(['"', '%', '\r', '\n']));format!("\"{path}\"") };
                let result = std::process::Command::new("cmd.exe").args(["/D", "/C", "mklink", "/J"]).raw_arg(quoted(&link)).raw_arg(quoted(outside)).creation_flags(0x08000000).output().unwrap();
                assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            }
            self.junction = Some(link);
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let Ok(owned) = self.owned.canonicalize() else { return; };let Ok(temporary) = std::env::temp_dir().canonicalize() else { return; };
            if owned.parent() != Some(temporary.as_path()) || !owned.file_name().and_then(|name|name.to_str()).is_some_and(|name|name.starts_with("rvn-web-transaction-")) { return; }
            if let Some(link) = &self.junction {
                #[cfg(windows)] if fs::remove_dir(link).is_err() { return; }
                #[cfg(unix)] if fs::remove_file(link).is_err() { return; }
            }
            if crate::desktop_paths::validate_entry(&owned).is_err() || crate::desktop_paths::check_tree(&owned).is_err() { return; }
            let _ = fs::remove_dir_all(owned);
        }
    }
    fn snapshot(root: &Path, exclude_output: bool) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, directory: &Path, exclude_output: bool, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(directory).unwrap() { let path = entry.unwrap().path();let relative = path.strip_prefix(root).unwrap();
                if exclude_output && relative.starts_with("dist-web") { continue; }
                let metadata = crate::desktop_paths::validate_entry(&path).unwrap();
                if metadata.is_dir() { walk(root, &path, exclude_output, files); } else { files.insert(relative.to_path_buf(), fs::read(path).unwrap()); }
            }
        }
        let mut files = BTreeMap::new();walk(root, root, exclude_output, &mut files);files
    }
    #[test]fn failed_runtime_preparation_preserves_every_previous_export_input_and_save_byte() {
        let fixture = Fixture::new(true);let before = snapshot(&fixture.project, false);
        let error = build(&fixture.project, &fixture.config(), "Safe", || anyhow::bail!("runtime preparation failed"), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("runtime preparation failed"));assert_eq!(snapshot(&fixture.project, false), before);assert!(fixture.stages().is_empty());
    }
    #[test]fn unsafe_copy_root_fails_before_runtime_and_missing_module_never_removes_previous_export() {
        let fixture = Fixture::new(true);fixture.write("rvn.toml", "[project]\nmain_script='main.rvn'\n[paths]\nassets='.'\n");let before = snapshot(&fixture.project, false);
        assert!(build(&fixture.project, &fixture.config(), "Safe", || panic!("preflight must run first"), |_| Ok(())).is_err());assert_eq!(snapshot(&fixture.project, false), before);
        let fixture = Fixture::new(true);let before = snapshot(&fixture.project, false);fs::write(fixture.runtime.join("game_bg.wasm"), b"truncated").unwrap();
        assert!(build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |_| Ok(())).is_err());assert_eq!(snapshot(&fixture.project, false), before);assert!(fixture.stages().is_empty());
    }
    #[test]fn actual_project_copy_failure_cleans_only_stage_and_preserves_previous_export() {
        let fixture = Fixture::new(true);fs::create_dir(fixture.runtime.join("rvn.toml")).unwrap();fs::write(fixture.runtime.join("rvn.toml/block.txt"), b"conflicting runtime directory").unwrap();
        let before = snapshot(&fixture.project, false);
        let error = build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("unable to copy file"), "{error:#}");assert_eq!(snapshot(&fixture.project, false), before);assert!(fixture.stages().is_empty());
    }
    #[test]fn changed_manifest_during_preparation_keeps_prior_export_and_the_new_user_config() {
        let fixture = Fixture::new(true);let previous = snapshot(&fixture.target(), false);let changed = b"[project]\nmain_script='main.rvn'\nstart_label='start'\nname='Changed'\n";
        let error = build(&fixture.project, &fixture.config(), "Safe", || { fixture.write("rvn.toml", changed);Ok(fixture.runtime()) }, |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("configuration changed"));assert_eq!(snapshot(&fixture.target(), false), previous);assert_eq!(fs::read(fixture.project.join("rvn.toml")).unwrap(), changed);assert!(fixture.stages().is_empty());
    }
    #[test]fn publication_failure_restores_the_actual_previous_directory_and_input_bytes() {
        let fixture = Fixture::new(true);let before = snapshot(&fixture.project, false);
        let error = build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |stage| {
            assert!(!fixture.target().exists());assert_eq!(fs::read(stage.join("game.js")).unwrap(), fs::read(fixture.runtime.join("game.js")).unwrap());anyhow::bail!("publication interrupted after preservation")
        }).unwrap_err();
        assert!(format!("{error:#}").contains("previous Web export was restored"), "{error:#}");assert_eq!(snapshot(&fixture.project, false), before);assert!(fixture.stages().is_empty());
    }
    #[test]fn changed_manifest_or_runtime_after_preservation_traverses_real_rollback() {
        for change_runtime in [false, true] {
            let fixture = Fixture::new(true);let previous = snapshot(&fixture.target(), false);
            let error = build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |_| {
                assert!(!fixture.target().exists());
                if change_runtime { fs::write(fixture.runtime.join("game.js"), b"new runtime bytes")?; }
                else { fixture.write("rvn.toml", "[project]\nmain_script='main.rvn'\nstart_label='start'\nname='Changed'\n"); }
                Ok(())
            }).unwrap_err();
            assert!(format!("{error:#}").contains("previous Web export was restored"), "{error:#}");assert_eq!(snapshot(&fixture.target(), false), previous);assert!(fixture.stages().is_empty());
        }
    }
    #[test]fn successful_web_publication_preserves_nested_scripts_runtime_and_manual_rollback() {
        let fixture = Fixture::new(true);fixture.write("rvn.toml", "[project]\nmain_script='scripts/main.rvn'\nstart_label='start'\n");
        fixture.write("scripts/main.rvn", "use \"chapter.rvn\"\r\nlabel start\r\njump chapter\r\n");fixture.write("scripts/chapter.rvn", "label chapter\r\n\"Été — 雨\"\r\nreturn\r\n");
        let inputs = snapshot(&fixture.project, true);let previous = snapshot(&fixture.target(), false);
        let published = build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |_| Ok(())).unwrap();
        assert_eq!(published.directory, fixture.target().canonicalize().unwrap());assert_eq!(snapshot(&published.backup.unwrap(), false), previous);
        assert_eq!(snapshot(&fixture.project, true), inputs);assert!(fixture.stages().is_empty());assert!(!published.directory.join("saves").exists());
        for name in ["game.js", "game_bg.wasm", "snippets/helper.js"] { assert_eq!(fs::read(published.directory.join(name)).unwrap(), fs::read(fixture.runtime.join(name)).unwrap()); }
        for name in ["scripts/main.rvn", "scripts/chapter.rvn", "assets/picture.txt", "locales/fr.toml"] { assert_eq!(fs::read(published.directory.join(name)).unwrap(), fs::read(fixture.project.join(name)).unwrap()); }
        assert!(published.directory.join("index.html").is_file());let manifest = fs::read_to_string(published.directory.join("rvn_web_manifest.toml")).unwrap();assert!(manifest.contains("scripts/main.rvn")&&manifest.contains("scripts/chapter.rvn"));
    }
    #[test]fn competing_output_does_not_get_replaced_and_previous_export_remains_in_backup() {
        let fixture = Fixture::new(true);let previous = snapshot(&fixture.target(), false);
        let error = build(&fixture.project, &fixture.config(), "Safe", || Ok(fixture.runtime()), |_| { fs::create_dir(fixture.target())?;fs::write(fixture.target().join("third-party.txt"), b"competing export")?;Ok(()) }).unwrap_err();
        assert!(format!("{error:#}").contains("rollback suspended"), "{error:#}");assert_eq!(fs::read(fixture.target().join("third-party.txt")).unwrap(), b"competing export");
        let backups: Vec<_> = fs::read_dir(fixture.project.join("dist-web")).unwrap().map(|entry|entry.unwrap().path()).filter(|path|path.file_name().unwrap().to_string_lossy().starts_with(".rvn-web-backup-")).collect();
        assert_eq!(backups.len(), 1);assert_eq!(snapshot(&backups[0], false), previous);assert!(fixture.stages().is_empty());
    }
    #[test]fn dist_web_junction_is_refused_before_runtime_and_external_export_remains_exact() {
        let mut fixture = Fixture::new(false);let outside = fixture.owned.join("outside");fs::create_dir_all(outside.join("Safe")).unwrap();fs::write(outside.join("Safe/old.html"), b"external previous export").unwrap();
        let before = snapshot(&outside, false);fixture.link_dist(&outside);let inputs = snapshot(&fixture.project, true);
        let error = build(&fixture.project, &fixture.config(), "Safe", || panic!("junction must be refused before runtime"), |_| Ok(())).unwrap_err();
        assert!(format!("{error:#}").contains("symlink/reparse"), "{error:#}");assert_eq!(snapshot(&outside, false), before);assert_eq!(snapshot(&fixture.project, true), inputs);
    }
}
