//! Portable desktop source closure. Resolve `use` just as the parser/check do,
//! but never ship files outside the project, linked files, or private outputs.
use anyhow::{bail, ensure, Context, Result};
use rvn_parser::Statement;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) struct Scripts {
    // Retain the validated original bytes, including line endings. Do not
    // serialize a parsed AST or reopen potentially changed inputs during copy.
    files: BTreeMap<PathBuf, Vec<u8>>,
    directories: BTreeSet<PathBuf>,
}

impl Scripts {
    pub(crate) fn copy_to(&self, destination: &Path) -> Result<()> {
        // Retain empty directories traversed by a raw use such as
        // empty/../shared/file.rvn. Copy no unrelated directory contents.
        for relative in &self.directories {
            fs::create_dir_all(destination.join(relative))?;
        }
        for (relative, bytes) in &self.files {
            let target = destination.join(relative);
            fs::create_dir_all(target.parent().context("missing script parent")?)?;
            fs::write(&target, bytes).with_context(|| {
                format!("unable to write bundled script '{}'", target.display())
            })?;
        }
        Ok(())
    }
}

struct Collector {
    root: PathBuf,
    saves: PathBuf,
    loaded: HashSet<PathBuf>,
    stack: Vec<PathBuf>,
    files: BTreeMap<PathBuf, Vec<u8>>,
    directories: BTreeSet<PathBuf>,
}

pub(crate) fn collect(project: &Path, main_script: &Path, saves: &Path) -> Result<Scripts> {
    let root = fs::canonicalize(project)
        .with_context(|| format!("unable to resolve project '{}'", project.display()))?;
    let mut collector = Collector {
        root: root.clone(),
        saves: normalize_relative(saves).context("the saves directory must be project-relative")?,
        loaded: HashSet::new(),
        stack: Vec::new(),
        files: BTreeMap::new(),
        directories: BTreeSet::new(),
    };
    let main = collector.resolve(&root, main_script)?;
    collector.visit(&main)?;
    Ok(Scripts {
        files: collector.files,
        directories: collector.directories,
    })
}

fn normalize_relative(path: &Path) -> Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir => ensure!(
                relative.pop(),
                "path escapes the project: '{}'",
                path.display()
            ),
            Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "absolute paths cannot be shipped portably: '{}'",
                    path.display()
                )
            }
        }
    }
    ensure!(
        !relative.as_os_str().is_empty(),
        "path must name a file or directory inside the project"
    );
    Ok(relative)
}

#[cfg(test)]
fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points also redirect the original path.
        return metadata.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

