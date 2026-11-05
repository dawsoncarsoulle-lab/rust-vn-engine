//! Preparation of explicitly authored scene programs. Legacy flattening stays
//! in Engine; this compiler records actual provenance and generated ownership.
use crate::{eval::EvalError, scene::ResumePosition};
#[path="program_native_loops.rs"]
mod native_loops;
use native_loops::{NativeBodyRequest,PreparedNativeLoops};
use rvn_parser::{
    BinOpKind, Expr, NarrativeContext, ParsedProgram, Script, Statement, StatementSpan,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AstRoutePart {
    Root { index: usize },
    Body { index: usize },
    Then { index: usize },
    Else { index: usize },
    Choice { option: usize, index: usize },
    Hotspot { hotspot: usize, index: usize },
    Sequence { branch: usize, index: usize },
    Producer { index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    Loop,
    Then,
    Else,
    Choice,
    Hotspot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeneratedRole {
    EndGuard,
    EndLabel,
    BranchLabel { branch: BranchKind, ordinal: usize },
    BranchReturn { branch: BranchKind, ordinal: usize },
    ForCollection,
    ForCursor,
    ForItem,
    ForAdvance,
    ForCondition,
    ForBreakGuard,
    ForBreak,
}

/// Actual compiler IDs, or canonical identities declared by a versioned source
/// plan and checked against its frozen bytes. Unmarked RVN has no guessed IDs.
/// Graph still owns catalogue/type/wiring validation of those declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphInstructionOrigin {
    pub source: Option<PathBuf>,
    pub graph: u64,
    pub node: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionOrigin {
    pub source: Option<PathBuf>,
    pub span: Option<Range<usize>>,
    pub route: Vec<AstRoutePart>,
    pub generated: Option<GeneratedRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<GraphInstructionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_step: Option<rvn_parser::scheduled_source::ScheduledStepSource>,
}

impl InstructionOrigin {
    fn generated(&self, role: GeneratedRole) -> Self {
        let mut origin = self.clone();
        origin.generated = Some(role);
        origin
    }
    fn program_end(role: GeneratedRole) -> Self {
        Self {
            source: None,
            span: None,
            route: Vec::new(),
            generated: Some(role),
            graph:None,scheduled_step:None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedNarrative {
    pub script: Script,
    pub scene_entries: HashMap<usize, String>,
    pub label_table: HashMap<String, usize>,
    pub branch_returns: HashSet<usize>,
    pub generated_targets: HashSet<String>,
    pub temporary_writes: HashMap<usize, String>,
    pub init_scan_end: usize,
    pub origins: Vec<InstructionOrigin>,
    /// Disposable nested computation sites; never part of the Save position.
    pub debug_origins: Vec<crate::editor_debugger::DebugOrigin>,
    /// Derived from real compiler branches; not a separate execution owner.
    pub(crate) native_loops: PreparedNativeLoops,
}

impl PreparedNarrative {
    /// Prepare only explicit scenes. Neither helper names nor background
    /// statements opt a legacy script into a different AST/story identity.
    pub fn prepare(script: Script) -> Result<Self, EvalError> {
        Self::compile(script, None)
    }

    pub fn from_parsed(program: ParsedProgram) -> Result<Self, EvalError> {
        if program.context != NarrativeContext::ExplicitScenes
            || NarrativeContext::from_script(&program.script) != program.context
        {
            return Err(invalid(
                "Le programme ne possède pas d'entrée de scène explicite.",
            ));
        }
        if program.origins.len() != program.script.len() {
            return Err(invalid(
                "Une origine réelle est requise pour chaque instruction auteur.",
            ));
        }
        // Reparse frozen snapshots, never the filesystem. This checks source,
        // AST and every nested range before a fabricated origin can be saved.
        let mut sources = HashMap::new();
        for source in &program.sources {
            if sources.contains_key(&source.path) {
                return Err(invalid("Identité de source ambiguë dans la provenance."));
            }
            let parsed = rvn_parser::parse_with_origins(&source.source, program.context)
                .map_err(|error| invalid(format!("Source de provenance invalide : {error}")))?;
            sources.insert(source.path.clone(), parsed);
        }
        let mut used = HashSet::new();
        for (statement, origin) in program.script.iter().zip(&program.origins) {
            let parsed = sources
                .get(&origin.source)
                .ok_or_else(|| invalid("Source d'origine absente du snapshot gelé."))?;
            if !used.insert((
                origin.source.clone(),
                origin.span.range.start,
                origin.span.range.end,
            )) {
                return Err(invalid(
                    "La même origine auteur est répétée dans le programme résolu.",
                ));
            }
            let matches = parsed
                .script
                .iter()
                .zip(&parsed.spans)
                .any(|(actual, span)| actual == statement && span == &origin.span);
            if !matches {
                return Err(invalid(
                    "L'AST ou le range d'origine ne correspond pas aux bytes de source.",
                ));
            }
        }
        Self::compile(program.script, Some(program.origins))
    }

    pub fn position(&self, pc: usize) -> ResumePosition {
        ResumePosition {
            pc,
            origin: self.origins.get(pc).cloned(),
        }
    }

    pub fn validate_position(&self, position: &ResumePosition) -> Result<(), String> {
        if self.origins.len() != self.script.len() {
            return Err("La provenance du programme préparé est incomplète.".into());
        }
        if position.pc > self.script.len() {
            return Err("Position narrative hors du programme préparé.".into());
        }
        if position.origin.as_ref() != self.origins.get(position.pc) {
            return Err("La position narrative ne correspond pas à son origine préparée.".into());
        }
        Ok(())
    }

    fn compile(
        script: Script,
        origins: Option<Vec<rvn_parser::StatementOrigin>>,
    ) -> Result<Self, EvalError> {
        if NarrativeContext::from_script(&script) != NarrativeContext::ExplicitScenes {
            return Err(invalid(
                "Une entrée `scene label` réelle est requise pour cette préparation.",
            ));
        }
        crate::flow_control::validate(&script,false,0)?;
        let mut labels = HashSet::new();
        validate_structure(&script, 0, false, false, &mut labels)?;
        let debug_origins=crate::editor_debugger::collect_origins(&script,origins.as_deref()).map_err(invalid)?;
        let reserved = reserved_symbols(&script)?;
        let authored_native = if rvn_parser::scheduled_source::native_loop_contract::contains_explicit_loop(&script) {
            rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(&script).map_err(invalid)?;
            Some(std::sync::Arc::new(script.clone()))
        } else { None };
        let mut compiler = Compiler {
            reserved,
            counter: 0,
            extra: Vec::new(),
            generated_targets: HashSet::new(),
            native_bodies: Vec::new(),
        };
        let mut nodes = Vec::with_capacity(script.len());
        for (index, statement) in script.into_iter().enumerate() {
            let origin = origins.as_ref().map(|origins| &origins[index]);
            nodes.push(authored_node(
                statement,
                origin.map(|origin| origin.source.clone()).unwrap_or(None),
                origin.map(|origin| &origin.span),
                vec![AstRoutePart::Root { index }],
            )?);
        }
        let mut instructions = compiler.sequence(nodes)?;
        let init_scan_end = instructions.len();
        let end = compiler.symbol("end")?;
        compiler.generated_targets.insert(end.clone());
        instructions.push(Instruction {
            statement: Statement::Jump {
                target: end.clone(),
            },
            origin: InstructionOrigin::program_end(GeneratedRole::EndGuard),
            temporary: None,
        });
        instructions.append(&mut compiler.extra);
        instructions.push(Instruction {
            statement: Statement::Label { name: end },
            origin: InstructionOrigin::program_end(GeneratedRole::EndLabel),
            temporary: None,
        });
        let mut result = Self {
            script: Vec::with_capacity(instructions.len()),
            origins: Vec::with_capacity(instructions.len()),
            debug_origins,
            scene_entries: HashMap::new(),
            label_table: HashMap::new(),
            branch_returns: HashSet::new(),
            temporary_writes: HashMap::new(),
            generated_targets: compiler.generated_targets,
            init_scan_end,
            native_loops: PreparedNativeLoops::default(),
        };
        for (pc, instruction) in instructions.into_iter().enumerate() {
            if let Statement::Label { name } | Statement::SceneLabel { name } =
                &instruction.statement
            {
                if result.label_table.insert(name.clone(), pc).is_some() {
                    return Err(invalid(format!("Label dupliqué : {name}")));
                }
            }
            if let Statement::SceneLabel { name } = &instruction.statement {
                result.scene_entries.insert(pc, name.clone());
            }
            if matches!(
                instruction.origin.generated,
                Some(GeneratedRole::BranchReturn { .. })
            ) {
                result.branch_returns.insert(pc);
            }
            if let Some(name) = instruction.temporary {
                result.temporary_writes.insert(pc, name);
            }
            result.script.push(instruction.statement);
            result.origins.push(instruction.origin);
        }
        result.native_loops = PreparedNativeLoops::resolve(authored_native,compiler.native_bodies,&result)?;
        Ok(result)
    }
}

fn invalid(message: impl Into<String>) -> EvalError {
    EvalError::InvalidFunction(message.into())
}

fn child_statements(statement: &Statement) -> Vec<(&Statement, AstRoutePart)> {
    match statement {
        Statement::FunctionLibrary{scope}=>scope.members().iter().enumerate()
            .map(|(index,member)|(member.declaration(),AstRoutePart::Body{index})).collect(),
        Statement::Init { body }
        | Statement::Function { body, .. }
        | Statement::UnitFunction { body, .. }
        | Statement::Screen { body, .. }
        | Statement::Handler { body, .. }
        | Statement::ScheduledConsumer { body, .. }
        | Statement::While { body, .. }
        | Statement::ForEach { body, .. }
        | Statement::ForLoopWithBreak { body, .. } => body
            .iter()
            .enumerate()
            .map(|(index, s)| (s, AstRoutePart::Body { index }))
            .collect(),
        Statement::Sequence { branches } => branches.iter().enumerate().flat_map(|(branch,body)|body.iter().enumerate().map(move |(index,s)|(s,AstRoutePart::Sequence{branch,index}))).collect(),
        Statement::StandardMacroDispatch{dispatch}=>dispatch.branches.iter().enumerate().flat_map(|(branch,body)|body.body.iter().enumerate().map(move |(index,s)|(s,AstRoutePart::Sequence{branch,index}))).collect(),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => then_branch
            .iter()
            .enumerate()
            .map(|(index, s)| (s, AstRoutePart::Then { index }))
            .chain(
                else_branch
                    .iter()
                    .enumerate()
                    .map(|(index, s)| (s, AstRoutePart::Else { index })),
            )
            .collect(),
        Statement::Choice { options } => options
            .iter()
            .enumerate()
            .flat_map(|(option, entry)| {
                entry
                    .body
                    .iter()
                    .enumerate()
                    .map(move |(index, s)| (s, AstRoutePart::Choice { option, index }))
            })
            .collect(),
        Statement::Imagemap { hotspots, .. } => hotspots
            .iter()
            .enumerate()
            .flat_map(|(hotspot, entry)| {
                entry
                    .body
                    .iter()
                    .enumerate()
                    .map(move |(index, s)| (s, AstRoutePart::Hotspot { hotspot, index }))
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn validate_structure(
    script: &[Statement],
    depth: usize,
    lexical: bool,
    init: bool,
    labels: &mut HashSet<String>,
) -> Result<(), EvalError> {
    if depth > 128 {
        return Err(EvalError::ExecutionLimit {
            limit: "128 blocs narratifs imbriqués",
        });
    }
    for statement in script {
        match statement {
            Statement::FunctionLibrary{scope} if depth==0=>{
                scope.validate().map_err(invalid)?;
                // Members remain lexical computations, not nested narrative roots.
                for member in scope.members(){match member.declaration(){
                    Statement::UnitFunction{body,..}=>crate::eval::FunctionLibrary::validate_unit_definition_body(body)?,
                    Statement::Function{body,..}=>crate::eval::validate_screen(body)?,
                    _=>return Err(invalid("Library member has no actual function declaration")),
                }}
                continue;
            },
            Statement::FunctionLibrary{..}=>return Err(invalid("Library scope must be a root declaration")),
            Statement::LibraryUnitCall{..} if lexical=>return Err(invalid("Qualified Unit cannot execute through a pure lexical computation")),
            Statement::UnitFunction{body,..} if depth==0 => { crate::eval::FunctionLibrary::validate_unit_definition_body(body)?; continue; },
            Statement::UnitFunction{..}|Statement::UnitReturn => return Err(invalid("Unit declaration/return has no genuine root/invocation owner")),
            Statement::UnitCall{..} if lexical => return Err(invalid("Unit statement cannot execute through a pure Function, Screen or Handler")),
            Statement::Use { .. } => {
                return Err(invalid(
                    "Les instructions Use doivent être résolues avant préparation.",
                ))
            }
            Statement::SceneLabel { name } => {
                if depth > 0 {
                    return Err(invalid("Une entrée SceneLabel doit être à la racine."));
                }
                validate_label_name(name)?;
                if !labels.insert(name.clone()) {
                    return Err(invalid(format!("Label dupliqué : {name}")));
                }
            }
            Statement::Label { name } => {
                validate_label_name(name)?;
                if !labels.insert(name.clone()) {
                    return Err(invalid(format!("Label dupliqué : {name}")));
                }
            }
            Statement::LocalVar { .. } if init && !lexical => {
                return Err(invalid("LocalVar narratif interdit dans Init."))
            }
            _ => {}
        }
        let children = child_statements(statement);
        if !children.is_empty() {
            let child_script: Vec<_> = children.iter().map(|(s, _)| (*s).clone()).collect();
            let next_lexical = lexical
                || matches!(
                    statement,
                    Statement::Function { .. }
                        | Statement::Screen { .. }
                        | Statement::Handler { .. }
                );
            let next_init = init || matches!(statement, Statement::Init { .. });
            validate_structure(&child_script, depth + 1, next_lexical, next_init, labels)?;
        }
    }
    Ok(())
}

fn validate_label_name(name: &str) -> Result<(), EvalError> {
    let parsed = rvn_parser::parse(&format!("label {name}"));
    if !matches!(parsed.as_ref().map(Vec::as_slice), Ok([Statement::Label {name: actual}]) if actual == name)
    {
        return Err(invalid(format!("Nom de label invalide : {name:?}")));
    }
    Ok(())
}

fn reserved_symbols(script: &Script) -> Result<HashSet<String>, EvalError> {
    // Include authored references as well as declarations. Otherwise an
    // unresolved user alias could accidentally resolve to a generated symbol.
    let value = serde_json::to_value(script).map_err(|error| invalid(error.to_string()))?;
    let mut pending = vec![&value];
    let mut names = HashSet::new();
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::String(value) => {
                names.insert(value.clone());
            }
            serde_json::Value::Array(values) => pending.extend(values),
            serde_json::Value::Object(values) => pending.extend(values.values()),
            _ => {}
        }
    }
    Ok(names)
}

struct Node {
    statement: Statement,
    origin: InstructionOrigin,
    children: Vec<Node>,
    temporary: Option<String>,
}

fn authored_node(
    statement: Statement,
    source: Option<PathBuf>,
    span: Option<&StatementSpan>,
    route: Vec<AstRoutePart>,
) -> Result<Node, EvalError> {
    let child_refs = child_statements(&statement);
    if let Some(span) = span {
        if span.children.len() != child_refs.len() {
            return Err(invalid(
                "Les branches de provenance ne correspondent pas à l'AST.",
            ));
        }
    }
    let mut children = Vec::with_capacity(child_refs.len());
    for (index, (child, part)) in child_refs.into_iter().enumerate() {
        let mut child_route = route.clone();
        child_route.push(part);
        children.push(authored_node(
            child.clone(),
            source.clone(),
            span.map(|span| &span.children[index]),
            child_route,
        )?);
    }
    Ok(Node {
        statement,
        children,
        temporary: None,
        origin: InstructionOrigin {
            source,
            span: span.map(|span| span.range.clone()),
            route,
            generated: None,
            graph:None,scheduled_step:None,
        },
    })
}

struct Instruction {
    statement: Statement,
    origin: InstructionOrigin,
    temporary: Option<String>,
}
struct Compiler {
    reserved: HashSet<String>,
    counter: u64,
    extra: Vec<Instruction>,
    generated_targets: HashSet<String>,
    native_bodies: Vec<NativeBodyRequest>,
}

impl Compiler {
    fn symbol(&mut self, role: &str) -> Result<String, EvalError> {
        loop {
            self.counter = self
                .counter
                .checked_add(1)
                .ok_or(EvalError::NumericOverflow)?;
            let candidate = format!("__rvn_scene_{role}_{}", self.counter);
            if self.reserved.insert(candidate.clone()) {
                return Ok(candidate);
            }
        }
    }

    fn branch(
        &mut self,
        nodes: Vec<Node>,
        origin: &InstructionOrigin,
        branch: BranchKind,
        ordinal: usize,
    ) -> Result<Vec<Statement>, EvalError> {
        let block = self.sequence(nodes)?;
        if block.is_empty() && branch != BranchKind::Loop {
            return Ok(Vec::new());
        }
        let target = self.symbol("branch")?;
        self.generated_targets.insert(target.clone());
        self.extra.push(Instruction {
            statement: Statement::Label {
                name: target.clone(),
            },
            origin: origin.generated(GeneratedRole::BranchLabel { branch, ordinal }),
            temporary: None,
        });
        self.extra.extend(block);
        self.extra.push(Instruction {
            statement: Statement::Return,
            origin: origin.generated(GeneratedRole::BranchReturn { branch, ordinal }),
            temporary: None,
        });
        Ok(vec![Statement::Call { target }])
    }

    fn guarded_range_children(&mut self,nodes: Vec<Node>, stop: &str) -> Result<Vec<Node>,EvalError> {
        let mut result=Vec::new();
        for mut node in nodes {
            match node.statement.clone() {
                Statement::LoopBreak => {node.statement=Statement::SetVar{name:stop.into(),value:Expr::Bool(true)};node.temporary=Some(stop.into());node.origin=node.origin.generated(GeneratedRole::ForBreak);},
                Statement::If{..}|Statement::Sequence{..}|Statement::Choice{..}|Statement::Imagemap{..}=>node.children=self.guarded_range_children(node.children,stop)?,
                Statement::While{condition,body}=>{
                    node.statement=Statement::While{condition:Expr::And(Box::new(crate::flow_control::active(stop)),Box::new(condition)),body};
                    node.children=self.guarded_range_children(node.children,stop)?;
                },
                Statement::ScheduledConsumer{version:2,scope,consumer,steps,body} if matches!(body.as_slice(),[Statement::ForEach{..}])=>{
                    let original=node.origin.clone();let mut guarded=self.guarded_range_children(node.children,stop)?;
                    let mut entry=guarded.remove(0).children.remove(0);
                    let first=entry.children.first_mut().ok_or_else(||invalid("Guarded iterator lost collection entry"))?;
                    first.statement=Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![first.statement.clone()]};first.origin=original;
                    entry.statement=Statement::Sequence{branches:vec![entry.children.iter().map(|n|n.statement.clone()).collect()]};node=entry;
                },
                Statement::ScheduledConsumer{version:2,body,..} if matches!(body.as_slice(),[Statement::If{..}|Statement::Choice{..}|Statement::Imagemap{..}])=>{
                    let inner=node.children.first_mut().ok_or_else(||invalid("Guarded consumer lost authored control"))?;inner.children=self.guarded_range_children(std::mem::take(&mut inner.children),stop)?;
                },
                Statement::ScheduledConsumer{version:2,scope,consumer,steps,body} if matches!(body.as_slice(),[Statement::While{..}])=>{
                    let inner=node.children.pop().ok_or_else(||invalid("Guarded While lost actual condition/body"))?;let Statement::While{condition,..}=inner.statement else{unreachable!()};let flag=self.symbol("range_consumer_while")?;let origin=node.origin.clone();
                    let mut children=self.guarded_range_children(inner.children,stop)?;let then_branch=children.iter().map(|n|n.statement.clone()).collect();let end=Node{statement:Statement::SetVar{name:flag.clone(),value:Expr::Bool(false)},origin:origin.generated(GeneratedRole::ForCursor),children:vec![],temporary:Some(flag.clone())};let else_branch=vec![end.statement.clone()];children.push(end);
                    let conditional=Node{statement:Statement::If{condition,then_branch,else_branch},origin:inner.origin,children,temporary:None};let header=Node{statement:Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![conditional.statement.clone()]},origin:origin.clone(),children:vec![conditional],temporary:None};
                    let children=vec![Node{statement:Statement::SetVar{name:flag.clone(),value:Expr::Bool(true)},origin:origin.generated(GeneratedRole::ForCursor),children:vec![],temporary:Some(flag.clone())},Node{statement:Statement::While{condition:Expr::And(Box::new(crate::flow_control::active(stop)),Box::new(Expr::Var(flag))),body:vec![header.statement.clone()]},origin:origin.generated(GeneratedRole::ForCondition),children:vec![header],temporary:None}];
                    node=Node{statement:Statement::Sequence{branches:vec![children.iter().map(|n|n.statement.clone()).collect()]},origin:origin.generated(GeneratedRole::ForBreakGuard),children,temporary:None};
                },
                Statement::ForEach{name,collection,..}=>{
                    let items=self.symbol("range_items")?;let cursor=self.symbol("range_each")?;let origin=node.origin.clone();
                    let mut iteration=vec![Node{statement:Statement::SetVar{name,value:Expr::Index{target:Box::new(Expr::Var(items.clone())),index:Box::new(Expr::Var(cursor.clone()))}},origin:origin.generated(GeneratedRole::ForItem),children:vec![],temporary:None},Node{statement:Statement::SetVar{name:cursor.clone(),value:Expr::BinOp{op:BinOpKind::Add,left:Box::new(Expr::Var(cursor.clone())),right:Box::new(Expr::Int(1))}},origin:origin.generated(GeneratedRole::ForAdvance),children:vec![],temporary:Some(cursor.clone())}];
                    iteration.extend(self.guarded_range_children(node.children,stop)?);
                    let condition=Expr::And(Box::new(crate::flow_control::active(stop)),Box::new(Expr::BinOp{op:BinOpKind::Lt,left:Box::new(Expr::Var(cursor.clone())),right:Box::new(Expr::Call{name:"len".into(),args:vec![Expr::Var(items.clone())]})}));
                    let children=vec![Node{statement:Statement::SetVar{name:items.clone(),value:Expr::Call{name:"__rvn_iterable".into(),args:vec![collection]}},origin:origin.generated(GeneratedRole::ForCollection),children:vec![],temporary:Some(items)},Node{statement:Statement::SetVar{name:cursor.clone(),value:Expr::Int(0)},origin:origin.generated(GeneratedRole::ForCursor),children:vec![],temporary:Some(cursor)},Node{statement:Statement::While{condition,body:iteration.iter().map(|n|n.statement.clone()).collect()},origin:origin.generated(GeneratedRole::ForCondition),children:iteration,temporary:None}];
                    node=Node{statement:Statement::Sequence{branches:vec![children.iter().map(|n|n.statement.clone()).collect()]},origin:origin.generated(GeneratedRole::ForBreakGuard),children,temporary:None};
                },
                _=>{}
            }
            result.push(Node{statement:Statement::If{condition:crate::flow_control::active(stop),then_branch:vec![node.statement.clone()],else_branch:vec![]},origin:node.origin.generated(GeneratedRole::ForBreakGuard),children:vec![node],temporary:None});
        } Ok(result)
    }
    fn range_loop(&mut self,name:String,first:Expr,last:Expr,children:Vec<Node>,origin:&InstructionOrigin)->Result<Vec<Instruction>,EvalError>{
        let bounds=self.symbol("range_bounds")?;let cursor=self.symbol("range_cursor")?;let stop=self.symbol("range_stop")?;
        let mut out=Vec::new();
        for (name,value,role) in [(bounds.clone(),Expr::Call{name:"__rvn_for_bounds".into(),args:vec![first,last]},GeneratedRole::ForCollection),(cursor.clone(),Expr::Int(0),GeneratedRole::ForCursor),(stop.clone(),Expr::Bool(false),GeneratedRole::ForCursor)] {
            out.push(Instruction{statement:Statement::SetVar{name:name.clone(),value},origin:origin.generated(role),temporary:Some(name)});
        }
        let at=|index|Expr::Index{target:Box::new(Expr::Var(bounds.clone())),index:Box::new(Expr::Int(index))};
        let mut iteration=vec![Node{statement:Statement::SetVar{name,value:Expr::BinOp{op:BinOpKind::Add,left:Box::new(at(0)),right:Box::new(Expr::Var(cursor.clone()))}},origin:origin.generated(GeneratedRole::ForItem),children:vec![],temporary:None},Node{statement:Statement::SetVar{name:cursor.clone(),value:Expr::BinOp{op:BinOpKind::Add,left:Box::new(Expr::Var(cursor.clone())),right:Box::new(Expr::Int(1))}},origin:origin.generated(GeneratedRole::ForAdvance),children:vec![],temporary:Some(cursor.clone())}];
        iteration.extend(self.guarded_range_children(children,&stop)?);
        let body=self.branch(iteration,origin,BranchKind::Loop,0)?;
        out.push(Instruction{statement:Statement::While{condition:Expr::And(Box::new(crate::flow_control::active(&stop)),Box::new(Expr::BinOp{op:BinOpKind::Lt,left:Box::new(Expr::Var(cursor)),right:Box::new(at(1))})),body},origin:origin.generated(GeneratedRole::ForCondition),temporary:None});
        Ok(out)
    }

    fn sequence(&mut self, nodes: Vec<Node>) -> Result<Vec<Instruction>, EvalError> {
        let mut out = Vec::new();
        for Node {
            statement,
            origin,
            mut children,
            temporary,
        } in nodes
        {
            let statement = match statement {
                statement @ Statement::ScheduledConsumer{version:3,..} => {
                    rvn_parser::scheduled_source::native_loop_contract::binding(&statement).map_err(invalid)?.ok_or_else(||invalid("Native compiler header has no source binding"))?;
                    if children.len()!=1{return Err(invalid("Native compiler lost the actual source loop child"));}
                    let inner=children.pop().unwrap();
                    let calls=self.branch(inner.children,&origin,BranchKind::Loop,0)?;
                    let [Statement::Call{target}]=calls.as_slice() else{return Err(invalid("Native loop body did not produce its actual branch"));};
                    self.native_bodies.push(NativeBodyRequest{header:origin.clone(),target:target.clone()});
                    // Keep the exact authored consumer, steps and body for phase
                    // provenance. Execution will enter the derived branch only.
                    statement
                },
                Statement::ScheduledConsumer{version:2,scope,consumer,steps,body} if matches!(body.as_slice(),[Statement::If{..}|Statement::Choice{..}|Statement::Imagemap{..}|Statement::While{..}|Statement::Sequence{..}])=>{
                    let inner=children.pop().ok_or_else(||invalid("Scheduled control lost its authored child/origin"))?;
                    if matches!(inner.statement,Statement::Sequence{..}){
                        out.push(Instruction{statement:Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![Statement::Sequence{branches:vec![]}]},origin,temporary});
                        out.extend(self.sequence(inner.children)?);continue;
                    }
                    let mut lowered=self.sequence(vec![inner])?;if lowered.len()!=1{return Err(invalid("Scheduled If did not lower to one actual instruction"));}
                    Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![lowered.pop().unwrap().statement]}
                },
                Statement::ScheduledConsumer{version:2,scope,consumer,steps,body} if matches!(body.as_slice(),[Statement::ForEach{..}|Statement::ForLoopWithBreak{..}])=>{
                    let inner=children.pop().ok_or_else(||invalid("Scheduled iterator lost its authored child/origin"))?;
                    let mut lowered=self.sequence(vec![inner])?;
                    if lowered.is_empty(){return Err(invalid("Scheduled iterator has no actual collection/bounds instruction"));}
                    let first=lowered.remove(0);
                    out.push(Instruction{statement:Statement::ScheduledConsumer{version:2,scope,consumer,steps,body:vec![first.statement]},origin,temporary:first.temporary});
                    out.extend(lowered);continue;
                },
                Statement::Sequence { branches } => {
                    let mut child=children.into_iter();
                    for branch in branches { out.extend(self.sequence(child.by_ref().take(branch.len()).collect())?); }
                    continue;
                }
                Statement::ForLoopWithBreak { name, first, last, .. } => {
                    out.extend(self.range_loop(name,first,last,children,&origin)?);
                    continue;
                }
                Statement::ForEach {
                    name, collection, ..
                } => {
                    let items = self.symbol("items")?;
                    let cursor = self.symbol("cursor")?;
                    out.push(Instruction {
                        statement: Statement::SetVar {
                            name: items.clone(),
                            value: Expr::Call {
                                name: "__rvn_iterable".into(),
                                args: vec![collection],
                            },
                        },
                        origin: origin.generated(GeneratedRole::ForCollection),
                        temporary: Some(items.clone()),
                    });
                    out.push(Instruction {
                        statement: Statement::SetVar {
                            name: cursor.clone(),
                            value: Expr::Int(0),
                        },
                        origin: origin.generated(GeneratedRole::ForCursor),
                        temporary: Some(cursor.clone()),
                    });
                    let mut iteration = vec![
                        Node {
                            statement: Statement::SetVar {
                                name,
                                value: Expr::Index {
                                    target: Box::new(Expr::Var(items.clone())),
                                    index: Box::new(Expr::Var(cursor.clone())),
                                },
                            },
                            origin: origin.generated(GeneratedRole::ForItem),
                            children: Vec::new(),
                            temporary: None,
                        },
                        Node {
                            statement: Statement::SetVar {
                                name: cursor.clone(),
                                value: Expr::BinOp {
                                    op: BinOpKind::Add,
                                    left: Box::new(Expr::Var(cursor.clone())),
                                    right: Box::new(Expr::Int(1)),
                                },
                            },
                            origin: origin.generated(GeneratedRole::ForAdvance),
                            children: Vec::new(),
                            temporary: Some(cursor.clone()),
                        },
                    ];
                    iteration.append(&mut children);
                    let body = self.branch(iteration, &origin, BranchKind::Loop, 0)?;
                    out.push(Instruction {
                        statement: Statement::While {
                            condition: Expr::BinOp {
                                op: BinOpKind::Lt,
                                left: Box::new(Expr::Var(cursor)),
                                right: Box::new(Expr::Call {
                                    name: "len".into(),
                                    args: vec![Expr::Var(items)],
                                }),
                            },
                            body,
                        },
                        origin: origin.generated(GeneratedRole::ForCondition),
                        temporary: None,
                    });
                    continue;
                }
                Statement::While { condition, .. } => Statement::While {
                    condition,
                    body: self.branch(children, &origin, BranchKind::Loop, 0)?,
                },
                Statement::If {
                    condition,
                    then_branch,
                    ..
                } => {
                    let other = children.split_off(then_branch.len());
                    Statement::If {
                        condition,
                        then_branch: self.branch(children, &origin, BranchKind::Then, 0)?,
                        else_branch: self.branch(other, &origin, BranchKind::Else, 0)?,
                    }
                }
                Statement::Choice { mut options } => {
                    let mut children = children.into_iter();
                    for (ordinal, option) in options.iter_mut().enumerate() {
                        let body = children.by_ref().take(option.body.len()).collect();
                        option.body = self.branch(body, &origin, BranchKind::Choice, ordinal)?;
                    }
                    Statement::Choice { options }
                }
                Statement::Imagemap {
                    background,
                    hover_image,
                    mut hotspots,
                } => {
                    let mut children = children.into_iter();
                    for (ordinal, hotspot) in hotspots.iter_mut().enumerate() {
                        let body = children.by_ref().take(hotspot.body.len()).collect();
                        hotspot.body = self.branch(body, &origin, BranchKind::Hotspot, ordinal)?;
                    }
                    Statement::Imagemap {
                        background,
                        hover_image,
                        hotspots,
                    }
                }
                // Init and lexical declarations retain their authored bodies.
                other => other,
            };
            out.push(Instruction {
                statement,
                origin,
                temporary,
            });
        }
        Ok(out)
    }
}
