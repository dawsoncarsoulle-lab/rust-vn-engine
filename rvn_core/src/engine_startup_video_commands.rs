//! Bounded observations of actions accepted by one trusted startup candidate.
//! These are neither executable commands nor portable/client admission data.
use super::*;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum StartupVideoCommandKind{Play,Pause,Resume,Stop,Skip,Seek,Volume}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct StartupVideoCommand{kind:StartupVideoCommandKind,name:String}
impl StartupVideoCommand{
    pub fn name(&self)->&str{&self.name}
    pub fn kind(&self)->StartupVideoCommandKind{self.kind}
}
#[derive(Default)]
pub(in crate::engine) struct StartupVideoTrace{commands:Vec<StartupVideoCommand>,bytes:usize}
impl StartupVideoTrace{
    pub(in crate::engine) fn record(&mut self,command:&crate::eval::UiCommand)->Result<(),String>{
        use crate::eval::UiCommand;use StartupVideoCommandKind as K;
        let(kind,name)=match command{
            UiCommand::VideoPlay{name,..}=>(K::Play,name),UiCommand::VideoPause{name}=>(K::Pause,name),
            UiCommand::VideoResume{name}=>(K::Resume,name),UiCommand::VideoStop{name}=>(K::Stop,name),
            UiCommand::VideoSkip{name}=>(K::Skip,name),UiCommand::VideoSeek{name,..}=>(K::Seek,name),
            UiCommand::VideoVolume{name,..}=>(K::Volume,name),_=>return Ok(()),
        };
        let bytes=self.bytes.checked_add(name.len()).and_then(|bytes|bytes.checked_add(32));
        if self.commands.len()>=128||bytes.is_none_or(|bytes|bytes>64*1024){return Err("Startup accepted video actions exceed 128 entries or 64 KiB".into());}
        // Size admission precedes the only payload clone. Never truncate.
        self.commands.push(StartupVideoCommand{kind,name:name.clone()});self.bytes=bytes.unwrap();Ok(())
    }
    pub(in crate::engine) fn snapshot(&self)->Result<Vec<StartupVideoCommand>,String>{
        if self.commands.len()>128||self.bytes>64*1024{return Err("Startup video action observation exceeds its budget".into());}
        Ok(self.commands.clone())
    }
}
impl<R:Renderer> Engine<R>{
    pub(in crate::engine) fn record_startup_video_command(&self,command:&crate::eval::UiCommand)->Result<(),RuntimeError>{
        if let Some(trace)=&self.startup_video_trace{
            trace.lock().map_err(|_|self.scene_resource_error("Startup video action observation unavailable".into()))?.record(command).map_err(|error|self.scene_resource_error(error))?;
        }Ok(())
    }
}
