//! Typed source declaration for a Widget Handler macro expansion.
//! A declaration is inert. Core admits it against the frozen physical Handler
//! and its actual Widget class before an owner cell can be accessed.
use super::{id, node, DeclaredEndpoint};
use crate::{Expr, Statement};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    DoOnce,
    DoN,
    FlipFlop,
    Gate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Control {
    Enter,
    Reset,
    Open,
    Close,
    Toggle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Output {
    Completed,
    Exit,
    A,
    B,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataOutput {
    Counter,
    IsA,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub root: DeclaredEndpoint,
    pub node: DeclaredEndpoint,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub output: Output,
    pub pin: u64,
    pub body: Vec<Statement>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub site: Site,
    pub control: Control,
    /// StartClosed for Enter DoOnce or any Gate control; N for Enter DoN.
    /// Sampling is lazy: the runtime asks the owner cell whether initialization
    /// is still needed before evaluating StartClosed.
    pub operand: Option<Expr>,
    /// Local source capability only; never retained producer values/owner state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_operand_v1: Option<DeferredOperandV1>,
    pub branches: Vec<Branch>,
}
impl Kind {
    pub fn node_kind(self) -> &'static str {
        match self {
            Self::DoOnce => "do_once",
            Self::DoN => "do_n",
            Self::FlipFlop => "flip_flop",
            Self::Gate => "gate",
        }
    }
    fn roles(self) -> &'static [&'static str] {
        match self {
            Self::DoOnce => &["exec_in", "reset", "start_closed", "completed"],
            Self::DoN => &["exec_in", "n", "reset", "exit", "counter"],
            Self::FlipFlop => &["exec_in", "a", "b", "is_a"],
            Self::Gate => &["exec_in", "open", "close", "toggle", "start_closed", "exit"],
        }
    }
    fn outputs(self) -> &'static [(Output, &'static str)] {
        match self {
            Self::DoOnce => &[(Output::Completed, "completed")],
            Self::DoN | Self::Gate => &[(Output::Exit, "exit")],
            Self::FlipFlop => &[(Output::A, "a"), (Output::B, "b")],
        }
    }
    pub fn control_role(self, control: Control) -> Result<&'static str, String> {
        match (self, control) {
            (_, Control::Enter) => Ok("exec_in"),
            (Self::DoOnce | Self::DoN, Control::Reset) => Ok("reset"),
            (Self::Gate, Control::Open) => Ok("open"),
            (Self::Gate, Control::Close) => Ok("close"),
            (Self::Gate, Control::Toggle) => Ok("toggle"),
            _ => Err("Control is absent from the actual macro signature".into()),
        }
    }
    pub fn operand_required(self, control: Control) -> bool {
        matches!(
            (self, control),
            (Self::DoOnce | Self::DoN, Control::Enter) | (Self::Gate, _)
        )
    }
}
// Infallible source formatting, deliberately not validation or authority.
// Preserve even invalid candidate ids/order rather than repairing/truncating;
// decode()/validate() remain mandatory before import or runtime admission.
fn endpoint_source(endpoint: &DeclaredEndpoint) -> String {
    let ports = endpoint
        .ports
        .iter()
        .map(|p| {
            let incoming = if let Some(links) = &p.exec_incoming {
                format!(
                    "[{}]",
                    links
                        .iter()
                        .map(|link| format!("{}>{}", link.edge, link.output_pin))
                        .collect::<Vec<_>>()
                        .join("+")
                )
            } else {
                format!(
                    "{}{}",
                    p.incoming.unwrap_or(0),
                    p.source_pin.map(|id| format!("~{id}")).unwrap_or_default()
                )
            };
            format!("{}={}@{}", p.role, p.pin, incoming)
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "n{}:{}{}|{}",
        endpoint.node,
        endpoint.kind,
        if endpoint.exec_links_v1 {
            "!exec_links_v1"
        } else {
            ""
        },
        ports
    )
}
impl Site {
    pub fn scope_source(&self) -> String {
        format!("g{}/{}", self.root.graph, endpoint_source(&self.root))
    }
    pub fn node_source(&self) -> String {
        endpoint_source(&self.node)
    }
    /// Reuses the existing declaration decoder. No second endpoint language.
    pub fn decode(scope: &str, expansion: &str) -> Result<Self, String> {
        let (graph, root) = scope
            .strip_prefix('g')
            .ok_or("Macro graph prefix")?
            .split_once('/')
            .ok_or("Macro actual source root missing")?;
        let graph = id(graph)?;
        let site = Self {
            root: node(root, graph)?,
            node: node(expansion, graph)?,
        };
        site.validate()?;
        Ok(site)
    }
    pub fn kind(&self) -> Result<Kind, String> {
        match self.node.kind.as_str() {
            "do_once" => Ok(Kind::DoOnce),
            "do_n" => Ok(Kind::DoN),
            "flip_flop" => Ok(Kind::FlipFlop),
            "gate" => Ok(Kind::Gate),
            _ => Err("Not a supported typed StandardMacro node".into()),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let kind = self.kind()?;
        if self.root.graph == 0
            || self.root.graph == u64::MAX
            || self.root.graph != self.node.graph
            || self.root.node == 0
            || self.node.node == 0
            || self.root.node == self.node.node
            || self.root.node == u64::MAX
            || self.node.node == u64::MAX
            || self.root.kind != "handler_entry"
            || self.root.exec_links_v1 != self.node.exec_links_v1
        {
            return Err("Macro requires its actual distinct Widget Handler root/node".into());
        }
        for endpoint in [&self.root, &self.node] {
            if endpoint.legacy_terminal.is_some()
                || endpoint.native_loop_output.is_some()
                || endpoint.legacy_string_sequence
            {
                return Err(
                    "Macro identity cannot borrow another feature's output projection".into(),
                );
            }
            let mut roles = BTreeSet::new();
            let mut pins = BTreeSet::new();
            for port in &endpoint.ports {
                if port.pin == 0
                    || port.pin == u64::MAX
                    || !roles.insert(port.role.as_str())
                    || !pins.insert(port.pin)
                {
                    return Err("Macro endpoint has invalid or repeated actual ports".into());
                }
            }
        }
        // Handler root may have real data argument outputs. Its unique exec
        // output and all data outputs must retain output direction semantics.
        if self
            .root
            .ports
            .iter()
            .filter(|p| p.role == "exec_out")
            .count()
            != 1
            || self.root.ports.iter().any(|p| {
                p.incoming.is_some() || p.source_pin.is_some() || p.exec_incoming.is_some()
            })
        {
            return Err("Macro root has no actual Handler execution output".into());
        }
        let actual = self
            .node
            .ports
            .iter()
            .map(|p| p.role.as_str())
            .collect::<BTreeSet<_>>();
        if actual != kind.roles().iter().copied().collect() {
            return Err("Macro signature differs from its typed roles".into());
        }
        let root_pins = self
            .root
            .ports
            .iter()
            .map(|p| p.pin)
            .collect::<BTreeSet<_>>();
        if self.node.ports.iter().any(|p| root_pins.contains(&p.pin)) {
            return Err("Macro pins alias its source root".into());
        }
        for port in &self.node.ports {
            let execution = matches!(
                port.role.as_str(),
                "exec_in" | "reset" | "open" | "close" | "toggle"
            );
            let input = execution || matches!(port.role.as_str(), "start_closed" | "n");
            if !input
                && (port.incoming.is_some()
                    || port.source_pin.is_some()
                    || port.exec_incoming.is_some())
                || !execution && port.exec_incoming.is_some()
                || execution && port.source_pin.is_some()
                || execution && self.node.exec_links_v1 != port.exec_incoming.is_some()
                || port.source_pin.is_some() && port.incoming.is_none()
            {
                return Err("Macro actual port direction/source declaration differs".into());
            }
        }
        Ok(())
    }
    pub fn data_pin(&self, output: DataOutput) -> Result<u64, String> {
        self.validate()?;
        let role = match (self.kind()?, output) {
            (Kind::DoN, DataOutput::Counter) => "counter",
            (Kind::FlipFlop, DataOutput::IsA) => "is_a",
            _ => return Err("Data output is absent from actual macro signature".into()),
        };
        Ok(self.node.ports.iter().find(|p| p.role == role).unwrap().pin)
    }
}
/// Strict defaults-only source slice; signed literals retain real int32 range.
pub fn literal_int32(value: &Expr) -> Result<i32, String> {
    let value = match value {
        Expr::Int(value) => *value,
        Expr::Neg(value) => match value.as_ref() {
            Expr::Int(value) => value.checked_neg().ok_or("Macro signed literal overflow")?,
            _ => return Err("Macro N default must be a signed literal".into()),
        },
        _ => return Err("Macro N default must be a signed literal".into()),
    };
    i32::try_from(value).map_err(|_| "Macro N default is outside signed int32".into())
}
impl Dispatch {
    pub fn validate(&self) -> Result<(), String> {
        self.site.validate()?;
        let kind = self.site.kind()?;
        let role = kind.control_role(self.control)?;
        let input = self
            .site
            .node
            .ports
            .iter()
            .find(|p| p.role == role)
            .unwrap();
        if input.incoming.is_none() && input.exec_incoming.as_ref().is_none_or(Vec::is_empty) {
            return Err("Dispatch has no actual incoming execution edge".into());
        }
        if self.operand.is_some() != kind.operand_required(self.control) {
            return Err("Macro control operand arity differs from signature".into());
        }
        if self.deferred_operand_v1.is_some() && self.operand.is_none() {
            return Err("Deferred macro plan has no actual operand".into());
        }
        if let Some(value) = &self.operand {
            let key = if kind == Kind::DoN {
                "n"
            } else {
                "start_closed"
            };
            let port = self.site.node.ports.iter().find(|p| p.role == key).unwrap();
            if let Some(plan) = &self.deferred_operand_v1 {
                if port.incoming.is_none() || port.source_pin.is_none() {
                    return Err("Deferred macro operand requires its actual connected input/output pins".into());
                }
                plan.validate(&self.site, port.source_pin.unwrap(), value)?;
            } else if port.incoming.is_some() || port.source_pin.is_some() {
                return Err("Connected macro operand requires its deferred source plan".into());
            } else if key == "n" {
                literal_int32(value)?;
            } else if !matches!(value, Expr::Bool(_)) {
                return Err("Macro StartClosed default must retain its Boolean literal".into());
            }
        }
        let expected = if self.control == Control::Enter {
            kind.outputs()
        } else {
            &[]
        };
        if self.branches.len() != expected.len() {
            return Err("Macro execution output count differs".into());
        }
        for (branch, (output, role)) in self.branches.iter().zip(expected) {
            if branch.output != *output
                || self
                    .site
                    .node
                    .ports
                    .iter()
                    .find(|p| p.role == *role)
                    .unwrap()
                    .pin
                    != branch.pin
            {
                return Err("Macro execution branch is not its actual output pin".into());
            }
        }
        Ok(())
    }
}

/// Versioned only at the local operand syntax/field, not a Graph/Save schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeferredOperandV1 {
    pub steps: Vec<crate::ConsumerStep>,
}
impl Dispatch {
    pub fn operand_steps(&self) -> &[crate::ConsumerStep] {
        self.deferred_operand_v1.as_ref().map_or(&[], |plan| plan.steps.as_slice())
    }
    /// All immutable expression fields for bounds/reference/admission visitors.
    /// Evaluation must still use operand_need before touching any of them.
    pub fn operand_expressions(&self) -> Vec<&Expr> {
        self.operand_steps().iter().map(|step| match step {
            crate::ConsumerStep::Capture { value, .. }
            | crate::ConsumerStep::InspectTextTable { value, .. } => value,
        }).chain(self.operand.iter()).collect()
    }
}
impl DeferredOperandV1 {
    pub fn validate(&self, site: &Site, source_pin: u64, value: &Expr) -> Result<(), String> {
        use std::collections::{BTreeMap, BTreeSet};
        if self.steps.len() > 1024 { return Err("Deferred macro exceeds1024 producers".into()); }
        let mut captures = BTreeSet::new();
        let mut inspectors = BTreeSet::new();
        let mut endpoints = BTreeMap::new();
        let mut work = 0usize;
        for step in &self.steps {
            let (id, expression, inspect) = match step {
                crate::ConsumerStep::Capture { producer, value } => (producer, value, false),
                crate::ConsumerStep::InspectTextTable { producer, value } => (producer, value, true),
            };
            if id.is_empty() || id.len() > 256 || captures.contains(id) || inspectors.contains(id) {
                return Err("Deferred macro has invalid/duplicate producer identity".into());
            }
            let raw = if let Some(rest) = id.strip_prefix("p:") {
                let (role, raw) = rest.split_once('/').ok_or("Deferred legacy producer projection")?;
                if inspect || !matches!(role, "valid" | "table_id" | "key") { return Err("Deferred legacy terminal differs".into()); }
                raw
            } else { id.as_str() };
            let endpoint = node(raw, site.node.graph)?;
            if endpoint.exec_links_v1 != site.root.exec_links_v1 || endpoint.node == site.root.node {
                return Err("Deferred producer differs from its actual source graph/root".into());
            }
            if inspect && endpoint.kind != "get_string_table_id_and_key" { return Err("Deferred inspect is not its actual inspector".into()); }
            deferred_refs(expression, &captures, &inspectors, 0, &mut work)?;
            endpoints.insert(id.clone(), endpoint);
            if inspect { inspectors.insert(id.clone()); } else { captures.insert(id.clone()); }
        }
        deferred_refs(value, &captures, &inspectors, 0, &mut work)?;
        // The existing DAG analysis is read-only and carries no loop/frame
        // ownership. All gathered steps must belong to this one actual input.
        if crate::loop_phase_dependencies::capture_step_indices(&self.steps,value)? != (0..self.steps.len()).collect::<Vec<_>>() {
            return Err("Deferred macro contains a producer outside its actual input dependency DAG".into());
        }
        match value {
            Expr::Call { name, args } if name == "__rvn_captured_v1" => {
                let [Expr::Str(id)] = args.as_slice() else { return Err("Deferred terminal capture arity".into()); };
                let endpoint = endpoints.get(id).ok_or("Deferred terminal producer missing")?;
                if !endpoint.ports.iter().any(|port| port.pin == source_pin && port.incoming.is_none() && port.exec_incoming.is_none()) {
                    return Err("Deferred terminal lost its actual source output PinId".into());
                }
            }
            Expr::Call { name, args } if name == "__rvn_terminal_v1" => {
                let [Expr::Str(id), Expr::Str(role)] = args.as_slice() else { return Err("Deferred terminal inspector arity".into()); };
                let endpoint = endpoints.get(id).ok_or("Deferred inspector missing")?;
                if !inspectors.contains(id) || !endpoint.ports.iter().any(|port| port.pin == source_pin && port.role == *role) {
                    return Err("Deferred inspector lost its actual output PinId".into());
                }
            }
            Expr::StandardMacroData { site: read, output, pin } => {
                if read.root != site.root || read.data_pin(*output)? != *pin || *pin != source_pin {
                    return Err("Deferred live read changed its actual owner/source pin".into());
                }
            }
            _ => return Err("Deferred operand must select its actual connected producer output".into()),
        }
        Ok(())
    }
}
fn deferred_refs(value: &Expr, captures: &BTreeSet<String>, inspectors: &BTreeSet<String>, depth: usize, work: &mut usize) -> Result<(), String> {
    if depth > 256 || *work >= 100_000 { return Err("Deferred operand expression budget".into()); }
    *work += 1;
    match value {
        Expr::LibraryCall{call}=>for(_,value)in call.arguments(){deferred_refs(value,captures,inspectors,depth+1,work)?;},
        Expr::Call { name, args } => {
            if name == "__rvn_captured_v1" && !matches!(args.as_slice(), [Expr::Str(id)] if captures.contains(id)) {
                return Err("Deferred capture requires an earlier actual producer".into());
            }
            if name == "__rvn_terminal_v1" && !matches!(args.as_slice(), [Expr::Str(id), Expr::Str(role)] if inspectors.contains(id) && matches!(role.as_str(), "valid" | "table_id" | "key")) {
                return Err("Deferred terminal requires an earlier actual inspector".into());
            }
            for value in args { deferred_refs(value, captures, inspectors, depth + 1, work)?; }
        }
        Expr::ListLit(values) => for value in values { deferred_refs(value, captures, inspectors, depth + 1, work)?; },
        Expr::Neg(value) | Expr::Not(value) => deferred_refs(value, captures, inspectors, depth + 1, work)?,
        Expr::BinOp { left, right, .. } | Expr::And(left, right) | Expr::Or(left, right) => {
            deferred_refs(left, captures, inspectors, depth + 1, work)?;
            deferred_refs(right, captures, inspectors, depth + 1, work)?;
        }
        Expr::Index { target, index } => { deferred_refs(target, captures, inspectors, depth + 1, work)?; deferred_refs(index, captures, inspectors, depth + 1, work)?; }
        Expr::StandardMacroData { site, output, pin } => { if site.data_pin(*output)? != *pin { return Err("Deferred live macro read pin differs".into()); } }
        _ => {}
    }
    Ok(())
}
