mod authoring_collections;
mod blueprint_policy;
mod catalog;
mod callable_refactor;
mod choice;
mod codegen;
mod native_frame_preparation;
mod empty_native_init_0984;
pub use empty_native_init_0984::import_native_frame_source;
mod native_loop_demand;
pub use native_loop_demand::*;
mod native_source_binding;
mod native_debug_origins;
pub use native_debug_origins::*;
pub use native_source_binding::*;
pub use native_frame_preparation::*;
mod component;
mod document;
mod ids;
mod import;
mod imagemap_spec;
mod interface_authoring;
mod interface_clipboard;
mod migration;
mod motion_authoring;
mod pin_catalog;
mod standard_macros_1020;
pub use standard_macros_1020::StandardMacroContract;
mod project;
mod source_project;
mod widget_runtime_binding_0950;
pub use widget_runtime_binding_0950::*;
#[cfg(not(target_arch = "wasm32"))]
mod source_workspace;
mod type_inference;
mod reroute_net;
mod text_owner;
mod text_pin;
pub mod widget_viewport_graph1003;
mod widget_factory_graph_1001;
mod widget_object_class_graph1002;
mod types;
mod validation;
mod variable_rename;
mod variable_initializers;
mod variable_initial_value;
mod variable_binding_types;

pub use document::*;
pub use callable_refactor::*;
pub use ids::*;
pub use interface_authoring::*;
pub use interface_clipboard::*;
pub use migration::*;
pub use type_inference::VariableScope;
pub use types::*;
pub use text_owner::*;
pub use text_pin::GraphTextEdit;
pub use validation::*;
pub use variable_initializers::*;
pub use variable_initial_value::LocalVariableTypeProposal;
pub use variable_rename::{VariableBindingContext, VariableBindingProjection};

