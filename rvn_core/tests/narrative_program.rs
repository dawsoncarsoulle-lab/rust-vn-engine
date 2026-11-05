use rvn_core::{
    program::{AstRoutePart, BranchKind, GeneratedRole, InstructionOrigin, PreparedNarrative},
    scene::ResumePosition,
};
use rvn_parser::{
    parse, parse_file_program_with_uses, parse_program, Expr, NarrativeContext, Statement,
};
use std::{collections::HashSet, fs, path::PathBuf};

fn prepared(source: &str) -> PreparedNarrative {
    PreparedNarrative::from_parsed(parse_program(source).unwrap()).unwrap()
}

#[test]
fn only_explicit_entries_create_scene_entries_and_authored_reserved_looking_helpers_stay_authored()
{
    let program = prepared("label __internal_choice_1\nreturn\nscene label intro\ncall __internal_choice_1\nscene forest\nlabel __script_end\nreturn\n");
    assert_eq!(program.scene_entries.len(), 1);
    let pc = program.label_table["intro"];
    assert_eq!(
        program.scene_entries.get(&pc).map(String::as_str),
        Some("intro")
    );
    for name in ["__internal_choice_1", "__script_end"] {
        let pc = program.label_table[name];
        assert!(!program.scene_entries.contains_key(&pc));
        assert!(!program.generated_targets.contains(name));
        assert!(program.origins[pc].generated.is_none());
    }
    assert!(
        PreparedNarrative::prepare(parse("label intro\nscene forest\nreturn\n").unwrap()).is_err()
    );
    assert!(program.origins.iter().all(|origin| origin.graph.is_none()));
}

#[test]
fn preparation_refuses_duplicate_labels_nested_entries_init_locals_and_unresolved_uses() {
    for source in [
        "scene label intro\nlabel intro\n",
        "scene label intro\nscene label intro\n",
        "scene label intro\nif true { label helper }\nlabel helper\n",
    ] {
        let script = parse(source).unwrap();
        let snapshot = script.clone();
        assert!(PreparedNarrative::prepare(script).is_err());
        assert_eq!(parse(source).unwrap(), snapshot);
    }
    let entry = Statement::SceneLabel {
        name: "intro".into(),
    };
    for invalid in [
        Statement::Init {
            body: vec![Statement::LocalVar {
                name: "n".into(),
                value: Expr::Int(1),
            }],
        },
        Statement::If {
            condition: Expr::Bool(true),
            then_branch: vec![Statement::SceneLabel {
                name: "nested".into(),
            }],
            else_branch: vec![],
        },
        Statement::Use {
            paths: vec!["missing.rvn".into()],
        },
        Statement::Label {
            name: "bad\nlabel injected".into(),
        },
    ] {
        assert!(PreparedNarrative::prepare(vec![entry.clone(), invalid]).is_err());
    }
}

#[test]
fn lowering_preserves_lexical_bodies_and_gives_each_narrative_branch_real_provenance() {
    let source = "init { for x in [1] { set total = x } }\nfunction f(items) { for x in items { local n = x } return 1 }\nscreen s() { while false { local x = 1 } return 2 }\nhandler h() { if true { local x = 1 } return 3 }\nscene label intro\nif true { set n = 1 } else { set n = 2 }\nchoice { \"a\" => { set n = 3 } \"b\" => { set n = 4 } }\nimagemap { background: \"map.png\" hotspot { area: (0,0,20,20) } => { set n = 5 } hotspot { area: (20,0,40,20) } => { set n = 6 } }\nwhile false { set n = 7 }\nfor x in [1,2] { set n = x }\nreturn\n";
    let parsed = parse_program(source).unwrap();
    let lexical = parsed.script[..4].to_vec();
    let program = PreparedNarrative::from_parsed(parsed).unwrap();
    assert_eq!(&program.script[..4], lexical.as_slice());
    assert_eq!(program.branch_returns.len(), 8);
    assert_eq!(program.generated_targets.len(), 9); // eight branches and the end guard
    for pc in &program.branch_returns {
        assert!(matches!(program.script[*pc], Statement::Return));
        assert!(matches!(
            program.origins[*pc].generated,
            Some(GeneratedRole::BranchReturn { .. })
        ));
        assert!(program.origins[*pc].span.is_some());
    }
    let authored_return = program
        .origins
        .iter()
        .position(|origin| {
            origin.route == [AstRoutePart::Root { index: 10 }] && origin.generated.is_none()
        })
        .unwrap();
    assert!(matches!(program.script[authored_return], Statement::Return));
    assert!(!program.branch_returns.contains(&authored_return));
    for (branch, ordinal) in [
        (BranchKind::Choice, 0),
        (BranchKind::Choice, 1),
        (BranchKind::Hotspot, 0),
        (BranchKind::Hotspot, 1),
    ] {
        assert!(
            program
                .origins
                .iter()
                .any(|origin| origin.generated
                    == Some(GeneratedRole::BranchLabel { branch, ordinal }))
        );
    }
}

