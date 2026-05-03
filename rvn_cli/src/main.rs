use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rvn_bevy::run_game;
use serde::Deserialize;

mod check;
use check::{check_project, CheckOptions};

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
        /// Treat warnings as command failures and print the full summary.
        #[arg(long)]
        strict: bool,
    },

    /// Build a validated, distributable data package for an RVN project.
    Build {
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
            let report = check_project(&project, CheckOptions::default());
            if report.has_errors() {
                print_project_diagnostics(&project, &report.diagnostics);
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
        Commands::Check { project, strict } => {
            let report = check_project(&project, CheckOptions { strict });
            if report.diagnostics.is_empty() {
                println!("Check completed: 0 errors, 0 warnings");
            } else {
                print_project_diagnostics(&project, &report.diagnostics);
                if report.failed {
                    eprintln!(
                        "\nCheck failed: {} errors, {} warnings",
                        report.error_count(),
                        report.warning_count()
                    );
                } else {
                    println!(
                        "\nCheck completed: {} errors, {} warnings",
                        report.error_count(),
                        report.warning_count()
                    );
                }
            }
            if report.failed {
                std::process::exit(1);
            }
        }
        Commands::Build { project } => {
            build_project(&project)?;
        }
        Commands::Dev { project } => {
            if let Err(e) = dev_project(&project) {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn print_project_diagnostics(project: &str, diagnostics: &[check::Diagnostic]) {
    eprintln!("RVN project validation in {project}:");
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}");
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
    config_contents = config_contents
        .replace("Mon Jeu RVN", name)
        .replace("La Clairière des Échos", name);
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

fn build_project(project: &str) -> Result<()> {
    let project_dir = Path::new(project);

    println!("Check en cours...");
    let report = check_project(project, CheckOptions::default());
    if report.has_errors() {
        print_project_diagnostics(project, &report.diagnostics);
        anyhow::bail!("rvn build aborted: fix the project errors above before building.");
    }

    let cfg = load_project_config(project_dir).map_err(anyhow::Error::msg)?;
    let game_name = cfg
        .project
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .map(sanitize_game_name)
        .unwrap_or_else(|| {
            project_dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(sanitize_game_name)
                .unwrap_or_else(|| "Game".to_string())
        });
    let platform = detect_platform()?;
    let dist_dir = create_dist_structure(project_dir, &game_name, &platform)?;

    println!("Build runtime...");
    let runtime_binary = build_runtime()?;

    println!("Copie fichiers...");
    copy_project_files(project_dir, &dist_dir, &cfg)?;

    let output_binary = dist_dir.join(&game_name);
    fs::copy(&runtime_binary, &output_binary).with_context(|| {
        format!(
            "unable to copy runtime binary '{}' to '{}'",
            runtime_binary.display(),
            output_binary.display()
        )
    })?;

    println!("Build terminé.");
    println!("Build terminé : {}/", dist_dir.display());
    println!("Lancez : ./{}", game_name);
    Ok(())
}

fn sanitize_game_name(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "Game".to_string()
    } else {
        sanitized
    }
}

fn detect_platform() -> Result<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux-x64".to_string()),
        (os, arch) => anyhow::bail!("unsupported build platform: {os}-{arch}"),
    }
}

fn create_dist_structure(project_dir: &Path, game_name: &str, platform: &str) -> Result<PathBuf> {
    let dist_dir = project_dir
        .join("dist")
        .join(format!("{game_name}-{platform}"));
    if dist_dir.exists() {
        fs::remove_dir_all(&dist_dir).with_context(|| {
            format!(
                "unable to remove previous build directory '{}'",
                dist_dir.display()
            )
        })?;
    }
    fs::create_dir_all(dist_dir.join("data")).with_context(|| {
        format!(
            "unable to create build data directory '{}'",
            dist_dir.join("data").display()
        )
    })?;
    Ok(dist_dir)
}

fn build_runtime() -> Result<PathBuf> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("unable to resolve RVN workspace root")?;
    let manifest_path = workspace_root.join("Cargo.toml");
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "rvn_bevy", "--manifest-path"])
        .arg(&manifest_path)
        .status()
        .context("unable to invoke cargo build for rvn_bevy runtime")?;

    if !status.success() {
        anyhow::bail!("runtime build failed with status {status}");
    }

    let binary_name = if cfg!(windows) {
        "rvn_bevy.exe"
    } else {
        "rvn_bevy"
    };
    Ok(workspace_root
        .join("target")
        .join("release")
        .join(binary_name))
}

fn copy_project_files(project_dir: &Path, dist_dir: &Path, cfg: &ProjectConfig) -> Result<()> {
    let data_dir = dist_dir.join("data");
    copy_file_relative(project_dir, &data_dir, Path::new("rvn.toml"))?;
    copy_optional_file_relative(project_dir, &data_dir, Path::new(&cfg.paths.theme))?;
    copy_dir_relative(project_dir, &data_dir, Path::new(&cfg.paths.assets))?;
    copy_dir_relative(project_dir, &data_dir, Path::new(&cfg.paths.locales))?;

    let main_script = Path::new(&cfg.project.main_script);
    if let Some(script_dir) = main_script.parent().filter(|p| !p.as_os_str().is_empty()) {
        copy_dir_relative(project_dir, &data_dir, script_dir)?;
    } else {
        copy_file_relative(project_dir, &data_dir, main_script)?;
    }

    fs::create_dir_all(data_dir.join(&cfg.paths.saves)).with_context(|| {
        format!(
            "unable to create saves directory '{}' in build output",
            cfg.paths.saves
        )
    })?;
    Ok(())
}

fn copy_file_relative(project_dir: &Path, dist_dir: &Path, relative: &Path) -> Result<()> {
    let src = project_dir.join(relative);
    let dst = dist_dir.join(relative);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory '{}'", parent.display()))?;
    }
    fs::copy(&src, &dst).with_context(|| {
        format!(
            "unable to copy file '{}' to '{}'",
            src.display(),
            dst.display()
        )
    })?;
    Ok(())
}

fn copy_optional_file_relative(project_dir: &Path, dist_dir: &Path, relative: &Path) -> Result<()> {
    let src = project_dir.join(relative);
    if !src.exists() {
        return Ok(());
    }
    copy_file_relative(project_dir, dist_dir, relative)
}

fn copy_dir_relative(project_dir: &Path, dist_dir: &Path, relative: &Path) -> Result<()> {
    let src = project_dir.join(relative);
    let dst = dist_dir.join(relative);
    copy_dir_all(&src, &dst)
        .with_context(|| format!("unable to copy directory '{}'", src.display()))
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("unable to create directory '{}'", dst.display()))?;

    for entry in fs::read_dir(src).with_context(|| format!("unable to read '{}'", src.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dst_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("unable to create directory '{}'", parent.display())
                })?;
            }
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "unable to copy file '{}' to '{}'",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
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
