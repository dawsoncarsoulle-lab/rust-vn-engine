//! Identity references accompanying the SAME source recovery transaction.
//! This is metadata, never another editable graph/tree or persistence writer.
use super::*;
use rvn_parser::widget_asset::{validate_relative_locator, WidgetBlueprintV1};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetRecovery {
    /// Project-relative .rvnwidget destination. The App additionally admits
    /// its physical parent against the project's configured asset directory.
    pub locator: String,
    /// Canonical shared JSON v1 containing references only, never graph copies.
    pub blueprint_json: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SourceRecoveryBatch {
    pub enums:Vec<rvn_parser::enum_recovery1025::EnumRecoveryRecord1025>,
    pub asset_placements:Option<SourceAssetPlacements>,
    pub widget_memberships: Vec<WidgetMembershipRecovery>,
    pub graphs: Vec<GraphDocument>,
    pub widgets: Vec<WidgetRecovery>,
}
impl WidgetRecovery {
    pub fn blueprint(&self) -> Result<WidgetBlueprintV1, String> {
        validate_relative_locator(&self.locator, Some("rvnwidget"))?;
        let blueprint = WidgetBlueprintV1::decode(self.blueprint_json.as_bytes())?;
        if blueprint.encode()? != self.blueprint_json {
            return Err("Recovered Widget metadata must use the canonical shared codec".into());
        }
        Ok(blueprint)
    }
}
pub(super) fn validate(
    widgets: &[WidgetRecovery],
    graphs: &[GraphDocument],
    baseline: &[GraphDocument],
    source: &str,
) -> Result<(), String> {
    if widgets.len() > 256 {
        return Err("Widget recovery exceeds 256 unpublished assets".into());
    }
    let mut bytes = 0usize;
    let mut uuids = BTreeSet::new();
    let mut locators = BTreeSet::new();
    let mut members = BTreeSet::new();
    let graph_map = graphs
        .iter()
        .map(|graph| (graph.graph_id, graph))
        .collect::<BTreeMap<_, _>>();
    if graph_map.len() != graphs.len() {
        return Err("Recovery repeats a graph identity".into());
    }
    let saved = baseline
        .iter()
        .map(|graph| graph.graph_id)
        .collect::<BTreeSet<_>>();
    for entry in widgets {
        bytes = bytes
            .checked_add(entry.blueprint_json.len())
            .and_then(|n| n.checked_add(entry.locator.len()))
            .ok_or("Widget recovery byte budget overflow")?;
        if bytes > 4 * 1024 * 1024 {
            return Err("Widget recovery metadata exceeds four MiB".into());
        }
        let blueprint = entry.blueprint()?;
        if blueprint.source != source
            || !uuids.insert(blueprint.uuid.clone())
            || !locators.insert(entry.locator.to_lowercase())
        {
            return Err("Recovered Widget UUID, source or locator is ambiguous".into());
        }
        let mut screens = 0;
        for reference in &blueprint.members {
            let id = crate::GraphId::new(reference.get());
            if saved.contains(&id) || !members.insert(id) {
                return Err(
                    "Unpublished Widget recovery aliases a saved graph or another Widget member"
                        .into(),
                );
            }
            let graph = graph_map
                .get(&id)
                .ok_or("Recovered Widget references an absent authored graph")?;
            match &graph.kind{
                crate::GraphKind::Screen{..}if *reference==blueprint.screen_graph=>screens+=1,
                crate::GraphKind::Handler{..}|crate::GraphKind::Function{..}if *reference!=blueprint.screen_graph=>{},
                _=>return Err("Recovered Widget must reference exactly one Screen and only Handler/Function siblings".into()),
            }
        }
        if screens != 1 {
            return Err("Recovered Widget has no unique actual Screen".into());
        }
    }
    Ok(())
}

pub(super) fn preserve_identities(
    previous: &SourceRecoveryBatch,
    graphs: &[GraphDocument],
    widgets: &[WidgetRecovery],
) -> Result<(), String> {
    let next_ids = graphs
        .iter()
        .map(|graph| graph.graph_id)
        .collect::<BTreeSet<_>>();
    for old in &previous.widgets {
        let before = old.blueprint()?;
        if widgets.iter().any(|next| next == old) {
            continue;
        }
        if let Some(next)=widgets.iter().find(|entry|entry.blueprint().is_ok_and(|value|value.uuid==before.uuid)){
            let after=next.blueprint()?;
            if next.locator!=old.locator||after.source!=before.source||after.screen_graph!=before.screen_graph{
                return Err("An unpublished member edit must retain its actual UUID, source, Screen and locator".into());
            }
            let old_ids=before.members.iter().map(|id|crate::GraphId::new(id.get())).collect::<BTreeSet<_>>();
            let new_ids=after.members.iter().map(|id|crate::GraphId::new(id.get())).collect::<BTreeSet<_>>();
            for id in new_ids.difference(&old_ids){
                if previous.graphs.iter().any(|graph|graph.graph_id==*id)
                    || !graphs.iter().any(|graph|graph.graph_id==*id&&matches!(graph.kind,crate::GraphKind::Handler{..})){
                    return Err("An unpublished event member must be a genuinely new Handler, never an adopted graph".into());
                }
            }
            for id in old_ids.difference(&new_ids){
                if next_ids.contains(id)||!previous.graphs.iter().any(|graph|graph.graph_id==*id&&matches!(graph.kind,crate::GraphKind::Handler{..})){
                    return Err("Removing an unpublished event member must remove its real Handler model".into());
                }
            }
            continue;
        }
        if before
            .members
            .iter()
            .any(|id| next_ids.contains(&crate::GraphId::new(id.get())))
        {
            return Err("An unpublished Widget identity cannot be dropped or replaced while any of its real members survive recovery".into());
        }
    }
    Ok(())
}

/// Defense against a legacy graph-only save accidentally publishing a Widget
/// recovery's graphs and archiving its only UUID definition. Full manifest
/// parsing remains in the single App asset factory; these exact participants
/// still undergo the existing writer's physical checks/CAS under its lock.
pub(super) fn publication_participants(
    batch: &SourceRecoveryBatch,
    additional: &[SourceRefactorFile],
) -> Result<(), String> {
    if batch.widgets.is_empty() && batch.widget_memberships.is_empty() {
        return Ok(());
    }
    if additional
        .iter()
        .filter(|file| file.path == "rvn.toml" && file.after.is_some())
        .count()
        != 1
    {
        return Err("Unpublished Widget recovery requires the real manifest CAS participant in the same source publication".into());
    }
    for widget in &batch.widgets {
        let exact = additional
            .iter()
            .filter(|file| file.path == widget.locator)
            .collect::<Vec<_>>();
        let [file] = exact.as_slice() else {
            return Err("First Widget save requires its exact .rvnwidget participant".into());
        };
        if file.before.is_some() || file.after.as_deref() != Some(widget.blueprint_json.as_str()) {
            return Err("First Widget publication differs from its actual recovered UUID/reference definition".into());
        }
    }
    membership_publication_participants(batch, additional)?;
    Ok(())
}

#[path="source_widget_membership_recovery1003.rs"]
mod membership1003;
pub use membership1003::WidgetMembershipRecovery;
pub(super) use membership1003::{validate_memberships,membership_baseline_files,preserve_memberships,membership_publication_participants};
