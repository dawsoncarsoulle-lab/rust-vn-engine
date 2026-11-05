//! Qualified entries in the ONE existing FunctionLibrary definition table.
//! Caller identity is process-local frame state, never a call argument or UI ID.
use super::*;
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset,LibraryFunctionAccess,QualifiedLibraryFunction,MAX_FUNCTION_LIBRARY_MEMBERS};
use rvn_parser::function_library_program1023::{FunctionLibraryScope,LibraryCallMode,LibraryParameterId,QualifiedLibraryCall};

#[derive(Clone,Debug,PartialEq,Eq,Hash)]
pub(super) enum FunctionSymbol{Legacy(String),Library(QualifiedLibraryFunction)}
impl FunctionSymbol{
    pub(super) fn legacy(name:&str)->Self{Self::Legacy(name.to_owned())}
    /// Diagnostic/source provenance key only, never callable name resolution.
    pub(super) fn scope_key(&self)->String{match self{Self::Legacy(name)=>name.clone(),Self::Library(reference)=>format!("library:{}:{}",reference.library.as_str(),reference.graph_id)}}
}
impl std::fmt::Display for FunctionSymbol{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{match self{Self::Legacy(name)=>f.write_str(name),Self::Library(reference)=>write!(f,"{}:{}",reference.library.as_str(),reference.graph_id)}}
}
#[derive(Clone,Debug)]
pub(super) struct LibraryMetadata{
    pub(super) name:String,
    pub(super) parameters:Vec<LibraryParameterId>,
    pub(super) access:LibraryFunctionAccess,
    pub(super) pure:bool,
}
pub(super) struct ResolvedLibraryCall<'a,'b>{
    pub(super) symbol:FunctionSymbol,
    pub(super) definition:&'a(Vec<String>,Vec<Statement>),
    pub(super) arguments:Vec<&'b Expr>,
}
impl FunctionLibrary{
    pub(super) fn legacy_definition(&self,name:&str)->Option<&(Vec<String>,Vec<Statement>)>{self.definitions.get(&FunctionSymbol::legacy(name))}
    pub(super) fn legacy_unit(&self,name:&str)->bool{self.unit_functions.contains(&FunctionSymbol::legacy(name))}
    /// Immutable program construction, like from_script. It is not Source IO,
    /// Registry admission, an editor owner, or a host capability grant.
    pub fn from_script_with_library_assets(script:&[Statement],assets:&[FunctionLibraryAsset])->EvalResult<Self>{
        let mut library=Self::from_legacy_script(script)?;
        if assets.len()>MAX_FUNCTION_LIBRARY_MEMBERS{return Err(EvalError::InvalidFunction("Library owner count exceeds codec bound".into()));}
        let mut asset_ids=std::collections::BTreeSet::new();
        for asset in assets{if !asset_ids.insert(asset.id()){return Err(EvalError::InvalidFunction("Library asset index repeats UUID".into()));}}
        let mut owners=std::collections::BTreeSet::new();let mut graphs=std::collections::BTreeSet::new();let mut parameters=std::collections::BTreeSet::new();
        for statement in script{if let Statement::FunctionLibrary{scope}=statement{
            if !owners.insert(scope.library()){return Err(EvalError::InvalidFunction("Library Source repeats its original UUID".into()));}
            let asset=assets.iter().find(|asset|asset.id()==scope.library()).ok_or_else(||EvalError::InvalidFunction("Library declaration has no admitted original membership codec".into()))?;
            scope.validate_asset(asset).map_err(EvalError::InvalidFunction)?;
            for member in scope.members(){
                if !graphs.insert(member.graph_id())||graphs.len()>MAX_FUNCTION_LIBRARY_MEMBERS||member.parameter_ids().iter().any(|id|!parameters.insert(id)){
                    return Err(EvalError::InvalidFunction("Library Source repeats GraphId/ParameterId or exceeds bound".into()));
                }
                let reference=QualifiedLibraryFunction{library:scope.library().clone(),graph_id:member.graph_id()};
                let symbol=FunctionSymbol::Library(reference);
                let (name,names,body,unit)=match member.declaration(){Statement::Function{name,parameters,body}=>(name,parameters,body,false),Statement::UnitFunction{name,parameters,body}=>(name,parameters,body,true),_=>return Err(EvalError::InvalidFunction("Library member lost actual function kind".into()))};
                crate::flow_control::validate(body,false,0)?;
                if unit{unit_sequenced::validate_body(body,0)?;}else{validate_computation(body,true)?;}
                let metadata=asset.members().iter().find(|row|row.graph_id==member.graph_id()).ok_or_else(||EvalError::InvalidFunction("Library member metadata absent".into()))?;
                if unit{library.unit_functions.insert(symbol.clone());}
                library.library_metadata.insert(symbol.clone(),LibraryMetadata{name:name.clone(),parameters:member.parameter_ids().to_vec(),access:metadata.access,pure:metadata.pure});
                if library.definitions.insert(symbol,(names.clone(),body.clone())).is_some(){return Err(EvalError::InvalidFunction("Library definition identity repeated".into()));}
            }
        }}
        for asset in assets{if !asset.members().is_empty()&&!owners.contains(asset.id()){return Err(EvalError::InvalidFunction("Nonempty Library asset has no original Source scope".into()));}}
        library.library_assets=assets.to_vec();
        for (symbol,(_,body)) in &library.definitions{library.validate_library_body_calls1023(body,Some(symbol))?;}
        library.validate_unit_calls()?;Ok(library)
    }
    pub(super) fn resolve_library_symbol1023(&self,reference:&QualifiedLibraryFunction,mode:LibraryCallMode,actual_frame:Option<&FunctionSymbol>)->EvalResult<FunctionSymbol>{
        reference.validate().map_err(EvalError::InvalidFunction)?;
        let symbol=FunctionSymbol::Library(reference.clone());
        let metadata=self.library_metadata.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Original qualified Library member is absent; no name fallback".into()))?;
        if metadata.access!=LibraryFunctionAccess::Public{
            let allowed=match actual_frame{
                Some(caller@FunctionSymbol::Library(caller_reference))=>caller_reference.library==reference.library&&self.library_metadata.contains_key(caller),
                _=>false,
            };
            if !allowed{return Err(EvalError::InvalidFunction("Private Library call is outside its actual current frame".into()));}
        }
        let unit=self.unit_functions.contains(&symbol);
        if unit!=(mode==LibraryCallMode::Completion)||metadata.pure!=(mode==LibraryCallMode::Value){return Err(EvalError::InvalidFunction("Qualified Library result/exec shape differs".into()));}
        Ok(symbol)
    }
    pub(super) fn resolve_library_call<'a,'b>(&'a self,call:&'b QualifiedLibraryCall,mode:LibraryCallMode,actual_frame:Option<&FunctionSymbol>)->EvalResult<ResolvedLibraryCall<'a,'b>>{
        call.validate().map_err(EvalError::InvalidFunction)?;
        let symbol=self.resolve_library_symbol1023(call.reference(),mode,actual_frame)?;
        let metadata=self.library_metadata.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Original Library metadata absent".into()))?;
        if metadata.parameters.len()!=call.arguments().len(){return Err(EvalError::InvalidFunction("Qualified Library arity differs".into()));}
        let arguments=metadata.parameters.iter().map(|id|call.arguments().iter().find(|(candidate,_)|candidate==id).map(|(_,expr)|expr).ok_or_else(||EvalError::InvalidFunction("Qualified Library ParameterId is stale/foreign".into()))).collect::<EvalResult<Vec<_>>>()?;
        let definition=self.definitions.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Qualified definition disappeared from canonical table".into()))?;
        Ok(ResolvedLibraryCall{symbol,definition,arguments})
    }
}

