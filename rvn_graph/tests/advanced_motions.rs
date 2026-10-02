use rvn_graph::*;
use rvn_ui::motion::PathPoint;
const SOURCE: &str = r#"
function warp(t){return t*t}
function flight(){return motion_spline(2,[{"x":0,"y":0},{"x":100,"y":-50},{"x":200,"y":0}],motion_curve("warp",65))}
label start
scene "room.png"
motion.play("background",motion_parallel([flight(),motion_tween(2,{}, {"opacity":0.5},motion_bezier(0.25,0,0.75,1))]))
"Done"
"#;
#[test]
fn new_animations_import_compile_and_reimport_as_real_pure_nodes() {
    let ast = rvn_parser::parse(SOURCE).unwrap();
    let graphs = import_script(&ast).unwrap();
    for kind in [
        NodeKind::MotionSpline,
        NodeKind::MotionCurve,
        NodeKind::MotionBezier,
    ] {
        assert!(graphs
            .iter()
            .any(|graph| graph.nodes.values().any(|node| node.kind == kind)));
    }
    let source = transpile_project(&graphs).unwrap().source;
    let second = import_script(&rvn_parser::parse(&source).unwrap()).unwrap();
    assert_eq!(
        transpile_project(&second).unwrap().ast,
        transpile_project(&graphs).unwrap().ast
    );
}
#[test]
fn visual_spline_editing_is_atomic_and_preserves_shared_point_nodes() {
    let mut graph = import_script(&rvn_parser::parse(SOURCE).unwrap())
        .unwrap()
        .into_iter()
        .find(|graph| matches!(&graph.kind,GraphKind::Function{name,..} if name=="flight"))
        .unwrap();
    let spline = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MotionSpline)
        .unwrap()
        .id;
    let original = graph.motion_path_points(spline).unwrap();
    let before = graph.clone();
    graph
        .set_motion_path_point(spline, 1, PathPoint { x: 125.0, y: -75.0 })
        .unwrap();
    graph
        .append_motion_path_point(spline, PathPoint { x: 300.0, y: 10.0 })
        .unwrap();
    assert_eq!(graph.motion_path_points(spline).unwrap().len(), 4);
    graph.remove_motion_path_point(spline, 3).unwrap();
    assert_eq!(graph.motion_path_points(spline).unwrap()[1].x, 125.0);
    for (id, node) in &before.nodes {
        assert_eq!(graph.nodes[id].position, node.position);
    }
    let source = transpile(&graph).unwrap().source;
    let new = import_script(&rvn_parser::parse(&source).unwrap())
        .unwrap()
        .into_iter()
        .find(|graph| matches!(&graph.kind,GraphKind::Function{name,..} if name=="flight"))
        .unwrap();
    let spline_new = new
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MotionSpline)
        .unwrap()
        .id;
    assert_eq!(
        new.motion_path_points(spline_new).unwrap(),
        graph.motion_path_points(spline).unwrap()
    );
    let valid = graph.clone();
    assert!(graph
        .set_motion_path_point(
            spline,
            1,
            PathPoint {
                x: f32::NAN,
                y: 0.0
            }
        )
        .is_err());
    assert_eq!(graph, valid);
    assert_eq!(before.motion_path_points(spline).unwrap(), original);
}
#[test]
fn source_first_spline_edits_preserve_comments_ids_and_node_positions() {
    let source = SOURCE.replace(
        "{\"x\":100,\"y\":-50}",
        "// A deliberate bend.\n{\"x\":100,\"y\":-50}",
    );
    let mut project = SourceProject::open(&source, &[]).unwrap();
    let mut graphs = project.graphs().to_vec();
    let graph = graphs
        .iter_mut()
        .find(|graph| matches!(&graph.kind,GraphKind::Function{name,..}if name=="flight"))
        .unwrap();
    let spline = graph
        .nodes
        .values()
        .find(|node| node.kind == NodeKind::MotionSpline)
        .unwrap()
        .id;
    let positions: Vec<_> = graph
        .nodes
        .iter()
        .map(|(id, node)| (*id, node.position))
        .collect();
    graph
        .set_motion_path_point(spline, 1, PathPoint { x: 150.0, y: -75.0 })
        .unwrap();
    project.apply_visual(&source, &graphs).unwrap();
    assert!(project.source().contains("// A deliberate bend."));
    let graph = project
        .graphs()
        .iter()
        .find(|graph| matches!(&graph.kind,GraphKind::Function{name,..}if name=="flight"))
        .unwrap();
    for (id, position) in positions {
        assert_eq!(graph.nodes[&id].position, position);
    }
    let points = graph.motion_path_points(spline).unwrap();
    assert_eq!(points[1], PathPoint { x: 150.0, y: -75.0 });
}
