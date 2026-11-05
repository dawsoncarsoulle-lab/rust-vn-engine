//! Admit the Widget classes in the actual Desktop data directory, without an
//! editor cache, Engine, Bevy World, Init execution or another source compiler.
use anyhow::{ensure, Context, Result};
use rvn_parser::widget_asset::{self, LocatorCase, WidgetBlueprintV1, WidgetIndexInput};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::Read, path::{Component, Path, PathBuf}};

const SOURCE_BYTES: usize = 16 * 1024 * 1024;
const DESCRIPTOR_BYTES: usize = 4 * 1024 * 1024;

/// Content and closure membership, not a runtime/class capability. The final
/// publication fence reconstructs admission, then compares these exact inputs.
pub(crate) struct Prepared { inputs: BTreeMap<PathBuf, [u8; 32]> }
impl Prepared {
    pub(crate) fn verify(&self, root: &Path, cfg: &crate::ProjectConfig, manifest: &[u8]) -> Result<()> {
        let current = prepare(root, cfg, manifest)?;
        ensure!(self.inputs == current.inputs,
            "Desktop Widget source/descriptor inputs changed after preflight; the previous export is retained");
        Ok(())
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }

fn read_inside(root: &Path, relative: &Path, maximum: usize) -> Result<Vec<u8>> {
    ensure!(!relative.as_os_str().is_empty() && relative.components().all(|part| matches!(part, Component::Normal(_))),
        "Desktop Widget input must be project-relative: '{}'", relative.display());
    let mut path = root.to_path_buf();
    for part in relative.components() { path.push(part); crate::desktop_paths::validate_entry(&path)?; }
    ensure!(path.canonicalize()? == path, "Desktop Widget input redirects from its exact path: '{}'", relative.display());
    let mut file = fs::File::open(&path).with_context(|| format!("Desktop Widget input '{}'", relative.display()))?;
    ensure!(file.metadata()?.is_file(), "Desktop Widget input is not a regular file: '{}'", relative.display());
    let mut bytes = Vec::new();
    file.by_ref().take((maximum + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= maximum, "Desktop Widget input exceeds {maximum} bytes: '{}'", relative.display());
    Ok(bytes)
}

pub(crate) fn prepare(directory: &Path, cfg: &crate::ProjectConfig, manifest: &[u8]) -> Result<Prepared> {
    let root = directory.canonicalize().context("Desktop Widget data directory")?;
    // The build already freezes this complete manifest. Apply no new source or
    // Native validation to a project with an absent/empty valid Widget index.
    ensure!(fs::read(root.join("rvn.toml"))? == manifest, "Desktop delivered manifest differs from the prepared project");
    let config: toml::Value = toml::from_str(std::str::from_utf8(manifest)?)?;
    let value = config.get("widget_blueprints");
    let entries = value.and_then(toml::Value::as_table).map(|table| table.iter().map(|(uuid, locator)|
        (uuid.as_str(), locator.as_str())).collect::<Vec<_>>());
    if let Some(entries) = &entries {
        ensure!(entries.len() <= 256, "Desktop Widget index exceeds 256 classes");
        for (uuid, locator) in entries {
            rvn_parser::localized_text::validate_text_asset_id(uuid).map_err(anyhow::Error::msg)
                .with_context(|| format!("Desktop [widget_blueprints] UUID '{uuid}'"))?;
            let locator = locator.with_context(|| format!("Desktop Widget {uuid} index locator must be a string"))?;
            widget_asset::validate_relative_locator(locator, Some("rvnwidget")).map_err(anyhow::Error::msg)
                .with_context(|| format!("Desktop Widget {uuid} index locator '{locator}'"))?;
        }
    }
    let input = match (value, &entries) {
        (None, _) => WidgetIndexInput::Absent,
        (Some(_), Some(entries)) => WidgetIndexInput::Table(entries),
        (Some(_), None) => WidgetIndexInput::NotTable,
    };
    let case = if cfg!(windows) { LocatorCase::Insensitive } else { LocatorCase::Sensitive };
    let index = widget_asset::validate_index(input, case).map_err(anyhow::Error::msg)
        .context("Desktop [widget_blueprints] index")?;
    let mut inputs = BTreeMap::from([(PathBuf::from("rvn.toml"), digest(manifest))]);
    if index.is_empty() { return Ok(Prepared { inputs }); }
    ensure!(index.len() <= 256, "Desktop Widget index exceeds 256 classes");
    ensure!(manifest.len() <= SOURCE_BYTES, "Desktop Widget manifest exceeds the native loader's 16 MiB limit");
    // Decode the delivered configuration itself, rather than adopting stale
    // caller paths. The existing Desktop copier must have used the same values.
    let delivered: crate::ProjectConfig = toml::from_str(std::str::from_utf8(manifest)?)?;
    ensure!(delivered.project.main_script == cfg.project.main_script && delivered.paths.assets == cfg.paths.assets,
        "Desktop Widget source/assets configuration changed before staging");
    widget_asset::validate_relative_locator(&cfg.paths.assets, None).map_err(anyhow::Error::msg)?;
    for journal in [".rvn-authoring.transaction.json", ".rvn-refactor.transaction.json"] {
        ensure!(!root.join(journal).try_exists()?, "Desktop Widget data contains pending transaction '{journal}'");
    }
    ensure!(!root.join(".rvn-authoring.json").try_exists()?,
        "Desktop Widget admission must use shipped source identities, not an editor cache");
    let mut descriptors = Vec::with_capacity(index.len());
    let mut source_names = BTreeSet::new();
    let mut descriptor_bytes = 0usize;
    for (uuid, locator) in &index {
        let relative = Path::new(&cfg.paths.assets).join(locator);
        let bytes = read_inside(&root, &relative, widget_asset::MAX_FILE_BYTES)
            .with_context(|| format!("Desktop Widget {uuid} at '{}'", relative.display()))?;
        descriptor_bytes = descriptor_bytes.checked_add(bytes.len()).and_then(|n| n.checked_add(locator.len()))
            .filter(|n| *n <= DESCRIPTOR_BYTES).context("Desktop Widget descriptor/locator set exceeds 4 MiB")?;
        let descriptor = WidgetBlueprintV1::decode(&bytes).map_err(anyhow::Error::msg)
            .with_context(|| format!("Desktop Widget {uuid} descriptor '{}'", relative.display()))?;
        ensure!(descriptor.uuid == *uuid, "Desktop Widget index UUID {uuid} differs from descriptor '{}' UUID {}", relative.display(), descriptor.uuid);
        source_names.insert(descriptor.source.clone());
        inputs.insert(relative, digest(&bytes));
        descriptors.push((locator, descriptor, bytes));
    }
    // These are exactly the files the ordinary player will parse, including
    // its actual imports. No authoring presentation is supplied to Graph.
    let program = rvn_parser::parse_file_program_with_uses(root.join(&cfg.project.main_script))
        .map_err(anyhow::Error::msg).context("Desktop Widget delivered source program")?;
    let mut source_bytes = 0usize;
    for source in &program.sources {
        let path = source.path.as_ref().context("Desktop Widget source has no physical origin")?;
        let relative = path.strip_prefix(&root).context("Desktop Widget import escapes the delivered project")?;
        source_bytes = source_bytes.checked_add(source.source.len()).filter(|n| *n <= SOURCE_BYTES)
            .context("Desktop Widget source closure exceeds 16 MiB")?;
        let bytes = read_inside(&root, relative, SOURCE_BYTES)?;
        ensure!(bytes == source.source.as_bytes(), "Desktop Widget source changed during admission: '{}'", relative.display());
        inputs.insert(relative.to_owned(), digest(&bytes));
    }
    let mut models = Vec::with_capacity(source_names.len());
    for source in &source_names {
        let path = root.join(source);
        let frozen = program.sources.iter().find(|file| file.path.as_ref() == Some(&path))
            .with_context(|| format!("Desktop indexed Widget source '{source}' is outside the delivered import closure"))?;
        let participants = descriptors.iter().filter(|(_, descriptor, _)| &descriptor.source == source)
            .map(|(locator, descriptor, _)| format!("{} at {} members [{}]", descriptor.uuid, locator,
                descriptor.members.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))).collect::<Vec<_>>().join("; ");
        let model = rvn_graph::SourceProject::open_native_preparation_at(&path, frozen.source.clone(), &[])
            .map_err(anyhow::Error::msg).with_context(|| format!("Desktop Widget source '{source}' ({participants}); save/upgrade its actual owned members before exporting"))?;
        models.push((source.as_str(), model));
    }
    for (locator, descriptor, _) in &descriptors {
        let (_, model) = models.iter().find(|(source, _)| *source == descriptor.source)
            .context("Desktop Widget source model disappeared")?;
        for member in &descriptor.members {
            let graph = model.graphs().iter().find(|graph| graph.graph_id.get() == member.get())
                .with_context(|| format!("Desktop Widget {} at '{locator}', source '{}', member {member} has no source-embedded identity; upgrade that member in its Widget editor and Save before exporting", descriptor.uuid, descriptor.source))?;
            ensure!(!graph.inspector_evaluation.is_legacy(),
                "Desktop Widget {} at '{locator}', member {member} ({:?}) is Legacy; upgrade that member in its Widget editor and Save before exporting", descriptor.uuid, graph.kind);
            ensure!(matches!(&graph.kind, rvn_graph::GraphKind::Screen {..}|rvn_graph::GraphKind::Handler {..}|rvn_graph::GraphKind::Function {..}),
                "Desktop Widget {} at '{locator}', member {member} has unsupported definition kind {:?}", descriptor.uuid, graph.kind);
            ensure!(matches!(&graph.kind, rvn_graph::GraphKind::Screen {..}) == (*member == descriptor.screen_graph),
                "Desktop Widget {} at '{locator}', member {member} ({:?}) does not match its designated Screen {}", descriptor.uuid, graph.kind, descriptor.screen_graph);
        }
    }
    let assets = descriptors.iter().map(|(locator, _, bytes)| rvn_graph::WidgetRuntimeAsset { locator, bytes }).collect::<Vec<_>>();
    let sources = models.iter().map(|(source, project)| rvn_graph::WidgetRuntimeSource { source, project }).collect::<Vec<_>>();
    let entries = index.iter().map(|(uuid, locator)| (uuid.as_str(), Some(locator.as_str()))).collect::<Vec<_>>();
    let admission = rvn_graph::prepare_widget_runtime_classes(&root, &program, WidgetIndexInput::Table(&entries), case, &assets, &sources)
        .map_err(anyhow::Error::msg).context("Desktop Widget class source/member admission failed")?;
    // Reuse the existing CLI compiler→Core typed bridge, without constructing
    // an Engine or running authored code. Unsupported actual program contracts
    // get the same independent Core admission as the real loader.
    crate::widget_runtime_loader_0950::core_classes(&admission).map_err(anyhow::Error::msg)
        .context("Desktop Widget class runtime contract admission failed")?;
    for (relative, expected) in &inputs {
        let maximum = if relative.extension().and_then(|e| e.to_str()) == Some("rvnwidget") { widget_asset::MAX_FILE_BYTES } else { SOURCE_BYTES };
        ensure!(digest(&read_inside(&root, relative, maximum)?) == *expected,
            "Desktop Widget input changed while preparing classes: '{}'", relative.display());
    }
    Ok(Prepared { inputs })
}

#[cfg(test)]
#[path = "desktop_widget_tests227.rs"]
mod tests;
