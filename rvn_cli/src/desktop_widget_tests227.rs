//! Targeted CPU regressions only; fake runtime bytes are never launched.
use super::*;
use rvn_graph::{GraphKind, SourceWorkspace};
use rvn_parser::widget_asset::{WidgetBlueprintV1, WidgetGraphRef16};
use std::sync::atomic::{AtomicU64, Ordering};

const UUID: &str = "17411a3e431e4c9388b78a2b9825b130";
const LOCATOR: &str = "widgets/Panel.rvnwidget";
struct Fixture { owned: PathBuf, project: PathBuf, runtime: PathBuf }
impl Fixture {
    fn new(native: bool, rooted: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let owned = std::env::temp_dir().join(format!("rvn-desktop-widget227-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(owned.join("project/assets/widgets")).unwrap();
        let owned = owned.canonicalize().unwrap();
        let f = Self { project: owned.join("project"), runtime: owned.join("runtime.exe"), owned };
        fs::write(&f.runtime, b"isolated fake runtime; never launched").unwrap();
        let returned = if rooted {
            r#"component("root","column",{"layout_contract":"widget_v1"},[component("go","button",{"events":{"click":"clicked"}},[component("caption","text",{"text":"Été — 雨"},[])])])"#
        } else { "[]" };
        let raw = format!("screen panel(){{return {returned}}}\nhandler clicked(event){{return 0}}\nfunction value(){{return 0}}\ninit{{set hits=0}}\nlabel start\n\"Ready\"\nreturn\n");
        let mut graphs = rvn_graph::import_script(&rvn_parser::parse(&raw).unwrap()).unwrap();
        if native { for graph in &mut graphs { graph.enable_native_frame_preparation().unwrap(); } }
        let source = if native { rvn_graph::transpile_project(&graphs).unwrap().source } else { raw };
        f.write("main.rvn", &source);
        f.write("rvn.toml", "[project]\nmain_script='main.rvn'\nstart_label='start'\n");
        // Native source carries its own real identities. A legacy fixture has
        // a genuine linked authoring cache, which the Desktop bundle excludes.
        if !native {
            let _linked = SourceWorkspace::link(&f.project, &f.project.join("main.rvn"), &graphs).unwrap();
        }
        let screen = graphs.iter().find(|g| matches!(g.kind, GraphKind::Screen {..})).unwrap().graph_id;
        let members = graphs.iter().filter(|g| matches!(g.kind, GraphKind::Screen {..}|GraphKind::Handler {..}|GraphKind::Function {..}))
            .map(|g| WidgetGraphRef16::from_u64(g.graph_id.get())).collect();
        let descriptor = WidgetBlueprintV1 { uuid: UUID.into(), source: "main.rvn".into(),
            screen_graph: WidgetGraphRef16::from_u64(screen.get()), members };
        f.write(&format!("assets/{LOCATOR}"), descriptor.encode().unwrap());
        f.write("rvn.toml", format!("[project]\nmain_script='main.rvn'\nstart_label='start'\n[widget_blueprints]\n'{UUID}'='{LOCATOR}'\n"));
        f.write("locales/fr.toml", b"title='Bonjour'\n");
        f.write("saves/slot.sav", b"original personal progress");
        fs::create_dir_all(f.target().join("data/saves")).unwrap();
        fs::write(f.target().join("old.exe"), b"previous executable").unwrap();
        fs::write(f.target().join("data/saves/slot.sav"), b"previous exported progress").unwrap();
        f
    }
    fn write(&self, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = self.project.join(relative); fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, bytes).unwrap();
    }
    fn config(&self) -> crate::ProjectConfig { crate::load_project_config(&self.project).unwrap() }
    fn manifest(&self) -> Vec<u8> { fs::read(self.project.join("rvn.toml")).unwrap() }
    fn target(&self) -> PathBuf { self.project.join("dist").join(format!("Safe-{}", crate::detect_platform().unwrap())) }
    fn data(&self) -> PathBuf {
        let destination = self.owned.join("portable");
        fs::create_dir(&destination).unwrap();
        crate::copy_project_files(&self.project, &destination, &self.config()).unwrap();
        destination.join("data")
    }
    fn stages(&self) -> Vec<PathBuf> {
        fs::read_dir(self.project.join("dist")).unwrap().map(|e| e.unwrap().path())
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(".rvn-desktop-stage-")).collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let (Ok(owned), Ok(temp)) = (self.owned.canonicalize(), std::env::temp_dir().canonicalize()) else { return; };
        if owned.parent() != Some(temp.as_path()) || !owned.file_name().unwrap().to_string_lossy().starts_with("rvn-desktop-widget227-") { return; }
        if crate::desktop_paths::validate_entry(&owned).is_ok() && crate::desktop_paths::check_tree(&owned).is_ok() { let _ = fs::remove_dir_all(owned); }
    }
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path(); let meta = crate::desktop_paths::validate_entry(&path).unwrap();
            if meta.is_dir() { walk(root, &path, files); } else { files.insert(path.strip_prefix(root).unwrap().to_owned(), fs::read(path).unwrap()); }
        }
    }
    let mut result = BTreeMap::new(); walk(root, root, &mut result); result
}

