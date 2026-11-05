use rvn_core::{string_table::{StringTableAsset, StringTableEntry, StringTableReference}, string_table_file::StringTableFile,
    string_table_project::{load_native, url_path, validate_declarations, validate_web_manifest, StringTableDeclarations, StringTablePreparation}};
use std::{collections::BTreeMap, fs, path::PathBuf};
const ID: &str = "17411a3e431e4c9388b78a2b9825b130";
const OTHER: &str = "3fc40c36f4e544ed82943e4c6fe9d87d";
fn bytes(id: &str, source: &str) -> Vec<u8> {
    StringTableFile::new(StringTableAsset { id: id.into(), namespace: "VN".into(), entries: BTreeMap::from([
        ("hello".into(), StringTableEntry {metadata:Default::default(), source: source.into(), developer_notes: String::new() }),
    ]) }).unwrap().encode().unwrap()
}
fn declared() -> StringTableDeclarations { BTreeMap::from([(ID.into(), "tables/Été %2f.rvnstrings".into())]) }

#[test]
fn declaration_paths_ids_aliases_and_literal_http_paths_have_one_explicit_contract() {
    validate_declarations(&declared()).unwrap();
    for path in ["", "/outside.rvnstrings", "../outside.rvnstrings", "tables/../x.rvnstrings", "./x.rvnstrings", "tables//x.rvnstrings", "C:/x.rvnstrings", "tables\\x.rvnstrings", "file.json", "x.rvnstrings#key", "x.rvnstrings?other", "x\nr.rvnstrings"] {
        assert!(validate_declarations(&BTreeMap::from([(ID.into(), path.into())])).is_err(), "{path}");
    }
    assert!(validate_declarations(&BTreeMap::from([("opaque:old".into(), "x.rvnstrings".into())])).is_err());
    assert!(validate_declarations(&BTreeMap::from([(ID.into(), "x.rvnstrings".into()), (OTHER.into(), "x.rvnstrings".into())])).is_err());
    assert_eq!(url_path("tables/Été %2f.rvnstrings"), "tables/%C3%89t%C3%A9%20%252f.rvnstrings");
    validate_web_manifest(&declared(), None).unwrap();
    validate_web_manifest(&declared(), Some(&declared())).unwrap();
    assert!(validate_web_manifest(&declared(), Some(&BTreeMap::new())).is_err());
    assert!(validate_web_manifest(&BTreeMap::new(), Some(&declared())).is_err());
}

#[test]
fn incomplete_wrong_duplicate_and_foreign_assets_never_publish_a_partial_batch() {
    assert!(StringTablePreparation::new(&declared()).unwrap().finish().is_err());
    let mut preparation = StringTablePreparation::new(&declared()).unwrap();
    assert!(preparation.supply(ID, &bytes(OTHER, "Wrong identity")).is_err());
    assert!(preparation.supply(OTHER, &bytes(OTHER, "Undeclared")).is_err());
    preparation.supply(ID, &bytes(ID, "Hello")).unwrap();
    assert!(preparation.supply(ID, &bytes(ID, "Second")).is_err());
    let resources = preparation.finish().unwrap();
    assert_eq!(resources.display(&StringTableReference { table: ID.into(), key: "hello".into() }, None).unwrap(), "Hello");
}

#[test]
fn cumulative_project_capacity_cannot_be_bypassed_by_individually_valid_files() {
    let ids: Vec<_> = (1..=9).map(|index| format!("{index:032x}")).collect();
    let declarations = ids.iter().enumerate().map(|(index, id)| (id.clone(), format!("{index}.rvnstrings"))).collect();
    let mut preparation = StringTablePreparation::new(&declarations).unwrap();
    let source = "X".repeat(8 * 1_048_576 - 1_024);
    for id in &ids[..8] { preparation.supply(id, &bytes(id, &source)).unwrap(); }
    assert!(preparation.supply(&ids[8], &bytes(&ids[8], &source)).is_err());
    assert!(preparation.finish().is_err());
}

struct Owned(PathBuf);
impl Owned {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rvn-table-project-0112-{}-{stamp}", std::process::id()));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        let Ok(root) = self.0.canonicalize() else { return; };
        let Ok(temp) = std::env::temp_dir().canonicalize() else { return; };
        if root.parent() == Some(temp.as_path()) && root.file_name().is_some_and(|name| name.to_string_lossy().starts_with(&format!("rvn-table-project-0112-{}-", std::process::id()))) {
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[test]
fn native_loading_and_explicit_relocation_keep_identity_and_refuse_current_bad_files() {
    let owned = Owned::new(); let assets = owned.0.join("assets"); fs::create_dir(&assets).unwrap();
    fs::create_dir(assets.join("tables")).unwrap();
    let original = bytes(ID, "Bonjour"); let path = assets.join("tables/Été %2f.rvnstrings"); fs::write(&path, &original).unwrap();
    let first = load_native(&assets, &declared()).unwrap();
    let reference = StringTableReference { table: ID.into(), key: "hello".into() };
    assert_eq!(first.display(&reference, None).unwrap(), "Bonjour");
    let renamed = assets.join("renamed.rvnstrings"); fs::rename(&path, &renamed).unwrap();
    assert!(load_native(&assets, &declared()).is_err());
    let relocated = BTreeMap::from([(ID.into(), "renamed.rvnstrings".into())]);
    assert_eq!(load_native(&assets, &relocated).unwrap().display(&reference, None).unwrap(), "Bonjour");
    assert_eq!(fs::read(&renamed).unwrap(), original);
    fs::write(&renamed, bytes(OTHER, "Other identity")).unwrap();
    assert!(load_native(&assets, &relocated).is_err());
    assert_eq!(first.display(&reference, None).unwrap(), "Bonjour", "existing unpublished/old context remains isolated");
    fs::write(&renamed, b"invalid json").unwrap(); assert!(load_native(&assets, &relocated).is_err());
    assert!(load_native(&owned.0.join("absent"), &BTreeMap::new()).is_ok(), "old projects without tables perform no extra root I/O");
}
