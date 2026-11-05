//! Bounded observation transport; this never executes a client AST.
use std::{io::Read,path::Path};
use rvn_core::debugger_provenance::{Overlay,MAX_BYTES,SIDECAR};
pub(crate) fn read_overlay(root:&Path)->Result<Option<Overlay>,String>{
    let sidecar=root.join(SIDECAR);
    match std::fs::symlink_metadata(&sidecar){
        Ok(metadata)=>{
            if !metadata.is_file()||metadata.file_type().is_symlink()||metadata.len()>MAX_BYTES as u64{return Err("Invalid bounded compiler debugger sidecar".into());}
            let mut bytes=Vec::new();std::fs::File::open(&sidecar).map_err(|e|e.to_string())?.take((MAX_BYTES+1)as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
            if bytes.len()>MAX_BYTES{return Err("Compiler debugger sidecar grew beyond its bound".into());}
            Overlay::decode(&bytes).map(Some)
        },
        Err(error)if error.kind()==std::io::ErrorKind::NotFound=>Ok(None),
        Err(error)=>Err(error.to_string()),
    }
}
