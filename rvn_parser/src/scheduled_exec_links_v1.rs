//! Versioned ExecLinksV1 list fragment codec; never source or owner authority.
//! This validates candidates against actual AST signatures, never source or owner
//! authority. Full frozen-source/Graph reconstruction and legacy guards remain
//! mandatory. Limits below are local call limits, not expanded runtime budgets.
use crate::{scheduled_operand_contract, scheduled_source::native_loop_contract, Statement};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub bytes: usize,
    pub links: usize,
    pub declarations: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incoming {
    pub edge: u64,
    pub output_pin: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Domain {
    pub source_ordinal: usize,
    pub definition_root_ordinal: usize,
    pub graph: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    pub domain: Domain,
    pub node: u64,
    pub kind: &'a str,
    pub statement: &'a Statement,
}

#[derive(Clone, Copy, Debug)]
pub struct Output<'a> {
    pub owner: Node<'a>,
    pub role: &'a str,
    pub pin: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    pub owner: Node<'a>,
    pub role: &'a str,
    pub pin: u64,
    pub links: &'a [Incoming],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Budget {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    Syntax {
        item: String,
        reason: &'static str,
    },
    InvalidId {
        field: &'static str,
        value: String,
    },
    UnsupportedSignature {
        kind: String,
    },
    Signature {
        kind: String,
        reason: String,
    },
    WrongRole {
        kind: String,
        role: String,
        direction: &'static str,
    },
    LoopOwner {
        domain: Domain,
        node: u64,
    },
    EdgeOrder {
        previous: u64,
        edge: u64,
    },
    DuplicateOutput {
        output_pin: u64,
    },
    GraphDefinitionCollision {
        source_ordinal: usize,
        graph: u64,
    },
    NodeCollision {
        domain: Domain,
        node: u64,
    },
    RoleCollision {
        domain: Domain,
        node: u64,
        role: String,
    },
    PinCollision {
        domain: Domain,
        pin: u64,
    },
    TargetSetCollision {
        domain: Domain,
        pin: u64,
    },
    EdgeCollision {
        domain: Domain,
        edge: u64,
    },
    MissingOutput {
        domain: Domain,
        pin: u64,
    },
    OutputFanout {
        domain: Domain,
        pin: u64,
    },
    SelfConnection {
        domain: Domain,
        node: u64,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecLinksV1 candidate: {self:?}")
    }
}
impl std::error::Error for Error {}

