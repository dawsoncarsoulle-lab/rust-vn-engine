use rvn_parser::parse;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rvn_bevy::run_game;
use rvn_parser::{parse_file_with_uses, Expr, InterpolatedText, Statement, TextSegment};
use serde::Deserialize;

static DEFAULT_TEMPLATE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/template/default");

/// Command line interface for RVN.
#[derive(Parser)]
#[command(name = "rvn")]
#[command(about = "Rust Visual Novel CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new visual novel project.
    New {
        /// Name of the project and output directory.
        name: String,
    },

    /// Run an existing visual novel project.
    Run {
        /// Path to the project directory.
        #[arg(value_name = "PROJECT_DIR", default_value = ".")]
        project: String,
    },

    /// Check an existing visual novel project for script and asset errors.
    Check {
        /// Path to the project directory.
        #[arg(value_name = "PROJECT_DIR", default_value = ".")]
        project: String,
    },

    /// Launch an existing visual novel project in development mode.
    Dev {
        /// Path to the project directory.
        #[arg(value_name = "PROJECT_DIR", default_value = ".")]
        project: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name } => create_project(&name)?,
        Commands::Run { project } => {
            if let Err(errors) = check_project(&project) {
                print_project_errors(&project, &errors);
                eprintln!(
                    "\nrvn run aborted: fix the project errors above before launching the runtime."
                );
                std::process::exit(1);
            }

            if let Err(e) = run_game(&project) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        Commands::Check { project } => match check_project(&project) {
            Ok(()) => println!("✅ No issues found in {project}."),
            Err(errors) => {
                print_project_errors(&project, &errors);
                std::process::exit(1);
            }
        },
        Commands::Dev { project } => {
            if let Err(e) = dev_project(&project) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_project_errors(project: &str, errors: &[String]) {
    eprintln!("❌ RVN project validation failed in {project}:");
    for err in errors {
        eprintln!("- {err}");
    }
}

fn create_project(name: &str) -> Result<()> {
    let project_dir = Path::new(name);

    if project_dir.exists() {
        anyhow::bail!("cannot create project: directory '{}' already exists", name);
    }

    write_embedded_template(&DEFAULT_TEMPLATE, project_dir).with_context(|| {
        format!(
            "failed to create project from embedded template into '{}'",
            project_dir.display()
        )
    })?;

    let rvn_toml_path = project_dir.join("rvn.toml");
    let mut config_contents = fs::read_to_string(&rvn_toml_path).with_context(|| {
        format!(
            "unable to read configuration file '{}'",
            rvn_toml_path.display()
        )
    })?;
    config_contents = config_contents.replace("Mon Jeu RVN", name);
    fs::write(&rvn_toml_path, config_contents).with_context(|| {
        format!(
            "unable to write configuration file '{}'",
            rvn_toml_path.display()
        )
    })?;

    println!("✅ New RVN project '{name}' created.");
    println!("To get started:");
    println!("  cd {name}");
    println!("  rvn run .");
    println!();
    println!("If `rvn` is not installed in your PATH yet, run from the parent directory:");
    println!("  ./rvn run {name}");
    Ok(())
}

fn write_embedded_template(dir: &Dir<'_>, target_root: &Path) -> Result<()> {
    fs::create_dir_all(target_root).with_context(|| {
        format!(
            "unable to create project directory '{}'",
            target_root.display()
        )
    })?;

    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::Dir(subdir) => {
                write_embedded_template(subdir, target_root)?;
            }
            include_dir::DirEntry::File(file) => {
                let output_path = target_root.join(file.path());
                if let Some(parent) = output_path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("unable to create directory '{}'", parent.display())
                    })?;
                }
                fs::write(&output_path, file.contents()).with_context(|| {
                    format!("unable to write template file '{}'", output_path.display())
                })?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct ProjectConfig {
    project: ProjectSection,
    #[serde(default)]
    paths: PathsSection,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    #[allow(dead_code)]
    title: Option<String>,
    #[serde(default = "default_main_script")]
    main_script: String,
    start_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PathsSection {
    #[serde(default = "default_assets")]
    assets: String,
    #[serde(default = "default_locales")]
    locales: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_saves")]
    saves: String,
}

impl Default for PathsSection {
    fn default() -> Self {
        Self {
            assets: default_assets(),
            locales: default_locales(),
            theme: default_theme(),
            saves: default_saves(),
        }
    }
}

fn default_main_script() -> String {
    "scripts/main.rvn".to_string()
}

fn default_assets() -> String {
    "assets".to_string()
}

fn default_locales() -> String {
    "locales".to_string()
}

fn default_theme() -> String {
    "theme.toml".to_string()
}

fn default_saves() -> String {
    "saves".to_string()
}

fn load_project_config(project_dir: &Path) -> std::result::Result<ProjectConfig, String> {
    let path = project_dir.join("rvn.toml");
    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

fn check_project(project: &str) -> std::result::Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let project_dir = Path::new(project);

    let cfg = match load_project_config(project_dir) {
        Ok(cfg) => cfg,
        Err(e) => return Err(vec![e]),
    };

    let script_path = project_dir.join(&cfg.project.main_script);
    let script = match parse_file_with_uses(&script_path) {
        Ok(script) => script,
        Err(e) => {
            return Err(vec![format!(
                "Script loading error from {}:\n{e}",
                script_path.display()
            )])
        }
    };

    validate_interactions(&script, &mut errors);

    let mut labels = Vec::new();
    let mut jumps = Vec::new();
    let mut declared_characters = HashSet::new();
    let mut used_characters = Vec::new();
    let mut backgrounds = Vec::new();
    let mut sprites = Vec::new();
    let mut music_files = Vec::new();
    let mut sfx_files = Vec::new();
    let mut assigned_vars = HashSet::new();
    let mut used_vars = Vec::new();

    collect_from_script(
        &script,
        &mut labels,
        &mut jumps,
        &mut declared_characters,
        &mut used_characters,
        &mut backgrounds,
        &mut sprites,
        &mut music_files,
        &mut sfx_files,
        &mut assigned_vars,
        &mut used_vars,
    );

    let mut seen_labels = HashSet::new();
    for label in &labels {
        if !seen_labels.insert(label.clone()) {
            errors.push(format!("Duplicate label '{label}'."));
        }
    }

    let label_set: HashSet<String> = labels.into_iter().collect();
    if let Some(start_label) = &cfg.project.start_label {
        if !label_set.contains(start_label) {
            errors.push(format!(
                "Configuration error: start_label `{start_label}` does not exist in the resolved project scripts. Add `label {start_label}` or update rvn.toml."
            ));
        }
    }
    for target in &jumps {
        if !label_set.contains(target) {
            errors.push(format!("Undefined label target '{target}'."));
        }
    }

    for id in &used_characters {
        if !declared_characters.contains(id) {
            errors.push(format!(
                "Undefined character id '{id}'. Declare it in init with character.create(...)."
            ));
        }
    }

    for var in &used_vars {
        // Internal variables can be provided by the engine and should not be reported.
        if var.starts_with("__") {
            continue;
        }
        if !assigned_vars.contains(var) {
            errors.push(format!(
                "Variable '{var}' is used before being assigned with `set {var} = ...`."
            ));
        }
    }

    let assets_dir = project_dir.join(&cfg.paths.assets);
    let bg_exts = ["png", "jpg", "jpeg", "webp"];
    for bg in &backgrounds {
        if !asset_exists(&assets_dir, bg, &bg_exts) {
            errors.push(format!("RVN asset error: background `{bg}` not found under `{}`. Accepted extensions: png, jpg, jpeg, webp.", assets_dir.display()));
        }
    }

    let sprite_exts = ["png", "jpg", "jpeg", "webp"];
    for (character_id, emotion) in &sprites {
        let sprite_path = match emotion {
            Some(emotion) => format!("sprites/{character_id}/{emotion}"),
            None => format!("sprites/{character_id}/default"),
        };
        if !asset_exists(&assets_dir, &sprite_path, &sprite_exts) {
            errors.push(format!("RVN asset error: sprite `{sprite_path}` not found under `{}`. Expected something like `assets/{sprite_path}.png`.", assets_dir.display()));
        }
    }

    let audio_exts = ["ogg", "mp3", "wav", "flac"];
    for music in &music_files {
        let path = if music.starts_with("music/") {
            music.clone()
        } else {
            format!("music/{music}")
        };
        if !asset_exists(&assets_dir, &path, &audio_exts) {
            errors.push(format!("RVN asset error: music `{path}` not found under `{}`. Accepted extensions: ogg, mp3, wav, flac.", assets_dir.display()));
        }
    }

    for sfx in &sfx_files {
        let path = if sfx.starts_with("sfx/") {
            sfx.clone()
        } else {
            format!("sfx/{sfx}")
        };
        if !asset_exists(&assets_dir, &path, &audio_exts) {
            errors.push(format!("RVN asset error: sound effect `{path}` not found under `{}`. Accepted extensions: ogg, mp3, wav, flac.", assets_dir.display()));
        }
    }

    if !project_dir.join(&cfg.paths.theme).exists() {
        errors.push(format!(
            "Project file error: theme file `{}` not found.",
            project_dir.join(&cfg.paths.theme).display()
        ));
    }
    if !project_dir.join(&cfg.paths.locales).exists() {
        errors.push(format!(
            "Project directory error: locales directory `{}` not found.",
            project_dir.join(&cfg.paths.locales).display()
        ));
    }
    if !project_dir.join(&cfg.paths.saves).exists() {
        errors.push(format!(
            "Project directory error: saves directory `{}` not found.",
            project_dir.join(&cfg.paths.saves).display()
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_interactions(script: &[Statement], errors: &mut Vec<String>) {
    for stmt in script {
        match stmt {
            Statement::Use { .. } => {}
            Statement::Init { body } => validate_interactions(body, errors),
            Statement::Choice { options } => {
                if options.is_empty() {
                    errors.push("Script validation error: `choice` block has no option. Add at least one `\"Label\" => { ... }` entry.".to_string());
                }
                for (_, body) in options {
                    validate_interactions(body, errors);
                }
            }
            Statement::Imagemap { hotspots, .. } => {
                if hotspots.is_empty() {
                    errors.push("Script validation error: `imagemap` has no hotspot. Add at least one `hotspot { area: (...) } => { ... }` block.".to_string());
                }
                for hotspot in hotspots {
                    validate_interactions(&hotspot.body, errors);
                }
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                validate_interactions(then_branch, errors);
                validate_interactions(else_branch, errors);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_from_script(
    script: &[Statement],
    labels: &mut Vec<String>,
    jumps: &mut Vec<String>,
    declared_characters: &mut HashSet<String>,
    used_characters: &mut Vec<String>,
    backgrounds: &mut Vec<String>,
    sprites: &mut Vec<(String, Option<String>)>,
    music_files: &mut Vec<String>,
    sfx_files: &mut Vec<String>,
    assigned_vars: &mut HashSet<String>,
    used_vars: &mut Vec<String>,
) {
    for stmt in script {
        match stmt {
            Statement::Use { .. } => {}
            Statement::Init { body } => collect_from_script(
                body,
                labels,
                jumps,
                declared_characters,
                used_characters,
                backgrounds,
                sprites,
                music_files,
                sfx_files,
                assigned_vars,
                used_vars,
            ),
            Statement::CharacterCreate { id, .. } => {
                declared_characters.insert(id.clone());
            }
            Statement::Dialogue { character_id, text } => {
                if let Some(id) = character_id {
                    used_characters.push(id.clone());
                }
                collect_text_vars(text, used_vars);
            }
            Statement::Choice { options } => {
                for (label, body) in options {
                    collect_text_vars(label, used_vars);
                    collect_from_script(
                        body,
                        labels,
                        jumps,
                        declared_characters,
                        used_characters,
                        backgrounds,
                        sprites,
                        music_files,
                        sfx_files,
                        assigned_vars,
                        used_vars,
                    );
                }
            }
            Statement::SetVar { name, value } => {
                collect_expr_vars(value, used_vars);
                assigned_vars.insert(name.clone());
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                collect_expr_vars(condition, used_vars);
                collect_from_script(
                    then_branch,
                    labels,
                    jumps,
                    declared_characters,
                    used_characters,
                    backgrounds,
                    sprites,
                    music_files,
                    sfx_files,
                    assigned_vars,
                    used_vars,
                );
                collect_from_script(
                    else_branch,
                    labels,
                    jumps,
                    declared_characters,
                    used_characters,
                    backgrounds,
                    sprites,
                    music_files,
                    sfx_files,
                    assigned_vars,
                    used_vars,
                );
            }
            Statement::Label { name } => labels.push(name.clone()),
            Statement::Jump { target } | Statement::Call { target } => jumps.push(target.clone()),
            Statement::Scene { background, .. } => backgrounds.push(background.clone()),
            Statement::ShowSprite {
                character_id,
                emotion,
                ..
            } => {
                used_characters.push(character_id.clone());
                sprites.push((character_id.clone(), emotion.clone()));
            }
            Statement::HideSprite { character_id, .. }
            | Statement::MoveSprite { character_id, .. } => {
                used_characters.push(character_id.clone())
            }
            Statement::MethodCall {
                target,
                method,
                arg,
                ..
            } => {
                if method == "show" {
                    used_characters.push(target.clone());
                    sprites.push((target.clone(), arg.clone()));
                }
            }
            Statement::MusicPlay { file, .. } => music_files.push(file.clone()),
            Statement::SfxPlay { file, .. } | Statement::SfxStop { file, .. } => {
                sfx_files.push(file.clone())
            }
            Statement::Imagemap {
                background,
                hover_image,
                hotspots,
            } => {
                backgrounds.push(background.clone());
                if let Some(hover_image) = hover_image {
                    backgrounds.push(hover_image.clone());
                }
                for hotspot in hotspots {
                    collect_from_script(
                        &hotspot.body,
                        labels,
                        jumps,
                        declared_characters,
                        used_characters,
                        backgrounds,
                        sprites,
                        music_files,
                        sfx_files,
                        assigned_vars,
                        used_vars,
                    );
                }
            }
            Statement::Config { .. }
            | Statement::Return
            | Statement::MusicStop { .. }
            | Statement::MusicVolume { .. }
            | Statement::TypewriterSet { .. }
            | Statement::TypewriterSpeed { .. } => {}
        }
    }
}

fn collect_text_vars(text: &InterpolatedText, out: &mut Vec<String>) {
    for segment in &text.0 {
        if let TextSegment::Interp(expr) = segment {
            collect_expr_vars(expr, out);
        }
    }
}

fn collect_expr_vars(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Var(name) => out.push(name.clone()),
        Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
            collect_expr_vars(left, out);
            collect_expr_vars(right, out);
        }
        Expr::Neg(inner) | Expr::Not(inner) => collect_expr_vars(inner, out),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}

fn asset_exists(assets_dir: &Path, path_without_or_with_ext: &str, extensions: &[&str]) -> bool {
    let direct = assets_dir.join(path_without_or_with_ext);
    if direct.extension().is_some() {
        return direct.exists();
    }

    for ext in extensions {
        if assets_dir
            .join(format!("{path_without_or_with_ext}.{ext}"))
            .exists()
        {
            return true;
        }
    }

    false
}

fn dev_project(project: &str) -> std::result::Result<(), String> {
    let project_dir = Path::new(project);
    let cfg = load_project_config(project_dir)?;
    let script_path = project_dir.join(&cfg.project.main_script);
    let scripts_dir = script_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| project_dir.join("scripts"));

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to determine current executable path: {e}"))?;

    println!("🚀 RVN dev mode started for {project}");
    println!("👀 Watching {}", scripts_dir.display());
    println!("🎮 F9 = debug overlay, F10 = step while overlay is visible");

    let mut child = spawn_run_process(&current_exe, project)?;

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;
    watcher
        .watch(&scripts_dir, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch {}: {e}", scripts_dir.display()))?;

    loop {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(_event) => {
                println!("🔁 Script changed, restarting game…");
                restart_child(&mut child, &current_exe, project)?;
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(_status) = child
                    .try_wait()
                    .map_err(|e| format!("Failed to check game process: {e}"))?
                {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("File watcher disconnected".to_string());
            }
        }
    }

    Ok(())
}

fn spawn_run_process(current_exe: &Path, project: &str) -> std::result::Result<Child, String> {
    Command::new(current_exe)
        .arg("run")
        .arg(project)
        .spawn()
        .map_err(|e| format!("Failed to start game process: {e}"))
}

fn restart_child(
    child: &mut Child,
    current_exe: &Path,
    project: &str,
) -> std::result::Result<(), String> {
    let _ = child.kill();
    let _ = child.wait();
    *child = spawn_run_process(current_exe, project)?;
    Ok(())
}
