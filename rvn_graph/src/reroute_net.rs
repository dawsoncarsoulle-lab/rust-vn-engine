//! Independent edit-time constraints for transparent reroute networks.
//! Serialized pin types are a compatibility cache, never a disconnected net's authority.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(crate) struct RerouteNet {
    pub nodes: BTreeSet<NodeId>,
    pub producers: BTreeSet<PinId>,
    pub consumers: BTreeSet<PinId>,
    pub valid: bool,
    pub too_deep: bool,
}

#[derive(Default)]
pub(crate) struct RerouteTopology {
    pub nets: Vec<RerouteNet>,
    pub by_node: BTreeMap<NodeId, usize>,
}

impl GraphDocument {
    pub(crate) fn reroute_topology(&self) -> RerouteTopology {
        self.reroute_topology_with_link(None)
    }

    // A prospective link uses two real existing PinIds. It allocates no
    // EdgeId, changes no document/counter, and grants no runtime authority.
    pub(crate) fn reroute_topology_with_link(&self, extra: Option<(PinId,PinId)>) -> RerouteTopology {
        let routes: BTreeSet<_> = self.nodes.values().filter(|n| n.kind == NodeKind::Reroute)
            .map(|n| n.id).collect();
        let mut adjacent: BTreeMap<NodeId, Vec<(PinId, PinId)>> = BTreeMap::new();
        for (a,b) in self.edges.values().map(|edge|(edge.output,edge.input)).chain(extra) {
            let (Some(output), Some(input)) = (self.pins.get(&a), self.pins.get(&b)) else { continue; };
            if routes.contains(&output.node) { adjacent.entry(output.node).or_default().push((output.id, input.id)); }
            if routes.contains(&input.node) && input.node != output.node { adjacent.entry(input.node).or_default().push((output.id, input.id)); }
        }
        let mut topology = RerouteTopology::default();
        for root in routes.iter().copied() {
            if topology.by_node.contains_key(&root) { continue; }
            let index = topology.nets.len();
            let mut net = RerouteNet { valid: true, ..Default::default() };
            let mut pending = vec![root];
            while let Some(node) = pending.pop() {
                if !net.nodes.insert(node) { continue; }
                topology.by_node.insert(node,index);
                let input = self.pin_by_key(node,"value");
                let output = self.pin_by_key(node,"value_out");
                net.valid &= self.nodes[&node].pins.len()==2
                    && matches!((input,output), (Some(a),Some(b)) if a.direction==PinDirection::Input && b.direction==PinDirection::Output && a.node==node && b.node==node)
                    && adjacent.get(&node).into_iter().flatten().filter(|(_,b)|self.pins.get(b).is_some_and(|p|p.node==node)).count()<=1;
                for (a,b) in adjacent.get(&node).into_iter().flatten().copied() {
                    let (Some(source),Some(target)) = (self.pins.get(&a),self.pins.get(&b)) else { net.valid=false; continue; };
                    if source.direction!=PinDirection::Output || target.direction!=PinDirection::Input { net.valid=false; }
                    if routes.contains(&source.node) {
                        net.valid &= self.pin_by_key(source.node,"value_out").is_some_and(|p|p.id==a);
                        if !net.nodes.contains(&source.node) { pending.push(source.node); }
                    }
                    else { net.producers.insert(a); }
                    if routes.contains(&target.node) {
                        net.valid &= self.pin_by_key(target.node,"value").is_some_and(|p|p.id==b);
                        if !net.nodes.contains(&target.node) { pending.push(target.node); }
                    }
                    else { net.consumers.insert(b); }
                }
            }
            // The existing inference depth limit is a path limit, not a
            // fan-out limit: a hundred sibling consumers are still one step.
            let mut incoming: BTreeMap<_,usize>=net.nodes.iter().map(|n|(*n,0)).collect();
            let mut children: BTreeMap<NodeId,Vec<NodeId>>=BTreeMap::new();
            for node in &net.nodes {
                for(a,b)in adjacent.get(node).into_iter().flatten() {
                    let(Some(source),Some(target))=(self.pins.get(a),self.pins.get(b))else{continue;};
                    if source.node==*node && net.nodes.contains(&target.node) {
                        children.entry(*node).or_default().push(target.node);
                        *incoming.get_mut(&target.node).unwrap()+=1;
                    }
                }
            }
            let mut ready: Vec<_>=incoming.iter().filter(|(_,n)|**n==0).map(|(n,_)|*n).collect();
            let mut depth: BTreeMap<_,usize>=net.nodes.iter().map(|n|(*n,1)).collect();
            let mut visited=0;
            while let Some(node)=ready.pop() {
                visited+=1;let next=depth[&node]+1;
                for child in children.get(&node).into_iter().flatten() {
                    let value=depth.get_mut(child).unwrap();*value=(*value).max(next);
                    let remaining=incoming.get_mut(child).unwrap();*remaining-=1;
                    if *remaining==0 {ready.push(*child);}
                }
            }
            net.too_deep=visited!=net.nodes.len()||depth.values().any(|depth|*depth>128);
            topology.nets.push(net);
        }
        topology
    }

    /// Read-only cable constraint. Does not invent a source value or mutate old JSON.
    pub fn reroute_constraint_type(&self, node: NodeId) -> Option<ValueType> {
        if self.nodes.get(&node)?.kind != NodeKind::Reroute { return None; }
        Some(crate::type_inference::reroute_constraint(self,node))
    }

    pub fn reroute_constraint_types(&self) -> BTreeMap<PinId, ValueType> {
        crate::type_inference::reroute_constraints(self)
    }

