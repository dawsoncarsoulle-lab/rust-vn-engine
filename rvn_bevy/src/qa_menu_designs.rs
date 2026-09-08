//! Screenshots of editor-authored designs, in disposable projects only.
use super::*;
use crate::resources::DialogueHistory;
use crate::systems::{save_menu::{SaveMenuState,SaveMenuMode,SaveMenuOrigin},settings_menu::SettingsMenuState};
pub(super) fn drive(qa:Res<Qa>,engine:Res<VnEngine>,state:Res<State<VnState>>,
    mut next:ResMut<NextState<VnState>>,mut save:ResMut<SaveMenuState>,
    mut settings:ResMut<SettingsMenuState>,menus:Res<crate::menu_documents::Menus>,
    mut history:ResMut<DialogueHistory>,
    texts:Query<(&Text,&Node,&Parent)>,nodes:Query<&Node>,
    paths:Res<crate::project_paths::ProjectPaths>,mut windows:Query<(Entity,&mut Window),With<PrimaryWindow>>,
    mut shots:ResMut<ScreenshotManager>,mut exit:EventWriter<AppExit>,
    mut phase:Local<(usize,f64,usize,Vec<u8>)>,mut initial_window:Local<String>) {
    let now=qa.started.elapsed().as_secs_f64();
    assert!(now<30.0,"Design preview timed out at phase {}",phase.0);
    if now<1.5 || now-phase.1<0.7{return;}
    let (window,mut dimensions)=windows.single_mut();
    if initial_window.is_empty(){
        // Some desktops restore a smaller initial window. Request the fixture's
        // size after mapping and wait for the compositor before taking evidence.
        let config:toml::Value=toml::from_str(&std::fs::read_to_string(paths.root.join("rvn.toml")).unwrap()).unwrap();
        let width=config["window"]["width"].as_integer().unwrap() as f32;
        let height=config["window"]["height"].as_integer().unwrap() as f32;
        dimensions.resolution.set(width,height);
        *initial_window="requested".into();phase.1=now;return;
    }
    let mut capture=|name:&str|shots.save_screenshot_to_disk(window,qa.dir.join(format!("{name}.png"))).unwrap();
    match phase.0 {
        0=>{
            assert_eq!(menus.document().unwrap().pages.len(),11);
            phase.3=std::fs::read(paths.root.join("menus.rvnui")).unwrap();
            phase.2=engine.0.state.pc;
            *initial_window=format!("{} × {} logical pixels, scale {}",dimensions.width(),dimensions.height(),dimensions.scale_factor());
            // QA redirects saves to its output directory, unlike the editor preview.
            // Seed only that explicitly isolated location; never relax preview guards.
            assert_eq!(paths.saves,qa.dir.join("saves"));
            let manager=rvn_core::save::SaveManager::new(&paths.saves,1000).unwrap();
            for (slot,title) in [(1,"Exemple — Première sauvegarde"),(2,"Exemple — Un titre assez long pour vérifier les accents et la disposition")]{
                engine.0.save(&manager,slot,title.into(),"preview.rvn".into()).unwrap();
                assert_eq!(manager.load(slot).unwrap().pc,phase.2);
            }
            capture("title");
        },
        1=>settings.active=true,
        2=>{assert_eq!(engine.0.state.pc,phase.2);capture("settings");},
        3=>{settings.active=false;save.open(SaveMenuMode::Save,SaveMenuOrigin::TitleScreen);},
        4=>{
            assert_eq!(engine.0.state.pc,phase.2);
            let (_,text,parent)=texts.iter().find(|(text,_,_)|text.sections.iter().any(|s|s.value.contains("Un titre assez long"))).expect("Filled save summary missing");
            assert!(nodes.get(parent.get()).unwrap().size().y+1.0>=text.size().y,"The long save summary is clipped by its field");
            capture("save");
        },
        5=>{save.active=false;next.set(VnState::Gallery);},
        6=>{assert_eq!(engine.0.state.pc,phase.2);capture("gallery");},
        7=>{
            history.add(String::new(),"Exemple de narration sans nom affiché.".into());
            for i in 1..=12 {history.add("Personnage de démonstration".into(),format!("Ligne {i} — Une phrase assez longue pour contrôler les accents, les retours à la ligne et le défilement dans chacun des modèles personnalisés."));}
            next.set(VnState::History);
        },
        8=>{assert_eq!(engine.0.state.pc,phase.2);capture("history");},
        9=>next.set(VnState::Stepping),
        10=>{if *state.get()!=VnState::Waiting{return;}phase.2=engine.0.state.pc;capture("dialogue");},
        11=>next.set(VnState::Menu),
        12=>{assert_eq!(engine.0.state.pc,phase.2);capture("pause");},
        13=>{assert!(dimensions.resizable);dimensions.resolution.set(960.0,600.0);},
        14=>{if dimensions.width()!=960.0||dimensions.height()!=600.0{return;}assert_eq!(engine.0.state.pc,phase.2);capture("pause-small");},
        15=>dimensions.resolution.set(1280.0,720.0),
        16=>{if dimensions.width()!=1280.0||dimensions.height()!=720.0{return;}assert_eq!(engine.0.state.pc,phase.2);capture("pause-restored");},
        _=>{
            assert_eq!(std::fs::read(paths.root.join("menus.rvnui")).unwrap(),phase.3);
            std::fs::write(qa.dir.join("design.txt"),format!("Nine native captures; document unchanged; menus did not advance the story. Initial window: {}. Live resize to 960 × 600 and back to 1280 × 720 verified.\n",*initial_window)).unwrap();
            exit.send(AppExit::Success);return;
        }
    }
    phase.0+=1;phase.1=now;
}