impl Computation<'_>{
    pub(super) fn call_definition1023(&mut self,symbol:FunctionSymbol,arguments:Vec<Value>)->EvalResult<Value>{
        let library=self.library;
        let (parameters,body)=library.definitions.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Canonical function definition absent".into()))?;
        let name=match &symbol{FunctionSymbol::Legacy(name)=>name.clone(),FunctionSymbol::Library(_)=>library.library_metadata.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Library definition metadata absent".into()))?.name.clone()};
        if arguments.len() != parameters.len() {
            return Err(EvalError::InvalidFunction(format!(
                "{name} attend {} argument(s), reçu {}",
                parameters.len(),
                arguments.len()
            )));
        }
        if self.calls >= MAX_FUNCTION_DEPTH {
            return Err(EvalError::ExecutionLimit {
                limit: "64 appels de fonction imbriqués",
            });
        }
        for value in self.globals.values() {
            self.value_work = self
                .value_work
                .checked_add(crate::value_limits::value_work(value)?)
                .ok_or(EvalError::NumericOverflow)?;
            if self.value_work > 1_000_000 {
                return Err(EvalError::ExecutionLimit {
                    limit: "1 000 000 copies ou caractères de valeurs par calcul",
                });
            }
        }
        // Lexical scope: never inherit another function's parameters or locals.
        let mut locals = self.globals.clone();
        locals.extend(parameters.iter().cloned().zip(arguments));
        // Each invocation owns fresh terminals, including recursion/reentrancy.
        let caller_unit_locals=self.unit_locals.take();
        let caller_frame = std::mem::take(&mut self.consumer_frame);
        let caller_storage = std::mem::take(&mut self.consumer_storage);
        let caller_consumer = self.active_consumer.take();
        let caller_break_owners = std::mem::take(&mut self.range_break_owners);
        let caller_symbol = self.current_function1023.replace(symbol);
        self.calls += 1;
        let result = self.block(body, &mut locals);
        self.calls -= 1;
        self.current_function1023 = caller_symbol;
        self.consumer_frame = caller_frame;
        self.consumer_storage = caller_storage;
        self.active_consumer = caller_consumer;
        self.range_break_owners = caller_break_owners;
        self.unit_locals=caller_unit_locals;
        result?
            .ok_or_else(|| EvalError::InvalidFunction(format!("{name} n’a retourné aucune valeur")))
    }
    pub(super) fn library_value1023(&mut self,call:&QualifiedLibraryCall,variables:&HashMap<String,Value>,depth:usize)->EvalResult<Value>{
        let library=self.library;
        // Resolve mode/private scope/all ParameterIds BEFORE any argument RNG.
        let resolved=library.resolve_library_call(call,LibraryCallMode::Value,self.current_function1023.as_ref())?;
        let values=resolved.arguments.into_iter().map(|expr|self.eval(expr,variables,depth)).collect::<EvalResult<Vec<_>>>()?;
        if let Some(result)=self.debugger_library_call1023(&resolved.symbol,&values){return result;}
        self.call_definition1023(resolved.symbol,values)
    }
}

