use super::*;
use crate::bevy_renderer::BevyRenderer;
use rvn_core::{Engine, Interaction, string_table::{StringTableEntry, StringTableResources}};
use std::{fs, path::PathBuf, time::{Duration, Instant, SystemTime, UNIX_EPOCH}, sync::atomic::{AtomicU64, Ordering}};

const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
const OTHER: &str = "17411a3e431e4c9388b78a2b9825b131";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { root: PathBuf, app: App }
fn table(id: &str, source: &str) -> StringTableAsset {
    StringTableAsset { id: id.into(), namespace: "VN".into(), entries: BTreeMap::from([("hello".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: "Contexte été".into() })]) }
}
fn write(root: &std::path::Path, id: &str, source: &str) {
    fs::write(root.join("Table ÉTÉ.RVNSTRINGS"), StringTableFile::new(table(id, source)).unwrap().encode().unwrap()).unwrap();
}
fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!("rvn-string-table-loader-0113-{}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&root).unwrap(); write(&root, ID, "First");
    let mut resources = StringTableResources::default(); resources.publish(table(ID, "First")).unwrap();
    let source = format!("init {{set text=__rvn_string_table_text(\"{ID}\",\"hello\")}}\nlabel start\n\"Value: [text]\"\n");
    let engine = Engine::new_with_text_resources(rvn_parser::parse(&source).unwrap(), BevyRenderer::new(), 8, resources).unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin { file_path: root.to_string_lossy().into_owned(), watch_for_changes_override: Some(false), ..default() })
        .insert_resource(VnEngine(engine)).add_event::<VnCommand>()
        .add_plugins(StringTableAssetsPlugin::new(BTreeMap::from([(ID.into(), "Table ÉTÉ.RVNSTRINGS".into())])));
    app.update(); Fixture { root, app }
}
fn pump(f: &mut Fixture, predicate: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        f.app.update();
        if predicate(f.app.world()) { return; }
        assert!(Instant::now() < deadline, "Actual AssetServer load/reload timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn ready(world: &World) -> bool {
    let host = world.resource::<TableHost>();
    world.resource::<Assets<LoadedStringTable>>().get(host.bound[ID].handle.id()).is_some()
}
fn source(world: &World) -> Option<&str> { world.resource::<VnEngine>().0.text_resources().ready_table(ID).map(|table| table.entries["hello"].source.as_str()) }
fn reload(f: &Fixture) { f.app.world().resource::<AssetServer>().reload("Table ÉTÉ.RVNSTRINGS"); }

#[test]
fn actual_async_file_loader_retains_handles_and_initial_duplicate_events_preserve_core_identity() {
    let mut f = fixture(); let stamp = f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp();
    pump(&mut f, ready); for _ in 0..4 { f.app.update(); }
    assert_eq!(f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp(), stamp);
    assert!(f.app.world().resource::<StringTableDiagnostics>().0.is_empty());
    assert!(f.app.world().resource::<TableHost>().bound[ID].handle.is_strong());
    { let mut engine = f.app.world_mut().resource_mut::<VnEngine>();
      assert!(matches!(engine.0.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text=="Value: First")); }
    assert_eq!(fs::read(f.root.join("Table ÉTÉ.RVNSTRINGS")).unwrap(), StringTableFile::new(table(ID, "First")).unwrap().encode().unwrap());
}

#[test]
fn actual_reload_replaces_live_reference_and_preserves_independent_resource_snapshot() {
    let mut f = fixture(); pump(&mut f, ready);
    let old = f.app.world().resource::<VnEngine>().0.text_resources().clone();
    let before = old.revision_stamp(); write(&f.root, ID, "New été"); reload(&f);
    pump(&mut f, |world| source(world)==Some("New été"));
    let stamp = { let mut live = f.app.world_mut().resource_mut::<VnEngine>();
      assert_ne!(live.0.text_resources().revision_stamp(), before);
      assert!(matches!(live.0.step_until_interaction().unwrap(), Some(Interaction::Dialogue { text, .. }) if text=="Value: New été"));
      live.0.text_resources().revision_stamp() };
    assert_eq!(old.ready_table(ID).unwrap().entries["hello"].source, "First");
    reload(&f); for _ in 0..10 { f.app.update(); std::thread::sleep(Duration::from_millis(5)); }
    assert_eq!(f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp(), stamp);
}

#[test]
fn actual_async_metadata_reload_reaches_runtime_without_changing_visible_source_or_old_snapshot() {
    let mut f=fixture();pump(&mut f,ready);
    let old=f.app.world().resource::<VnEngine>().0.text_resources().clone();let before=old.revision_stamp();
    let mut extended=table(ID,"First");extended.entries.get_mut("hello").unwrap().set_metadata("Speaker".into(),"Éloïse".into()).unwrap();
    let bytes=StringTableFile::new(extended.clone()).unwrap().encode().unwrap();fs::write(f.root.join("Table ÉTÉ.RVNSTRINGS"),&bytes).unwrap();reload(&f);
    pump(&mut f,|world|world.resource::<VnEngine>().0.text_resources().ready_table(ID).is_some_and(|t|t.entries["hello"].metadata_value("speaker")==Some("Éloïse")));
    let resources=f.app.world().resource::<VnEngine>().0.text_resources();
    assert_ne!(resources.revision_stamp(),before);assert_eq!(source(f.app.world()),Some("First"));assert_eq!(resources.ready_table(ID),Some(&extended));
    assert!(old.ready_table(ID).unwrap().entries["hello"].metadata.is_empty());assert_eq!(fs::read(f.root.join("Table ÉTÉ.RVNSTRINGS")).unwrap(),bytes);
}

#[test]
fn actual_async_metadata_reload_updates_table_queries_at_the_next_execution_boundary() {
    let mut f=fixture();pump(&mut f,ready);
    let old=f.app.world().resource::<VnEngine>().0.text_resources().clone();
    let message=format!("Meta:[__rvn_string_table_metadata(\"{ID}\",\"hello\",\"speaker\")] Source:[__rvn_string_table_source(\"{ID}\",\"hello\")] Keys:[len(__rvn_string_table_keys(\"{ID}\"))]");
    let quoted=serde_json::to_string(&message).unwrap();
    let script=rvn_parser::parse(&format!("label start\n{quoted}\n{quoted}\n")).unwrap();
    let query_engine=Engine::new_with_text_resources(script,BevyRenderer::new(),8,old.clone()).unwrap();
    f.app.world_mut().insert_resource(VnEngine(query_engine));
    {
        let mut live=f.app.world_mut().resource_mut::<VnEngine>();
        assert!(matches!(live.0.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Meta: Source:First Keys:1"));
    }
    let mut extended=table(ID,"First");extended.entries.get_mut("hello").unwrap().set_metadata("Speaker".into(),"Éloïse".into()).unwrap();
    let bytes=StringTableFile::new(extended).unwrap().encode().unwrap();
    fs::write(f.root.join("Table ÉTÉ.RVNSTRINGS"),&bytes).unwrap();reload(&f);
    pump(&mut f,|world|world.resource::<VnEngine>().0.text_resources().ready_table(ID).is_some_and(|table|table.entries["hello"].metadata_value("speaker")==Some("Éloïse")));
    let stamp=f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp();
    {
        let mut live=f.app.world_mut().resource_mut::<VnEngine>();
        live.0.advance_dialogue().unwrap();
        assert!(matches!(live.0.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text=="Meta:Éloïse Source:First Keys:1"));
        assert_eq!(live.0.text_resources().revision_stamp(),stamp);
    }
    assert!(old.ready_table(ID).unwrap().entries["hello"].metadata.is_empty());
    assert_eq!(fs::read(f.root.join("Table ÉTÉ.RVNSTRINGS")).unwrap(),bytes);
}

#[test]
fn retained_table_asset_identity_survives_actual_reload_and_removal_at_dialogue_boundaries() {
    use rvn_parser::Value;
    let mut f=fixture();pump(&mut f,ready);
    let old=f.app.world().resource::<VnEngine>().0.text_resources().clone();
    let message=format!("ID:[__rvn_string_table_id(reference)] Source:[__rvn_string_table_source(\"{ID}\",\"hello\")] Ready:[__rvn_string_table_registered(\"{ID}\")]");
    let quoted=serde_json::to_string(&message).unwrap();
    let script=rvn_parser::parse(&format!(
        "init {{set reference=__rvn_string_table_asset(\"{ID}\")}}\nlabel start\n{quoted}\n{quoted}\nset fresh=__rvn_string_table_asset(\"{ID}\")\n{quoted}\n"
    )).unwrap();
    f.app.world_mut().insert_resource(VnEngine(Engine::new_with_text_resources(script,BevyRenderer::new(),8,old.clone()).unwrap()));
    let reference={
        let mut live=f.app.world_mut().resource_mut::<VnEngine>();
        let stamp=live.0.text_resources().revision_stamp();
        assert!(matches!(live.0.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text==format!("ID:{ID} Source:First Ready:true")));
        assert_eq!(live.0.text_resources().revision_stamp(),stamp);
        let reference=live.0.state.vars["reference"].clone();
        assert_eq!(reference,Value::Dict(BTreeMap::from([
            ("kind".into(),Value::Str("string_table_asset".into())),("id".into(),Value::Str(ID.into())),
        ])));
        reference
    };
    write(&f.root,ID,"Reloaded été");
    let bytes=fs::read(f.root.join("Table ÉTÉ.RVNSTRINGS")).unwrap();reload(&f);
    pump(&mut f,|world|source(world)==Some("Reloaded été"));
    {
        let mut live=f.app.world_mut().resource_mut::<VnEngine>();
        let stamp=live.0.text_resources().revision_stamp();
        live.0.advance_dialogue().unwrap();
        assert!(matches!(live.0.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text==format!("ID:{ID} Source:Reloaded été Ready:true")));
        assert_eq!(live.0.state.vars["reference"],reference);
        assert_eq!(live.0.text_resources().revision_stamp(),stamp);
    }
    let handle=f.app.world().resource::<TableHost>().bound[ID].handle.clone();
    f.app.world_mut().resource_mut::<Assets<LoadedStringTable>>().remove(handle.id());
    pump(&mut f,|world|world.resource::<VnEngine>().0.text_resources().ready_table(ID).is_none());
    {
        let mut live=f.app.world_mut().resource_mut::<VnEngine>();
        let stamp=live.0.text_resources().revision_stamp();
        live.0.advance_dialogue().unwrap();
        assert!(matches!(live.0.step_until_interaction().unwrap(),Some(Interaction::Dialogue{text,..})if text==format!("ID:{ID} Source: Ready:false")));
        assert_eq!(live.0.state.vars["reference"],reference);
        assert_eq!(live.0.state.vars["fresh"],Value::Str(String::new()));
        assert_eq!(live.0.text_resources().revision_stamp(),stamp);
    }
    assert_eq!(old.ready_table(ID).unwrap().entries["hello"].source,"First");
    assert_eq!(fs::read(f.root.join("Table ÉTÉ.RVNSTRINGS")).unwrap(),bytes);
}

#[test]
fn wrong_identity_and_bad_file_report_failure_without_replacing_last_valid_source_then_recover() {
    let mut f = fixture(); pump(&mut f, ready);
    let before = f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp();
    write(&f.root, OTHER, "Foreign"); reload(&f);
    pump(&mut f, |world| world.resource::<StringTableDiagnostics>().0.contains_key(ID));
    assert_eq!(source(f.app.world()), Some("First"));
    assert_eq!(f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp(), before);
    fs::write(f.root.join("Table ÉTÉ.RVNSTRINGS"), b"{broken").unwrap(); reload(&f);
    pump(&mut f, |world| world.resource::<StringTableDiagnostics>().0.get(ID).is_some_and(|error| error.contains("Invalid string-table")));
    assert_eq!(source(f.app.world()), Some("First"));
    write(&f.root, ID, "Recovered"); reload(&f);
    pump(&mut f, |world| source(world)==Some("Recovered") && !world.resource::<StringTableDiagnostics>().0.contains_key(ID));
}

#[test]
fn events_from_unbound_asset_cannot_replace_declared_table_and_removed_asset_revokes_resource() {
    let mut f = fixture(); pump(&mut f, ready);
    let foreign = f.app.world_mut().resource_mut::<Assets<LoadedStringTable>>().add(LoadedStringTable(Arc::new(table(ID, "Unbound"))));
    f.app.update(); f.app.update();
    assert_eq!(source(f.app.world()), Some("First"));
    assert!(f.app.world().resource::<Assets<LoadedStringTable>>().get(foreign.id()).is_some());
    let handle = f.app.world().resource::<TableHost>().bound[ID].handle.clone();
    f.app.world_mut().resource_mut::<Assets<LoadedStringTable>>().remove(handle.id());
    pump(&mut f, |world| world.resource::<VnEngine>().0.text_resources().ready_table(ID).is_none());
}

#[test]
fn late_loaded_notification_cannot_erase_an_actual_current_file_load_failure() {
    let mut f = fixture(); pump(&mut f, ready); for _ in 0..3 { f.app.update(); }
    fs::write(f.root.join("Table ÉTÉ.RVNSTRINGS"), b"{broken").unwrap(); reload(&f);
    pump(&mut f, |world| world.resource::<StringTableDiagnostics>().0.contains_key(ID));
    let handle = f.app.world().resource::<TableHost>().bound[ID].handle.clone();
    assert!(matches!(f.app.world().resource::<AssetServer>().load_state(handle.id()), bevy::asset::LoadState::Failed(_)));
    let stamp = f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp();
    // Replay the old notification, not an invented load result or asset.
    f.app.world_mut().send_event(AssetEvent::<LoadedStringTable>::LoadedWithDependencies { id: handle.id() });
    f.app.update(); f.app.update();
    assert!(f.app.world().resource::<StringTableDiagnostics>().0.contains_key(ID));
    assert_eq!(source(f.app.world()), Some("First"));
    assert_eq!(f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp(), stamp);
    write(&f.root, ID, "First"); reload(&f);
    pump(&mut f, |world| !world.resource::<StringTableDiagnostics>().0.contains_key(ID));
    assert_eq!(f.app.world().resource::<VnEngine>().0.text_resources().revision_stamp(), stamp);
}

#[test]
fn configured_http_asset_root_preserves_project_url_and_encodes_literal_names_once() {
    assert_eq!(http_asset_root("https://example.test/novel/", "artwork ÉTÉ%/"), "https://example.test/novel/artwork%20%C3%89T%C3%89%25/");
    assert_eq!(http_asset_root(".", "assets\\tables"), "./assets/tables");
    assert_eq!(http_asset_path("Table %20 ÉTÉ.RVNSTRINGS"), "Table%20%2520%20%C3%89T%C3%89.RVNSTRINGS");
}