fn bound(resource: &'static str, actual: usize, limit: usize) -> Result<(), Error> {
    if actual > limit {
        Err(Error::Budget {
            resource,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}
fn checked_id(value: u64, field: &'static str) -> Result<u64, Error> {
    if value == 0 || value == u64::MAX {
        Err(Error::InvalidId {
            field,
            value: value.to_string(),
        })
    } else {
        Ok(value)
    }
}
// Numeric list tokens use the existing canonical positive-u64 convention. This
// is not a second NodeId decoder: no node/kind/scope/header is parsed here.
fn number(raw: &str, field: &'static str) -> Result<u64, Error> {
    let value = raw.parse::<u64>().map_err(|_| Error::InvalidId {
        field,
        value: raw.into(),
    })?;
    checked_id(value, field)?;
    if value.to_string() != raw {
        return Err(Error::InvalidId {
            field,
            value: raw.into(),
        });
    }
    Ok(value)
}

#[derive(Clone, Copy)]
enum Direction {
    Input,
    Output,
}
fn signature(node: Node<'_>, role: &str, direction: Direction) -> Result<(), Error> {
    checked_id(node.domain.graph, "graph")?;
    checked_id(node.node, "node")?;
    // Reuse the actual AST-to-static-port contract. A source-supplied category
    // or an exec-looking role cannot make a data port an execution port.
    if scheduled_operand_contract::static_consumer_contract(node.kind).is_some() {
        let ports =
            scheduled_operand_contract::actual_static_consumer_ports(node.statement, node.kind)
                .map_err(|reason| Error::Signature {
                    kind: node.kind.into(),
                    reason,
                })?;
        let exists = ports.iter().any(|known| known == role);
        let valid = exists
            && match direction {
                Direction::Input => role == "exec_in",
                Direction::Output => {
                    role == "exec_out"
                        || role == "completed"
                        || (node.kind == "imagemap" && role.starts_with("hotspot_"))
                }
            };
        if valid {
            return Ok(());
        }
    } else if matches!(
        node.statement,
        Statement::ScheduledConsumer { version: 3, .. }
    ) {
        // The existing v3 decoder validates actual root/kind/ports/index/body;
        // it is not reconstructed from strings or from a claimed Exec tag.
        let binding = native_loop_contract::binding(node.statement)
            .map_err(|reason| Error::Signature {
                kind: node.kind.into(),
                reason,
            })?
            .ok_or_else(|| Error::UnsupportedSignature {
                kind: node.kind.into(),
            })?;
        let actual_kind = match binding.kind {
            native_loop_contract::Kind::Array => "for_each",
            native_loop_contract::Kind::Range => "for_loop_with_break",
        };
        if binding.graph != node.domain.graph
            || binding.node != node.node
            || actual_kind != node.kind
        {
            return Err(Error::LoopOwner {
                domain: node.domain,
                node: node.node,
            });
        }
        let valid = match direction {
            Direction::Input => {
                role == "exec_in"
                    || (binding.kind == native_loop_contract::Kind::Range && role == "break")
            }
            Direction::Output => role == "body" || role == "completed",
        };
        if valid {
            return Ok(());
        }
    } else {
        return Err(Error::UnsupportedSignature {
            kind: node.kind.into(),
        });
    }
    Err(Error::WrongRole {
        kind: node.kind.into(),
        role: role.into(),
        direction: match direction {
            Direction::Input => "Execution/Input",
            Direction::Output => "Execution/Output",
        },
    })
}

fn list(links: &[Incoming], limits: Limits) -> Result<(), Error> {
    bound("links", links.len(), limits.links)?;
    let mut previous = 0;
    let mut outputs = BTreeSet::new();
    for link in links {
        checked_id(link.edge, "edge")?;
        checked_id(link.output_pin, "output_pin")?;
        if link.edge <= previous {
            return Err(Error::EdgeOrder {
                previous,
                edge: link.edge,
            });
        }
        if !outputs.insert(link.output_pin) {
            return Err(Error::DuplicateOutput {
                output_pin: link.output_pin,
            });
        }
        previous = link.edge;
    }
    Ok(())
}

pub fn decode(
    raw: &str,
    owner: Node<'_>,
    role: &str,
    limits: Limits,
) -> Result<Vec<Incoming>, Error> {
    bound("bytes", raw.len(), limits.bytes)?;
    signature(owner, role, Direction::Input)?;
    decode_fragment(raw, limits)
}

/// Syntax only. Identity readers must additionally verify the real input role
/// against the existing AST/Graph contract before admitting any source.
/// This factoring shares the original codec; it is not a type/owner grant.
pub fn decode_fragment(raw: &str, limits: Limits) -> Result<Vec<Incoming>, Error> {
    bound("bytes", raw.len(), limits.bytes)?;
    let inner = raw
        .strip_prefix('[')
        .and_then(|raw| raw.strip_suffix(']'))
        .ok_or_else(|| Error::Syntax {
            item: raw.into(),
            reason: "Expected one bracketed Exec list; legacy scalar is not this format",
        })?;
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    // Count before allocating records. Local byte and count limits both apply.
    let count = inner.bytes().filter(|byte| *byte == b'+').count() + 1;
    bound("links", count, limits.links)?;
    let mut result = Vec::new();
    for item in inner.split('+') {
        let (edge, output) = item.split_once('>').ok_or_else(|| Error::Syntax {
            item: item.into(),
            reason: "Expected exactly edge>output_pin",
        })?;
        if output.contains('>') {
            return Err(Error::Syntax {
                item: item.into(),
                reason: "Extra endpoint separator",
            });
        }
        result.push(Incoming {
            edge: number(edge, "edge")?,
            output_pin: number(output, "output_pin")?,
        });
    }
    list(&result, limits)?;
    Ok(result)
}

pub fn encode(
    links: &[Incoming],
    owner: Node<'_>,
    role: &str,
    limits: Limits,
) -> Result<String, Error> {
    signature(owner, role, Direction::Input)?;
    encode_fragment(links, limits)
}

/// Syntax only, for an existing reader/writer with actual catalogue/AST proof.
pub fn encode_fragment(links: &[Incoming], limits: Limits) -> Result<String, Error> {
    list(links, limits)?;
    // Measure exact ASCII bytes before allocating output. No truncation or
    // sorting of invalid candidate sets silently repairs source evidence.
    let digits = |value: u64| value.ilog10() as usize + 1;
    let bytes = links
        .iter()
        .try_fold(2usize, |size, link| {
            size.checked_add(digits(link.edge) + digits(link.output_pin) + 1)
        })
        .and_then(|size| size.checked_add(links.len().saturating_sub(1)))
        .ok_or(Error::Budget {
            resource: "bytes",
            limit: limits.bytes,
            actual: usize::MAX,
        })?;
    bound("bytes", bytes, limits.bytes)?;
    let mut raw = String::with_capacity(bytes);
    raw.push('[');
    for (index, link) in links.iter().enumerate() {
        if index != 0 {
            raw.push('+');
        }
        use std::fmt::Write;
        write!(raw, "{}>{}", link.edge, link.output_pin).expect("Writing to String");
    }
    raw.push(']');
    Ok(raw)
}

// Pure collision/endpoint consistency check over borrowed declarations. These
// are candidate records, not runtime ownership or trusted source receipts.
// Forward topology references work because all output declarations are indexed
// before any incoming set is resolved. This never admits forward data reads.
pub fn validate_sets<'a>(
    outputs: &[Output<'a>],
    inputs: &[Input<'a>],
    limits: Limits,
) -> Result<(), Error> {
    let declarations = outputs
        .len()
        .checked_add(inputs.len())
        .ok_or(Error::Budget {
            resource: "declarations",
            limit: limits.declarations,
            actual: usize::MAX,
        })?;
    bound("declarations", declarations, limits.declarations)?;
    let mut roots: BTreeMap<(usize, u64), usize> = BTreeMap::new();
    let mut nodes: BTreeMap<(Domain, u64), (&str, &Statement)> = BTreeMap::new();
    let mut roles: BTreeMap<(Domain, u64, &str), u64> = BTreeMap::new();
    let mut pins: BTreeMap<(Domain, u64), (u64, &str, bool)> = BTreeMap::new();
    let mut actual_outputs: BTreeMap<(Domain, u64), u64> = BTreeMap::new();
    fn admit_node<'a>(
        roots: &mut BTreeMap<(usize, u64), usize>,
        nodes: &mut BTreeMap<(Domain, u64), (&'a str, &'a Statement)>,
        node: Node<'a>,
    ) -> Result<(), Error> {
        let graph_key = (node.domain.source_ordinal, node.domain.graph);
        if roots
            .get(&graph_key)
            .is_some_and(|old| *old != node.domain.definition_root_ordinal)
        {
            return Err(Error::GraphDefinitionCollision {
                source_ordinal: node.domain.source_ordinal,
                graph: node.domain.graph,
            });
        }
        roots.insert(graph_key, node.domain.definition_root_ordinal);
        let value = (node.kind, node.statement);
        if nodes
            .get(&(node.domain, node.node))
            .is_some_and(|old| *old != value)
        {
            return Err(Error::NodeCollision {
                domain: node.domain,
                node: node.node,
            });
        }
        nodes.insert((node.domain, node.node), value);
        Ok(())
    }
    fn admit_pin<'a>(
        roles: &mut BTreeMap<(Domain, u64, &'a str), u64>,
        pins: &mut BTreeMap<(Domain, u64), (u64, &'a str, bool)>,
        node: Node<'_>,
        pin: u64,
        role: &'a str,
        output: bool,
    ) -> Result<(), Error> {
        checked_id(pin, "pin")?;
        let role_key = (node.domain, node.node, role);
        if roles.get(&role_key).is_some_and(|old| *old != pin) {
            return Err(Error::RoleCollision {
                domain: node.domain,
                node: node.node,
                role: role.into(),
            });
        }
        roles.insert(role_key, pin);
        let value = (node.node, role, output);
        if pins
            .get(&(node.domain, pin))
            .is_some_and(|old| *old != value)
        {
            return Err(Error::PinCollision {
                domain: node.domain,
                pin,
            });
        }
        pins.insert((node.domain, pin), value);
        Ok(())
    }
    for output in outputs {
        signature(output.owner, output.role, Direction::Output)?;
        admit_node(&mut roots, &mut nodes, output.owner)?;
        admit_pin(
            &mut roles,
            &mut pins,
            output.owner,
            output.pin,
            output.role,
            true,
        )?;
        actual_outputs.insert((output.owner.domain, output.pin), output.owner.node);
    }
    let mut targets: BTreeMap<(Domain, u64), &[Incoming]> = BTreeMap::new();
    let mut edges: BTreeMap<(Domain, u64), (u64, u64)> = BTreeMap::new();
    let mut successors: BTreeMap<(Domain, u64), (u64, u64)> = BTreeMap::new();
    for input in inputs {
        signature(input.owner, input.role, Direction::Input)?;
        admit_node(&mut roots, &mut nodes, input.owner)?;
        admit_pin(
            &mut roles,
            &mut pins,
            input.owner,
            input.pin,
            input.role,
            false,
        )?;
        list(input.links, limits)?;
        // This also enforces the exact per-list byte budget for caller-built
        // candidates, not only candidates that came through decode().
        encode(input.links, input.owner, input.role, limits)?;
        if targets
            .get(&(input.owner.domain, input.pin))
            .is_some_and(|old| *old != input.links)
        {
            return Err(Error::TargetSetCollision {
                domain: input.owner.domain,
                pin: input.pin,
            });
        }
        targets.insert((input.owner.domain, input.pin), input.links);
        for link in input.links {
            let source_node = actual_outputs
                .get(&(input.owner.domain, link.output_pin))
                .ok_or(Error::MissingOutput {
                    domain: input.owner.domain,
                    pin: link.output_pin,
                })?;
            if *source_node == input.owner.node {
                return Err(Error::SelfConnection {
                    domain: input.owner.domain,
                    node: *source_node,
                });
            }
            let endpoints = (link.output_pin, input.pin);
            if edges
                .get(&(input.owner.domain, link.edge))
                .is_some_and(|old| *old != endpoints)
            {
                return Err(Error::EdgeCollision {
                    domain: input.owner.domain,
                    edge: link.edge,
                });
            }
            let successor = (link.edge, input.pin);
            if successors
                .get(&(input.owner.domain, link.output_pin))
                .is_some_and(|old| *old != successor)
            {
                return Err(Error::OutputFanout {
                    domain: input.owner.domain,
                    pin: link.output_pin,
                });
            }
            edges.insert((input.owner.domain, link.edge), endpoints);
            successors.insert((input.owner.domain, link.output_pin), successor);
        }
    }
    Ok(())
}

/// Shared structural recognition for this first source adapter. Static
/// consumer roles reuse the existing shared contract; structural flow roles
/// are checked again against the actual Graph catalogue on full reconstruction.
/// Unknown dynamic consumers are explicit gaps, never Exec tags supplied by a
/// source. This function alone grants no AST/Graph/runtime authority.
pub fn is_execution_input_role(kind: &str, role: &str) -> bool {
    if matches!(kind,"do_once"|"do_n"|"flip_flop"|"gate") {
        return match kind {
            "do_once"|"do_n"=>matches!(role,"exec_in"|"reset"),
            "flip_flop"=>role=="exec_in",
            "gate"=>matches!(role,"exec_in"|"open"|"close"|"toggle"),_=>false,
        };
    }
    if role == "break" {
        return kind == "for_loop_with_break";
    }
    role == "exec_in"
        && (scheduled_operand_contract::static_consumer_contract(kind)
            .is_some_and(|contract| contract.ports.contains(&role))
            || matches!(
                kind,
                "set_variable"
                    | "local_variable"
                    | "if"
                    | "while"
                    | "for_each"
                    | "for_loop_with_break"
                    | "sequence"
                    | "choice"
                    | "branch_end"
                    | "function_return"
                    | "return"
            ))
}

pub fn is_execution_output_role(kind: &str, role: &str) -> bool {
    if matches!(kind,"do_once"|"do_n"|"flip_flop"|"gate") {
        return match kind {"do_once"=>role=="completed","do_n"|"gate"=>role=="exit",
            "flip_flop"=>matches!(role,"a"|"b"),_=>false};
    }
    if role == "exec_out" {
        return matches!(
            kind,
            "init"
                | "label"
                | "function_entry"
                | "screen_entry"
                | "handler_entry"
                | "set_variable"
                | "local_variable"
        ) || scheduled_operand_contract::static_consumer_contract(kind)
            .is_some_and(|contract| contract.ports.contains(&role));
    }
    match kind {
        "if" => matches!(role, "then" | "else" | "completed"),
        "while" | "for_each" | "for_loop_with_break" => matches!(role, "body" | "completed"),
        "sequence" => {
            role == "completed"
                || role.strip_prefix("then_").is_some_and(|n| {
                    n.parse::<usize>()
                        .is_ok_and(|index| index < 32 && index.to_string() == n)
                })
        }
        "choice" => {
            role == "completed"
                || role.strip_prefix("option_").is_some_and(|n| {
                    n.parse::<usize>()
                        .is_ok_and(|index| index < 65_536 && index.to_string() == n)
                })
        }
        "imagemap" => {
            role == "completed"
                || role.strip_prefix("hotspot_").is_some_and(|n| {
                    n.parse::<usize>()
                        .is_ok_and(|index| index < 65_536 && index.to_string() == n)
                })
        }
        _ => false,
    }
}
