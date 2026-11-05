//! A published Widget's reference delta in the SAME source Recovery/writer.
//! Birth metadata remains subject to its original no-saved-members invariant.
use super::*;
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetMembershipRecovery {
    pub locator:String,
    pub before_blueprint_json:String,
    pub after_blueprint_json:String,
}
impl WidgetMembershipRecovery {
    pub fn before(&self)->Result<WidgetBlueprintV1,String>{
        validate_relative_locator(&self.locator,Some("rvnwidget"))?;
        WidgetBlueprintV1::decode(self.before_blueprint_json.as_bytes())
    }
    pub fn after(&self)->Result<WidgetBlueprintV1,String>{
        let before=self.before()?;let after=WidgetBlueprintV1::decode(self.after_blueprint_json.as_bytes())?;
        if after.uuid!=before.uuid||after.source!=before.source||after.screen_graph!=before.screen_graph||after.encode()?!=self.after_blueprint_json {
            return Err("Le delta publié doit conserver le vrai UUID, Screen et source, avec un descriptor candidat canonique.".into());
        }
        Ok(after)
    }
}
fn refs(value:&WidgetBlueprintV1)->BTreeSet<crate::GraphId>{value.members.iter().map(|id|crate::GraphId::new(id.get())).collect()}
pub(crate) fn validate_memberships(entries:&[WidgetMembershipRecovery],births:&[WidgetRecovery],graphs:&[GraphDocument],baseline:&[GraphDocument],source:&str)->Result<(),String>{
    if entries.len()+births.len()>256{return Err("Le lot Recovery Widget dépasse ses 256 identités.".into());}
    let actual=graphs.iter().map(|g|(g.graph_id,g)).collect::<BTreeMap<_,_>>();let saved=baseline.iter().map(|g|(g.graph_id,g)).collect::<BTreeMap<_,_>>();
    let mut uuids=BTreeSet::new();let mut locators=BTreeSet::new();let mut owned=BTreeSet::new();let mut before_owned=BTreeSet::new();let mut bytes=0usize;
    for birth in births{let value=birth.blueprint()?;uuids.insert(value.uuid);locators.insert(birth.locator.to_lowercase());for id in value.members{owned.insert(crate::GraphId::new(id.get()));}bytes=bytes.checked_add(birth.locator.len()+birth.blueprint_json.len()).ok_or("Budget Recovery Widget épuisé.")?;}
    for entry in entries{
        bytes=bytes.checked_add(entry.locator.len()).and_then(|n|n.checked_add(entry.before_blueprint_json.len())).and_then(|n|n.checked_add(entry.after_blueprint_json.len())).ok_or("Budget Recovery Widget épuisé.")?;
        if bytes>4*1024*1024{return Err("Les métadonnées Recovery Widget dépassent quatre MiB.".into());}
        let before=entry.before()?;let after=entry.after()?;let old=refs(&before);let new=refs(&after);
        if before.source!=source||!uuids.insert(before.uuid.clone())||!locators.insert(entry.locator.to_lowercase())||old==new{return Err("Le delta publié répète une identité, change de source ou ne modifie aucun membre.".into());}
        for id in &old{
            if !before_owned.insert(*id){return Err("Deux baselines de delta revendiquent le même membre.".into());}
            let graph=saved.get(id).ok_or("La baseline du delta ne contient pas son vrai membre sauvegardé.")?;
            match (&graph.kind,id.get()==before.screen_graph.get()){
                (crate::GraphKind::Screen{..},true)|(crate::GraphKind::Handler{..}|crate::GraphKind::Function{..},false)=>{},
                _=>return Err("La baseline du delta ne possède pas son vrai Screen et ses compagnons.".into()),
            }
        }
        for id in &new{
            if !owned.insert(*id){return Err("Deux Widgets Recovery revendiquent le même membre candidat.".into());}
            let graph=actual.get(id).ok_or("Le delta référence un membre candidat absent.")?;
            match (&graph.kind,id.get()==after.screen_graph.get()){
                (crate::GraphKind::Screen{..},true)|(crate::GraphKind::Handler{..}|crate::GraphKind::Function{..},false)=>{},
                _=>return Err("Le candidat du delta ne possède pas son vrai Screen et ses compagnons.".into()),
            }
        }
        for id in new.difference(&old){
            if saved.contains_key(id)||!matches!(actual[id].kind,crate::GraphKind::Handler{..}){return Err("Un nouveau membre événement doit être un vrai Handler neuf, sans adopter de graphe sauvegardé.".into());}
        }
        for id in old.difference(&new){
            if actual.contains_key(id)||!matches!(saved[id].kind,crate::GraphKind::Handler{..}){return Err("Le retrait d’un membre événement doit supprimer son vrai Handler et sa définition du candidat.".into());}
        }
    }
    Ok(())
}
pub(crate) fn membership_baseline_files(entries:&[WidgetMembershipRecovery])->Vec<SourceRefactorFile>{entries.iter().map(|entry|SourceRefactorFile{path:entry.locator.clone(),before:Some(entry.before_blueprint_json.clone()),after:Some(entry.before_blueprint_json.clone())}).collect()}
pub(crate) fn preserve_memberships(previous:&SourceRecoveryBatch,graphs:&[GraphDocument],entries:&[WidgetMembershipRecovery])->Result<(),String>{
    let present=graphs.iter().map(|g|g.graph_id).collect::<BTreeSet<_>>();
    for old in &previous.widget_memberships{
        let baseline=old.before()?;
        if let Some(next)=entries.iter().find(|next|next.before().is_ok_and(|before|before.uuid==baseline.uuid)){
            if next.locator!=old.locator||next.before_blueprint_json!=old.before_blueprint_json{return Err("La baseline physique d’un delta Widget récupéré a changé.".into());}
        }else{
            let before=refs(&baseline);let after=refs(&old.after()?);
            if !before.is_subset(&present)||after.difference(&before).any(|id|present.contains(id)){return Err("Le delta récupéré ne peut disparaître avant restauration de tous ses membres publiés.".into());}
        }
    }
    Ok(())
}
pub(crate) fn membership_publication_participants(batch:&SourceRecoveryBatch,files:&[SourceRefactorFile])->Result<(),String>{
    for entry in &batch.widget_memberships{
        let exact=files.iter().filter(|file|file.path==entry.locator).collect::<Vec<_>>();let[file]=exact.as_slice()else{return Err("La publication du delta exige son unique vrai descriptor CAS.".into());};
        if file.before.as_deref()!=Some(entry.before_blueprint_json.as_str())||file.after.as_deref()!=Some(entry.after_blueprint_json.as_str()){return Err("Le descriptor publié ne correspond pas au delta Widget gardé.".into());}
    }
    Ok(())
}