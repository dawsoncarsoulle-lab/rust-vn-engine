//! Regression probes for the real strict/normal provider policy. NOT RUN here.
use super::*;

#[test]
fn strict0697_same_missing_file_refuses_strict_but_normal_uses_real_fallback(){
    let f=fixture();
    let strict=closed_native_locale_root_for_launch(&f.root.join("locales"),true).unwrap();
    let normal=closed_native_locale_root_for_launch(&f.root.join("locales"),false).unwrap();
    assert_eq!(strict.selection_mode(),LocaleSelectionMode::StrictDeclared);
    assert_eq!(normal.selection_mode(),LocaleSelectionMode::NormalCompatible);
    fs::remove_file(f.root.join("locales/en.toml")).unwrap();
    assert!(load_closed_native_frozen_locales(&strict,&declarations()).is_err());
    let locale=load_closed_native_frozen_locales(&normal,&declarations()).unwrap();
    assert_eq!(locale.current_lang(),"en");assert_eq!(locale.translate("First"),"Premier");
    assert!(!f.root.join("locales/en.toml").exists());
}

#[test]
fn strict0697_retained_root_removal_is_not_accepted_as_a_strict_empty_catalogue(){
    let mut f=fixture();ready(&mut f);let path=f.root.join("locales");
    let strict=closed_native_locale_root_for_launch(&path,true).unwrap();
    let normal=closed_native_locale_root_for_launch(&path,false).unwrap();
    fs::remove_file(path.join("en.toml")).unwrap();fs::remove_file(path.join("fr.toml")).unwrap();fs::remove_dir(&path).unwrap();
    assert!(load_closed_native_frozen_locales(&strict,&declarations()).is_err());
    let locale=load_closed_native_frozen_locales(&normal,&declarations()).unwrap();
    assert_eq!(locale.current_lang(),"en");assert_eq!(locale.translate("First"),"First");assert!(!path.exists());
}

#[test]
fn strict0697_actual_failed_notfound_blocks_publication_readiness_and_then_recovers(){
    let mut f=fixture();let before=ready(&mut f);
    assert_eq!(locale_selection_mode(f.app.world()).unwrap(),LocaleSelectionMode::StrictDeclared);
    let live=live_receipt(f.app.world()).unwrap();let id=f.app.world().resource::<LocaleAssetHost>().bound["en"].id();
    let retained=f.app.world().resource::<VnEngine>().0.locale.as_ref().unwrap().portable_receipt().unwrap();
    fs::remove_file(f.root.join("locales/en.toml")).unwrap();reload_locale(&f,"en");
    pump(&mut f,|world|native_locale_provider::failed_not_found(&world.resource::<AssetServer>().load_state(id)));
    assert!(live_receipt(f.app.world()).is_err());assert!(portable_receipt(f.app.world()).is_err());
    assert!(startup_loads_ready(f.app.world()).is_err(),"strict missing source is a real refusal, never endless loading or normal absence");
    assert!(accepts_live_receipt(f.app.world(),&live).is_err());
    assert_eq!(f.app.world().resource::<VnEngine>().0.locale.as_ref().unwrap().portable_receipt().unwrap(),retained);
    write_locale(&f.root,"en","Recovered strict");reload_locale(&f,"en");
    pump(&mut f,|world|portable_receipt(world).is_ok_and(|receipt|receipt.locales.cache["en"].strings.get("First").is_some_and(|v|v=="Recovered strict")));
    let after=portable_receipt(f.app.world()).unwrap();assert!(after.locale_absent.is_empty());assert_eq!(after.locale_loader_policy,1);
    assert_ne!(before,after);assert!(startup_loads_ready(f.app.world()).unwrap());
    assert_eq!(f.app.world().resource::<LocaleAssetHost>().bound["en"].id(),id);
    assert_eq!(locale_selection_mode(f.app.world()).unwrap(),LocaleSelectionMode::StrictDeclared);
}