    /// Endpoint type for authoring: reroutes constrain a whole cable, unlike an
    /// ordinary unknown producer which must remain a dynamic data source.
    pub fn connection_output_type(&self, pin: PinId) -> Option<ValueType> {
        let model=self.pins.get(&pin)?;
        if self.nodes.get(&model.node)?.kind==NodeKind::Reroute { self.reroute_constraint_type(model.node) }
        else { self.effective_pin_type(pin) }
    }

    pub fn resolved_pin_is_execution(&self, pin: PinId) -> bool {
        self.pins.get(&pin).is_some_and(|p| {
            if self.nodes.get(&p.node).is_some_and(|n|n.kind==NodeKind::Reroute) {
                self.reroute_constraint_type(p.node)==Some(ValueType::Execution)
            } else { p.value_type.is_execution() }
        })
    }

    /// An actual neutral knot can acquire Execution. This exception never
    /// grants Execution to an ordinary Any data pin or a data-anchored net.
    pub fn reroute_wildcard_exec_connection(&self, output: PinId, input: PinId) -> bool {
        let (Some(a),Some(b))=(self.pins.get(&output),self.pins.get(&input)) else { return false; };
        if a.direction!=PinDirection::Output || b.direction!=PinDirection::Input || a.node==b.node { return false; }
        let topology=self.reroute_topology();
        let neutral = |pin: &GraphPin| {
            let Some(index)=topology.by_node.get(&pin.node) else { return false; };
            let net=&topology.nets[*index];
            net.valid && !net.too_deep
                && crate::type_inference::reroute_edit_constraint(self,pin.node)==Some(ValueType::Any)
                && net.producers.iter().chain(&net.consumers).all(|id| self.pins.get(id).is_some_and(|p|p.value_type.is_execution()))
        };
        ((neutral(a) && self.resolved_pin_is_execution(input))
            || (neutral(b) && self.resolved_pin_is_execution(output)))
            && self.reroute_exec_connection_is_resolvable(output,input)
    }

    /// Preview only execution admission. Data authoring keeps its existing
    /// dynamic/bounded behavior. Real endpoints, no model mutation or IDs.
    pub fn reroute_exec_connection_is_resolvable(&self, output:PinId,input:PinId)->bool {
        let(Some(a),Some(b))=(self.pins.get(&output),self.pins.get(&input))else{return false;};
        if a.direction!=PinDirection::Output||b.direction!=PinDirection::Input{return false;}
        if ![a.node,b.node].iter().any(|id|self.nodes.get(id).is_some_and(|n|n.kind==NodeKind::Reroute)) {
            return self.resolved_pin_is_execution(output)&&self.resolved_pin_is_execution(input);
        }
        crate::type_inference::reroute_exec_connection_resolvable(self,output,input)
    }

    /// Normalize only components touched by a successful explicit edit. The
    /// existing caller owns the cloned trial and its single Undo/Save boundary.
    pub(crate) fn refresh_reroute_components(&mut self, seeds: &[NodeId]) {
        let topology=self.reroute_topology();
        let indices: BTreeSet<_>=seeds.iter().filter_map(|n|topology.by_node.get(n).copied()).collect();
        let mut updates=Vec::new();
        for index in indices {
            let net=&topology.nets[index];
            if !net.valid { continue; }
            let Some(node)=net.nodes.iter().next() else { continue; };
            // An exhausted component is deliberately neutral in the view;
            // its unexamined serialized cache is not rewritten by this edit.
            let Some(ty)=crate::type_inference::reroute_edit_constraint(self,*node) else { continue; };
            for node in &net.nodes {
                for key in ["value","value_out"] {
                    if let Some(pin)=self.pin_by_key(*node,key) {
                        updates.push((pin.id,ty.clone()));
                    }
                }
            }
        }
        for (id,ty) in updates { if let Some(pin)=self.pins.get_mut(&id) { pin.value_type=ty; } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reroute1003_exec_links_v1_still_refuses_non_catalogue_execution_input_role(){
        let source="init {set gate=false flow.for_loop(i,1,3) {if gate {flow.break()} if not gate {flow.break()}}}\nlabel start\nreturn\n";
        let mut graphs=import_script(&rvn_parser::parse(source).unwrap()).unwrap();
        let g=graphs.iter_mut().find(|g|matches!(g.kind,GraphKind::Init)).unwrap();
        g.enable_native_frame_preparation().unwrap();let range=g.nodes.values().find(|n|n.kind==NodeKind::ForLoopWithBreak).unwrap().id;
        *g=g.prepare_native_loop_demand(range).unwrap().graph;g.enable_exec_links_source_preparation().unwrap();
        let edge=g.edges.values().find(|e|g.pins[&e.output].value_type==ValueType::Execution).cloned().unwrap();g.remove_edge(edge.id).unwrap();
        let r=g.add_catalog_node(NodeKind::Reroute,[40.,40.]).unwrap();let input=g.pin_by_key(r,"value").unwrap().id;let output=g.pin_by_key(r,"value_out").unwrap().id;g.connect(edge.output,input).unwrap();g.connect(output,edge.input).unwrap();
        assert!(g.resolved_pin_is_execution(input));let before=g.clone();
        assert!(crate::native_frame_preparation::encode_node(g,r).unwrap_err().contains("actual catalogue execution role"));
        assert_eq!(*g,before);
        // A persisted old Any cache must not evade the same role check.
        g.set_reroute_type(r,ValueType::Any).unwrap();let cached=g.clone();
        assert!(crate::native_frame_preparation::encode_node(g,r).unwrap_err().contains("actual catalogue execution role"));assert_eq!(*g,cached);
        assert_eq!(node_definition(NodeKind::Reroute).pins.iter().find(|p|p.key=="value").unwrap().value_type,ValueType::Any);
    }
}