#[test]
fn desktop_widget_preflight_accepts_native_empty_and_rooted_exact_delivered_members() {
    for rooted in [false, true] {
        let f = Fixture::new(true, rooted);
        if rooted {
            let manifest = String::from_utf8(f.manifest()).unwrap().replace("main_script='main.rvn'", "main_script='./main.rvn'");
            f.write("rvn.toml", manifest);
        }
        // This original editor file is not a runtime participant or a donor of
        // Native identity; even invalid editor bytes must not be copied/read.
        f.write(".rvn-authoring.json", b"private editor cache, deliberately not decoded");
        let original = snapshot(&f.project); let data = f.data();
        assert!(!data.join(".rvn-authoring.json").exists());
        let admitted = prepare(&data, &f.config(), &f.manifest()).unwrap();
        admitted.verify(&data, &f.config(), &f.manifest()).unwrap();
        assert!(admitted.inputs.contains_key(Path::new("main.rvn")));
        assert!(admitted.inputs.contains_key(&Path::new("assets").join(LOCATOR)));
        assert_eq!(fs::read(data.join("main.rvn")).unwrap(), fs::read(f.project.join("main.rvn")).unwrap());
        assert_eq!(snapshot(&f.project), original);
    }
    check_imported_class_closure();
}

#[test]
fn desktop_widget_preflight_refuses_legacy_members_despite_original_editor_cache() {
    let f = Fixture::new(false, true); assert!(f.project.join(".rvn-authoring.json").is_file());
    let original = snapshot(&f.project); let data = f.data(); assert!(!data.join(".rvn-authoring.json").exists());
    let error = prepare(&data, &f.config(), &f.manifest()).err().expect("legacy identity cannot ship");
    let text = format!("{error:#}");
    for needle in [UUID, LOCATOR, "member", "upgrade", "Save"] { assert!(text.contains(needle), "{text}"); }
    assert_eq!(snapshot(&f.project), original);
}

#[test]
fn desktop_widget_preflight_rejects_malformed_index_descriptor_uuid_and_missing_member() {
    for case in 0..5 {
        let f = Fixture::new(true, false); let data = f.data(); let descriptor = data.join("assets").join(LOCATOR);
        match case {
            0 => fs::write(data.join("rvn.toml"), "widget_blueprints='not a table'\n[project]\nmain_script='main.rvn'\n").unwrap(),
            1 => fs::write(&descriptor, b"malformed Widget descriptor").unwrap(),
            2 | 3 | 4 => {
                let mut value = WidgetBlueprintV1::decode(&fs::read(&descriptor).unwrap()).unwrap();
                if case == 2 { value.uuid = "797679920f7142be99d9d6fe5553ca34".into(); }
                else if case == 3 { value.members.push(WidgetGraphRef16::from_u64(u64::MAX)); }
                else { value.screen_graph = *value.members.iter().find(|id| **id != value.screen_graph).unwrap(); }
                fs::write(&descriptor, value.encode().unwrap()).unwrap();
            }
            _ => unreachable!(),
        }
        let manifest = fs::read(data.join("rvn.toml")).unwrap();
        let text = format!("{:#}", prepare(&data, &f.config(), &manifest).err().expect("invalid delivered Widget must refuse"));
        if case == 0 { assert!(text.contains("[widget_blueprints]"), "{text}"); }
        else { assert!(text.contains(UUID) && text.contains(LOCATOR), "{text}"); }
        if case == 3 { assert!(text.contains("ffffffffffffffff") && text.contains("member"), "{text}"); }
    }
}

#[test]
fn desktop_widget_preflight_empty_index_preserves_non_widget_source_behavior() {
    for empty_table in [false, true] {
        let f = Fixture::new(false, false); let data = f.data();
        let manifest = format!("[project]\nmain_script='main.rvn'\n{}", if empty_table { "[widget_blueprints]\n" } else { "" });
        fs::write(data.join("rvn.toml"), &manifest).unwrap();
        // Existing CLI validation, not this additional Widget pass, owns all
        // non-Widget source checks. No Widget pass may demand a Native program.
        fs::write(data.join("main.rvn"), b"non-Widget parser ownership remains unchanged").unwrap();
        let prepared = prepare(&data, &f.config(), manifest.as_bytes()).unwrap();
        assert_eq!(prepared.inputs.len(), 1);
        prepared.verify(&data, &f.config(), manifest.as_bytes()).unwrap();
    }
}

