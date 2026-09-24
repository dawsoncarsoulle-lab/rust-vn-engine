//! Reproducible project conversion. Refuses any existing destination.
use rvn_core::{Engine, Interaction, Renderer, SpriteState};
use rvn_graph::*;
use rvn_parser::{Hotspot, Position, Script, Statement, Transition};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug)]
struct ImportError(String);
impl<T: std::fmt::Display> From<T> for ImportError {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}
type Result<T> = std::result::Result<T, ImportError>;

#[derive(Default)]
struct Trace {
    events: Vec<String>,
    endings: BTreeSet<String>,
}
impl Renderer for Trace {
    fn set_background(&mut self, p: &str, t: &Transition) {
        self.events.push(format!("scene {p} {t}"));
    }
    fn show_cinematic(&mut self, p: &str, t: Option<&str>) {
        self.events
            .push(format!("cg {p} {}", canonical_transition(t)));
    }
    fn hide_cinematic(&mut self, t: Option<&str>) {
        self.events
            .push(format!("cg hide {}", canonical_transition(t)));
    }
    fn unlock_ending(&mut self, id: &str) {
        self.endings.insert(id.into());
        self.events.push(format!("ending {id}"));
    }
    fn show_sprite(
        &mut self,
        id: &str,
        e: Option<&str>,
        p: &Position,
        t: &Transition,
        _: Option<&SpriteState>,
    ) {
        self.events.push(format!("show {id} {e:?} {p} {t}"));
    }
    fn hide_sprite(&mut self, id: &str, t: &Transition, _: &SpriteState) {
        self.events.push(format!("hide {id} {t}"));
    }
    fn move_sprite(&mut self, id: &str, p: &Position, t: &Transition, _: &SpriteState) {
        self.events.push(format!("move {id} {p} {t}"));
    }
    fn show_dialogue(&mut self, _: Option<&str>, _: &str) {}
    fn show_choice(&mut self, _: &[String]) -> usize {
        0
    }
    fn music_play(&mut self, p: &str, t: &Transition, _: Option<&str>) {
        self.events.push(format!("music {p} {t}"));
    }
    fn music_stop(&mut self, t: &Transition) {
        self.events.push(format!("music stop {t}"));
    }
    fn music_set_volume(&mut self, v: f32) {
        self.events.push(format!("volume {v}"));
    }
    fn sfx_play(&mut self, p: &str, t: &Transition) {
        self.events.push(format!("sfx {p} {t}"));
    }
    fn sfx_stop(&mut self, p: &str, t: &Transition) {
        self.events.push(format!("sfx stop {p} {t}"));
    }
    fn show_imagemap(&mut self, _: &str, _: Option<&str>, _: &[Hotspot]) -> usize {
        0
    }
    fn set_typewriter_config(&mut self, v: f32) {
        self.events.push(format!("typewriter {v}"));
    }
}
fn canonical_transition(t: Option<&str>) -> String {
    match t {
        None | Some("none") => "none".into(),
        Some("fade") => "fade(500)".into(),
        Some(v) => v.into(),
    }
}
fn play(script: &Script, route: [usize; 5], save_dir: Option<&Path>) -> Result<Trace> {
    let mut engine = Engine::new(script.clone(), Trace::default(), 20)?;
    engine.state.pc = engine
        .script
        .iter()
        .position(|s| matches!(s,Statement::Label { name } if name=="title_intro"))
        .ok_or("title_intro absent")?;
    let mut choice = 0;
    let mut saved = false;
    for _ in 0..1500 {
        let interaction = engine.step_until_interaction()?;
        match interaction {
            Some(Interaction::Dialogue { character, text }) => {
                engine
                    .renderer
                    .events
                    .push(format!("dialogue {character:?} {text}"));
                if choice >= 3 && !saved {
                    if let Some(dir) = save_dir {
                        let manager = rvn_core::save::SaveManager::new(dir, 10)?;
                        engine.save(
                            &manager,
                            1,
                            "archiviste-check".into(),
                            "project.generated.rvn".into(),
                        )?;
                        let before = engine.state.clone();
                        let events = engine.renderer.events.len();
                        engine.advance_dialogue()?;
                        engine.load(&manager, 1)?;
                        assert_eq!(
                            serde_json::to_value(&engine.state)?,
                            serde_json::to_value(&before)?,
                            "save/load must restore the exact narrative state"
                        );
                        engine.renderer.events.truncate(events); // restore_screen is not a narrative event
                    }
                    saved = true;
                }
                engine.advance_dialogue()?;
            }
            Some(Interaction::Choice { options }) => {
                let selected = match choice {
                    0 => route[0],
                    1 => route[1],
                    2 => route[3],
                    3 => route[4],
                    _ => return Err("Unexpected extra choice".into()),
                };
                if selected >= options.len() {
                    return Err("Choice out of range".into());
                }
                engine
                    .renderer
                    .events
                    .push(format!("choice {options:?} -> {selected}"));
                choice += 1;
                engine.submit_choice(selected)?;
            }
            Some(Interaction::Imagemap {
                background,
                hover_image,
                hotspots,
            }) => {
                engine.renderer.events.push(format!(
                    "map {background} {hover_image:?} {:?} -> {}",
                    hotspots
                        .iter()
                        .map(|h| (&h.name, &h.area, &h.hover_area))
                        .collect::<Vec<_>>(),
                    route[2]
                ));
                engine.submit_hotspot(route[2])?;
            }
            None if engine.is_finished() => {
                let mut vars: Vec<_> = engine.state.vars.into_iter().collect();
                vars.sort_by(|a, b| a.0.cmp(&b.0));
                engine
                    .renderer
                    .events
                    .push(format!("final variables {vars:?}"));
                if engine.renderer.endings.len() != 1 {
                    return Err("Expected exactly one ending".into());
                }
                return Ok(engine.renderer);
            }
            None => {}
        }
    }
    Err("Story did not finish within 1500 interactions".into())
}
fn walk<'a>(script: &'a [Statement], out: &mut Vec<&'a Statement>) {
    for s in script {
        out.push(s);
        match s {
            Statement::Init { body } => walk(body, out),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                walk(then_branch, out);
                walk(else_branch, out);
            }
            Statement::Choice { options } => {
                for o in options {
                    walk(&o.body, out)
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                for h in hotspots {
                    walk(&h.body, out)
                }
            }
            _ => {}
        }
    }
}
fn check_assets(root: &Path, script: &Script) -> Result<BTreeSet<PathBuf>> {
    let mut all = Vec::new();
    walk(script, &mut all);
    let mut paths = BTreeSet::new();
    for s in all {
        match s {
            Statement::Scene { background, .. } => {
                paths.insert(PathBuf::from(background));
            }
            Statement::MusicPlay { file, .. } => {
                paths.insert(if file.starts_with("music/") {
                    file.into()
                } else {
                    PathBuf::from(format!("music/{file}"))
                });
            }
            Statement::SfxPlay { file, .. } | Statement::SfxStop { file, .. } => {
                paths.insert(file.into());
            }
            Statement::ShowSprite {
                character_id,
                emotion: Some(e),
                ..
            } => {
                paths.insert(format!("sprites/{character_id}/{e}.png").into());
            }
            Statement::CinematicShow { id, .. } => {
                paths.insert(format!("cgs/{id}.png").into());
            }
            Statement::Imagemap {
                background,
                hover_image,
                ..
            } => {
                paths.insert(background.into());
                if let Some(p) = hover_image {
                    paths.insert(p.into());
                }
            }
            _ => {}
        }
    }
    paths.insert("ui/textbox.png".into());
    for p in &paths {
        if !root.join("assets").join(p).is_file() {
            return Err(format!("Missing asset: {}", p.display()).into());
        }
    }
    Ok(paths)
}
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let e = entry?;
        let ty = e.file_type()?;
        if ty.is_symlink() {
            return Err(format!("Symlink refused: {}", e.path().display()).into());
        }
        if ty.is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        } else {
            fs::copy(e.path(), to.join(e.file_name()))?;
        }
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let verify = args.len() == 3 && args[2] == "--verify";
    if args.len() != 2 && !verify {
        return Err("Usage: import_archiviste SOURCE_PROJECT DESTINATION [--verify]".into());
    }
    let source = fs::canonicalize(&args[0])?;
    let dest = PathBuf::from(&args[1]);
    if dest.exists() && !verify {
        return Err(format!("Destination exists; no files changed: {}", dest.display()).into());
    }
    let original = rvn_parser::parse_file_with_uses(source.join("scripts/main.rvn"))?;
    // Regression proof: the source repeats ARIA's intro and returns without a caller.
    let source_error = play(&original, [0, 0, 0, 0, 0], None)
        .err()
        .ok_or("Expected source flow defect was not reproduced; inspect before patching")?
        .0;
    println!("Original flow defect reproduced: {source_error}");
    let mut fixed = original.clone();
    let index = fixed
        .iter()
        .position(|s| matches!(s,Statement::Call{target} if target=="ch1_aria_intro"))
        .ok_or("Missing ARIA call")?;
    if !matches!(fixed.get(index+1),Some(Statement::Label{name}) if name=="ch1_aria_intro") {
        return Err("Source changed: refusing automatic flow correction".into());
    }
    fixed.insert(
        index + 1,
        Statement::Jump {
            target: "ch1_morning".into(),
        },
    );
    let assets = check_assets(&source, &fixed)?;
    let font = Path::new("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
    let font_license = Path::new("/usr/share/doc/fonts-dejavu-core/copyright");
    if !font.is_file() || !font_license.is_file() {
        return Err("Install fonts-dejavu-core to provide French glyphs and its license".into());
    }
    fn read_graphs(dir: &Path, graphs: &mut Vec<GraphDocument>) -> Result<()> {
        let mut entries = fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.path());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                read_graphs(&path, graphs)?;
            } else if path.extension().is_some_and(|e| e == "rvngraph") {
                graphs.push(GraphDocument::from_json(&fs::read_to_string(path)?)?);
            }
        }
        Ok(())
    }
    let graphs = if verify {
        let mut graphs = Vec::new();
        read_graphs(&dest.join("graphs"), &mut graphs)?;
        graphs
    } else {
        import_script(&fixed)?
    };
    if graphs.len() != 25 {
        return Err(format!("Expected 25 documents, got {}", graphs.len()).into());
    }
    for g in &graphs {
        let restored = GraphDocument::from_json(&g.to_pretty_json()?)?;
        if restored != *g {
            return Err("Graph round trip changed its contents".into());
        }
    }
    let compiled = transpile_project(&graphs)?;
    // Every user-visible statement must remain, not only the visited ones.
    let mut before = Vec::new();
    walk(&fixed, &mut before);
    let mut after = Vec::new();
    walk(&compiled.ast, &mut after);
    let dialogues = |s: Vec<&Statement>| {
        let mut v: Vec<_> = s
            .into_iter()
            .filter_map(|s| {
                if let Statement::Dialogue { character_id, text } = s {
                    Some(format!("{character_id:?} {text:?}"))
                } else {
                    None
                }
            })
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        dialogues(before),
        dialogues(after),
        "All dialogue ASTs must survive unchanged"
    );
    let mut endings = BTreeSet::new();
    let mut routes = 0;
    for a in 0..3 {
        for b in 0..3 {
            for h in 0..3 {
                for m in 0..3 {
                    for c in 0..2 {
                        let route = [a, b, h, m, c];
                        let expected = play(&fixed, route, None)?;
                        let actual = play(&compiled.ast, route, None)?;
                        if expected.events != actual.events {
                            let diff = expected
                                .events
                                .iter()
                                .zip(&actual.events)
                                .position(|(a, b)| a != b)
                                .unwrap_or(expected.events.len().min(actual.events.len()));
                            return Err(format!(
                                "Route {route:?}, event {diff}: source={:?}; blueprint={:?}",
                                expected.events.get(diff),
                                actual.events.get(diff)
                            )
                            .into());
                        }
                        endings.extend(actual.endings);
                        routes += 1;
                    }
                }
            }
        }
    }
    if endings.len() != 4 {
        return Err(format!("Only these endings are reachable: {endings:?}").into());
    }
    println!(
        "Verified {routes} routes, {} endings, {} asset references",
        endings.len(),
        assets.len()
    );
    if verify {
        let exported = rvn_parser::parse_file_with_uses(dest.join("project.generated.rvn"))?;
        for a in 0..3 {
            for b in 0..3 {
                for h in 0..3 {
                    for m in 0..3 {
                        for c in 0..2 {
                            let route = [a, b, h, m, c];
                            assert_eq!(
                                play(&compiled.ast, route, None)?.events,
                                play(&exported, route, None)?.events,
                                "Saved export differs from the graphs"
                            );
                        }
                    }
                }
            }
        }
        let qa = dest.join("verification/saves");
        assert_eq!(
            play(&compiled.ast, [2, 0, 0, 2, 0], None)?.events,
            play(&compiled.ast, [2, 0, 0, 2, 0], Some(&qa))?.events
        );
        println!("Existing graphs, saved export and save/load verified; no story files changed");
        return Ok(());
    }
    // New directory is reserved only after conversion and parity checks succeed.
    fs::create_dir(&dest)?;
    for directory in ["assets", "locales"] {
        copy_tree(&source.join(directory), &dest.join(directory))?;
    }
    fs::create_dir_all(dest.join("assets/fonts"))?;
    fs::copy(font, dest.join("assets/fonts/DejaVuSans.ttf"))?;
    fs::copy(font_license, dest.join("assets/fonts/LICENSE-DejaVu.txt"))?;
    let theme = fs::read_to_string(source.join("theme.toml"))?
        .replace(
            "[textbox]\n",
            "[textbox]\nbackground_color = \"#000000D9\"\n",
        )
        .replace(
            "[text.name]\n",
            "[text.name]\nfont_path = \"fonts/DejaVuSans.ttf\"\n",
        )
        .replace(
            "[text.dialogue]\n",
            "[text.dialogue]\nfont_path = \"fonts/DejaVuSans.ttf\"\n",
        )
        .replace(
            "[choice]\n",
            "[choice]\nfont_path = \"fonts/DejaVuSans.ttf\"\n",
        );
    fs::write(dest.join("theme.toml"), theme)?;
    copy_tree(&source.join("scripts"), &dest.join("reference/scripts"))?;
    fs::copy(
        source.join("rvn.toml"),
        dest.join("reference/rvn.original.toml"),
    )?;
    fs::copy(
        source.join("theme.toml"),
        dest.join("reference/theme.original.toml"),
    )?;
    let manifest = fs::read_to_string(source.join("rvn.toml"))?
        .replace("scripts/main.rvn", "project.generated.rvn");
    fs::write(dest.join("rvn.toml"), manifest)?;
    fs::create_dir(dest.join("saves"))?;
    for g in &graphs {
        let (folder, name) = match &g.kind {
            GraphKind::Init => (
                "00_initialisation".to_string(),
                "initialisation".to_string(),
            ),
            GraphKind::Label { name } => {
                let folder = if name.starts_with("ch1_") || name == "title_intro" {
                    "01_la_routine"
                } else if name.starts_with("ch2_") {
                    "02_le_signal"
                } else if name.starts_with("ch3_") {
                    "03_la_decision"
                } else {
                    "04_la_surface"
                };
                (folder.into(), name.clone())
            }
            _ => unreachable!(),
        };
        let dir = dest.join("graphs").join(folder);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{name}.rvngraph")), g.to_pretty_json()?)?;
    }
    fs::write(dest.join("project.generated.rvn"), &compiled.source)?;
    let qa = dest.join("verification");
    fs::create_dir(&qa)?;
    let baseline = play(&compiled.ast, [2, 0, 0, 2, 0], None)?;
    let restored = play(&compiled.ast, [2, 0, 0, 2, 0], Some(&qa.join("saves")))?;
    assert_eq!(
        baseline.events, restored.events,
        "Loading a save must preserve the rest of the route"
    );
    let manager = rvn_core::PersistentDataManager::new(qa.join("saves"))?;
    let mut data = rvn_core::PersistentData::default();
    data.seen_endings = endings.clone();
    manager.save(&data)?;
    assert_eq!(manager.load()?.seen_endings, endings);
    let nodes: usize = graphs.iter().map(|g| g.nodes.len()).sum();
    fs::write(dest.join("LIRE-MOI.md"),format!("# La Dernière Archiviste — Blueprint\n\n25 graphes éditables, {nodes} nœuds, quatre chapitres et quatre fins.\n\nOuvrir `graphs/01_la_routine/title_intro.rvngraph` dans l’éditeur. Le raccourci A cadre le graphe ; utiliser la recherche du panneau Projet pour retrouver un label. Enregistrer les changements puis utiliser **Exporter projet**. Le moteur doit lancer ce dossier, dont `rvn.toml` pointe sur `project.generated.rvn`.\n\nLes personnages, variables et médias sont disponibles dans leurs panneaux. Les textes sont dans les nœuds Texte connectés aux dialogues.\n\n## Fidélité et corrections\n\nLes scripts originaux sont conservés dans `reference/scripts`, mais ne sont pas importés par l’export. Aucun fichier ni aucune sauvegarde de l’original n’a été modifié.\n\n- Défaut original reproduit : `{source_error}`. Ajout de `jump ch1_morning` après `call ch1_aria_intro`, comme l’indiquait la ligne commentée du scénario.\n- Les passages implicites entre labels sont désormais des sauts explicites.\n- Appeler un label possède une sortie Après retour. L’export termine proprement après les crédits.\n\n## Vérifications automatiques\n\n{routes} combinaisons de décisions comparées au scénario corrigé : mêmes dialogues, options, zones, événements audiovisuels et variables finales. Quatre fins atteintes : {endings:?}. {assets_count} ressources référencées présentes. Aller-retour JSON des 25 graphes et save/load après un choix significatif vérifiés. Les sauvegardes de test sont uniquement dans `verification/saves`, pas dans `saves`.\n\n## Reproduire\n\nDepuis le dépôt rust-VN : `cargo run --release -p rvn_graph --example import_archiviste -- \"DOSSIER_SOURCE\" \"NOUVEAU_DOSSIER\"`. Toute destination existante est refusée pour préserver les retouches manuelles.\n\nLe thème et les traductions restent leurs fichiers TOML habituels. Le contrôle graphique est consigné séparément dans `verification/RECETTE.md`.\n",assets_count=assets.len()))?;
    let readme_path = dest.join("LIRE-MOI.md");
    let mut readme = fs::read_to_string(&readme_path)?;
    readme.push_str("\n## Références et police\n\nLes destinations sont des nœuds Référence de label connectés aux sauts/appels : clic droit, rechercher Référence de label, puis renseigner Destination dans l’inspecteur. Les positions sont également explicites. Le thème reçoit son champ obligatoire textbox.background_color et la police DejaVu Sans pour les accents ; le thème original reste dans reference/theme.original.toml.\n\nAjouter --verify à la commande de conversion pour contrôler des graphes existants et leur export sans les remplacer (les sauvegardes de contrôle dans verification/saves sont actualisées).\n");
    fs::write(readme_path, readme)?;
    println!("Created {} ({nodes} nodes)", dest.display());
    Ok(())
}
