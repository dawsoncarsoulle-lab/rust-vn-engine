//! Source-owned Init control. Saved loop data is not an execution capability.
use super::*;

impl std::fmt::Debug for StartupState {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        let mut value=f.debug_struct("StartupState");
        value.field("roots",&self.roots).field("root",&self.root).field("body",&self.body)
            .field("nested",&self.nested).field("exit_pc",&self.exit_pc).field("waiting_video",&self.waiting_video);
        if !self.loops.is_empty(){value.field("loops",&self.loops);}
        if !self.imagemaps.is_empty(){value.field("imagemaps",&self.imagemaps);}
        value.finish()
    }
}
impl std::fmt::Debug for StartupOperation {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        let mut value=f.debug_struct("StartupOperation");
        value.field("root",&self.root).field("body",&self.body).field("route",&self.route)
            .field("pc",&self.pc).field("instance",&self.instance).field("epoch",&self.epoch)
            .field("origin",&self.origin).field("video_feedback",&self.video_feedback);
        if !self.loop_visits.is_empty(){value.field("loop_visits",&self.loop_visits);}
        value.finish()
    }
}
impl StartupState {
    pub(super) fn loop_visits(&self)->Vec<StartupLoopVisit>{
        self.loops.iter().map(|frame|StartupLoopVisit{header:frame.header.clone(),visits:frame.visits}).collect()
    }
    pub(super) fn position_key(&self)->(usize,Vec<wire::Route>,Vec<StartupLoopVisit>){
        (self.root,self.route(),self.loop_visits())
    }
    pub(super) fn statement_at<'a>(&self,script:&'a[Statement],route:&[wire::Route])->Result<&'a Statement,String>{
        let root=self.current().ok_or("Init control lost its actual root")?;
        let Some((wire::Route::Statement(index),parent))=route.split_last()else{return Err("Init control route must end at a statement".into());};
        Self::body_at(script,root.pc,parent)?.get(*index).ok_or_else(||"Init control is outside its frozen source".into())
    }
    pub(super) fn control_route<'a>(&self,script:&'a[Statement],header:&[wire::Route])->Result<(Vec<wire::Route>,&'a Statement),String>{
        let mut route=header.to_vec();let mut node=self.statement_at(script,&route)?;
        while let Statement::ScheduledConsumer{body,..}=node {
            let [child]=body.as_slice()else{return Err("Init control requires a unique actual consumer body".into());};
            route.extend([wire::Route::Body,wire::Route::Statement(0)]);
            if route.len()>129{return Err("Init control exceeds the existing route budget".into());}
            node=child;
        }
        Ok((route,node))
    }
    pub(super) fn loop_body_route(&self,script:&[Statement],header:&[wire::Route])->Result<Vec<wire::Route>,String>{
        let(route,node)=self.control_route(script,header)?;
        if !matches!(node,Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}){
            return Err("Init continuation header is not an actual loop".into());
        }
        Ok(route)
    }
    fn validate_native_break_site(&self,script:&[Statement],frame:&PortableStartupLoop)->Result<(),String>{
        let Some(site)=&frame.native_break else{return Ok(());};
        if !matches!(frame.state,PortableStartupLoopState::DemandV1{cursor:crate::native_loop_cursor::DemandLoopCursor::RangeV1{..}})||site.visits!=frame.visits{
            return Err("Saved Native Break kind/visit differs from its actual range owner".into());
        }
        let owner=self.loop_body_route(script,&frame.header)?;
        let mut body=owner.clone();body.push(wire::Route::Body);
        let current=self.route();
        if !site.route.starts_with(&body)||!current.starts_with(&body){
            return Err("Saved Native Break is outside its still active actual body".into());
        }
        let(_,leaf)=self.control_route(script,&site.route)?;
        if !matches!(leaf,Statement::LoopBreak){return Err("Saved Native Break site is not a real source Break".into());}
        let mut nearest=None;
        for end in (1..site.route.len()).step_by(2){
            if matches!(site.route.get(end),Some(wire::Route::Body))&&matches!(self.statement_at(script,&site.route[..end])?,Statement::ForLoopWithBreak{..}){
                nearest=Some(site.route[..end].to_vec());
            }
        }
        if nearest.as_ref()!=Some(&owner){return Err("Saved Native Break borrowed another lexical range owner".into());}
        Ok(())
    }
    pub(super) fn enter_body(&mut self,script:&[Statement],header:&[wire::Route])->Result<(),String>{
        let(mut route,node)=self.control_route(script,header)?;
        let body=match node{Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..}=>body,_=>return Err("Init loop body lost its source kind".into())};
        if body.is_empty(){return self.set_route(header.to_vec());}
        route.extend([wire::Route::Body,wire::Route::Statement(0)]);self.set_route(route)
    }
    fn enter_sequence(&mut self,script:&[Statement],header:&[wire::Route])->Result<(),String>{
        let(mut route,node)=self.control_route(script,header)?;
        let Statement::Sequence{branches}=node else{return Err("Init Sequence lost its frozen source".into());};
        if let Some(index)=branches.iter().position(|branch|!branch.is_empty()){
            route.extend([wire::Route::Branch(index),wire::Route::Statement(0)]);self.set_route(route)
        }else{self.advance_from(script,header.to_vec())}
    }
    pub(super) fn restore_loop_frames(&mut self,portable:&PortableStartup,script:&[Statement])->Result<(),String>{
        crate::save::consumer_checkpoint::preflight_serialized(&portable.loops,crate::save::consumer_checkpoint::Save15::MAX_BYTES).map_err(|e|e.to_string())?;
        if portable.loops.len()>128{return Err("Saved Init loop stack exceeds the existing source route bound".into());}
        let current=self.route();let mut expected=Vec::new();
        for end in (1..current.len()).step_by(2){
            let prefix=&current[..end];
            if matches!(current.get(end),Some(wire::Route::Body))&&matches!(self.statement_at(script,prefix)?,Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}){
                expected.push(prefix.to_vec());
            }
        }
        let mut bodies=Vec::new();
        for frame in &portable.loops {
            self.validate_native_break_site(script,frame)?;
            if frame.visits==0||frame.visits>crate::eval::MAX_COMPUTATION_STEPS{return Err("Saved Init loop visit exceeds the existing computation budget".into());}
            let(body,node)=self.control_route(script,&frame.header)?;
            if bodies.contains(&body){return Err("Saved Init loop repeats an active source header".into());}
            for record in &frame.gather{record.validate_loop_data().map_err(|e|e.to_string())?;}
            let header=self.statement_at(script,&frame.header)?;
            if rvn_parser::scheduled_source::native_loop_contract::binding(header)?.is_some()!=matches!(frame.state,PortableStartupLoopState::DemandV1{..}){
                return Err("Saved Init iterator contract version differs from its actual source header".into());
            }
            let ports=crate::eval::debugger_eval::PortableRecord::loop_ports_unbound(header,&frame.gather).map_err(|e|e.to_string())?;
            match (&frame.state,node,ports){
                (PortableStartupLoopState::DemandV1{cursor},_,input)=>{
                    let binding=rvn_parser::scheduled_source::native_loop_contract::binding(header)?.ok_or("Saved Native cursor lost its source v3 owner")?;
                    cursor.validate_body().map_err(|e|e.to_string())?;
                    if cursor.visits()!=frame.visits || !matches!((&binding.kind,cursor),(rvn_parser::scheduled_source::native_loop_contract::Kind::Array,crate::native_loop_cursor::DemandLoopCursor::ArrayV1{..})|(rvn_parser::scheduled_source::native_loop_contract::Kind::Range,crate::native_loop_cursor::DemandLoopCursor::RangeV1{..})){
                        return Err("Saved Native cursor kind/visits differ from its actual source owner".into());
                    }
                    crate::eval::debugger_eval::PortableRecord::validate_native_loop_cursor(header,&frame.gather,cursor).map_err(|e|e.to_string())?;
                    // Validate that this retained Test actually entered this
                    // body, without drawing RNG or executing any producer.
                    let mut prior=cursor.clone();match &mut prior{crate::native_loop_cursor::DemandLoopCursor::ArrayV1{visits,..}|crate::native_loop_cursor::DemandLoopCursor::RangeV1{visits,..}=>*visits-=1};
                    if !prior.enter(&input).map_err(|e|e.to_string())?{return Err("Saved Native active body has no successful actual header input".into());}
                },
                (PortableStartupLoopState::While,Statement::While{..},_)=>{},
                (PortableStartupLoopState::ForEach{items,next},Statement::ForEach{..},Value::List(ports))=>{
                    let[Value::List(actual)]=ports.as_slice()else{return Err("Saved ForEach lost its retained collection tuple".into());};
                    if items!=actual||*next!=frame.visits||*next>items.len(){return Err("Saved ForEach collection/cursor differs from its actual retained inputs".into());}
                },
                (PortableStartupLoopState::ForRange{first,count,next},Statement::ForLoopWithBreak{..},Value::List(ports))=>{
                    let[a,b]=ports.as_slice()else{return Err("Saved range lost its retained bounds tuple".into());};
                    let(actual,n)=crate::flow_control::bounds(a,b).map_err(|e|e.to_string())?;
                    if *first!=actual||*count!=n as usize||*next!=frame.visits||*next>*count{return Err("Saved range bounds/cursor differ from its actual retained inputs".into());}
                },_=>return Err("Saved Init continuation kind differs from its actual header and retained inputs".into()),
            }
            bodies.push(body);
        }
        if portable.loops.last().is_some_and(|frame|frame.header==current){
            let body=bodies.last().unwrap();if expected.last()!=Some(body){expected.push(body.clone());}
        }
        if bodies!=expected{return Err("Saved Init loop stack differs from the actual nested source context".into());}
        self.loops=portable.loops.clone();Ok(())
    }
}
impl PortableStartup {
    pub(in crate::engine) fn remap_loop_origins(&mut self,program:&rvn_parser::ParsedProgram,export:bool)->Result<(),crate::save::SaveError>{
        for frame in &mut self.imagemaps{crate::save::consumer_checkpoint::remap_instruction_origin(&mut frame.header_origin,program,export)?;}
        for frame in &mut self.loops{
            crate::save::consumer_checkpoint::remap_instruction_origin(&mut frame.header_origin,program,export)?;
            if let Some(site)=&mut frame.native_break{crate::save::consumer_checkpoint::remap_instruction_origin(&mut site.origin,program,export)?;}
        }Ok(())
    }
    pub(in crate::engine) fn remap_loop_namespaces(&mut self,map:&std::collections::BTreeMap<String,String>)->Result<(),String>{
        for frame in &mut self.imagemaps{for record in &mut frame.gather{record.remap_loop_namespaces(map).map_err(|e|e.to_string())?;}}
        for frame in &mut self.loops{for record in &mut frame.gather{record.remap_loop_namespaces(map).map_err(|e|e.to_string())?;}}
        Ok(())
    }
}
impl<R:Renderer> Engine<R>{
    pub(in crate::engine) fn is_owned_startup_control(statement:&Statement)->bool{
        // v3 is an explicit loop contract, whereas consumer_leaf deliberately
        // retains its historical v2 command classification. Route v3 to the
        // real Startup kernel BEFORE the generic action-input gather. This
        // classifier grants no owner: operation/source/plan checks still run,
        // and only that kernel supplies the reached First/Test environment.
        if matches!(statement,Statement::ScheduledConsumer{version:3,..}){
            return rvn_parser::scheduled_source::native_loop_contract::binding(statement).is_ok_and(|binding|binding.is_some());
        }
        matches!(Self::consumer_leaf(Some(statement)),Some(Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}|Statement::Sequence{..}|Statement::LoopBreak))
    }
    pub(super) fn startup_origin_for_route(&self,route:&[wire::Route])->Result<crate::program::InstructionOrigin,RuntimeError>{
        let state=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Init source route has no actual owner".into()))?;
        let root=state.current().ok_or_else(||self.scene_resource_error("Init source route has no current root".into()))?;
        let map=self.functions.debug_source_map.as_ref().ok_or_else(||self.scene_resource_error("Init source map absent".into()))?;
        let declaration=map.origins.get(root.pc).ok_or_else(||self.scene_resource_error("Init declaration origin absent".into()))?;
        map.sites.iter().find(|site|site.scope.is_empty()&&site.route==route&&site.origin.scheduled_step.is_none()
            &&site.declaration.source==declaration.source&&site.declaration.span==declaration.span)
            .map(|site|site.origin.clone()).ok_or_else(||self.scene_resource_error("Init control has no genuine source range".into()))
    }
    pub(in crate::engine) fn validate_startup_loop_gathers(&self)->Result<(),RuntimeError>{
        let Some(startup)=&self.startup else{return Ok(());};
        for frame in &startup.loops{
            startup.validate_native_break_site(&self.script,frame).map_err(|e|self.scene_resource_error(e))?;
            if frame.header_origin!=self.startup_origin_for_route(&frame.header)?{return Err(self.scene_resource_error("Saved Init loop header provenance differs from the frozen source".into()));}
            if let Some(site)=&frame.native_break{
                if site.origin!=self.startup_origin_for_route(&site.route)?{return Err(self.scene_resource_error("Saved Native Break provenance differs from its frozen source site".into()));}
            }
            let statement=startup.statement_at(&self.script,&frame.header).map_err(|e|self.scene_resource_error(e))?;
            let scope=if let Statement::ScheduledConsumer{scope,..}=statement{
                self.consumer_program.validate_plan_binding(statement,startup.current().unwrap().pc,true).map_err(|e|self.eval_err(e,"Saved Init loop plan"))?;
                Some(self.consumer_program.bound_scope(scope,startup.current().unwrap().pc).map_err(|e|self.eval_err(e,"Saved Init loop scope"))?)
            }else{None};
            let ports=self.functions.validate_retained_loop_inputs(statement,scope.as_deref(),&frame.header,&frame.gather,self.locale.as_ref())
                .map_err(|e|self.eval_err(e,"Saved Init loop inputs"))?;
            if let PortableStartupLoopState::DemandV1{cursor}=&frame.state{
                crate::eval::debugger_eval::PortableRecord::validate_native_loop_cursor(statement,&frame.gather,cursor).map_err(|e|self.eval_err(e,"Saved Native Init loop phase"))?;
                if cursor.visits()!=frame.visits{return Err(self.scene_resource_error("Saved Native Init loop visit stamp differs".into()));}
            }
            if matches!(frame.state,PortableStartupLoopState::While){let mut work=0;if !crate::eval::truthy_bounded(&ports,Some(self.functions.text_resources()),&mut work).map_err(|e|self.eval_err(e,"Saved Init While"))?{return Err(self.scene_resource_error("Saved active While has no successful retained header".into()));}}
        }
        Ok(())
    }
    pub(in crate::engine) fn rebind_startup_loop_gathers(&mut self,program:&rvn_parser::ParsedProgram)->Result<(),crate::save::SaveError>{
        // Both continuation families use the same actual frozen programme.
        // A map-only wait must not bypass source/gather hydration because it
        // has no enclosing loop frame.
        self.rebind_startup_imagemap_gathers(program)?;
        if self.startup.as_ref().is_none_or(|startup|startup.loops.is_empty()){return Ok(());}
        let(_,map)=crate::save::consumer_checkpoint::portable_namespaces(program,&self.script,&self.consumer_program)?;
        let inverse=map.into_iter().map(|(live,logical)|(logical,live)).collect();
        for frame in &mut self.startup.as_mut().unwrap().loops{
            crate::save::consumer_checkpoint::remap_instruction_origin(&mut frame.header_origin,program,false)?;
            if let Some(site)=&mut frame.native_break{crate::save::consumer_checkpoint::remap_instruction_origin(&mut site.origin,program,false)?;}
            for record in &mut frame.gather{
            record.remap_loop_namespaces(&inverse).map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))?;
        }}
        self.validate_startup_loop_gathers().map_err(|e|crate::save::SaveError::Incompatible(e.to_string()))
    }
    pub(in crate::engine) fn exec_startup_control(&mut self,statement:Statement)->Result<(),RuntimeError>{
        let header=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Init control requires its real Startup owner".into()))?.route();
        let(_,leaf)=self.startup.as_ref().unwrap().control_route(&self.script,&header).map_err(|e|self.scene_resource_error(e))?;
        let leaf=leaf.clone();
        if matches!(leaf,Statement::LoopBreak){
            let owner=self.startup.as_ref().unwrap().loops.iter().rposition(|frame|matches!(frame.state,PortableStartupLoopState::ForRange{..}|PortableStartupLoopState::DemandV1{cursor:crate::native_loop_cursor::DemandLoopCursor::RangeV1{..}}))
                .ok_or_else(||self.scene_resource_error("Init break has no active actual range owner".into()))?;
            let target=self.startup.as_ref().unwrap().loops[owner].header.clone();
            if matches!(self.startup.as_ref().unwrap().loops[owner].state,PortableStartupLoopState::DemandV1{..}){
                let origin=self.startup_origin_for_route(&header)?;
                let startup=self.startup.as_mut().unwrap();
                let frame=&mut startup.loops[owner];
                if frame.native_break.is_none(){frame.native_break=Some(NativeStartupBreak{route:header.clone(),origin,visits:frame.visits});}
                // Break is a macro input call. Its already engaged Sequence
                // and inner continuations return normally; only the owner's
                // NEXT comparison is suppressed when its body actually ends.
                startup.advance_from(&self.script,header).map_err(|e|self.scene_resource_error(e))?;
                return Ok(());
            }
            let startup=self.startup.as_mut().unwrap();startup.loops.truncate(owner);
            startup.advance_from(&self.script,target).map_err(|e|self.scene_resource_error(e))?;return Ok(());
        }
        if matches!(statement,Statement::ScheduledConsumer{version:3,..}){
            return self.exec_startup_native_loop(statement,header);
        }
        if self.startup.as_ref().unwrap().loops.last().is_some_and(|frame|frame.header==header&&!matches!(frame.state,PortableStartupLoopState::While)){
            return self.resume_startup_iteration(&header);
        }
        let values=if matches!(statement,Statement::ScheduledConsumer{..}){
            let(next,value)=self.gather_scheduled_action_inputs(&statement)?;self.state=next;value
        }else{
            let environment=self.vars_for_eval();let globals=self.lexical_vars_for_eval();
            let expressions:Vec<_>=match &leaf{
                Statement::While{condition,..}=>vec![condition],Statement::ForEach{collection,..}=>vec![collection],
                Statement::ForLoopWithBreak{first,last,..}=>vec![first,last],Statement::Sequence{..}=>vec![],_=>unreachable!()};
            let mut values=Vec::new();for expression in expressions{
                values.push(self.functions.eval_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), expression,&environment,&globals,&mut self.state.random,self.locale.as_ref()).map_err(|e|self.eval_err(e,"Init loop input"))?);
            }
            if matches!(leaf,Statement::While{..}){values.pop().unwrap()}else{Value::List(values)}
        };
        if matches!(leaf,Statement::Sequence{..}){
            self.startup.as_mut().unwrap().enter_sequence(&self.script,&header).map_err(|e|self.scene_resource_error(e))?;return Ok(());
        }
        let probe=self.functions.debug_probe.as_ref().ok_or_else(||self.scene_resource_error("Init header lost its real computation trace".into()))?;
        let gather=Probe::completed_loop_records(probe,&header).map_err(|e|self.eval_err(e,"Init header retained inputs"))?;
        let origin=self.startup_origin_for_route(&header)?;
        let state=match (&leaf,values){
            (Statement::While{..},value)=>{
                let mut work=0;let entered=crate::eval::truthy_bounded(&value,Some(self.functions.text_resources()),&mut work).map_err(|e|self.eval_err(e,"Init While"))?;
                if !entered{
                    if self.startup.as_ref().unwrap().loops.last().is_some_and(|frame|frame.header==header){self.startup.as_mut().unwrap().loops.pop();}
                    self.startup.as_mut().unwrap().advance_from(&self.script,header).map_err(|e|self.scene_resource_error(e))?;return Ok(());
                }
                PortableStartupLoopState::While
            },
            (Statement::ForEach{..},Value::List(mut ports))if ports.len()==1=>{
                let Value::List(items)=ports.remove(0)else{return Err(self.scene_resource_error("Init ForEach requires its real list input".into()));};
                crate::value_limits::value_work(&Value::List(items.clone())).map_err(|e|self.eval_err(e,"Init ForEach collection"))?;
                PortableStartupLoopState::ForEach{items,next:0}
            },
            (Statement::ForLoopWithBreak{..},Value::List(ports))if ports.len()==2=>{
                let(first,count)=crate::flow_control::bounds(&ports[0],&ports[1]).map_err(|e|self.eval_err(e,"Init range bounds"))?;
                PortableStartupLoopState::ForRange{first,count:count as usize,next:0}
            },_=>return Err(self.scene_resource_error("Init loop lost its actual typed input ports".into())),
        };
        let visits=if matches!(state,PortableStartupLoopState::While)&&self.startup.as_ref().unwrap().loops.last().is_some_and(|frame|frame.header==header){
            let old=self.startup.as_mut().unwrap().loops.pop().unwrap();old.visits.checked_add(1).ok_or_else(||self.scene_resource_error("Init loop visit overflow".into()))?
        }else{0};
        if visits>crate::eval::MAX_COMPUTATION_STEPS{return Err(self.scene_resource_error("Init loop exceeds the existing computation budget".into()));}
        self.startup.as_mut().unwrap().loops.push(PortableStartupLoop{header:header.clone(),header_origin:origin,gather,visits,state,native_break:None});
        crate::save::consumer_checkpoint::preflight_serialized(&self.startup.as_ref().unwrap().loops,crate::save::consumer_checkpoint::Save15::MAX_BYTES).map_err(|e|self.scene_resource_error(e.to_string()))?;
        if matches!(leaf,Statement::While{..}){
            if visits==0{self.startup.as_mut().unwrap().loops.last_mut().unwrap().visits=1;}
            self.startup.as_mut().unwrap().enter_body(&self.script,&header).map_err(|e|self.scene_resource_error(e))
        }else{self.resume_startup_iteration(&header)}
    }
    fn resume_startup_iteration(&mut self,header:&[wire::Route])->Result<(),RuntimeError>{
        let(_,node)=self.startup.as_ref().unwrap().control_route(&self.script,header).map_err(|e|self.scene_resource_error(e))?;
        let name=match node{Statement::ForEach{name,..}|Statement::ForLoopWithBreak{name,..}=>name.clone(),_=>return Err(self.scene_resource_error("Init iteration lost its actual variable binding".into()))};
        let frame=self.startup.as_mut().unwrap().loops.last_mut().unwrap();
        let next=match &mut frame.state{
            PortableStartupLoopState::ForEach{items,next}=>{
                let value=items.get(*next).cloned();if value.is_some(){*next+=1;}value
            },
            PortableStartupLoopState::ForRange{first,count,next}=>{
                if *next>=*count{None}else{let value=first.checked_add(*next as i64).ok_or_else(||RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::NumericOverflow),self.state.pc))?;*next+=1;Some(Value::Int(value))}
            },_=>return Err(RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::InvalidFunction("Init iterator has a non-iterator frame".into())),self.state.pc)),
        };
        if let Some(value)=next{
            frame.visits=frame.visits.checked_add(1).ok_or_else(||RuntimeError::no_stmt(crate::error::RuntimeErrorKind::EvalError(EvalError::NumericOverflow),self.state.pc))?;
            self.state.vars.insert(name,value);
            self.startup.as_mut().unwrap().enter_body(&self.script,header).map_err(|e|self.scene_resource_error(e))
        }else{
            self.startup.as_mut().unwrap().loops.pop();
            self.startup.as_mut().unwrap().advance_from(&self.script,header.to_vec()).map_err(|e|self.scene_resource_error(e))
        }
    }
}