#[test]
fn foreach_collection_cursor_and_lowered_advance_are_owned_temporaries_but_binder_keeps_set_semantics(
) {
    let program =
        prepared("scene label intro\nlocal item = 10\nfor item in [1,2] { \"pause\" }\nreturn\n");
    assert_eq!(program.temporary_writes.len(), 3);
    let names: HashSet<_> = program.temporary_writes.values().collect();
    assert_eq!(names.len(), 2);
    let mut roles = Vec::new();
    for (pc, name) in &program.temporary_writes {
        assert!(
            matches!(&program.script[*pc], Statement::SetVar {name: actual,..} if actual == name)
        );
        roles.push(program.origins[*pc].generated.clone().unwrap());
    }
    assert!(roles.contains(&GeneratedRole::ForCollection));
    assert!(roles.contains(&GeneratedRole::ForCursor));
    assert!(roles.contains(&GeneratedRole::ForAdvance));
    let advance = program
        .origins
        .iter()
        .position(|origin| origin.generated == Some(GeneratedRole::ForAdvance))
        .unwrap();
    assert!(advance > program.init_scan_end);
    let binder = program
        .origins
        .iter()
        .position(|origin| origin.generated == Some(GeneratedRole::ForItem))
        .unwrap();
    assert!(matches!(&program.script[binder], Statement::SetVar {name,..} if name == "item"));
    assert!(!program.temporary_writes.contains_key(&binder));
}

#[test]
fn generated_symbols_avoid_authored_declarations_and_unresolved_variable_and_target_references() {
    // References and labels may look reserved; binding names keep the strict parser rule.
    let source = "scene label __rvn_scene_branch_1\nset public_item = 40\nset items_reference = __rvn_scene_items_2\nset untouched = __rvn_scene_cursor_3\ncall __rvn_scene_branch_4\nfor item in [1] { if true { \"pause\" } }\nlabel __rvn_scene_end_9\nreturn\n";
    let parsed = parse_program(source).unwrap();
    let mut ast_candidate = parsed.script.clone();
    let program = PreparedNarrative::from_parsed(parsed).unwrap();
    let reserved = [
        "__rvn_scene_branch_1",
        "__rvn_scene_items_2",
        "__rvn_scene_cursor_3",
        "__rvn_scene_branch_4",
        "__rvn_scene_end_9",
    ];
    for name in reserved {
        assert!(!program.generated_targets.contains(name));
        assert!(!program
            .temporary_writes
            .values()
            .any(|temporary| temporary == name));
    }
    assert!(!program.label_table.contains_key("__rvn_scene_branch_4"));
    assert!(program
        .script
        .iter()
        .any(|s| matches!(s,Statement::Call {target} if target == "__rvn_scene_branch_4")));
    assert!(program.script.iter().any(|s| matches!(s,Statement::SetVar {name,value:Expr::Var(value)} if name == "untouched" && value == "__rvn_scene_cursor_3")));
    assert!(program.script.iter().any(|s| matches!(s,Statement::SetVar {name,value:Expr::Var(value)} if name == "items_reference" && value == "__rvn_scene_items_2")));
    assert!(parse_program("scene label intro\nset __rvn_scene_items_2 = 40\n").is_err());

    // AST-domain callers may still supply this declaration. Defend that entry
    // point without representing the prohibited binding as valid authored RVN.
    ast_candidate.insert(
        1,
        Statement::SetVar {
            name: "__rvn_scene_items_2".into(),
            value: Expr::Int(40),
        },
    );
    let ast_program = PreparedNarrative::prepare(ast_candidate).unwrap();
    assert!(ast_program.script.iter().any(|s| matches!(s,Statement::SetVar {name,value:Expr::Int(40)} if name == "__rvn_scene_items_2")));
    for name in reserved {
        assert!(!ast_program.generated_targets.contains(name));
        assert!(!ast_program
            .temporary_writes
            .values()
            .any(|temporary| temporary == name));
    }
}