fn check_imported_class_closure() {
    let f = Fixture::new(true, true);
    let widget_source = fs::read(f.project.join("main.rvn")).unwrap();
    f.write("widgets/panel.rvn", &widget_source);
    f.write("extras/one.rvn", "function side_one(){return 1}\n");
    f.write("main.rvn", "use \"widgets/panel.rvn\"\nuse \"extras/*.rvn\"\n");
    let path = f.project.join("assets").join(LOCATOR);
    let mut descriptor = WidgetBlueprintV1::decode(&fs::read(&path).unwrap()).unwrap();
    descriptor.source = "widgets/panel.rvn".into();
    fs::write(path, descriptor.encode().unwrap()).unwrap();
    let original = snapshot(&f.project);
    let data = f.data(); let prepared = prepare(&data, &f.config(), &f.manifest()).unwrap();
    assert!(prepared.inputs.contains_key(Path::new("widgets/panel.rvn")));
    assert!(prepared.inputs.contains_key(Path::new("extras/one.rvn")));
    // Existing file hashes are unchanged. A fresh parse must still detect the
    // new real wildcard participant rather than checking only known paths.
    fs::write(data.join("extras/two.rvn"), "function side_two(){return 2}\n").unwrap();
    let error = prepared.verify(&data, &f.config(), &f.manifest()).unwrap_err();
    assert!(format!("{error:#}").contains("inputs changed after preflight"), "{error:#}");
    assert_eq!(snapshot(&f.project), original);
}

#[cfg(not(feature = "video"))]
#[test]
fn desktop_widget_build_refuses_legacy_before_success_without_replacing_previous_export() {
    let f = Fixture::new(false, true); let before = snapshot(&f.project);
    let error = crate::build_desktop_project_with_runtime(&f.project, &f.config(), "Safe", || Ok(f.runtime.clone()), |_| Ok(())).unwrap_err();
    let text = format!("{error:#}"); assert!(text.contains(UUID) && text.contains(LOCATOR), "{text}");
    assert_eq!(snapshot(&f.project), before); assert!(f.stages().is_empty());
}

#[cfg(not(feature = "video"))]
#[test]
fn desktop_widget_build_publishes_native_data_without_editor_cache_and_keeps_originals() {
    let f = Fixture::new(true, true); f.write(".rvn-authoring.json", b"editor-only state");
    let inputs = snapshot(&f.project); let previous = snapshot(&f.target());
    let published = crate::build_desktop_project_with_runtime(&f.project, &f.config(), "Safe", || Ok(f.runtime.clone()), |_| Ok(())).unwrap();
    assert_eq!(snapshot(&published.backup.unwrap()), previous);
    let data = published.directory.join("data"); assert!(!data.join(".rvn-authoring.json").exists());
    prepare(&data, &f.config(), &f.manifest()).unwrap().verify(&data, &f.config(), &f.manifest()).unwrap();
    assert_eq!(fs::read_dir(data.join("saves")).unwrap().count(), 0);
    for (relative, bytes) in inputs { if !relative.starts_with("dist") { assert_eq!(fs::read(f.project.join(relative)).unwrap(), bytes); } }
    assert!(f.stages().is_empty());
}

#[cfg(not(feature = "video"))]
#[test]
fn desktop_widget_build_final_fence_refuses_changed_source_or_descriptor_and_restores_previous() {
    for descriptor_change in [false, true] {
        let f = Fixture::new(true, true); let before = snapshot(&f.project);
        let error = crate::build_desktop_project_with_runtime(&f.project, &f.config(), "Safe", || Ok(f.runtime.clone()), |stage| {
            if descriptor_change { fs::write(stage.join("data/assets").join(LOCATOR), b"changed after class preflight")?; }
            else { let path = stage.join("data/main.rvn"); let mut bytes = fs::read(&path)?; bytes.extend_from_slice(b"\n// changed source bytes, same class AST\n"); fs::write(path, bytes)?; }
            Ok(())
        }).unwrap_err();
        let text = format!("{error:#}"); assert!(text.contains("previous Desktop export was restored"), "{text}");
        if descriptor_change { assert!(text.contains(LOCATOR), "{text}"); }
        else { assert!(text.contains("inputs changed after preflight"), "{text}"); }
        assert_eq!(snapshot(&f.project), before); assert!(f.stages().is_empty());
    }
}
