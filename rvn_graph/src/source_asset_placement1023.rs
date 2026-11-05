//! Editor metadata values for the existing Source authority; no IO or writer.
//! Not a second asset definition, native-input grant, or executable narrative.
use crate::{GraphDocument,GraphId};
use serde::{Deserialize,Deserializer,Serialize,Serializer};
use std::collections::{BTreeMap,BTreeSet};

/// Mirrors the two existing Browser routing namespaces, without depending on
/// the UI crate or deriving authority from a file's presence or display name.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum SourcePlacementNamespace { Physical, SourceGraphs }

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceAssetPlacement {
    pub graph_id:GraphId,
    pub namespace:SourcePlacementNamespace,
    /// Exact project-relative physical directory, or exact logical directory
    /// in SourceGraphs. No graph serialization/locator is stored in this row.
    pub directory:String,
}
impl SourceAssetPlacement {
    pub fn validate(&self)->Result<(),String>{
        if self.graph_id.get()==0{return Err("Source asset placement requires a nonzero GraphId".into());}
        // Reuse the existing relative-locator lexical policy. The App must
        // still admit the actual canonical parent, project and captured route.
        rvn_parser::widget_asset::validate_relative_locator(&self.directory,None)
            .map_err(|error|format!("Invalid Source asset placement directory: {error}"))
    }
}
/// Complete immutable value map. Construction/decoding rejects repeated IDs
/// rather than silently accepting the last JSON entry. Ordering is canonical
/// by identity, never by name or folder spelling.
#[derive(Clone,Debug,Default,PartialEq,Eq)]
pub struct SourceAssetPlacements(BTreeMap<GraphId,SourceAssetPlacement>);
impl SourceAssetPlacements {
    pub fn from_entries(entries:impl IntoIterator<Item=SourceAssetPlacement>)->Result<Self,String>{
        let mut map=BTreeMap::new();
        for entry in entries {
            entry.validate()?;
            if map.insert(entry.graph_id,entry).is_some(){return Err("Source asset placement repeats a GraphId".into());}
        }
        Ok(Self(map))
    }
    pub fn is_empty(&self)->bool{self.0.is_empty()}
    pub fn len(&self)->usize{self.0.len()}
    pub fn get(&self,graph:GraphId)->Option<&SourceAssetPlacement>{self.0.get(&graph)}
    pub fn entries(&self)->impl Iterator<Item=&SourceAssetPlacement>{self.0.values()}
    /// Admission against exact candidate graph identities. This neither marks
    /// a graph as a scene nor infers a replacement from its name or path.
    /// A new Scene factory separately requires NarrativeMetadata::SceneEntry.
    pub fn validate_graphs(&self,graphs:&[GraphDocument])->Result<(),String>{
        let ids=graphs.iter().map(|graph|graph.graph_id).collect::<BTreeSet<_>>();
        if ids.len()!=graphs.len(){return Err("Source asset placement candidate repeats a graph identity".into());}
        if self.0.keys().any(|id|!ids.contains(id)){return Err("Source asset placement references a graph outside the exact candidate".into());}
        Ok(())
    }
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire { version:u32,entries:Vec<SourceAssetPlacement> }
impl Serialize for SourceAssetPlacements {
    fn serialize<S:Serializer>(&self,serializer:S)->Result<S::Ok,S::Error>{
        Wire{version:1,entries:self.entries().cloned().collect()}.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for SourceAssetPlacements {
    fn deserialize<D:Deserializer<'de>>(deserializer:D)->Result<Self,D::Error>{
        let wire=Wire::deserialize(deserializer)?;
        if wire.version!=1{return Err(serde::de::Error::custom("Unsupported Source asset placement metadata version"));}
        Self::from_entries(wire.entries).map_err(serde::de::Error::custom)
    }
}
#[cfg(test)]
#[path="source_asset_placement_tests1023.rs"]
mod tests;