#[test]
fn generated_choice_positions_have_distinct_origins_and_end_position_requires_none() {
    let program = prepared(
        "scene label intro\nchoice { \"a\" => { \"same\" } \"b\" => { \"same\" } }\nreturn\n",
    );
    let branches: Vec<_> = program
        .origins
        .iter()
        .enumerate()
        .filter(|(_, origin)| {
            matches!(
                origin.generated,
                Some(GeneratedRole::BranchLabel {
                    branch: BranchKind::Choice,
                    ..
                })
            )
        })
        .map(|(pc, _)| pc)
        .collect();
    assert_eq!(branches.len(), 2);
    assert_ne!(program.origins[branches[0]], program.origins[branches[1]]);
    let first = program.position(branches[0]);
    assert!(program.validate_position(&first).is_ok());
    assert!(program
        .validate_position(&ResumePosition {
            pc: branches[1],
            origin: first.origin.clone()
        })
        .is_err());
    assert!(program
        .validate_position(&ResumePosition {
            pc: branches[0],
            origin: None
        })
        .is_err());
    let end = program.position(program.script.len());
    assert!(end.origin.is_none());
    assert!(program.validate_position(&end).is_ok());
    assert!(program
        .validate_position(&ResumePosition {
            pc: program.script.len(),
            origin: first.origin
        })
        .is_err());
    assert!(program
        .validate_position(&program.position(usize::MAX))
        .is_err());
}

#[test]
fn from_parsed_refuses_fabricated_path_range_tree_context_or_duplicate_origin_without_io() {
    let original = parse_program("scene label intro\nif true { \"été 🍃\" }\nreturn\n").unwrap();
    let snapshot = original.clone();
    let mut wrong = original.clone();
    wrong.context = NarrativeContext::Legacy;
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.origins[1].source = Some(PathBuf::from("invented.rvn"));
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.origins[1].span.range.start += 1;
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.origins[1].span.children.clear();
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.origins.pop();
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.origins[1] = wrong.origins[0].clone();
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.sources.push(wrong.sources[0].clone());
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    let mut wrong = original.clone();
    wrong.sources[0].source = "scene label intro\nif true { \"changed\" }\nreturn\n".into();
    assert!(PreparedNarrative::from_parsed(wrong).is_err());
    assert_eq!(original, snapshot);
    assert!(PreparedNarrative::from_parsed(original).is_ok());
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rvn-prepared-étoile-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_import_origins_keep_file_bytes_and_explicit_routes_after_lowering() {
    let files = Files::new();
    let root = files.0.join("root.rvn");
    let helper = files.0.join("helper.rvn");
    let root_source = "use \"helper.rvn\"\nscene label intro\ncall helper\n";
    let helper_source = "// été 🍃\nlabel helper\nif true { \"first\" } else { choice { \"go\" => { \"second\" } } }\nreturn\n";
    fs::write(&root, root_source).unwrap();
    fs::write(&helper, helper_source).unwrap();
    let parsed = parse_file_program_with_uses(&root).unwrap();
    let program = PreparedNarrative::from_parsed(parsed).unwrap();
    for (text, route) in [
        (
            "\"first\"",
            vec![
                AstRoutePart::Root { index: 1 },
                AstRoutePart::Then { index: 0 },
            ],
        ),
        (
            "\"second\"",
            vec![
                AstRoutePart::Root { index: 1 },
                AstRoutePart::Else { index: 0 },
                AstRoutePart::Choice {
                    option: 0,
                    index: 0,
                },
            ],
        ),
    ] {
        let origin = program
            .origins
            .iter()
            .find(|origin| origin.route == route && origin.generated.is_none())
            .unwrap();
        assert_eq!(origin.source, Some(helper.canonicalize().unwrap()));
        assert_eq!(&helper_source[origin.span.clone().unwrap()], text);
        assert!(origin.graph.is_none());
    }
    assert_eq!(fs::read_to_string(root).unwrap(), root_source);
    assert_eq!(fs::read_to_string(helper).unwrap(), helper_source);
}

#[test]
fn instruction_origin_serialization_is_exact_and_positions_reject_program_provenance_loss() {
    let mut program = prepared("scene label intro\nif true { \"pause\" }\nreturn\n");
    for origin in &program.origins {
        let bytes = serde_json::to_vec(origin).unwrap();
        assert_eq!(
            serde_json::from_slice::<InstructionOrigin>(&bytes).unwrap(),
            *origin
        );
    }
    let valid = program.position(0);
    assert!(program.validate_position(&valid).is_ok());
    program.origins.clear();
    assert!(program
        .validate_position(&ResumePosition {
            pc: 0,
            origin: None
        })
        .is_err());
}
