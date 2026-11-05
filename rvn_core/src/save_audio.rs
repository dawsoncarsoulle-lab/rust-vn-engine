//! Fresh captured-state writers. The existing raw-state APIs keep their guard.
use super::*;
use crate::audio_checkpoint::PortableGameSnapshot;

impl SaveData {
    pub fn try_from_snapshot(snapshot:&PortableGameSnapshot,slot:u32,label:String,script_name:String)->Result<Self,SaveError> {
        let portable=crate::scene_resources::portable_story_state_captured_audio(snapshot.state()).map_err(SaveError::Incompatible)?;
        Self::checked_portable_data(&portable,slot,label,script_name)
    }
}

impl SaveManager {
    pub fn save_snapshot(&self,snapshot:&PortableGameSnapshot,slot:u32,label:String,script_name:String)->Result<(),SaveError> {
        self.check_slot(slot)?;self.check_writable(slot)?;
        self.write_captured(&self.slot_path(slot),snapshot,slot,label,script_name)
    }
    pub fn save_autosave_snapshot(&self,snapshot:&PortableGameSnapshot,label:String,script_name:String)->Result<(),SaveError> {
        self.write_captured(&self.autosave_path(),snapshot,0,label,script_name)
    }
    pub fn save_quicksave_snapshot(&self,snapshot:&PortableGameSnapshot,label:String,script_name:String)->Result<(),SaveError> {
        self.write_captured(&self.quicksave_path(),snapshot,0,label,script_name)
    }
    fn write_captured(&self,path:&Path,snapshot:&PortableGameSnapshot,slot:u32,label:String,script_name:String)->Result<(),SaveError> {
        // All portable continuation/reveal/source checks precede JSON and I/O.
        let data=SaveData::try_from_snapshot(snapshot,slot,label,script_name)?;
        let json=serde_json::to_string_pretty(&data)?;
        #[cfg(not(target_arch="wasm32"))]
        self.write_atomic(path,&json)?;
        #[cfg(target_arch="wasm32")]
        {
            let name=path.file_stem().and_then(|stem|stem.to_str()).ok_or_else(||SaveError::Incompatible("Invalid save storage key".into()))?;
            Self::local_storage()?.set_item(&self.storage_key(name),&json).map_err(|_|SaveError::Io(std::io::Error::new(std::io::ErrorKind::Other,"écriture localStorage impossible")))?;
        }
        Ok(())
    }
}
