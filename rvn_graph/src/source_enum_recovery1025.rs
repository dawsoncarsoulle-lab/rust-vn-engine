//! Enum data participates in the existing Source recovery, not a second model.
use super::*;
use rvn_parser::enum_recovery1025::{EnumRecoveryRecord1025 as Record,validate_enum_recovery1025};
use std::collections::BTreeSet;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum EnumRecoveryChange1025 {
    Upsert { #[serde(deserialize_with="required_before1025")] before:Option<Record>, after:Record },
    Remove { before:Record },
}
fn required_before1025<'de,D:serde::Deserializer<'de>>(d:D)->Result<Option<Record>,D::Error>{Option::<Record>::deserialize(d)}
impl EnumRecoveryChange1025 {
    pub fn asset_id(&self)->rvn_parser::enum_asset::EnumAssetId{match self{Self::Upsert{after,..}=>after.asset_id(),Self::Remove{before}=>before.asset_id()}}
}

pub(super) fn validate_records1025(records:&[Record],widgets:&[WidgetRecovery])->Result<(),String>{
    validate_enum_recovery1025(records,cfg!(windows))?;
    let ids=records.iter().map(|entry|entry.asset_id().to_hex()).collect::<BTreeSet<_>>();
    let paths=records.iter().map(|entry|path_key1025(entry.locator())).collect::<BTreeSet<_>>();
    for widget in widgets{
        let value=widget.blueprint()?;
        if ids.contains(&value.uuid)||paths.contains(&path_key1025(&widget.locator)){
            return Err("Enum recovery aliases a Widget identity or locator".into());
        }
    }
    Ok(())
}
fn same_path1025(a:&Path,b:&Path)->bool{if cfg!(windows){a.to_string_lossy().to_lowercase()==b.to_string_lossy().to_lowercase()}else{a==b}}
fn path_key1025(path:&str)->String{if cfg!(windows){path.replace('\\',"/").to_lowercase()}else{path.to_owned()}}

pub(super) fn apply_changes1025(previous:&[Record],changes:&[EnumRecoveryChange1025],recovery_only:bool)->Result<Vec<Record>,String>{
    validate_enum_recovery1025(previous,cfg!(windows))?;
    if changes.len()>256{return Err("Too many Enum recovery mutations".into());}
    let mut ids=BTreeSet::new();let mut next=previous.to_vec();
    for change in changes{
        if !ids.insert(change.asset_id()){return Err("Enum recovery mutations repeat an identity".into());}
        let index=next.iter().position(|entry|entry.asset_id()==change.asset_id());
        match change{
            EnumRecoveryChange1025::Upsert{before,after}=>{
                after.validate()?;
                match (before,index){
                    (None,None)=>next.push(after.clone()),
                    (Some(before),Some(index))if next[index]==*before=>{
                        if before.asset_id()!=after.asset_id()||before.locator()!=after.locator(){return Err("Enum recovery update changed its original identity or locator".into());}
                        if recovery_only&&before.baseline()!=after.baseline(){return Err("Recovery-only maintenance cannot acknowledge a published Enum baseline".into());}
                        next[index]=after.clone();
                    },
                    _=>return Err("Enum recovery changed before its exact mutation".into()),
                }
            },
            EnumRecoveryChange1025::Remove{before}=>{
                let index=index.filter(|index|next[*index]==*before).ok_or("Enum recovery removal lost its exact original record")?;
                next.remove(index);
            },
        }
    }
    validate_enum_recovery1025(&next,cfg!(windows))?;Ok(next)
}

/// All observations go through the SAME canonical target policy as publication.
/// Prospective before/after bytes permit crash replay without treating an
/// already-published file as an external edit, while unrelated edits still fail.
pub(super) fn check_baselines1025(root:&Path,records:&[Record],files:&[SourceRefactorFile],after:bool)->Result<(),String>{
    for record in records{
        let path=refactor::target(root,record.locator())?;
        let expected=record.saved_wire().map(str::to_owned);
        let mut proposed=None;
        for file in files{
            if same_path1025(&refactor::target(root,&file.path)?,&path){
                if proposed.is_some(){return Err("Enum publication repeats a physical recovery target".into());}
                proposed=Some(if after{file.after.clone()}else{file.before.clone()});
            }
        }
        let actual=match proposed{Some(value)=>value,None=>read_optional(&path)?};
        if actual!=expected{return Err("Enum recovery publication baseline changed; all cache and authored files were retained".into());}
    }
    Ok(())
}

pub(super) fn check_legacy_metadata1025(root:&Path,records:&[Record],files:&[SourceRefactorFile])->Result<(),String>{
    for record in records{
        let path=refactor::target(root,record.locator())?;
        for file in files{
            if same_path1025(&refactor::target(root,&file.path)?,&path)&&file.before!=file.after{
                return Err("Metadata save touching recovered Enum data requires an exact typed recovery transition".into());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_publication1025(root:&Path,changes:&[EnumRecoveryChange1025],files:&[SourceRefactorFile])->Result<(),String>{
    for change in changes{
        let (before,after)=match change{EnumRecoveryChange1025::Upsert{before,after}=>(before.as_ref(),Some(after)),EnumRecoveryChange1025::Remove{before}=>(Some(before),None)};
        let record=before.or(after).ok_or("Enum publication has no identity")?;
        let path=refactor::target(root,record.locator())?;
        let file=files.iter().find(|file|refactor::target(root,&file.path).ok().is_some_and(|candidate|same_path1025(&candidate,&path)));
        let needs_file=after.is_none()||before.map(|before|before.baseline())!=after.map(|after|after.baseline());
        if let Some(file)=file{
            if file.before!=before.and_then(|record|record.saved_wire()).map(str::to_owned){return Err("Enum file CAS does not name the original recovery baseline".into());}
            let wire=file.after.as_deref().ok_or("Enum save cannot delete its published file")?;
            let submitted=rvn_parser::enum_asset::EnumAsset::decode(wire.as_bytes())?;
            // Recovery may be an older autosave. The caller freezes the actual
            // submitted model in its authenticated owner ticket; Source still
            // proves its identity, exact file CAS and exact recovery before.
            if submitted.id()!=record.asset_id()||submitted.parts().name!=record.current().parts().name{return Err("Enum submitted file changed its recovered identity or name".into());}
            if let Some(after)=after{if after.saved_wire()!=Some(wire){return Err("Enum after-recovery did not adopt the actual saved bytes".into());}}
        }else if needs_file{return Err("Enum recovery publication is missing its actual file participant".into());}
    }
    Ok(())
}

#[cfg(test)]
#[path="source_enum_recovery_tests1025.rs"]
mod tests;
