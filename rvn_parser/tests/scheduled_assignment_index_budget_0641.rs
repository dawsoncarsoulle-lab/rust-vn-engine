//! Actual old multi-file source closure under the existing source/root/step bounds.
use rvn_parser::{Statement,scheduled_source::scheduled_step_sources};
#[test]
fn legacy_nested_source_beyond100k_keeps_the_existing_root_and_step_contract(){
    // Separate real files avoid the existing lexer's repeated byte→location scan
    // dominating this admission test; they do not reduce the AST statement count.
    let directory=std::path::PathBuf::from(std::env::var("RVN_INDEX_BUDGET_FIXTURES").unwrap());
    std::fs::create_dir_all(&directory).unwrap();let files=1001usize;let per_file=100usize;
    let mut main=String::new();
    for index in 0..files {
        let name=format!("old-{index}.rvn");
        let source=format!("function old_{index}() {{\n{}}}\n","set a=0\n".repeat(per_file));
        std::fs::write(directory.join(&name),source).unwrap();main.push_str(&format!("use \"{name}\"\n"));
    }
    main.push_str("scene label start\n\"old source\"\n");let path=directory.join("main.rvn");std::fs::write(&path,main).unwrap();
    let program=rvn_parser::parse_file_program_with_uses(&path).unwrap();
    assert_eq!(program.script.len(),files+2);assert_eq!(program.sources.len(),files+1);
    let nested:usize=program.script.iter().filter_map(|s|if let Statement::Function{body,..}=s{Some(body.len())}else{None}).sum();
    assert_eq!(nested,files*per_file);assert!(nested>100_000);
    assert!(program.sources.iter().map(|s|s.source.len()).sum::<usize>()<16*1024*1024);
    let steps=scheduled_step_sources(&program).expect("old valid nested statements must not gain a new global execution-like source limit");assert!(steps.is_empty());
}
