use rvn_core::{
    eval::FunctionLibrary,
    motion::{MotionState, MotionTarget},
};
use rvn_parser::{parse, Statement, Value};
use rvn_ui::motion::{Motion, Pose};
use std::collections::HashMap;

fn evaluate(source: &str, expression: &str) -> Result<Value, rvn_core::eval::EvalError> {
    let library = FunctionLibrary::from_script(&parse(source).unwrap())?;
    let statements = parse(&format!("set value = {expression}")).unwrap();
    let Statement::SetVar { value, .. } = &statements[0] else {
        unreachable!()
    };
    library.eval(value, &HashMap::new())
}
#[test]
fn custom_rvn_functions_are_sampled_bounded_and_captured_for_restoration() {
    let value=evaluate("function warp(t){return t*t}","motion_spline(2,[{\"x\":0,\"y\":0},{\"x\":100,\"y\":50},{\"x\":200,\"y\":0}],motion_curve(\"warp\",65))").unwrap();
    let motion = Motion::parse(rvn_core::ui::value_to_json(&value).unwrap()).unwrap();
    let pose = motion.sample(1.0, &Pose::default()).unwrap();
    assert!(pose.x < 100.0);
    let mut clocks = MotionState::default();
    clocks.play(MotionTarget::Background, motion).unwrap();
    clocks.tick(1.0).unwrap();
    let saved = serde_json::to_value(&clocks).unwrap();
    let restored: MotionState = serde_json::from_value(saved).unwrap();
    assert_eq!(restored.views().unwrap(), clocks.views().unwrap());
    assert_eq!(restored.views().unwrap()[0].pose, pose);
    let bezier = evaluate(
        "",
        "motion_tween(1,{}, {\"opacity\":0},motion_bezier(0.25,0,0.75,1))",
    )
    .unwrap();
    assert!(
        (Motion::parse(rvn_core::ui::value_to_json(&bezier).unwrap())
            .unwrap()
            .sample(0.5, &Pose::default())
            .unwrap()
            .opacity
            - 0.5)
            .abs()
            < 0.0001
    );
}
#[test]
fn custom_functions_cannot_hang_mutate_story_call_narrative_or_sample_randomness() {
    assert!(evaluate(
        "function bad(t){while true {} return t}",
        "motion_curve(\"bad\",65)"
    )
    .is_err());
    assert!(evaluate(
        "function bad(t){return random()}",
        "motion_curve(\"bad\",65)"
    )
    .is_err());
    assert!(evaluate("function bad(t){return 2*t}", "motion_curve(\"bad\",65)").is_err());
    assert!(evaluate("function bad(a,b){return a}", "motion_curve(\"bad\",65)").is_err());
    assert!(evaluate("function good(t){return t}", "motion_curve(\"good\",258)").is_err());
    assert!(evaluate("function good(t){return t}", "motion_curve(\"missing\",65)").is_err());
    let mut random = rvn_core::random::RandomState::default();
    let before = random;
    let library =
        FunctionLibrary::from_script(&parse("function bad(t){return random_int(0,1)}").unwrap())
            .unwrap();
    let ast = parse("set curve = motion_curve(\"bad\",65)").unwrap();
    let Statement::SetVar { value, .. } = &ast[0] else {
        unreachable!()
    };
    assert!(library
        .eval_with_random(value, &HashMap::new(), &mut random)
        .is_err());
    assert_eq!(random, before);
    assert!(parse("function bad(t){scene \"no.png\" return t}").is_err());
}