impl FunctionLibrary{
    pub(super) fn extend_library_source_declarations1023(
        &self,script:&[Statement],mut declarations:HashMap<FunctionSymbol,(Vec<String>,Vec<Statement>)>,mut units:std::collections::HashSet<FunctionSymbol>
    )->EvalResult<(HashMap<FunctionSymbol,(Vec<String>,Vec<Statement>)>,std::collections::HashSet<FunctionSymbol>)>{
        for statement in script{if let Statement::FunctionLibrary{scope}=statement{
            let asset=self.library_assets.iter().find(|asset|asset.id()==scope.library()).ok_or_else(||EvalError::InvalidFunction("Engine Library source metadata absent".into()))?;
            scope.validate_asset(asset).map_err(EvalError::InvalidFunction)?;
            for member in scope.members(){
                let symbol=FunctionSymbol::Library(QualifiedLibraryFunction{library:scope.library().clone(),graph_id:member.graph_id()});
                let metadata=self.library_metadata.get(&symbol).ok_or_else(||EvalError::InvalidFunction("Engine Library member metadata absent".into()))?;
                if metadata.name!=member.name()||metadata.parameters!=member.parameter_ids(){return Err(EvalError::InvalidFunction("Engine Library name/signature differs from actual Source scope".into()));}
                let (parameters,body,unit)=match member.declaration(){Statement::Function{parameters,body,..}=>(parameters,body,false),Statement::UnitFunction{parameters,body,..}=>(parameters,body,true),_=>return Err(EvalError::InvalidFunction("Engine Library member kind differs".into()))};
                if declarations.insert(symbol.clone(),(parameters.clone(),body.clone())).is_some(){return Err(EvalError::InvalidFunction("Engine Library Source repeats a declaration".into()));}
                if unit{units.insert(symbol);}
            }
        }}
        Ok((declarations,units))
    }
}

