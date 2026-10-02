//! Transactional spline editing. Only literal, authored control points are
//! draggable; computed paths remain connected Blueprint expressions.
use crate::*;
use rvn_ui::motion::PathPoint;

fn point_slots(graph: &GraphDocument, node: NodeId) -> Result<(String, String), String> {
    let model = graph.nodes.get(&node).ok_or("Missing spline point")?;
    if model.kind != NodeKind::FunctionCall
        || model.properties.get("function") != Some(&PropertyValue::String("dict".into()))
    {
        return Err("Spline points are computed: edit their Blueprint expression".into());
    }
    let count = match model.properties.get("input_count") {
        Some(PropertyValue::Int(count)) if *count == 4 => *count as usize,
        _ => return Err("A spline point needs exactly x and y fields".into()),
    };
    let (mut x, mut y) = (None, None);
    for index in (0..count).step_by(2) {
        let key = graph.literal_input_value(node, &format!("item_{index}"))?;
        let slot = format!("item_{}", index + 1);
        match key {
            Some(PropertyValue::String(key)) if key == "x" && x.is_none() => x = Some(slot),
            Some(PropertyValue::String(key)) if key == "y" && y.is_none() => y = Some(slot),
            _ => return Err("A spline point needs unique x and y fields".into()),
        }
    }
    Ok((x.ok_or("Missing point x")?, y.ok_or("Missing point y")?))
}
fn number(value: Option<PropertyValue>) -> Result<f32, String> {
    let value = match value {
        Some(PropertyValue::Float(value)) => value,
        Some(PropertyValue::Int(value)) => value as f64,
        _ => return Err("Spline coordinates are computed: edit their Blueprint expression".into()),
    };
    if !value.is_finite() || value.abs() > 500_000.0 {
        return Err("Spline points must be finite in ±500000 reference pixels".into());
    }
    Ok(value as f32)
}
fn point_number(graph: &GraphDocument, node: NodeId, key: &str) -> Result<f32, String> {
    if let Ok(value) = graph.literal_input_value(node, key) {
        return number(value);
    }
    // Negative authored numbers import as a Negate expression node, not as
    // a scalar literal. Recognize only that exact constant expression; do
    // not evaluate functions, variables or otherwise erase computed code.
    let expression =
        transpile_value_input(graph, node, key).map_err(|problem| problem.to_string())?;
    let ast = rvn_parser::parse(&format!("set __editor_point = {expression}"))
        .map_err(|problem| problem.to_string())?;
    fn scalar(expr: &rvn_parser::Expr) -> Option<f64> {
        match expr {
            rvn_parser::Expr::Int(value) => Some(*value as f64),
            rvn_parser::Expr::Float(value) => Some(f64::from(*value)),
            rvn_parser::Expr::Neg(value) => scalar(value).map(|value| -value),
            _ => None,
        }
    }
    let [rvn_parser::Statement::SetVar { value, .. }] = ast.as_slice() else {
        return Err("Expected a coordinate value".into());
    };
    number(scalar(value).map(PropertyValue::Float))
}
impl GraphDocument {
    pub fn motion_path_points(&self, node: NodeId) -> Result<Vec<PathPoint>, String> {
        if !self
            .nodes
            .get(&node)
            .is_some_and(|node| node.kind == NodeKind::MotionSpline)
        {
            return Err("Expected a spline animation node".into());
        }
        self.list_input_sources(node, "points")?
            .into_iter()
            .map(|point| {
                let point = point.ok_or("A spline point must be an authored x/y dictionary")?;
                let (x, y) = point_slots(self, point)?;
                Ok(PathPoint {
                    x: point_number(self, point, &x)?,
                    y: point_number(self, point, &y)?,
                })
            })
            .collect()
    }
    pub fn set_motion_path_point(
        &mut self,
        node: NodeId,
        index: usize,
        point: PathPoint,
    ) -> Result<(), String> {
        number(Some(PropertyValue::Float(f64::from(point.x))))?;
        number(Some(PropertyValue::Float(f64::from(point.y))))?;
        let _ = self.motion_path_points(node)?;
        let mut next = self.clone();
        let child = next.own_list_input_node(node, "points", index)?;
        let (x, y) = point_slots(&next, child)?;
        for (key, value) in [(x, point.x), (y, point.y)] {
            let pin = next
                .pin_by_key(child, &key)
                .ok_or("Missing coordinate pin")?
                .id;
            next.edges.retain(|_, edge| edge.input != pin);
            next.pins.get_mut(&pin).unwrap().default_value =
                Some(PropertyValue::Float(f64::from(value)));
        }
        *self = next;
        Ok(())
    }
    pub fn append_motion_path_point(
        &mut self,
        node: NodeId,
        point: PathPoint,
    ) -> Result<(), String> {
        let points = self.motion_path_points(node)?;
        if points.len() >= 128 {
            return Err("The visual spline designer supports at most 128 authored points; longer computed paths remain Blueprint expressions".into());
        }
        number(Some(PropertyValue::Float(f64::from(point.x))))?;
        number(Some(PropertyValue::Float(f64::from(point.y))))?;
        let mut next = self.clone();
        let position = next.nodes[&node].position;
        let child = next
            .add_catalog_node(
                NodeKind::FunctionCall,
                [
                    position[0] - 500.0,
                    position[1] + points.len() as f64 * 140.0,
                ],
            )
            .map_err(|error| error.to_string())?;
        next.set_property(child, "function", PropertyValue::String("dict".into()))
            .map_err(|error| error.to_string())?;
        next.resize_value_inputs(child, 4)
            .map_err(|error| error.to_string())?;
        for (index, value) in [
            PropertyValue::String("x".into()),
            PropertyValue::Float(f64::from(point.x)),
            PropertyValue::String("y".into()),
            PropertyValue::Float(f64::from(point.y)),
        ]
        .into_iter()
        .enumerate()
        {
            next.set_literal_input(child, &format!("item_{index}"), value)?;
        }
        next.append_list_input(node, "points", child, "result")?;
        *self = next;
        Ok(())
    }
    pub fn remove_motion_path_point(&mut self, node: NodeId, index: usize) -> Result<(), String> {
        if self.motion_path_points(node)?.len() <= 2 {
            return Err("A spline animation must retain at least two points".into());
        }
        self.remove_list_input(node, "points", index)
    }
}
