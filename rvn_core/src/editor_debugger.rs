//! Disposable debugger observations. No value here is saved or an input token.
use rvn_parser::Value;
use serde::{Deserialize, Serialize};

pub const MAX_BREAKPOINTS: usize = 256;
pub const MAX_WATCHES: usize = 64;
pub const MAX_DEBUG_TEXT: usize = 8192;
pub const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;

#[path="debugger_sites.rs"]
mod sites;
pub use sites::{DebugOrigin, declaration_scope, declaration_scope_keys};
pub(crate) use sites::{collect_origins, validate_site, validate_program_origins};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind { Into, Over, Out, Continue }

/// An opaque command is decoded only after admitting its retained completed
/// image, authenticated Engine and existing control ordinal.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Command {
    Configure{breakpoints:Vec<Breakpoint>,watches:Vec<Watch>},
    Run(StepKind), Sites{offset:usize}, Disable,
}
pub const MAX_COMMAND_BYTES:usize=64*1024;
impl Command {
    pub fn encode(&self)->Result<String,String>{bounded_json(self,MAX_COMMAND_BYTES)}
    pub fn decode(value:&str)->Result<Self,String>{
        if value.len()>MAX_COMMAND_BYTES{return Err("Oversized debugger command".into());}
        let command:Self=serde_json::from_str(value).map_err(|e|e.to_string())?;
        if let Self::Configure{breakpoints,watches}=&command {
            if breakpoints.len()>MAX_BREAKPOINTS||watches.len()>MAX_WATCHES{return Err("Debugger command count budget".into());}
            if watches.iter().any(|watch|!rvn_parser::is_variable_binding_name(&watch.name)||watch.name.len()>4096){return Err("Invalid debugger watch".into());}
            if breakpoints.iter().any(|breakpoint|breakpoint.site.as_ref().is_some_and(|site|site.route.len()>256||site.depth>64)){return Err("Invalid debugger site budget".into());}
        }
        Ok(command)
    }
}

/// An actual evaluator path within the prepared instruction, not an invented PC.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    Statement(usize), Body, Then, Else, Branch(usize), Option(usize),
    Argument(usize), Left, Right, Unary, Index, Collection, Condition, First, Last,
    /// Actual step index, admitted only with original lexer/reparse-verified
    /// source provenance. This is a producer boundary, not a pure Expr site.
    Producer(usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    pub function: Option<String>,
    pub route: Vec<Route>,
    pub depth: usize,
    pub statement: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakpoint {
    pub position: crate::scene::ResumePosition,
    /// None means the emitted narrative instruction itself. A function site
    /// additionally identifies its lexical body; it does not retarget by label.
    pub site: Option<Site>,
    pub enabled: bool,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct SiteEntry {pub breakpoint:Breakpoint,pub origin:Option<crate::program::InstructionOrigin>}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct SitePage {pub offset:usize,pub total:usize,pub entries:Vec<SiteEntry>}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchScope { Globals, Frame(usize) }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watch { pub scope: WatchScope, pub name: String }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WatchValue {
    Value(Value), NotInScope, NoProperty, NoDebugObject, TooLarge,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservedWatch { pub watch: Watch, pub value: WatchValue }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LexicalFrame {
    pub function: Option<String>,
    pub depth: usize,
    pub site:Option<Site>,
    pub origin:Option<crate::program::InstructionOrigin>,
    pub binding_total:usize,
    pub bindings: Vec<(String, WatchValue)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub instance: u64,
    pub epoch: u64,
    pub position: crate::scene::ResumePosition,
    pub site: Option<Site>,
    /// Actual captured UTF-8 range and graph IDs, when supplied by preparation.
    pub origin: Option<crate::program::InstructionOrigin>,
    pub frames: Vec<LexicalFrame>,
    pub watches: Vec<ObservedWatch>,
    pub evaluating: bool,
    /// The owned command is being drained; Preview time remains paused while
    /// this continuation runs. An explicit Pause stops drains, not the visit.
    #[serde(default)]
    pub running: bool,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub armed: Option<StepKind>,
    /// Candidate observations have not been published to GameState/ECS yet.
    pub candidate: bool,
    pub steps: usize,
    pub value_work: usize,
    pub error: Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    pub sites:Option<SitePage>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    pub breakpoints:Vec<Breakpoint>,
}
impl Snapshot {
    pub fn encode(&self)->Result<String,String>{bounded_json(self,MAX_SNAPSHOT_BYTES)}
    pub fn decode(value:&str,instance:u64,epoch:u64)->Result<Self,String>{
        if value.len()>MAX_SNAPSHOT_BYTES{return Err("Oversized debugger observation".into());}
        let snapshot:Self=serde_json::from_str(value).map_err(|e|e.to_string())?;
        if snapshot.instance!=instance||snapshot.epoch!=epoch{return Err("Foreign debugger observation".into());}
        if snapshot.frames.len()>65||snapshot.watches.len()>MAX_WATCHES||snapshot.breakpoints.len()>MAX_BREAKPOINTS||snapshot.site.as_ref().is_some_and(|site|site.route.len()>256||site.depth>64)
            ||snapshot.frames.iter().any(|frame|frame.bindings.len()>64||frame.depth>64)
            ||snapshot.sites.as_ref().is_some_and(|page|page.entries.len()>64||page.offset>page.total||page.entries.len()>page.total.saturating_sub(page.offset)){
                return Err("Debugger observation shape budget".into());}
        Ok(snapshot)
    }
}
fn bounded_json(value:&impl Serialize,limit:usize)->Result<String,String>{
    struct Counter{bytes:usize,limit:usize}
    impl std::io::Write for Counter {
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
            self.bytes=self.bytes.checked_add(bytes.len()).filter(|n|*n<=self.limit).ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidData,"debugger envelope budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self)->std::io::Result<()>{Ok(())}
    }
    let mut counter=Counter{bytes:0,limit};serde_json::to_writer(&mut counter,value).map_err(|e|e.to_string())?;
    serde_json::to_string(value).map_err(|e|e.to_string())
}

pub(crate) fn bounded_value(value: &Value) -> WatchValue {
    let mut remaining=MAX_SNAPSHOT_BYTES;
    bounded_value_with_budget(value, &mut remaining)
}
pub(crate) fn bounded_value_with_budget(value:&Value, remaining:&mut usize)->WatchValue {
    // Serialize into a counter before cloning. A large watch must not allocate
    // its rejected representation, nor bypass the aggregate frame bound.
    struct Counter{bytes:usize,limit:usize}
    impl std::io::Write for Counter{
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{
            self.bytes=self.bytes.checked_add(bytes.len()).filter(|n|*n<=self.limit)
                .ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidData,"debug observation budget"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self)->std::io::Result<()>{Ok(())}
    }
    let mut counter=Counter{bytes:0,limit:MAX_DEBUG_TEXT.min(*remaining)};
    if serde_json::to_writer(&mut counter,value).is_err(){return WatchValue::TooLarge;}
    *remaining-=counter.bytes;WatchValue::Value(value.clone())
}
