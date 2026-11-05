//! Reserve-only startup maintenance under the existing Source writer lock.
//! Never treats normalized/flattened recovery as an empty editable payload.
use super::*;
/// Counter-only rebasing is allowed after sidecar publication, including an
/// archive failure. All OTHER baseline JSON fields must remain exactly equal.
/// This pure projection grants no Source/model capability; open uses it only
/// to avoid reusing a genuinely retained committed graph identity.
pub(super) fn owned_reservation1025(bytes:&str,recovery:&Recovery,published:&Snapshot,sidecar:&str)->Result<Option<u64>,String>{
    validate_recovery_editorial(recovery)?;
    let Some(reserved)=recovery.graph_id_high_water else{return Ok(None);};
    if !recovery.widgets.is_empty()||!recovery.widget_memberships.is_empty()||!recovery.enums.is_empty()||!recovery.libraries.is_empty()
        ||recovery.asset_placements_after.is_some()||published.asset_placements.is_some(){return Ok(None);}
    if recovery.source!=published.source||recovery.baseline_source!=published.source_snapshot{return Ok(None);}
    let Some(before)=recovery.baseline_sidecar.as_deref()else{return Ok(None);};
    let old:Snapshot=serde_json::from_str(before).map_err(|error|error.to_string())?;
    let old_high=validated_snapshot_high_water(&old)?;let published_high=validated_snapshot_high_water(published)?;
    let mut old_raw:serde_json::Value=serde_json::from_str(before).map_err(|error|error.to_string())?;
    let mut published_raw:serde_json::Value=serde_json::from_str(sidecar).map_err(|error|error.to_string())?;
    let raw:serde_json::Value=serde_json::from_str(bytes).map_err(|error|error.to_string())?;
    let same_graphs=raw.get("graphs").is_some()&&raw.get("graphs")==published_raw.get("graphs");
    old_raw.as_object_mut().ok_or("Invalid reservation baseline")?.remove("graph_id_high_water");
    published_raw.as_object_mut().ok_or("Invalid published reservation baseline")?.remove("graph_id_high_water");
    if old_raw!=published_raw||!same_graphs||recovery.graphs!=published.graphs{return Ok(None);}
    if published_high<old_high||reserved<old_high.max(recovery.graphs.iter().map(|g|g.graph_id.get()).max().unwrap_or(0)){
        return Err("Reservation-only recovery has a regressed identity counter; cache retained".into());
    }
    validate_recovery_graph_shapes(&recovery.graphs)?;
    Ok(Some(reserved.max(published_high)))
}
impl SourceWorkspace {
    /// Archive only the exact authored baseline and its explicit monotone ID
    /// reservation. False leaves ordinary recovery untouched. Errors retain
    /// cache/counters; no document/publication receipt is manufactured.
    pub fn archive_reservation_only_recovery1025(&mut self)->Result<bool,String>{
        if self.expected_recovery.is_none(){return Ok(false);}
        let _guard=lock(&self.root)?;
        if self.root.join(JOURNAL).exists()||self.root.join(refactor::REFACTOR_JOURNAL).exists(){
            return Err("Finish the existing Source transaction before reservation-only recovery maintenance".into());
        }
        self.check_recovery_owner()?;self.check_recovery_baseline()?;
        if let Some(error)=&self.source_error{return Err(error.clone());}
        let bytes=self.expected_recovery.as_deref().ok_or("Reservation cache disappeared")?;
        let original=self.expected_sidecar.as_deref().ok_or("Reservation-only recovery requires its published sidecar")?;
        // Source4 Recovery denies unknown top-level fields. Raw graph/baseline
        // comparison above precedes every normalization/flattening operation.
        let recovery:Recovery=serde_json::from_str(bytes).map_err(|error|error.to_string())?;
        let snapshot:Snapshot=serde_json::from_str(original).map_err(|error|error.to_string())?;
        let Some(reserved)=owned_reservation1025(bytes,&recovery,&snapshot,original)? else{return Ok(false);};
        if self.asset_placements.is_some()||snapshot.graphs!=self.project.graphs()
            ||snapshot.source_snapshot!=self.project.source()||snapshot.imports!=self.project.imports_snapshot(){return Ok(false);}
        // The union remains consumed even if the following IO fails. Its
        // authentic cache/sidecar remains durable and retryable in that case.
        self.graph_id_high_water=self.graph_id_high_water().max(reserved);
        if self.has_pending_graph_identity_reservation()?{self.persist_reservation_archive_locked1025()?;}
        else{self.archive_owned_recovery_locked1025()?;}
        Ok(true)
    }
}
#[cfg(test)]
#[path="source_reservation_only_recovery_tests1025.rs"]
mod tests;