impl FunctionLibrary{
    // These bodies have already passed the ordinary computation/Unit validator.
    // actual_frame is derived solely from the canonical table iteration above.
    pub(super) fn validate_library_body_calls1023(&self,body:&[Statement],actual_frame:Option<&FunctionSymbol>)->EvalResult<()>{
        fn expression(library:&FunctionLibrary,expr:&Expr,frame:Option<&FunctionSymbol>)->EvalResult<()>{
            match expr{
                Expr::LibraryCall{call}=>{let resolved=library.resolve_library_call(call,LibraryCallMode::Value,frame)?;for expr in resolved.arguments{expression(library,expr,frame)?;}},
                Expr::Call{args,..}|Expr::ListLit(args)=>for expr in args{expression(library,expr,frame)?;},
                Expr::BinOp{left,right,..}|Expr::And(left,right)|Expr::Or(left,right)=>{expression(library,left,frame)?;expression(library,right,frame)?;},
                Expr::Neg(expr)|Expr::Not(expr)=>expression(library,expr,frame)?,
                Expr::Index{target,index}=>{expression(library,target,frame)?;expression(library,index,frame)?;},
                _=>{},
            }Ok(())
        }
        for statement in body{match statement{
            Statement::LibraryUnitCall{call}=>{let resolved=self.resolve_library_call(call,LibraryCallMode::Completion,actual_frame)?;for expr in resolved.arguments{expression(self,expr,actual_frame)?;}},
            Statement::UnitCall{arguments,..}=>for expr in arguments{expression(self,expr,actual_frame)?;},
            Statement::SetVar{value,..}|Statement::LocalVar{value,..}|Statement::FunctionReturn{value}=>expression(self,value,actual_frame)?,
            Statement::If{condition,then_branch,else_branch}=>{expression(self,condition,actual_frame)?;self.validate_library_body_calls1023(then_branch,actual_frame)?;self.validate_library_body_calls1023(else_branch,actual_frame)?;},
            Statement::Sequence{branches}=>for branch in branches{self.validate_library_body_calls1023(branch,actual_frame)?;},
            Statement::While{condition,body}=>{expression(self,condition,actual_frame)?;self.validate_library_body_calls1023(body,actual_frame)?;},
            Statement::ForEach{collection,body,..}=>{expression(self,collection,actual_frame)?;self.validate_library_body_calls1023(body,actual_frame)?;},
            Statement::ForLoopWithBreak{first,last,body,..}=>{expression(self,first,actual_frame)?;expression(self,last,actual_frame)?;self.validate_library_body_calls1023(body,actual_frame)?;},
            Statement::ScheduledConsumer{steps,body,..}=>{for step in steps{match step{rvn_parser::ConsumerStep::Capture{value,..}|rvn_parser::ConsumerStep::InspectTextTable{value,..}=>expression(self,value,actual_frame)?,}}self.validate_library_body_calls1023(body,actual_frame)?;},
            Statement::StandardMacroDispatch{dispatch}=>{for value in dispatch.operand_expressions(){expression(self,value,actual_frame)?;}for branch in &dispatch.branches{self.validate_library_body_calls1023(&branch.body,actual_frame)?;}},
            _=>{},
        }}Ok(())
    }
}

impl FunctionLibrary{
    pub(crate) fn library_assets1023(&self)->&[FunctionLibraryAsset]{&self.library_assets}
    pub(crate) fn validate_source_declarations1023(&self,script:&[Statement])->EvalResult<()>{
        let declarations=script.iter().filter_map(|statement|match statement{Statement::Function{name,parameters,body}|Statement::UnitFunction{name,parameters,body}=>Some((FunctionSymbol::legacy(name),(parameters.clone(),body.clone()))),_=>None}).collect();
        let units=script.iter().filter_map(|statement|if let Statement::UnitFunction{name,..}=statement{Some(FunctionSymbol::legacy(name))}else{None}).collect();
        let(declarations,units)=self.extend_library_source_declarations1023(script,declarations,units)?;
        if declarations!=self.definitions||units!=self.unit_functions{return Err(EvalError::InvalidFunction("Library bootstrap declarations differ from their actual frozen Source".into()));}Ok(())
    }
}

impl FunctionLibrary{
    pub(crate) fn invoke_library_unit_scoped1023(&self,call:&QualifiedLibraryCall,environment:&HashMap<String,Value>,globals:&HashMap<String,Value>,random:&mut RandomState,locale:Option<&crate::locale::LocaleManager>)->EvalResult<(HashMap<String,Value>,std::collections::HashSet<String>)>{
        // This is the original Scene root, not a claimed Library caller.
        self.resolve_library_call(call,LibraryCallMode::Completion,None)?;
        let mut computation=Computation::new(self,globals);computation.random=*random;computation.locale=locale;
        if let Some(result)=computation.debugger_library_unit1023(call,environment){result?;}else{computation.invoke_library_unit_expressions1023(call,environment)?;}
        *random=computation.random;Ok((computation.globals,computation.unit_global_writes))
    }
}