pub const GRAPH_SCHEMA_VERSION: u32 = 5;
/// Explicit narrative-scene metadata is opt-in. Legacy documents still use v5.
pub const GRAPH_NARRATIVE_SCHEMA_VERSION: u32 = 6;
/// Persistent localization ownership is opt-in, independent of scene semantics.
pub const GRAPH_TEXT_SCHEMA_VERSION: u32 = 7;
/// Retained table references on authored Text pins are explicitly opted in.
pub const GRAPH_TABLE_TEXT_SCHEMA_VERSION: u32 = 8;
/// Typed table query contracts are opt-in; unrelated v5-v8 documents stay unchanged.
pub const GRAPH_NAME_TABLE_SCHEMA_VERSION: u32 = 9;
/// Dedicated typed Text construction and the shared three-output inspector.
pub const GRAPH_NAME_TEXT_SCHEMA_VERSION: u32 = 10;
/// PRIVATE preparation only; ordinary export/import refuses this contract.
pub const GRAPH_NATIVE_FRAME_SCHEMA_VERSION: u32 = 11;
pub const GRAPH_MIXED_INSPECTOR_SCHEMA_VERSION: u32 = 12;
/// Private explicit loop preparation; no old graph is automatically promoted.
pub const GRAPH_NATIVE_LOOP_DEMAND_SCHEMA_VERSION: u32 = 13;
/// Root-reserved Unit/signature metadata. No legacy graph is promoted implicitly.
pub const GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION: u32 = 14;
/// Nominal Enum data is explicit opt-in; unrelated v5-v14 data is never promoted.
pub const GRAPH_NOMINAL_ENUM_SCHEMA_VERSION: u32 = 15;
pub const GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION: u32 = 16;
pub const GRAPH_MAX_SUPPORTED_SCHEMA_VERSION: u32 = GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION;
mod library_manifest1027;
mod library_current1027;
#[cfg(not(target_arch="wasm32"))]
pub mod source_function_library_recovery1023;
#[cfg(not(target_arch="wasm32"))]
pub mod function_library_creation1026;
pub use library_current1027::{prepare_library_current_cohort1027,PreparedLibraryCurrentCohort1027,prepare_initial_library_member1027,prepare_initial_library_member_in_current1027,validate_library_membership_shape1027,prepare_library_current_signatures1027,PreparedLibraryCurrentSignatures1027};
pub use library_manifest1027::{LibraryManifest1027,parse_library_manifest1027,MAX_LIBRARY_SOURCE_INPUT_BYTES1027};
mod library_graph1027;
mod library_call_authoring1027;
pub use library_graph1027::*;
mod enum_graph1026;
pub use enum_graph1026::{ENUM_VALUE_CONSTRUCTOR1026, replace_enum_global_initializer1026};
pub(crate) fn supports_unit_schema1026(version:u32)->bool {
    matches!(version, GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION | GRAPH_NOMINAL_ENUM_SCHEMA_VERSION | GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION)
}
pub(crate) fn uses_unit_context1026(graph:&GraphDocument)->bool {
    graph.schema_version == GRAPH_UNIT_SIGNATURE_SCHEMA_VERSION
        || matches!(graph.schema_version, GRAPH_NOMINAL_ENUM_SCHEMA_VERSION | GRAPH_QUALIFIED_LIBRARY_SCHEMA_VERSION)
            && (matches!(graph.kind,GraphKind::UnitFunction{..})
                || graph.nodes.values().any(|node|matches!(node.kind,NodeKind::UnitCall|NodeKind::UnitReturn))
                || graph.pins.values().any(|pin|pin.parameter_id.is_some()))
}
mod text_table_nodes {
//! Independent portable Text history nodes. Source projections select exact output pins.
use crate::*;
use rvn_parser::Expr;
use std::collections::BTreeSet;

pub(crate) fn inspector_output(function: &str) -> Option<&'static str> {
    match function { "__rvn_text_table_valid"=>Some("valid"), "__rvn_text_table_id"=>Some("table_id"), "__rvn_text_table_key"=>Some("key"), _=>None }
}
pub(crate) fn inspector_function(output: &str) -> Option<&'static str> {
    match output { "valid"=>Some("__rvn_text_table_valid"), "table_id"=>Some("__rvn_text_table_id"), "key"=>Some("__rvn_text_table_key"), _=>None }
}
fn known_pure(function: &str) -> bool {
    matches!(function, "__rvn_literal_name"|"__rvn_string_to_name"|"__rvn_name_to_string"|
        "__rvn_localizable_text"|"__rvn_invariant_text"|"__rvn_empty_text"|"__rvn_format_text"|
        "__rvn_string_table_text"|"__rvn_string_table_text_name"|
        "__rvn_text_table_valid"|"__rvn_text_table_id"|"__rvn_text_table_key"|
        "string_to_text"|"text_to_string"|"dict"|"list"|"__rvn_widget_object_class_v1"|"__rvn_enum_value_v1")
}
pub(crate) fn source_binding_names(expression:&Expr)->BTreeSet<String> {
    fn visit(expr:&Expr,names:&mut BTreeSet<String>) {
        match expr {
            Expr::Var(name)=>{names.insert(name.clone());},
            Expr::LibraryCall{call}=>for(_,argument)in call.arguments(){visit(argument,names);},
            Expr::Call{args,..}|Expr::ListLit(args)=>{for arg in args {visit(arg,names);}},
            Expr::BinOp{left,right,..}|Expr::Index{target:left,index:right}|Expr::And(left,right)|Expr::Or(left,right)=>{visit(left,names);visit(right,names);},
            Expr::Not(arg)|Expr::Neg(arg)=>visit(arg,names),
            _=>{},
        }
    }
    let mut names=BTreeSet::new();visit(expression,&mut names);names
}
/// Conservative: unknown functions may use random/input or have stateful host effects.
pub(crate) fn reproducible_source(expression: &Expr) -> bool {
    fn visit(expr: &Expr, depth: usize, work: &mut usize) -> bool {
        *work+=1;if depth>128 || *work>4096 { return false; }
        match expr {
            Expr::LibraryCall{..}=>false, // requires original qualified signature; never a stateless intrinsic
            Expr::StandardMacroData{..}=>false, // live owner state is not a reproducible frozen producer
            Expr::Int(_)|Expr::Float(_)|Expr::Bool(_)|Expr::Str(_)|Expr::Var(_)=>true,
            Expr::Call{name,args}=>known_pure(name)&&args.iter().all(|e|visit(e,depth+1,work)),
            Expr::ListLit(items)=>items.iter().all(|e|visit(e,depth+1,work)),
            Expr::BinOp{left,right,..}|Expr::Index{target:left,index:right}|Expr::And(left,right)|Expr::Or(left,right)=>visit(left,depth+1,work)&&visit(right,depth+1,work),
            Expr::Not(value)|Expr::Neg(value)=>visit(value,depth+1,work),
        }
    }
    visit(expression,0,&mut 0)
}
impl GraphDocument {
    pub fn validate_name_text_nodes(&self) -> Result<(),String> {
        for node in self.nodes.values().filter(|node| matches!(node.kind,NodeKind::TextFromStringTable|NodeKind::GetStringTableIdAndKey)) {
            if self.schema_version<crate::GRAPH_NAME_TEXT_SCHEMA_VERSION { return Err("Typed Text history nodes require graph schema10".into()); }
            let definitions=crate::node_definition(node.kind).pins;
            if definitions.len()!=node.pins.len() { return Err("Typed Text node has an invalid pin count".into()); }
            for expected in definitions {
                let pin=self.pin_by_key(node.id,expected.key).ok_or("Typed Text node has a missing pin")?;
                if pin.direction!=expected.direction || pin.value_type!=expected.value_type || pin.cardinality!=expected.cardinality {
                    return Err(format!("Typed Text pin contract mismatch: {}.{}",node.id,expected.key));
                }
                if let Some(value)=&pin.default_value {
                    if pin.direction!=PinDirection::Input || !pin.value_type.accepts(&crate::type_inference::property_type(value)) {
                        return Err(format!("Typed Text default mismatch: {}.{}",node.id,expected.key));
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate_inspector_input_is_reproducible(&self,node:NodeId)->Result<(),String> {
        fn visit(graph:&GraphDocument,pin:PinId,active:&mut BTreeSet<PinId>,depth:usize,work:&mut usize)->Result<(),String> {
            *work+=1;if depth>128 || *work>4096 || !active.insert(pin) {return Err("Text inspector input is cyclic or too complex".into());}
            let output=graph.pins.get(&pin).ok_or("Missing inspector source pin")?;
            let owner=graph.nodes.get(&output.node).ok_or("Missing inspector source node")?;
            // Read an already assigned or scoped value, never evaluate its initializer again.
            if matches!(owner.kind,NodeKind::VariableGet|NodeKind::SetVariable|NodeKind::LocalVariable|NodeKind::WidgetCreate|NodeKind::ForLoopWithBreak|NodeKind::ForEach) {
                active.remove(&pin);return Ok(());
            }
            if owner.kind==NodeKind::FunctionCall && !matches!(owner.properties.get("function"),Some(PropertyValue::String(name)) if known_pure(name)) {
                return Err("Capture an effectful Text expression in SET/Local before using the shared inspector".into());
            }
            if matches!(owner.kind,NodeKind::MethodCall|NodeKind::MotionCurve) { return Err("Capture effectful method/curve output before using the shared Text inspector".into()); }
            for input in &owner.pins {
                let input=&graph.pins[input];
                if input.direction==PinDirection::Input && !input.value_type.is_execution() {
                    for edge in graph.edges.values().filter(|edge|edge.input==input.id) { visit(graph,edge.output,active,depth+1,work)?; }
                }
            }
            active.remove(&pin);Ok(())
        }
        let input=self.pin_by_key(node,"text").ok_or("Missing inspector Text input")?;
        for edge in self.edges.values().filter(|edge|edge.input==input.id) {visit(self,edge.output,&mut BTreeSet::new(),0,&mut 0)?;}
        Ok(())
    }
}

}

pub use catalog::*;
pub use choice::choice_option_index;
pub use codegen::*;
pub use import::*;
pub use project::*;
pub use source_project::*;
#[cfg(not(target_arch = "wasm32"))]
pub use source_workspace::*;

mod parameter_id1003;
pub use parameter_id1003::ParameterId;
mod entry_parameter_identity1003;
mod unit_graph_source1003;
mod unit_parameter_authoring1003;
#[cfg(not(target_arch="wasm32"))]
mod unit_parameter_references1003;
mod unit_graph_authoring1003;

// Typed Enum resource admission; no Graph pin/runtime node activation.
pub mod enum_runtime_binding1025;
pub use enum_runtime_binding1025::admit_enum_assets1025;
