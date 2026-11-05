//! Prepared CPU contracts; no tests in packet0670 have been executed.
use super::*;

#[test]
fn locale_mode_legacy_root_registration_and_fixture_default_to_strict() {
    let mut f = fixture();
    ready(&mut f);
    assert_eq!(locale_selection_mode(f.app.world()).unwrap(), LocaleSelectionMode::StrictDeclared);
    let root = closed_native_locale_root(&f.root.join("locales")).unwrap();
    assert_eq!(root.selection_mode(), LocaleSelectionMode::StrictDeclared);
    assert_eq!(root.clone().selection_mode(), LocaleSelectionMode::StrictDeclared);
    assert!(accepts_live_receipt(f.app.world(), &live_receipt(f.app.world()).unwrap()).unwrap());
}

#[test]
fn locale_mode_verified_root_clone_carries_provenance_into_actual_registration() {
    let f = fixture();
    let root = closed_native_locale_root_for_launch(&f.root.join("locales"), false).unwrap();
    assert_eq!(root.selection_mode(), LocaleSelectionMode::NormalCompatible);
    let retained = root.clone();
    drop(root);
    let mut app = App::new();
    register_verified_text_sources(&mut app, f.root.join("assets").to_string_lossy().into_owned(), retained).unwrap();
    assert_eq!(locale_selection_mode(app.world()).unwrap(), LocaleSelectionMode::NormalCompatible);
    // Observing provenance before AssetServer does not manufacture a receipt.
    assert!(live_receipt(app.world()).is_err());
}

#[test]
fn locale_mode_drift_revokes_live_receipt_without_changing_all_present_portable_wire() {
    let mut f = fixture();
    let portable = ready(&mut f);
    assert_eq!(portable.locale_loader_policy, 1);
    let bytes = serde_json::to_vec(&portable).unwrap();
    let digest = portable.canonical_digest().unwrap();
    let live = live_receipt(f.app.world()).unwrap();
    assert!(accepts_live_receipt(f.app.world(), &live).unwrap());
    // Negative control: corrupt only this private retained field. The source
    // authority Arc, actual readers, catalogue content and publication stay put.
    f.app.world_mut().resource_mut::<KnownTextSources>().selection_mode = LocaleSelectionMode::NormalCompatible;
    let changed = live_receipt(f.app.world()).unwrap();
    assert!(Arc::ptr_eq(&changed.source_authority, &live.source_authority));
    assert_eq!(changed.locale_source_digest, live.locale_source_digest);
    assert_eq!(serde_json::to_vec(&portable_receipt(f.app.world()).unwrap()).unwrap(), bytes);
    assert_eq!(portable_receipt(f.app.world()).unwrap().canonical_digest().unwrap(), digest);
    assert!(!accepts_live_receipt(f.app.world(), &live).unwrap());
}

#[test]
fn locale_mode_late_registration_cannot_replace_actual_retained_provider() {
    let mut f = fixture();
    ready(&mut f);
    let live = live_receipt(f.app.world()).unwrap();
    let replacement = closed_native_locale_root_for_launch(&f.root.join("locales"), false).unwrap();
    let table_root = f.root.join("assets").to_string_lossy().into_owned();
    assert!(register_verified_text_sources(&mut f.app, table_root, replacement).is_err());
    assert_eq!(locale_selection_mode(f.app.world()).unwrap(), LocaleSelectionMode::StrictDeclared);
    assert!(accepts_live_receipt(f.app.world(), &live).unwrap());
}