impl Collector {
    fn allow_relative(&self, relative: &Path) -> Result<()> {
        let first = relative.components().next().and_then(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        });
        let output = first.is_some_and(|name| {
            [
                "saves",
                "exports",
                "dist",
                "dist-web",
                ".git",
                ".editor-history",
                ".rvn-backups",
                ".rvn-authoring.json",
                ".rvn-authoring.lock",
                "target",
            ]
            .iter()
            .any(|private| name.eq_ignore_ascii_case(private))
        });
        // Path comparisons on Windows must match filesystem case handling.
        let in_saves = if cfg!(windows) {
            relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .zip(
                    self.saves
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().to_lowercase()),
                )
                .all(|(left, right)| left == right)
                && relative.components().count() >= self.saves.components().count()
        } else {
            relative.starts_with(&self.saves)
        };
        ensure!(
            !output && !in_saves,
            "desktop script import refers to private saves/output: '{}'",
            relative.display()
        );
        Ok(())
    }

    fn resolve(&mut self, base: &Path, raw: &Path) -> Result<PathBuf> {
        let mut path = base.to_path_buf();
        for component in raw.components() {
            match component {
                Component::Normal(part) => {
                    path.push(part);
                    let relative = path
                        .strip_prefix(&self.root)
                        .context("import escapes project")?;
                    self.allow_relative(relative)?;
                    let metadata = crate::desktop_paths::validate_entry(&path)?;
                    if metadata.is_dir() {
                        self.directories.insert(relative.to_path_buf());
                    }
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    ensure!(
                        path != self.root && path.pop(),
                        "desktop import escapes project: '{}'",
                        raw.display()
                    );
                }
                Component::RootDir | Component::Prefix(_) => {
                    bail!(
                        "absolute desktop import cannot be shipped portably: '{}'",
                        raw.display()
                    )
                }
            }
        }
        let canonical = fs::canonicalize(base.join(raw))
            .with_context(|| format!("unable to resolve RVN import '{}'", path.display()))?;
        let relative = canonical
            .strip_prefix(&self.root)
            .context("desktop import escapes project")?;
        self.allow_relative(relative)?;
        Ok(canonical)
    }

    fn targets(&mut self, base: &Path, raw: &str) -> Result<Vec<PathBuf>> {
        if raw.ends_with("/*") || raw.ends_with("/*.rvn") {
            let dir_part = raw
                .strip_suffix("/*.rvn")
                .or_else(|| raw.strip_suffix("/*"))
                .unwrap();
            let directory = self.resolve(base, Path::new(dir_part))?;
            let mut files = Vec::new();
            for entry in fs::read_dir(&directory)
                .with_context(|| format!("unable to read use wildcard '{}'", directory.display()))?
            {
                let path = entry?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("rvn") {
                    files.push(self.resolve(
                        &directory,
                        Path::new(path.file_name().context("missing imported filename")?),
                    )?);
                }
            }
            files.sort();
            ensure!(
                !files.is_empty(),
                "no .rvn files for use wildcard '{}'",
                raw
            );
            Ok(files)
        } else {
            Ok(vec![self.resolve(base, Path::new(raw))?])
        }
    }

    fn visit(&mut self, path: &Path) -> Result<()> {
        if let Some(position) = self.stack.iter().position(|loaded| loaded == path) {
            let mut cycle: Vec<_> = self.stack[position..]
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            cycle.push(path.display().to_string());
            bail!("cycle de use détecté: {}", cycle.join(" -> "));
        }
        if !self.loaded.insert(path.to_path_buf()) {
            return Ok(());
        }
        ensure!(
            fs::metadata(path)?.is_file(),
            "RVN import is not a regular file: '{}'",
            path.display()
        );
        let bytes = fs::read(path)
            .with_context(|| format!("unable to read RVN import '{}'", path.display()))?;
        let source = std::str::from_utf8(&bytes)
            .with_context(|| format!("RVN import is not UTF-8: '{}'", path.display()))?;
        let script = rvn_parser::parse(source)
            .map_err(|error| anyhow::anyhow!("RVN parse error in '{}': {error}", path.display()))?;
        self.stack.push(path.to_path_buf());
        let base = path.parent().context("missing imported script parent")?;
        for statement in script {
            if let Statement::Use { paths } = statement {
                for raw in paths {
                    for target in self.targets(base, &raw)? {
                        self.visit(&target)?;
                    }
                }
            }
        }
        self.stack.pop();
        self.files
            .insert(path.strip_prefix(&self.root)?.to_path_buf(), bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        owned: PathBuf,
        project: PathBuf,
        output: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let owned = std::env::temp_dir().join(format!(
                "rvn-desktop-imports-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let project = owned.join("original");
            let output = owned.join("portable");
            fs::create_dir_all(&project).unwrap();
            Self {
                owned,
                project,
                output,
            }
        }
        fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
            let file = self.project.join(relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, bytes).unwrap();
        }
        fn remove_original(&self) {
            let owned = fs::canonicalize(&self.owned).unwrap();
            let original = fs::canonicalize(&self.project).unwrap();
            assert!(original.is_absolute() && original != owned && original.starts_with(&owned));
            assert!(!linked(&fs::symlink_metadata(&self.project).unwrap()));
            fs::remove_dir_all(&original).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // This uniquely created tree contains only this test's originals,
            // exported data and optional link targets, never user projects.
            let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
            let owned = fs::canonicalize(&self.owned).unwrap();
            assert!(owned.is_absolute() && owned.parent() == Some(temp.as_path()));
            assert!(owned
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("rvn-desktop-imports-"));
            assert!(!linked(&fs::symlink_metadata(&self.owned).unwrap()));
            let _ = fs::remove_dir_all(&owned);
        }
    }

    #[test]
    fn root_and_nested_desktop_scripts_run_without_original_sources() {
        // Cover authored root main, generated Blueprint root main, and the
        // established scripts/main path. The final import is relative to its
        // importing chapter, not to the project or root script.
        for main in ["main.rvn", "project.generated.rvn", "scripts/main.rvn"] {
            let fixture = Fixture::new();
            let chapter_use = if main.starts_with("scripts/") {
                "chapter.rvn"
            } else {
                "scripts/chapter.rvn"
            };
            let main_bytes =
                format!("use \"{chapter_use}\"\r\nlabel start\r\njump chapter\r\n").into_bytes();
            let chapter = b"use \"parts/neighbor.rvn\"\r\nlabel chapter\r\njump neighbor\r\n";
            let neighbor = "label neighbor\r\n\"Été à Montréal 🍃\"\r\nreturn\r\n".as_bytes();
            fixture.write(main, &main_bytes);
            fixture.write("scripts/chapter.rvn", chapter);
            fixture.write("scripts/parts/neighbor.rvn", neighbor);
            fixture.write("scripts/unused.rvn", "label unused\n\"not imported\"\n");
            fixture.write("scripts/personal.sav", b"personal progress");
            fixture.write("saves/slot-1.sav", b"original save remains private");
            fixture.write("exports/previous/main.rvn", b"old export remains private");
            fixture.write("rvn.toml", format!("[project]\ntitle='Portable imports'\nmain_script='{main}'\nstart_label='start'\n"));
            fs::create_dir_all(fixture.project.join("assets")).unwrap();
            fs::create_dir_all(fixture.project.join("locales")).unwrap();
            let cfg = crate::load_project_config(&fixture.project).unwrap();
            crate::copy_project_files(&fixture.project, &fixture.output, &cfg).unwrap();
            let data = fixture.output.join("data");
            for (path, bytes) in [
                (main, main_bytes.as_slice()),
                ("scripts/chapter.rvn", chapter.as_slice()),
                ("scripts/parts/neighbor.rvn", neighbor),
            ] {
                assert_eq!(
                    fs::read(data.join(path)).unwrap(),
                    bytes,
                    "exact bytes: {path}"
                );
            }
            assert!(!data.join("scripts/unused.rvn").exists());
            assert!(!data.join("scripts/personal.sav").exists());
            assert!(!data.join("exports").exists());
            assert_eq!(fs::read_dir(data.join("saves")).unwrap().count(), 0);
            // Remove only this test's source tree, then use the same manifest,
            // recursive parser and Engine start-label path as a standalone game.
            fixture.remove_original();
            let copied_cfg = crate::load_project_config(&data).unwrap();
            let script =
                rvn_parser::parse_file_with_uses(data.join(&copied_cfg.project.main_script))
                    .unwrap();
            let mut engine = rvn_core::Engine::new(script, rvn_core::TerminalRenderer, 32).unwrap();
            engine.state.pc = engine.script.iter().position(|statement| {
                matches!(statement, Statement::Label { name } if Some(name) == copied_cfg.project.start_label.as_ref())
            }).unwrap();
            assert!(
                matches!(engine.step_until_interaction().unwrap(), Some(rvn_core::Interaction::Dialogue { text, .. }) if text == "Été à Montréal 🍃")
            );
        }
    }

    #[test]
    fn wildcard_neighbor_and_repeated_imports_match_recursive_parser() {
        let fixture = Fixture::new();
        fixture.write(
            "main.rvn",
            "use \"scripts/*.rvn\"\nuse \"shared/value.rvn\"\nlabel start\n\"Ready\"\n",
        );
        fixture.write(
            "scripts/b.rvn",
            "use \"../shared/value.rvn\"\nlabel b\nreturn\n",
        );
        fs::create_dir_all(fixture.project.join("empty")).unwrap();
        fixture.write(
            "scripts/a.rvn",
            "use \"../empty/../shared/value.rvn\"\nlabel a\nreturn\n",
        );
        fixture.write("shared/value.rvn", "function value() { return 42 }\n");
        fixture.write(
            "scripts/ignored.RVN",
            "invalid and not part of parser wildcard",
        );
        let expected = rvn_parser::parse_file_with_uses(fixture.project.join("main.rvn")).unwrap();
        let scripts = collect(&fixture.project, Path::new("main.rvn"), Path::new("saves")).unwrap();
        assert_eq!(scripts.files.len(), 4);
        scripts.copy_to(&fixture.output).unwrap();
        assert!(fixture.output.join("empty").is_dir());
        assert_eq!(
            fs::read_dir(fixture.output.join("empty")).unwrap().count(),
            0
        );
        fixture.remove_original();
        let actual = rvn_parser::parse_file_with_uses(fixture.output.join("main.rvn")).unwrap();
        assert_eq!(
            actual, expected,
            "sorted wildcards and canonical dedup must agree with the actual parser"
        );
        let labels: Vec<_> = actual
            .iter()
            .filter_map(|statement| match statement {
                Statement::Label { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["a", "b", "start"]);
    }

    #[test]
    fn external_absolute_and_private_imports_fail_before_any_desktop_copy() {
        let fixture = Fixture::new();
        fs::write(
            fixture.owned.join("external.rvn"),
            "label outside\nreturn\n",
        )
        .unwrap();
        fixture.write(
            "rvn.toml",
            "[project]\nmain_script='main.rvn'\n[paths]\nsaves='scripts/progress'\n",
        );
        fixture.write("scripts/progress/personal.rvn", "label private\nreturn\n");
        fs::create_dir_all(fixture.project.join("assets")).unwrap();
        fs::create_dir_all(fixture.project.join("locales")).unwrap();
        for (path, expected) in [
            ("../external.rvn", "escapes project"),
            ("scripts/progress/personal.rvn", "private saves/output"),
            ("exports/old.rvn", "private saves/output"),
            ("dist/old.rvn", "private saves/output"),
            ("saves/old.rvn", "private saves/output"),
            (".editor-history/old.rvn", "private saves/output"),
            (".rvn-backups/old.rvn", "private saves/output"),
        ] {
            fixture.write("main.rvn", format!("use \"{path}\"\nlabel start\nreturn\n"));
            let cfg = crate::load_project_config(&fixture.project).unwrap();
            let error =
                crate::copy_project_files(&fixture.project, &fixture.output, &cfg).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
            assert!(
                !fixture.output.exists(),
                "failed preflight must not copy a partial desktop bundle"
            );
        }
        // Absolute paths remain dependent on the original machine even when
        // their target lies inside this project; never rewrite their RVN bytes.
        fixture.write("main.rvn", "label start\nreturn\n");
        let absolute_main = fs::canonicalize(fixture.project.join("main.rvn")).unwrap();
        assert!(format!(
            "{:#}",
            collect(&fixture.project, &absolute_main, Path::new("saves"))
                .err()
                .unwrap()
        )
        .contains("absolute"));
    }

    #[test]
    fn cycles_missing_and_empty_wildcards_are_refused_consistently() {
        let fixture = Fixture::new();
        fixture.write("a.rvn", "use \"main.rvn\"\nlabel a\nreturn\n");
        fs::create_dir_all(fixture.project.join("empty")).unwrap();
        for source in [
            "use \"a.rvn\"\nlabel start\nreturn\n",
            "use \"missing.rvn\"\nlabel start\nreturn\n",
            "use \"empty/*\"\nlabel start\nreturn\n",
        ] {
            fixture.write("main.rvn", source);
            assert!(rvn_parser::parse_file_with_uses(fixture.project.join("main.rvn")).is_err());
            assert!(collect(&fixture.project, Path::new("main.rvn"), Path::new("saves")).is_err());
            assert!(!fixture.output.exists());
        }
    }

    #[test]
    fn desktop_resources_refuse_root_external_output_and_progress_overlap_before_copying() {
        let fixture = Fixture::new();
        fixture.write("main.rvn", "label start\n\"Ready\"\n");
        fixture.write("assets/image.txt", b"original asset bytes");
        fixture.write("assets/private-saves/slot1.sav", b"personal save bytes");
        fs::create_dir_all(fixture.project.join("locales")).unwrap();
        for paths in [
            "assets='.'",
            "assets='../outside'",
            "locales='exports'",
            "theme='dist/secret.toml'",
            "saves='.'",
            "saves='../outside'",
            "saves='assets/private-saves'",
            "menus='../external.rvnui'",
            "theme='.rvn-backups/old.toml'",
        ] {
            fixture.write(
                "rvn.toml",
                format!("[project]\nmain_script='main.rvn'\n[paths]\n{paths}\n"),
            );
            let cfg = crate::load_project_config(&fixture.project).unwrap();
            let error =
                crate::copy_project_files(&fixture.project, &fixture.output, &cfg).unwrap_err();
            assert!(format!("{error:#}").contains("Desktop"), "{error:#}");
            assert!(
                !fixture.output.exists(),
                "preflight must not touch output for {paths}"
            );
            assert_eq!(
                fs::read(fixture.project.join("assets/image.txt")).unwrap(),
                b"original asset bytes"
            );
            assert_eq!(
                fs::read(fixture.project.join("assets/private-saves/slot1.sav")).unwrap(),
                b"personal save bytes"
            );
        }
        let absolute_assets = fixture
            .project
            .join("assets")
            .to_string_lossy()
            .into_owned();
        fixture.write(
            "rvn.toml",
            format!("[project]\nmain_script='main.rvn'\n[paths]\nassets={absolute_assets:?}\n"),
        );
        let cfg = crate::load_project_config(&fixture.project).unwrap();
        assert!(format!(
            "{:#}",
            crate::copy_project_files(&fixture.project, &fixture.output, &cfg).unwrap_err()
        )
        .contains("project-relative"));
        assert!(!fixture.output.exists());
        fixture.write("rvn.toml", "[project]\nmain_script='scripts/../main.rvn'\n");
        let cfg = crate::load_project_config(&fixture.project).unwrap();
        assert!(crate::copy_project_files(&fixture.project, &fixture.output, &cfg).is_err());
        assert!(!fixture.output.exists());
    }

    #[test]
    fn imported_symlink_or_windows_junction_is_explicitly_refused() {
        let fixture = Fixture::new();
        let outside = fixture.owned.join("linked-target");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("chapter.rvn"), "label chapter\nreturn\n").unwrap();
        fs::create_dir_all(fixture.project.join("assets")).unwrap();
        let link = fixture.project.join("assets").join("linked");
        assert!(fs::canonicalize(&outside)
            .unwrap()
            .starts_with(fs::canonicalize(&fixture.owned).unwrap()));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // A junction in this owned Temp tree needs no symlink privilege.
            // mklink is a cmd builtin, so its path arguments require cmd's
            // explicit quotes rather than CRT escaping. Keep each argument
            // separate and reject expansion characters in this owned fixture.
            let quoted_path = |path: &Path| {
                let text = path.to_str().expect("Private fixture path must be Unicode");
                assert!(!text.contains(['"', '%', '\r', '\n']));
                format!("\"{text}\"")
            };
            let result = std::process::Command::new("cmd.exe")
                .args(["/D", "/C", "mklink", "/J"])
                .raw_arg(quoted_path(&link))
                .raw_arg(quoted_path(&outside))
                .creation_flags(0x08000000)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "unable to create private test junction: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        fixture.write(
            "main.rvn",
            "use \"assets/linked/chapter.rvn\"\nlabel start\nreturn\n",
        );
        let error = collect(&fixture.project, Path::new("main.rvn"), Path::new("saves"))
            .err()
            .unwrap();
        assert!(
            format!("{error:#}").contains("symlink/reparse"),
            "{error:#}"
        );
        assert!(!fixture.output.exists());
        // The same actual junction must also be rejected when encountered
        // inside a resource directory, not just in the script import closure.
        fixture.write("main.rvn", "label start\nreturn\n");
        fixture.write("rvn.toml", "[project]\nmain_script='main.rvn'\n");
        fs::create_dir_all(fixture.project.join("locales")).unwrap();
        let cfg = crate::load_project_config(&fixture.project).unwrap();
        let error = crate::copy_project_files(&fixture.project, &fixture.output, &cfg).unwrap_err();
        assert!(
            format!("{error:#}").contains("symlink/reparse"),
            "{error:#}"
        );
        assert!(!fixture.output.exists());
        assert_eq!(
            fs::read(outside.join("chapter.rvn")).unwrap(),
            b"label chapter\nreturn\n"
        );
    }
}
