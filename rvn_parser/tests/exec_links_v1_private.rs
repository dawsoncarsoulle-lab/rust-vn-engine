//! Fragment/candidate codec tests only: these synthetic numeric unit records confer
//! no source/Graph identity, ownership, execution, native or pixel authority.
//! Include the actual shared AST signature implementation without editing F.
pub use rvn_parser::*;
#[path = "../src/scheduled_exec_links_v1.rs"]
mod codec;
#[path = "../src/scheduled_operand_contract.rs"]
mod scheduled_operand_contract;
use codec::{Domain, Error, Incoming, Input, Limits, Node, Output};

const LIMITS: Limits = Limits {
    bytes: 4096,
    links: 256,
    declarations: 1024,
};
const DOMAIN: Domain = Domain {
    source_ordinal: 0,
    definition_root_ordinal: 0,
    graph: 17,
};
fn config(value: &str) -> Statement {
    Statement::Config {
        key: "title".into(),
        value: value.into(),
    }
}
fn node(statement: &Statement, id: u64) -> Node<'_> {
    Node {
        domain: DOMAIN,
        node: id,
        kind: "config",
        statement,
    }
}
fn output(owner: Node<'_>, pin: u64) -> Output<'_> {
    Output {
        owner,
        role: "exec_out",
        pin,
    }
}
fn input<'a>(owner: Node<'a>, pin: u64, links: &'a [Incoming]) -> Input<'a> {
    Input {
        owner,
        role: "exec_in",
        pin,
        links,
    }
}
fn links() -> [Incoming; 2] {
    [
        Incoming {
            edge: 1,
            output_pin: 20,
        },
        Incoming {
            edge: 2,
            output_pin: 21,
        },
    ]
}
fn range(array: bool) -> Statement {
    let (identity, body) = if array {
        ("n2:for_each|exec_in=2@1,collection=3@0,body=4@0,completed=5@0,array_index=6@0,array_element=7@0",
         "for rvn_native_index_g17_n2 in [1,2] {}")
    } else {
        ("n2:for_loop_with_break|exec_in=2@1,first=3@0,last=4@0,break=5@0,body=6@0,index=7@0,completed=8@0",
         "flow.for_loop(rvn_native_index_g17_n2,0,2) {}")
    };
    let raw =
        format!("init{{flow.consume_v3(\"g17/n1:init|exec_out=1@0\",\"{identity}\"){{{body}}}}}");
    let mut script = rvn_parser::parse(&raw).unwrap();
    rvn_parser::scheduled_source::native_loop_contract::validate_authored_bindings(&script)
        .unwrap();
    let Statement::Init { mut body } = script.remove(0) else {
        unreachable!()
    };
    body.remove(0)
}

#[test]
fn codec_empty_and_sorted_multi_links_roundtrip_without_changing_inputs() {
    let s = config("target");
    let n = node(&s, 3);
    let before = links();
    assert_eq!(
        codec::decode("[]", n, "exec_in", LIMITS).unwrap(),
        Vec::<Incoming>::new()
    );
    assert_eq!(codec::encode(&[], n, "exec_in", LIMITS).unwrap(), "[]");
    assert_eq!(
        codec::decode("[1>20+2>21]", n, "exec_in", LIMITS).unwrap(),
        before
    );
    assert_eq!(
        codec::encode(&before, n, "exec_in", LIMITS).unwrap(),
        "[1>20+2>21]"
    );
    assert_eq!(before, links());
}

#[test]
fn codec_exact_byte_and_count_limits_apply_to_decode_and_encode() {
    let s = config("target");
    let n = node(&s, 3);
    let xs = links();
    let raw = "[1>20+2>21]";
    let exact = Limits {
        bytes: raw.len(),
        links: 2,
        declarations: 3,
    };
    assert!(codec::decode(raw, n, "exec_in", exact).is_ok());
    assert!(codec::encode(&xs, n, "exec_in", exact).is_ok());
    for encode in [false, true] {
        let low = Limits {
            bytes: raw.len() - 1,
            ..exact
        };
        let e = if encode {
            codec::encode(&xs, n, "exec_in", low).unwrap_err()
        } else {
            codec::decode(raw, n, "exec_in", low).unwrap_err()
        };
        assert!(matches!(
            e,
            Error::Budget {
                resource: "bytes",
                ..
            }
        ));
        let low = Limits { links: 1, ..exact };
        let e = if encode {
            codec::encode(&xs, n, "exec_in", low).unwrap_err()
        } else {
            codec::decode(raw, n, "exec_in", low).unwrap_err()
        };
        assert!(matches!(
            e,
            Error::Budget {
                resource: "links",
                ..
            }
        ));
    }
}

#[test]
fn codec_noncanonical_ids_and_bad_delimiters_are_explicit_refusals() {
    let s = config("target");
    let n = node(&s, 3);
    for raw in [
        "[0>20]",
        "[01>20]",
        "[+1>20]",
        "[1>0]",
        "[1>020]",
        "[18446744073709551615>20]",
        "[18446744073709551616>20]",
        "[1>18446744073709551615]",
        "[1>2\0]",
        "[ 1>20]",
        "[1> 20]",
        "[1>20+]",
        "[1>20++2>21]",
    ] {
        let e = codec::decode(raw, n, "exec_in", LIMITS).unwrap_err();
        assert!(
            matches!(e, Error::InvalidId { .. } | Error::Syntax { .. }),
            "{raw}: {e}"
        );
    }
    for raw in [
        "1",
        "0",
        "1~20",
        "[1]",
        "[1>20>21]",
        "[[1>20]]",
        "[1>20],",
        "[1>20]suffix",
    ] {
        assert!(
            matches!(
                codec::decode(raw, n, "exec_in", LIMITS),
                Err(Error::Syntax { .. }) | Err(Error::InvalidId { .. })
            ),
            "{raw}"
        );
    }
}

#[test]
fn codec_duplicate_or_unsorted_edges_and_duplicate_sources_are_not_repaired() {
    let s = config("target");
    let n = node(&s, 3);
    for raw in ["[1>20+1>21]", "[2>21+1>20]"] {
        assert!(matches!(
            codec::decode(raw, n, "exec_in", LIMITS),
            Err(Error::EdgeOrder { .. })
        ));
    }
    assert!(matches!(
        codec::decode("[1>20+2>20]", n, "exec_in", LIMITS),
        Err(Error::DuplicateOutput { output_pin: 20 })
    ));
    let wrong = [
        Incoming {
            edge: 2,
            output_pin: 21,
        },
        Incoming {
            edge: 1,
            output_pin: 20,
        },
    ];
    assert!(matches!(
        codec::encode(&wrong, n, "exec_in", LIMITS),
        Err(Error::EdgeOrder { .. })
    ));
}

#[test]
fn codec_actual_static_ast_contract_rejects_claimed_exec_tags_on_data_or_outputs() {
    let s = config("target");
    let n = node(&s, 3);
    for role in ["key", "value", "exec_out", "not_a_pin", "Exec_in"] {
        assert!(
            matches!(
                codec::decode("[]", n, role, LIMITS),
                Err(Error::WrongRole { .. })
            ),
            "{role}"
        );
    }
    let wrong = Node { kind: "scene", ..n };
    assert!(matches!(
        codec::decode("[]", wrong, "exec_in", LIMITS),
        Err(Error::Signature { .. })
    ));
    let unknown = Node {
        kind: "invented_exec_node",
        ..n
    };
    assert!(matches!(
        codec::decode("[]", unknown, "exec_in", LIMITS),
        Err(Error::UnsupportedSignature { .. })
    ));
    let branch = Statement::If {
        condition: Expr::Bool(true),
        then_branch: vec![],
        else_branch: vec![],
    };
    let unsupported = Node {
        statement: &branch,
        kind: "if",
        ..n
    };
    assert!(
        matches!(
            codec::decode("[]", unsupported, "exec_in", LIMITS),
            Err(Error::UnsupportedSignature { .. })
        ),
        "No parallel If signature table is invented"
    );
}

#[test]
fn codec_range_break_uses_the_existing_actual_v3_binding_contract() {
    let s = range(false);
    let n = Node {
        domain: DOMAIN,
        node: 2,
        kind: "for_loop_with_break",
        statement: &s,
    };
    assert_eq!(
        codec::decode("[1>20+2>21]", n, "break", LIMITS).unwrap(),
        links()
    );
    for role in ["first", "last", "index", "body", "completed"] {
        assert!(
            matches!(
                codec::decode("[]", n, role, LIMITS),
                Err(Error::WrongRole { .. })
            ),
            "{role}"
        );
    }
    let array = range(true);
    let a = Node {
        kind: "for_each",
        statement: &array,
        ..n
    };
    assert!(matches!(
        codec::decode("[]", a, "break", LIMITS),
        Err(Error::WrongRole { .. })
    ));
}

#[test]
fn codec_v3_owner_kind_and_real_port_contract_cannot_be_forged() {
    let mut s = range(false);
    {
        let n = Node {
            domain: DOMAIN,
            node: 2,
            kind: "for_loop_with_break",
            statement: &s,
        };
        assert!(matches!(
            codec::decode("[]", Node { node: 3, ..n }, "break", LIMITS),
            Err(Error::LoopOwner { .. })
        ));
        assert!(matches!(
            codec::decode(
                "[]",
                Node {
                    domain: Domain {
                        graph: 18,
                        ..DOMAIN
                    },
                    ..n
                },
                "break",
                LIMITS
            ),
            Err(Error::LoopOwner { .. })
        ));
    }
    let Statement::ScheduledConsumer { consumer, .. } = &mut s else {
        unreachable!()
    };
    *consumer = consumer.replace("break=5@0", "not_break=5@0");
    let n = Node {
        domain: DOMAIN,
        node: 2,
        kind: "for_loop_with_break",
        statement: &s,
    };
    assert!(matches!(
        codec::decode("[]", n, "break", LIMITS),
        Err(Error::Signature { .. })
    ));
}

#[test]
fn codec_index_accepts_distinct_ingress_and_identical_repeated_real_metadata() {
    let a = config("source-a");
    let b = config("source-b");
    let t = config("target");
    let xs = links();
    let outs = [output(node(&a, 1), 20), output(node(&b, 2), 21)];
    let target = input(node(&t, 3), 30, &xs);
    codec::validate_sets(&outs, &[target, target], LIMITS).unwrap();
    codec::validate_sets(&[outs[1], outs[0]], &[target], LIMITS).unwrap();
}

#[test]
fn codec_output_resolution_stays_in_the_exact_source_graph_root_domain() {
    let a = config("source");
    let t = config("target");
    let xs = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let target = input(node(&t, 3), 30, &xs);
    assert!(matches!(
        codec::validate_sets(&[], &[target], LIMITS),
        Err(Error::MissingOutput { pin: 20, .. })
    ));
    for domain in [
        Domain {
            source_ordinal: 1,
            ..DOMAIN
        },
        Domain {
            graph: 18,
            ..DOMAIN
        },
    ] {
        let out = output(
            Node {
                domain,
                ..node(&a, 1)
            },
            20,
        );
        assert!(matches!(
            codec::validate_sets(&[out], &[target], LIMITS),
            Err(Error::MissingOutput { pin: 20, .. })
        ));
    }
    let out = output(
        Node {
            domain: Domain {
                definition_root_ordinal: 1,
                ..DOMAIN
            },
            ..node(&a, 1)
        },
        20,
    );
    assert!(matches!(
        codec::validate_sets(&[out], &[target], LIMITS),
        Err(Error::GraphDefinitionCollision { .. })
    ));
}

#[test]
fn codec_same_local_ids_in_distinct_frozen_sources_are_not_collisions() {
    let a = config("source");
    let t = config("target");
    let xs = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let other = Domain {
        source_ordinal: 1,
        ..DOMAIN
    };
    let outs = [
        output(node(&a, 1), 20),
        output(
            Node {
                domain: other,
                ..node(&a, 1)
            },
            20,
        ),
    ];
    let ins = [
        input(node(&t, 3), 30, &xs),
        input(
            Node {
                domain: other,
                ..node(&t, 3)
            },
            30,
            &xs,
        ),
    ];
    codec::validate_sets(&outs, &ins, LIMITS).unwrap();
}

#[test]
fn codec_edge_reassignment_and_exec_output_fanout_refuse_neutrally() {
    let a = config("a");
    let b = config("b");
    let t = config("target");
    let u = config("other");
    let outs = [output(node(&a, 1), 20), output(node(&b, 2), 21)];
    let first = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let reassigned = [Incoming {
        edge: 1,
        output_pin: 21,
    }];
    let ins = [
        input(node(&t, 3), 30, &first),
        input(node(&u, 4), 31, &reassigned),
    ];
    let snapshot = first;
    assert!(matches!(
        codec::validate_sets(&outs, &ins, LIMITS),
        Err(Error::EdgeCollision { edge: 1, .. })
    ));
    let fanout = [Incoming {
        edge: 2,
        output_pin: 20,
    }];
    let ins = [
        input(node(&t, 3), 30, &first),
        input(node(&u, 4), 31, &fanout),
    ];
    assert!(matches!(
        codec::validate_sets(&outs, &ins, LIMITS),
        Err(Error::OutputFanout { pin: 20, .. })
    ));
    assert_eq!(first, snapshot);
}

#[test]
fn codec_pin_and_role_ownership_and_shared_node_semantics_are_not_guessed() {
    let a = config("a");
    let b = config("b");
    assert!(matches!(
        codec::validate_sets(
            &[output(node(&a, 1), 20), output(node(&b, 2), 20)],
            &[],
            LIMITS
        ),
        Err(Error::PinCollision { pin: 20, .. })
    ));
    assert!(matches!(
        codec::validate_sets(
            &[output(node(&a, 1), 20), output(node(&a, 1), 21)],
            &[],
            LIMITS
        ),
        Err(Error::RoleCollision { node: 1, .. })
    ));
    assert!(matches!(
        codec::validate_sets(
            &[output(node(&a, 1), 20), output(node(&b, 1), 20)],
            &[],
            LIMITS
        ),
        Err(Error::NodeCollision { node: 1, .. })
    ));
}

#[test]
fn codec_target_set_disagreement_and_same_node_connection_refuse() {
    let a = config("source");
    let b = config("source2");
    let t = config("target");
    let first = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let second = links();
    let outs = [output(node(&a, 1), 20), output(node(&b, 2), 21)];
    let ins = [
        input(node(&t, 3), 30, &first),
        input(node(&t, 3), 30, &second),
    ];
    assert!(matches!(
        codec::validate_sets(&outs, &ins, LIMITS),
        Err(Error::TargetSetCollision { pin: 30, .. })
    ));
    let out = output(node(&a, 1), 20);
    let target = input(node(&a, 1), 30, &first);
    assert!(matches!(
        codec::validate_sets(&[out], &[target], LIMITS),
        Err(Error::SelfConnection { node: 1, .. })
    ));
}

#[test]
fn codec_data_source_role_cannot_be_relabelled_as_an_execution_output() {
    let a = config("source");
    let t = config("target");
    let xs = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let out = Output {
        owner: node(&a, 1),
        role: "value",
        pin: 20,
    };
    assert!(matches!(
        codec::validate_sets(&[out], &[input(node(&t, 3), 30, &xs)], LIMITS),
        Err(Error::WrongRole { .. })
    ));
    let Statement::Config { value, .. } = &a else {
        unreachable!()
    };
    assert_eq!(value, "source");
}

#[test]
fn codec_caller_built_records_obey_id_list_and_declaration_budgets() {
    let a = config("source");
    let t = config("target");
    let xs = [Incoming {
        edge: 0,
        output_pin: 20,
    }];
    assert!(matches!(
        codec::validate_sets(
            &[output(node(&a, 1), 20)],
            &[input(node(&t, 3), 30, &xs)],
            LIMITS
        ),
        Err(Error::InvalidId { field: "edge", .. })
    ));
    let xs = [Incoming {
        edge: 1,
        output_pin: 20,
    }];
    let outs = [output(node(&a, 1), 20)];
    let ins = [input(node(&t, 3), 30, &xs)];
    assert!(matches!(
        codec::validate_sets(
            &outs,
            &ins,
            Limits {
                declarations: 1,
                ..LIMITS
            }
        ),
        Err(Error::Budget {
            resource: "declarations",
            ..
        })
    ));
    assert!(matches!(
        codec::validate_sets(&outs, &ins, Limits { bytes: 5, ..LIMITS }),
        Err(Error::Budget {
            resource: "bytes",
            ..
        })
    ));
    assert!(matches!(
        codec::decode(
            "[]",
            Node {
                node: 0,
                ..node(&t, 3)
            },
            "exec_in",
            LIMITS
        ),
        Err(Error::InvalidId { field: "node", .. })
    ));
}

#[test]
fn codec_is_inactive_and_does_not_reinterpret_legacy_source_or_metadata() {
    let source="init {set n=1}\nlabel start\nflow.consume_v2(\"opaque\",\"step\"){music.volume(\"0.25\")}\n\"done\"\n";
    let before = rvn_parser::parse_program(source).unwrap();
    let bytes = serde_json::to_vec(&before.script).unwrap();
    let s = config("target");
    assert!(codec::decode("1", node(&s, 3), "exec_in", LIMITS).is_err());
    assert_eq!(
        serde_json::to_vec(&rvn_parser::parse_program(source).unwrap().script).unwrap(),
        bytes
    );
    let json = r#"{"role":"exec_in","pin":2,"incoming":1}"#;
    let old: rvn_parser::scheduled_source::DeclaredPort = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&old).unwrap(), json);
    assert!(old.source_pin.is_none());
}
