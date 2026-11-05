//! Nested Init addresses are routes into the frozen authored AST, never PCs
//! manufactured by flattening a branch or executable statements from a DTO.
use super::*;
impl StartupState{
    pub(super) fn route(&self)->Vec<wire::Route>{let mut route=vec![wire::Route::Statement(self.body)];route.extend(self.nested.iter().cloned());route}
    pub(super) fn body_at<'a>(script:&'a[Statement],pc:usize,path:&[wire::Route])->Result<&'a[Statement],String>{
        let Some(Statement::Init{body})=script.get(pc)else{return Err("Startup route has no genuine Init root".into());};
        if path.len()>128||path.len()%2!=0{return Err("Startup child route budget/shape".into());}
        let mut body=body.as_slice();
        for pair in path.chunks_exact(2){
            let wire::Route::Statement(index)=pair[0]else{return Err("Startup child route requires a real statement index".into());};
            let node=body.get(index).ok_or("Startup child index is outside its frozen body")?;
            body=match(node,&pair[1]){
                (Statement::If{then_branch,..},wire::Route::Then)=>then_branch,
                (Statement::If{else_branch,..},wire::Route::Else)=>else_branch,
                (Statement::ScheduledConsumer{body,..}|Statement::While{body,..}|Statement::ForEach{body,..}|Statement::ForLoopWithBreak{body,..},wire::Route::Body)=>body,
                (Statement::Sequence{branches},wire::Route::Branch(index))=>branches.get(*index).ok_or("Startup Sequence branch is outside its frozen body")?,
                (Statement::Imagemap{hotspots,..},wire::Route::Option(index))=>&hotspots.get(*index).ok_or("Startup hotspot is outside its actual frozen input")?.body,
                _=>return Err("Startup child route differs from the frozen authored kind".into()),
            };
        }Ok(body)
    }
    pub(super) fn statement<'a>(&self,script:&'a[Statement])->Result<&'a Statement,String>{
        let root=self.current().ok_or("Startup child has no live root")?;let mut route=self.route();
        let Some(wire::Route::Statement(index))=route.pop()else{return Err("Startup route has no actual statement".into());};
        Self::body_at(script,root.pc,&route)?.get(index).ok_or_else(||"Startup statement is outside its frozen body".into())
    }
    pub(super) fn set_route(&mut self,route:Vec<wire::Route>)->Result<(),String>{
        if route.len()>129{return Err("Startup nested route budget".into());}
        let Some((wire::Route::Statement(index),tail))=route.split_first()else{return Err("Startup route has no real top-level body index".into());};
        if self.waiting_imagemap()&&self.imagemaps.last().is_some_and(|frame|frame.header!=route){return Err("A waiting Init input cannot move without its actual selection".into());}
        self.imagemaps.retain(|frame|frame.selected.is_none_or(|index|{let mut prefix=frame.header.clone();prefix.extend([wire::Route::Body,wire::Route::Statement(0),wire::Route::Option(index)]);route.starts_with(&prefix)}));
        self.body=*index;self.nested=tail.to_vec();Ok(())
    }
    pub(super) fn advance(&mut self,script:&[Statement])->Result<(),String>{self.advance_from(script,self.route())}
    pub(super) fn advance_from(&mut self,script:&[Statement],mut route:Vec<wire::Route>)->Result<(),String>{
        let pc=self.current().ok_or("Startup advance lost its genuine root")?.pc;
        loop{
            let Some(wire::Route::Statement(index))=route.pop()else{return Err("Startup advance route is not a statement".into());};
            let body=Self::body_at(script,pc,&route)?;
            if index.checked_add(1).is_some_and(|next|next<body.len()){
                route.push(wire::Route::Statement(index+1));return self.set_route(route);
            }
            if route.is_empty(){
                if !self.loops.is_empty(){return Err("Startup cannot leave an Init root with active loop continuations".into());}
                self.root+=1;self.body=0;self.nested.clear();self.imagemaps.clear();return self.skip_empty(script);
            }
            let part=route.pop().ok_or("Startup branch lost its actual parent")?;
            let parent=self.statement_at(script,&route)?;
            if matches!(&part,wire::Route::Body)&&matches!(parent,Statement::While{..}|Statement::ForEach{..}|Statement::ForLoopWithBreak{..}){
                let owner=self.loops.iter().rposition(|frame|self.loop_body_route(script,&frame.header).is_ok_and(|body|body==route))
                    .ok_or("Exhausted Init loop body has no source-owned continuation")?;
                let header=self.loops[owner].header.clone();
                if self.loops[owner].native_break.is_some(){
                    if owner+1!=self.loops.len(){return Err("Native Break cannot discard another active loop continuation".into());}
                    let PortableStartupLoopState::DemandV1{cursor}=&mut self.loops[owner].state else{return Err("Native Break has no demand owner".into());};
                    if cursor.after_body(true).map_err(|e|e.to_string())?!=crate::native_loop_cursor::AfterBody::CompletedWithoutComparison{
                        return Err("Native Break requested another comparison".into());
                    }
                    self.loops.pop();route=header;
                    // Advance straight to the actual Completed successor. No
                    // fabricated header operation, breakpoint or bound gather.
                    continue;
                }
                return self.set_route(header);
            }
            if let (wire::Route::Branch(index),Statement::Sequence{branches})=(part,parent){
                if let Some(next)=(index+1..branches.len()).find(|next|!branches[*next].is_empty()){
                    route.extend([wire::Route::Branch(next),wire::Route::Statement(0)]);
                    return self.set_route(route);
                }
            }
            // If/consumer/completed Sequence branches return to their actual
            // parent statement; a live loop instead returned to its header.
        }
    }
    pub(in crate::engine) fn enter_if(&mut self,script:&[Statement],mut route:Vec<wire::Route>,selected:bool)->Result<(),String>{
        let root=self.current().ok_or("Startup condition has no genuine root")?;
        let mut parent=route.clone();let Some(wire::Route::Statement(index))=parent.pop()else{return Err("Startup If route shape".into());};
        let Some(Statement::If{then_branch,else_branch,..})=Self::body_at(script,root.pc,&parent)?.get(index)else{return Err("Startup condition route is not an actual If".into());};
        let branch=if selected{then_branch}else{else_branch};
        if branch.is_empty(){return self.advance_from(script,route);}
        route.push(if selected{wire::Route::Then}else{wire::Route::Else});route.push(wire::Route::Statement(0));
        self.set_route(route)
    }
    pub(in crate::engine) fn actual_if_route(&self,script:&[Statement],then_branch:&[Statement],else_branch:&[Statement])->Result<Vec<wire::Route>,String>{
        let mut route=self.route();let mut node=self.statement(script)?;
        if let Statement::ScheduledConsumer{body,..}=node{
            let [child]=body.as_slice()else{return Err("Startup If consumer has no unique real body".into());};node=child;route.extend([wire::Route::Body,wire::Route::Statement(0)]);
        }
        if !matches!(node,Statement::If{then_branch:actual_then,else_branch:actual_else,..}if actual_then==then_branch&&actual_else==else_branch){return Err("Prepared startup If differs from its frozen source branches".into());}
        Ok(route)
    }
}
impl<R:Renderer> Engine<R>{
    pub(in crate::engine) fn execute_startup_if(&mut self,condition:rvn_parser::Expr,then_branch:&[Statement],else_branch:&[Statement])->Result<(),RuntimeError>{
        let route=self.startup.as_ref().ok_or_else(||self.scene_resource_error("Startup If requires its actual owner".into()))?.actual_if_route(&self.script,then_branch,else_branch).map_err(|e|self.scene_resource_error(e))?;
        let globals=self.vars_for_eval();let lexical=self.lexical_vars_for_eval();
        let selected=self.functions.eval_bool_scoped_with_locale_with_widget_objects(self.widget_object_context(&self.state)?.as_ref(), &condition,&globals,&lexical,&mut self.state.random,self.locale.as_ref()).map_err(|error|self.eval_err(error,"Initializer If condition"))?;
        self.startup.as_mut().unwrap().enter_if(&self.script,route,selected).map_err(|e|self.scene_resource_error(e))
    }
}
