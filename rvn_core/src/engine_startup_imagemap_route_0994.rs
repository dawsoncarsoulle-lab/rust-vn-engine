//! Frozen-source routing prerequisite for Startup Imagemap. This issues no
//! input capability and does not advance StartupState or admit an Init command.
//! The owner/backend stage must bind a real installed input receipt before it
//! can use the planned branch; Save15 wait admission remains unchanged here.
use crate::{debugger_provenance::SourceMap, editor_debugger::{self as wire, Route}, program::InstructionOrigin};
use rvn_parser::{ParsedProgram, Statement};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HotspotCursor {
    definition: usize,
    parent: Vec<Route>,
    index: usize,
    source_closure: [u8;32],
    source_content: [u8;32],
    emitted: [u8;32],
    parent_origin: InstructionOrigin,
    continuation: Option<(Vec<Route>, InstructionOrigin)>,
}
impl HotspotCursor {
    pub(super) fn parent_origin(&self) -> &InstructionOrigin { &self.parent_origin }
    pub(super) fn continuation(&self) -> Option<(&[Route], &InstructionOrigin)> {
        self.continuation.as_ref().map(|(route, origin)| (route.as_slice(), origin))
    }
    /// The real selection returns to this existing parent after its branch.
    /// An empty branch has no fabricated statement/PC or continuation frame.
    pub(super) fn return_route(&self) -> &[Route] { &self.parent }
    pub(super) fn validate_for(&self, program:&ParsedProgram, map:&SourceMap) -> Result<(),String> {
        let actual=prepare(program,map,self.definition,&self.parent,self.index)?;
        if *self!=actual { return Err("Startup hotspot cursor differs from its frozen source".into()); }
        Ok(())
    }
}

// This read-only resolver may walk a frozen hotspot branch. It deliberately
// does not extend StartupState::body_at/restore: saved input selection needs
// its future actual waiting-owner witness before accepting a new cursor.
fn statement_at<'a>(root:&'a Statement, route:&[Route]) -> Result<&'a Statement,String> {
    if route.is_empty() || route.len()>255 || route.len()%2==0 { return Err("Startup hotspot source route shape/budget".into()); }
    let Statement::Init{body}=root else { return Err("Startup hotspot has no genuine Init declaration".into()); };
    let mut body=body.as_slice();let mut parts=route;
    loop {
        let Some((Route::Statement(index),tail))=parts.split_first() else { return Err("Startup hotspot route requires an actual statement index".into()); };
        let node=body.get(*index).ok_or("Startup hotspot statement lies outside its frozen body")?;
        if tail.is_empty() { return Ok(node); }
        let (part,next)=tail.split_first().ok_or("Startup hotspot child route is incomplete")?;
        body=match (node,part) {
            (Statement::If{then_branch,..},Route::Then)=>then_branch,
            (Statement::If{else_branch,..},Route::Else)=>else_branch,
            (Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..},Route::Body)=>body,
            (Statement::Sequence{branches},Route::Branch(index))=>branches.get(*index).ok_or("Startup source Sequence branch is absent")?,
            (Statement::Imagemap{hotspots,..},Route::Option(index))=>&hotspots.get(*index).ok_or("Startup source hotspot is absent")?.body,
            _=>return Err("Startup source route does not match its actual authored parent".into()),
        };
        parts=next;
    }
}
fn same_site(a:&wire::DebugOrigin,b:&wire::DebugOrigin)->bool {
    // A verified compiler overlay may add graph identities, never alter the
    // source, declaration, AST route or scheduled producer identities.
    a.scope==b.scope && a.route==b.route
        && a.origin.source==b.origin.source && a.origin.span==b.origin.span
        && a.origin.route==b.origin.route && a.origin.generated==b.origin.generated
        && a.origin.scheduled_step==b.origin.scheduled_step
        && a.declaration.source==b.declaration.source && a.declaration.span==b.declaration.span
        && a.declaration.route==b.declaration.route && a.declaration.generated==b.declaration.generated
        && a.declaration.scheduled_step==b.declaration.scheduled_step
}
fn origin(program:&ParsedProgram,map:&SourceMap,actual:&SourceMap,definition:usize,route:&[Route])->Result<InstructionOrigin,String> {
    let root=program.origins.get(definition).ok_or("Startup hotspot definition origin is absent")?;
    let matches=|site:&&wire::DebugOrigin|site.scope.is_empty() && site.route==route && site.origin.scheduled_step.is_none()
        && site.declaration.source==root.source && site.declaration.span.as_ref()==Some(&root.span.range)
        && site.declaration.route==[crate::program::AstRoutePart::Root{index:definition}];
    let mut expected=actual.sites.iter().filter(matches);
    let expected=expected.next().ok_or("Startup hotspot has no original source site")?;
    let mut supplied=map.sites.iter().filter(matches);
    let supplied=supplied.next().ok_or("Startup hotspot has no captured source site")?;
    if map.sites.iter().filter(matches).count()!=1 || actual.sites.iter().filter(matches).count()!=1 || !same_site(supplied,expected) {
        return Err("Startup hotspot captured source site is ambiguous or changed".into());
    }
    Ok(supplied.origin.clone())
}
/// Resolves only actual frozen syntax. `map` still comes from the existing
/// admitted Engine source table; this plan is not a Startup/input/Save grant.
pub(super) fn prepare(program:&ParsedProgram,map:&SourceMap,definition:usize,parent:&[Route],index:usize)->Result<HotspotCursor,String> {
    let root=program.script.get(definition).ok_or("Startup hotspot definition is absent")?;
    let actual=SourceMap::from_program(program)?;
    if (map.closure,map.content,map.emitted)!=(actual.closure,actual.content,actual.emitted) {
        return Err("Startup hotspot frozen programme identity changed".into());
    }
    let leaf=statement_at(root,parent)?;
    let parent_origin=origin(program,map,&actual,definition,parent)?;
    let (imagemap,mut route)=match leaf {
        Statement::ScheduledConsumer{version:2,body,..}=>{
            let [imagemap @ Statement::Imagemap{..}]=body.as_slice() else { return Err("Startup input route is not the actual v2 Imagemap consumer".into()); };
            let mut route=parent.to_vec();route.extend([Route::Body,Route::Statement(0)]);(imagemap,route)
        },
        _=>return Err("Startup input route is not an actual Native Imagemap consumer".into()),
    };
    let Statement::Imagemap{hotspots,..}=imagemap else { unreachable!() };
    let hotspot=hotspots.get(index).ok_or("Startup hotspot index lies outside its actual source")?;
    let continuation=if hotspot.body.is_empty() { None } else {
        route.extend([Route::Option(index),Route::Statement(0)]);
        let statement=statement_at(root,&route)?;
        if hotspot.body.first()!=Some(statement) || !wire::validate_site(root,&wire::Site{function:None,route:route.clone(),depth:0,statement:true}) {
            return Err("Startup hotspot child differs from its real authored route".into());
        }
        let origin=origin(program,map,&actual,definition,&route)?;Some((route,origin))
    };
    Ok(HotspotCursor{definition,parent:parent.to_vec(),index,source_closure:map.closure,source_content:map.content,emitted:map.emitted,parent_origin,continuation})
}
#[cfg(test)]
#[path="engine_startup_imagemap_route_tests_0994.rs"]
mod tests;
