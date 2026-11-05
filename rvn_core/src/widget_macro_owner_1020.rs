//! Child of widget_class_source_0950: authentic class/source admission plus
//! transaction-owned cells. Not a VM, callback queue or process registry.
use super::*;
use crate::program::GraphInstructionOrigin;
use crate::scene_resources::{TargetIdentity, TargetKind};
use crate::standard_macro_state::{
    DoNLimit, DoNState, DoOnceState, FlipFlopBranch, FlipFlopState, GateInput, GateState,
};
use rvn_parser::scheduled_source::standard_macro_contract::{
    Control, DataOutput, Dispatch, Kind, Output, Site,
};
use rvn_parser::{Statement, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdmittedSite {
    class: WidgetClassIdentity,
    handler: String,
    declaration: Site,
    origin: GraphInstructionOrigin,
}
impl AdmittedSite {
    pub(crate) fn origin(&self) -> &GraphInstructionOrigin {
        &self.origin
    }
    pub(crate) fn declaration(&self) -> &Site {
        &self.declaration
    }
    pub(crate) fn key(&self) -> (u64, u64) {
        (self.declaration.node.graph, self.declaration.node.node)
    }
}
/// Immutable per-FunctionLibrary source admission. These rows contain no live
/// instances or cells. The source programme/class admission owns their lifetime.
#[derive(Clone, Debug, Default)]
pub(crate) struct MacroSites {
    sites: BTreeMap<(String, u64, u64), AdmittedSite>,
}
impl MacroSites {
    pub(crate) fn admit(
        classes: &AdmittedWidgetClasses,
        program: &rvn_parser::ParsedProgram,
    ) -> Result<Self, String> {
        if !classes.matches_program(program)? {
            return Err("Macro classes differ from actual frozen programme".into());
        }
        let mut result = Self::default();
        // Reparse each frozen physical source once for admission, not once per
        // Handler/member. This local cache holds no runtime owner or cells.
        let mut actual_sources = BTreeMap::new();
        let mut operand_sources = BTreeMap::new();
        for class in classes.classes.values() {
            for &(graph, ref kind, ref name, root_index) in &class.members {
                if *kind != WidgetMemberKind::Handler {
                    continue;
                }
                let statement = program
                    .script
                    .get(root_index)
                    .ok_or("Macro Handler root missing")?;
                let origin = program
                    .origins
                    .get(root_index)
                    .ok_or("Macro Handler origin missing")?;
                let Statement::Handler {
                    name: actual, body, ..
                } = statement
                else {
                    return Err("Macro class member is not its actual Handler".into());
                };
                if actual != name {
                    return Err("Macro Handler name differs from class source".into());
                }
                let file = program
                    .sources
                    .get(class.identity.source_ordinal)
                    .ok_or("Macro physical source missing")?;
                if origin.source != file.path {
                    return Err("Macro Handler left its admitted physical source".into());
                }
                // Reparse the physical frozen bytes, not a caller-supplied AST
                // with a still-valid SourceMap content digest.
                if !actual_sources.contains_key(&class.identity.source_ordinal) {
                    actual_sources.insert(
                        class.identity.source_ordinal,
                        rvn_parser::parse_with_origins(&file.source, program.context)
                            .map_err(|e| e.to_string())?,
                    );
                }
                let parsed = &actual_sources[&class.identity.source_ordinal];
                if !parsed
                    .script
                    .iter()
                    .zip(&parsed.spans)
                    .any(|(actual, span)| actual == statement && span == &origin.span)
                {
                    return Err(
                        "Macro Handler AST/span differs from actual frozen source bytes".into(),
                    );
                }
                let mut admit_site = |declaration: &Site| -> Result<(), String> {
                    declaration.validate()?;
                    if declaration.node.graph != graph {
                        return Err(
                            "Macro declaration differs from actual Widget member graph".into()
                        );
                    }
                    let admitted = AdmittedSite {
                        class: class.identity.clone(),
                        handler: name.clone(),
                        declaration: declaration.clone(),
                        origin: GraphInstructionOrigin {
                            source: origin.source.clone(),
                            graph,
                            node: Some(declaration.node.node),
                        },
                    };
                    let key = (class.identity.uuid.clone(), graph, declaration.node.node);
                    if let Some(prior) = result.sites.get(&key) {
                        if prior != &admitted {
                            return Err(
                                "One macro source identity declares different actual ports/root"
                                    .into(),
                            );
                        }
                    } else {
                        result.sites.insert(key, admitted);
                    }
                    Ok(())
                };
                let mut pending = vec![body.as_slice()];
                while let Some(block) = pending.pop() {
                    for statement in block {
                        if let Statement::StandardMacroDispatch { dispatch } = statement {
                            dispatch.validate()?;
                            if let Some(value)=&dispatch.operand {
                                let key=(class.identity.uuid.clone(),dispatch.site.node.graph,dispatch.site.node.node);
                                let definition=(value.clone(),dispatch.deferred_operand_v1.clone());
                                if operand_sources.get(&key).is_some_and(|prior|prior!=&definition){return Err("One actual macro source node changes its operand definition across visits".into());}
                                operand_sources.insert(key,definition);
                            }
                            admit_site(&dispatch.site)?;
                        }
                        let mut expressions =
                            crate::consumer_schedule::direct_expressions(statement);
                        if let Statement::ScheduledConsumer { steps, .. } = statement {
                            expressions.extend(steps.iter().map(|step| match step {
                                rvn_parser::ConsumerStep::Capture { value, .. }
                                | rvn_parser::ConsumerStep::InspectTextTable { value, .. } => value,
                            }));
                        }
                        while let Some(value) = expressions.pop() {
                            match value {
                                rvn_parser::Expr::StandardMacroData { site, output, pin } => {
                                    if site.data_pin(*output)? != *pin {
                                        return Err(
                                            "Macro live output lost its actual source PinId".into(),
                                        );
                                    }
                                    admit_site(site)?;
                                }
                                rvn_parser::Expr::Call { args, .. }
                                | rvn_parser::Expr::ListLit(args) => expressions.extend(args),
                                rvn_parser::Expr::BinOp { left, right, .. }
                                | rvn_parser::Expr::And(left, right)
                                | rvn_parser::Expr::Or(left, right) => {
                                    expressions.push(left);
                                    expressions.push(right);
                                }
                                rvn_parser::Expr::Index { target, index } => {
                                    expressions.push(target);
                                    expressions.push(index);
                                }
                                rvn_parser::Expr::Neg(value) | rvn_parser::Expr::Not(value) => {
                                    expressions.push(value)
                                }
                                _ => {}
                            }
                        }
                        pending.extend(crate::consumer_schedule::statement_children(statement));
                    }
                }
            }
        }
        Ok(result)
    }
    pub(crate) fn site(
        &self,
        class: &WidgetClassIdentity,
        declaration: &Site,
    ) -> Result<&AdmittedSite, String> {
        declaration.validate()?;
        let site = self
            .sites
            .get(&(
                class.uuid.clone(),
                declaration.node.graph,
                declaration.node.node,
            ))
            .ok_or("Macro expansion is absent from actual class source")?;
        if &site.class != class || &site.declaration != declaration {
            return Err("Macro source/class receipt or real ports changed".into());
        }
        Ok(site)
    }
    pub(crate) fn by_identity(
        &self,
        class: &WidgetClassIdentity,
        graph: u64,
        node: u64,
    ) -> Result<&AdmittedSite, String> {
        let site = self
            .sites
            .get(&(class.uuid.clone(), graph, node))
            .ok_or("Macro node is absent from current class source")?;
        if &site.class != class {
            return Err("Macro inspection source/class receipt is stale".into());
        }
        Ok(site)
    }
    pub(crate) fn class_has_handler(&self, class: &WidgetClassIdentity, handler: &str) -> bool {
        self.sites
            .values()
            .any(|site| &site.class == class && site.handler == handler)
    }
    pub(crate) fn validate_frame(&self, frame: &WidgetMacroFrame) -> Result<(), String> {
        if frame.cells.len() > crate::value_limits::MAX_COLLECTION_ITEMS {
            return Err("Macro cells exceed the existing bounded collection limit".into());
        }
        for (&(graph, node), cell) in &frame.cells {
            let site = self
                .sites
                .get(&(frame.class.uuid.clone(), graph, node))
                .ok_or("Retained macro cell is absent from actual class source")?;
            if site.class != frame.class || site.declaration.kind()? != cell.kind() {
                return Err(
                    "Retained macro cells differ from current source generation/family".into(),
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Cell {
    DoOnce(DoOnceState),
    DoN(DoNState),
    FlipFlop(FlipFlopState),
    Gate(GateState),
}
impl Cell {
    fn fresh(kind: Kind) -> Self {
        match kind {
            Kind::DoOnce => Self::DoOnce(Default::default()),
            Kind::DoN => Self::DoN(Default::default()),
            Kind::FlipFlop => Self::FlipFlop(Default::default()),
            Kind::Gate => Self::Gate(Default::default()),
        }
    }
    fn kind(&self) -> Kind {
        match self {
            Self::DoOnce(_) => Kind::DoOnce,
            Self::DoN(_) => Kind::DoN,
            Self::FlipFlop(_) => Kind::FlipFlop,
            Self::Gate(_) => Kind::Gate,
        }
    }
}
/// Clone is deliberately exact for candidate rollback, warm debug and detach.
/// Cold GameSave omits this field under an explicit VN adaptation chosen by Root.
/// Unreal's five bIsPersistent=false flags do not prove cold widget SaveGame
/// behavior: EventGraph shared lifetime and USaveGame serialization are distinct.
/// The hot debug DTO grants no owner capability; Engine/source is revalidated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MacroFrameWire", into = "MacroFrameWire")]
pub struct WidgetMacroFrame {
    /// Existing Engine namespace, stable through warm rollback/planning clones.
    lineage: u64,
    target: TargetIdentity,
    class: WidgetClassIdentity,
    cells: BTreeMap<(u64, u64), Cell>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum OperandNeed {
    None,
    StartClosed,
    Limit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operand {
    None,
    StartClosed(bool),
    Limit(DoNLimit),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellInspection {
    DoOnce { initialized: bool, completed: bool },
    DoN { initialized: bool, counter: i32 },
    FlipFlop { is_a: bool },
    Gate { initialized: bool, open: bool },
}
impl WidgetMacroFrame {
    /// The caller obtained target via the existing real resident/closed-owner
    /// routing. Stored inert identity must be checked against it on every use.
    pub(crate) fn owned(
        lineage: u64,
        target: TargetIdentity,
        binding: &WidgetInstanceBinding,
        prior: Option<&Self>,
        classes: &AdmittedWidgetClasses,
    ) -> Result<Self, String> {
        if lineage == 0 || target.kind != TargetKind::Interface || target.incarnation == 0 {
            return Err("Macro owner is not an actual interface incarnation".into());
        }
        let class = classes
            .class(&binding.class.uuid)
            .ok_or("Macro Widget class is not admitted")?;
        if class.identity != binding.class {
            return Err("Macro owner class source receipt is stale".into());
        }
        if let Some(prior) = prior {
            if prior.lineage != lineage || prior.target != target || prior.class != binding.class {
                return Err("Macro cells belong to another live instance/source generation".into());
            }
            Ok(prior.clone())
        } else {
            Ok(Self {
                lineage,
                target,
                class: binding.class.clone(),
                cells: Default::default(),
            })
        }
    }
    pub(crate) fn lineage(&self) -> u64 {
        self.lineage
    }
    pub(crate) fn target(&self) -> &TargetIdentity {
        &self.target
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub(crate) fn class(&self) -> &WidgetClassIdentity {
        &self.class
    }
    pub(crate) fn retained_bytes(&self) -> Result<usize, String> {
        self.cells
            .len()
            .checked_mul(std::mem::size_of::<((u64, u64), Cell)>())
            .ok_or_else(|| "Macro retained storage overflow".into())
    }
    fn cell(&self, site: &AdmittedSite) -> Result<Option<&Cell>, String> {
        if self.class != site.class {
            return Err("Macro cell cannot use a different actual Widget class source".into());
        }
        let value = self.cells.get(&site.key());
        if value.is_some_and(|cell| cell.kind() != site.declaration.kind().unwrap()) {
            return Err("Macro retained cell family differs from actual expansion".into());
        }
        Ok(value)
    }
    pub(crate) fn operand_need(
        &self,
        site: &AdmittedSite,
        control: Control,
    ) -> Result<OperandNeed, String> {
        let kind = site.declaration.kind()?;
        kind.control_role(control)?;
        let cell = self.cell(site)?;
        Ok(match (kind, control, cell) {
            (Kind::DoOnce, Control::Enter, Some(Cell::DoOnce(s))) if !s.initialized() => {
                OperandNeed::StartClosed
            }
            (Kind::DoOnce, Control::Enter, None) => OperandNeed::StartClosed,
            (Kind::Gate, _, Some(Cell::Gate(s))) if !s.initialized() => OperandNeed::StartClosed,
            (Kind::Gate, _, None) => OperandNeed::StartClosed,
            (Kind::DoN, Control::Enter, _) => OperandNeed::Limit,
            _ => OperandNeed::None,
        })
    }
    /// The caller resolves its typed AST operand in the SAME Computation,
    /// then mutates the candidate cell BEFORE scheduling a recipient body.
    pub(crate) fn step(
        &mut self,
        site: &AdmittedSite,
        control: Control,
        operand: Operand,
    ) -> Result<Option<Output>, String> {
        let need = self.operand_need(site, control)?;
        if !matches!(
            (need, operand),
            (OperandNeed::None, Operand::None)
                | (OperandNeed::StartClosed, Operand::StartClosed(_))
                | (OperandNeed::Limit, Operand::Limit(_))
        ) {
            return Err("Macro operand was not sampled on its actual activation phase".into());
        }
        let kind = site.declaration.kind()?;
        if !self.cells.contains_key(&site.key())
            && self.cells.len() >= crate::value_limits::MAX_COLLECTION_ITEMS
        {
            return Err("Macro cells exceed the existing bounded collection limit".into());
        }
        let cell = self
            .cells
            .entry(site.key())
            .or_insert_with(|| Cell::fresh(kind));
        let start_closed = matches!(operand, Operand::StartClosed(true));
        Ok(match (cell, control) {
            (Cell::DoOnce(state), Control::Enter) => {
                state.enter(start_closed).then_some(Output::Completed)
            }
            (Cell::DoOnce(state), Control::Reset) => {
                state.reset();
                None
            }
            (Cell::DoN(state), Control::Enter) => {
                let Operand::Limit(limit) = operand else {
                    unreachable!()
                };
                state.enter(limit).then_some(Output::Exit)
            }
            (Cell::DoN(state), Control::Reset) => {
                state.reset();
                None
            }
            (Cell::FlipFlop(state), Control::Enter) => Some(match state.enter() {
                FlipFlopBranch::A => Output::A,
                FlipFlopBranch::B => Output::B,
            }),
            (Cell::Gate(state), control) => state
                .step(
                    match control {
                        Control::Enter => GateInput::Enter,
                        Control::Open => GateInput::Open,
                        Control::Close => GateInput::Close,
                        Control::Toggle => GateInput::Toggle,
                        Control::Reset => unreachable!(),
                    },
                    start_closed,
                )
                .then_some(Output::Exit),
            _ => unreachable!("kind/control checked before any cell insertion"),
        })
    }
    pub(crate) fn data(
        &self,
        site: &AdmittedSite,
        output: DataOutput,
        pin: u64,
    ) -> Result<Value, String> {
        if site.declaration.data_pin(output)? != pin {
            return Err("Macro data read lost its actual output PinId".into());
        }
        // A pure data read never initializes or inserts a cell.
        Ok(match (output, self.cell(site)?) {
            (DataOutput::Counter, Some(Cell::DoN(state))) => Value::Int(i64::from(state.counter())),
            (DataOutput::Counter, None) => Value::Int(0),
            (DataOutput::IsA, Some(Cell::FlipFlop(state))) => Value::Bool(state.is_a()),
            (DataOutput::IsA, None) => Value::Bool(false),
            _ => return Err("Macro data output differs from retained cell family".into()),
        })
    }
    pub(crate) fn inspect(&self, site: &AdmittedSite) -> Result<CellInspection, String> {
        let fresh = Cell::fresh(site.declaration.kind()?);
        Ok(match self.cell(site)?.unwrap_or(&fresh) {
            Cell::DoOnce(s) => CellInspection::DoOnce {
                initialized: s.initialized(),
                completed: s.completed(),
            },
            Cell::DoN(s) => CellInspection::DoN {
                initialized: s.initialized(),
                counter: s.counter(),
            },
            Cell::FlipFlop(s) => CellInspection::FlipFlop { is_a: s.is_a() },
            Cell::Gate(s) => CellInspection::Gate {
                initialized: s.initialized(),
                open: s.is_open(),
            },
        })
    }
}

// Portable debugger DATA only. ScreenInstance's cold GameSave omits its frame.
// Decoding these rows does not admit an owner: Engine routing and exact request
// equality must revalidate current target/class/source before replay or commit.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MacroFrameWire {
    lineage: u64,
    target: TargetIdentity,
    class: WidgetClassIdentity,
    cells: Vec<MacroCellWire>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MacroCellWire {
    graph: u64,
    node: u64,
    value: CellInspection,
}
impl Cell {
    fn inspection(&self) -> CellInspection {
        match self {
            Self::DoOnce(s) => CellInspection::DoOnce {
                initialized: s.initialized(),
                completed: s.completed(),
            },
            Self::DoN(s) => CellInspection::DoN {
                initialized: s.initialized(),
                counter: s.counter(),
            },
            Self::FlipFlop(s) => CellInspection::FlipFlop { is_a: s.is_a() },
            Self::Gate(s) => CellInspection::Gate {
                initialized: s.initialized(),
                open: s.is_open(),
            },
        }
    }
    fn from_inspection(value: CellInspection) -> Result<Self, String> {
        Ok(match value {
            CellInspection::DoOnce {
                initialized,
                completed,
            } => Self::DoOnce(
                DoOnceState::from_snapshot(initialized, completed).map_err(str::to_owned)?,
            ),
            CellInspection::DoN {
                initialized,
                counter,
            } => Self::DoN(DoNState::from_snapshot(initialized, counter).map_err(str::to_owned)?),
            CellInspection::FlipFlop { is_a } => Self::FlipFlop(FlipFlopState::from_snapshot(is_a)),
            CellInspection::Gate { initialized, open } => {
                Self::Gate(GateState::from_snapshot(initialized, open).map_err(str::to_owned)?)
            }
        })
    }
}
impl From<WidgetMacroFrame> for MacroFrameWire {
    fn from(frame: WidgetMacroFrame) -> Self {
        Self {
            lineage: frame.lineage,
            target: frame.target,
            class: frame.class,
            cells: frame
                .cells
                .into_iter()
                .map(|((graph, node), value)| MacroCellWire {
                    graph,
                    node,
                    value: value.inspection(),
                })
                .collect(),
        }
    }
}
impl TryFrom<MacroFrameWire> for WidgetMacroFrame {
    type Error = String;
    fn try_from(wire: MacroFrameWire) -> Result<Self, String> {
        if wire.lineage == 0
            || wire.target.kind != TargetKind::Interface
            || wire.target.incarnation == 0
            || wire.target.alias.is_empty()
            || wire.target.alias.len() > 128
            || wire.cells.len() > crate::value_limits::MAX_COLLECTION_ITEMS
        {
            return Err("Invalid inert macro debugger frame shape".into());
        }
        let mut cells = BTreeMap::new();
        for row in wire.cells {
            if row.graph == 0 || row.node == 0 || row.graph == u64::MAX || row.node == u64::MAX {
                return Err("Invalid macro debugger source identity".into());
            }
            if cells
                .insert((row.graph, row.node), Cell::from_inspection(row.value)?)
                .is_some()
            {
                return Err("Repeated macro debugger cell identity".into());
            }
        }
        Ok(Self {
            lineage: wire.lineage,
            target: wire.target,
            class: wire.class,
            cells,
        })
    }
}
