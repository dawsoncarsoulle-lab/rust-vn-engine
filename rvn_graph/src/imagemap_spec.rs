//! One typed codec for the existing Imagemap hotspot property. Legacy colon
//! rows remain unchanged; extended rows preserve absent/empty/colon names.
use serde::{Serialize,Deserialize};
#[derive(Clone,Debug,PartialEq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HotspotSpec {
    pub name:Option<String>,pub area:[i32;4],pub hover_area:Option<[i32;4]>,
}
const PREFIX:&str="@rvn-hotspot-v1 ";
impl HotspotSpec {
    pub fn validate(&self)->Result<(),String>{
        for area in std::iter::once(&self.area).chain(self.hover_area.as_ref()){
            if area[2].checked_sub(area[0]).is_none_or(|width|width<=0)||area[3].checked_sub(area[1]).is_none_or(|height|height<=0){return Err("Invalid or overflowing hotspot rectangle".into());}
        }
        Ok(())
    }
    pub fn from_hotspot(h:&rvn_parser::Hotspot)->Result<Self,String>{
        if h.name.as_ref().is_some_and(|name|name.len()>16*1024*1024){return Err("Hotspot name exceeds16MiB".into());}
        let rect=|r:&rvn_parser::Rect|[r.x1,r.y1,r.x2,r.y2];
        let spec=Self{name:h.name.clone(),area:rect(&h.area),hover_area:h.hover_area.as_ref().map(rect)};spec.validate()?;Ok(spec)
    }
    pub fn encode(&self)->Result<String,String>{
        self.validate()?;
        if let Some(name)=self.name.as_ref().filter(|name|!name.contains(':')&&name.trim()==name.as_str()){
            let mut suffix=format!(":{}:{}:{}:{}",self.area[0],self.area[1],self.area[2],self.area[3]);
            if let Some(area)=self.hover_area{suffix.push_str(&format!(":{}:{}:{}:{}",area[0],area[1],area[2],area[3]));}
            let size=name.len().checked_add(suffix.len()).filter(|size|*size<=16*1024*1024).ok_or("Hotspot serialization exceeds16MiB")?;
            let mut row=String::with_capacity(size);row.push_str(name);row.push_str(&suffix);
            return Ok(row);
        }
        struct Bounded(Vec<u8>);
        impl std::io::Write for Bounded{
            fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{if self.0.len().checked_add(bytes.len()).is_none_or(|n|n>16*1024*1024){return Err(std::io::Error::other("Hotspot serialization exceeds16MiB"));}self.0.extend_from_slice(bytes);Ok(bytes.len())}
            fn flush(&mut self)->std::io::Result<()>{Ok(())}
        }
        let mut output=Bounded(PREFIX.as_bytes().to_vec());serde_json::to_writer(&mut output,self).map_err(|e|e.to_string())?;
        String::from_utf8(output.0).map_err(|e|e.to_string())
    }
    pub fn parse(raw:&str)->Result<Self,String>{
        // Same bound as generated native source, checked before JSON allocation.
        if raw.len()>16*1024*1024{return Err("Hotspot specification exceeds16MiB".into());}
        let result=if raw.starts_with(PREFIX)&&raw.ends_with('}'){
            serde_json::from_str(raw.strip_prefix(PREFIX).unwrap()).map_err(|e|e.to_string())?
        }else{
            let parts=raw.split(':').take(10).map(str::trim).collect::<Vec<_>>();
            if parts.len()!=5&&parts.len()!=9{return Err("Hotspot needs4 or8 coordinates".into());}
            let coords=parts[1..].iter().map(|part|part.parse::<i32>().map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
            Self{name:Some(parts[0].into()),area:[coords[0],coords[1],coords[2],coords[3]],hover_area:(coords.len()==8).then(||[coords[4],coords[5],coords[6],coords[7]])}
        };
        result.validate()?;Ok(result)
    }
}
pub(crate) fn label(raw:&str)->String{
    HotspotSpec::parse(raw).map(|spec|spec.name.unwrap_or_default()).unwrap_or_else(|_|raw.split(':').next().unwrap_or(raw).trim().into())
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn old_rows_are_byte_exact_and_optional_names_remain_distinct(){
        let old="path:0:0:100:200:1:2:101:202";assert_eq!(HotspotSpec::parse(old).unwrap().encode().unwrap(),old);
        for name in [None,Some(String::new()),Some(" name: exact😀 ".into())]{let spec=HotspotSpec{name,area:[0,0,100,200],hover_area:None};assert_eq!(HotspotSpec::parse(&spec.encode().unwrap()).unwrap(),spec);}
    }
    #[test]fn extended_specs_refuse_body_unknown_fields_and_invalid_rectangle(){
        assert!(HotspotSpec::parse("@rvn-hotspot-v1 {\"name\":null,\"area\":[0,0,1,1],\"hover_area\":null,\"body\":[]}").is_err());
        assert!(HotspotSpec::parse("bad:0:0:0:20").is_err());assert!(HotspotSpec::parse("bad:-2147483648:0:2147483647:20").is_err());
    }
}
