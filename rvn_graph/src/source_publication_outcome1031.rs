//! Process-local evidence from the actual typed publisher. No model/native grant.
use super::*;
use std::io;

/// A refusal before the executor has verified its entire exact after-image.
/// Compensation may itself need recovery; an error never grants Saved state.
#[derive(Debug)]
pub struct CauseBeforeCommit1031(String);
impl std::fmt::Display for CauseBeforeCommit1031 {
    fn fmt(&self, out:&mut std::fmt::Formatter<'_>)->std::fmt::Result { out.write_str(&self.0) }
}
impl std::error::Error for CauseBeforeCommit1031 {}
impl From<String> for CauseBeforeCommit1031 { fn from(value:String)->Self { Self(value) } }

/// Cleanup diagnostics carry no allocated error string after the commit point.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct CleanupWarning1031 { kind:io::ErrorKind, os_code:Option<i32> }
impl CleanupWarning1031 {
    pub fn kind1031(&self)->io::ErrorKind { self.kind }
    pub fn os_code1031(&self)->Option<i32> { self.os_code }
    fn from_error(error:io::Error)->Self { Self { kind:error.kind(),os_code:error.raw_os_error() } }

}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum CleanupStatus1031 { Removed, Pending(CleanupWarning1031) }

// This allocation is prepared BEFORE journal/target IO. It is not a receipt.
// Exact raw bytes bind the result to this transaction, not a later disk image.
#[derive(Debug)]
pub(in crate::source_workspace::refactor) struct PreparedCommit1031 { root:PathBuf, source:String, journal:String }
impl PreparedCommit1031 {
    pub(in crate::source_workspace::refactor) fn prepare(root:&Path, source:&str, journal:String)->Self {
        Self { root:root.to_path_buf(),source:source.into(),journal }
    }
    pub(super) fn check_original(&self, root:&Path)->Result<(),String> {
        if self.root!=root || read_optional(&root.join(REFACTOR_JOURNAL))?.as_deref()!=Some(self.journal.as_str()) {
            return Err("The exact original typed journal changed before verified publication".into())
        }
        Ok(())
    }
    pub(super) fn wire(&self)->&str { &self.journal }
}

/// Unique evidence of one actual physically verified typed publication.
/// No Clone/Default/Deserialize/public fields or public constructor exists.
/// It authorizes neither a subsequent write nor model/native completion.
#[derive(Debug)]
pub struct VerifiedCommitted1031 { original:Box<PreparedCommit1031>,cleanup:CleanupStatus1031 }
impl VerifiedCommitted1031 {
    pub fn source1031(&self)->&str { &self.original.source }
    pub fn root1031(&self)->&Path { &self.original.root }
    pub fn original_journal1031(&self)->&str { &self.original.journal }
    pub fn cleanup1031(&self)->CleanupStatus1031 { self.cleanup }
}

// The ONLY mint. Its sole production call is after the executor's final exact
// reads (files, recovery and original journal) have ALL succeeded.
pub(super) fn finish_verified1031(root:&Path, original:Box<PreparedCommit1031>,execution:&Execution1031)->VerifiedCommitted1031 {
    execution.minted();
    let cleanup=match execution.remove_journal(root) {
        Ok(())=>CleanupStatus1031::Removed,
        Err(error)=>CleanupStatus1031::Pending(CleanupWarning1031::from_error(error)),
    };
    VerifiedCommitted1031 { original,cleanup }
}

/// No callback, model installer, current-window shortcut, or native admission.
impl SourceWorkspace {
    pub fn discard_library_recovery_verified1031(&mut self,expected:&SourceRecoveryCohortLibrary1027)->Result<VerifiedCommitted1031,CauseBeforeCommit1031> {
        self.discard_library_verified_inner1031(expected,&Execution1031::default()).map_err(Into::into)
    }
    /// Explicit maintenance of a fully committed typed journal only. Legacy
    /// cold open keeps its existing refusal if journal cleanup still fails.
    /// This never installs a model or turns arbitrary before/half state Saved.
    pub fn cleanup_typed_journal_verified1031(root:&Path, original:&VerifiedCommitted1031)->Result<VerifiedCommitted1031,CauseBeforeCommit1031> {
        let root=fs::canonicalize(root).map_err(|e|CauseBeforeCommit1031::from(e.to_string()))?;
        if root!=original.root1031(){return Err(CauseBeforeCommit1031::from("Cleanup proof belongs to another original Source root".to_owned()));}
        let _lock=lock(&root).map_err(CauseBeforeCommit1031::from)?;
        super::super::cleanup_typed_journal1031(&root,original,&Execution1031::default()).map_err(Into::into)
    }
}

// A per-operation test seam. Production is a zero-sized silent observer; no
// environment/global failpoints, user callbacks or accepted-state injections.
#[derive(Default)]
pub(in crate::source_workspace::refactor) struct Execution1031 {
    #[cfg(test)]
    probe:Option<std::rc::Rc<std::cell::RefCell<Probe1031>>>,
}
impl Execution1031 {
    pub(super) fn before_journal(&self)->Result<(),String> {
        #[cfg(test)] { let mut probe=self.probe.as_ref().map(|p|p.borrow_mut());if let Some(p)=probe.as_mut(){p.prepared+=1;if p.fail==Fault1031::BeforeJournal{return Err("injected original journal refusal".into())}} }
        Ok(())
    }
    pub(super) fn backup(&self) { #[cfg(test)] if let Some(p)=&self.probe {p.borrow_mut().backups+=1;} }
    pub(super) fn target(&self)->Result<(),String> {
        #[cfg(test)] if let Some(p)=&self.probe {let mut p=p.borrow_mut();let index=p.target_writes;if p.fail==Fault1031::Target(index){return Err("injected target write refusal".into())}p.target_writes+=1;}
        Ok(())
    }
    pub(super) fn recovery(&self)->Result<(),String> {
        #[cfg(test)] if let Some(p)=&self.probe {let mut p=p.borrow_mut();if p.fail==Fault1031::Recovery{return Err("injected recovery write refusal".into())}p.recovery_writes+=1;}
        Ok(())
    }
    pub(super) fn before_verify(&self)->Result<(),String> {
        #[cfg(test)] if let Some(p)=&self.probe {if p.borrow().fail==Fault1031::Readback{return Err("injected final readback refusal".into())}}
        Ok(())
    }
    pub(super) fn minted(&self) { #[cfg(test)] if let Some(p)=&self.probe {p.borrow_mut().mints+=1;} }
    pub(super) fn remove_journal(&self,root:&Path)->io::Result<()> {
        #[cfg(test)] if let Some(p)=&self.probe {let mut p=p.borrow_mut();p.cleanup_attempts+=1;if p.fail==Fault1031::Cleanup{return Err(io::Error::from(io::ErrorKind::PermissionDenied))}
}
        fs::remove_file(root.join(REFACTOR_JOURNAL))
    }
}
#[cfg(test)]
#[derive(Default,Clone,Copy,PartialEq,Eq)]
pub(super) enum Fault1031 { #[default] None,BeforeJournal,Target(usize),Recovery,Readback,Cleanup }
#[cfg(test)]
#[derive(Default)]
pub(super) struct Probe1031 { fail:Fault1031,prepared:usize,backups:usize,target_writes:usize,recovery_writes:usize,mints:usize,cleanup_attempts:usize }

#[cfg(test)]
pub(super) fn injected1031(fail:Fault1031)->(Execution1031,std::rc::Rc<std::cell::RefCell<Probe1031>>){
    let probe=std::rc::Rc::new(std::cell::RefCell::new(Probe1031{fail,..Default::default()}));
    (Execution1031{probe:Some(probe.clone())},probe)
}
#[cfg(test)]
impl Probe1031 {
    pub(super) fn stats1031(&self)->(usize,usize,usize,usize,usize,usize){
        (self.prepared,self.backups,self.target_writes,self.recovery_writes,self.mints,self.cleanup_attempts)
    }
}

#[cfg(test)]
#[path="source_publication_outcome_tests1031.rs"]
mod tests;
