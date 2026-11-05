//! PRIVATE cross-layer preparation, not a second runtime authority or source
//! writer. This calls the real SourceWorkspace writer/loader and Core verifier.
use rvn_core::{
    consumer_schedule::ConsumerSourceBinding, debugger_provenance::SourceMap,
    program::GraphInstructionOrigin,
};
use rvn_graph::{
    NativeDebugOrigin, NativeDebugSourceMap, NativeOriginRole, NativeSourceSite, SourceWorkspace,
};
use rvn_parser::ParsedProgram;
use serde::{Deserialize, Serialize};
mod frontend_source;
pub use frontend_source::source_compiler_map;

/// Derived comparison receipt for a moved source tree. It contains no terminal
/// values, cursor, frame, UI instance or owner authority. Save15 must validate
/// those separately against the actual activation and ordered plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableNativeSourceReceipt {
    pub version: u32,
    pub emitted: [u8; 32],
    pub content: [u8; 32],
    pub source_bytes_in_inclusion_order: Vec<String>,
    pub root_source_ordinals: Vec<usize>,
    pub plans: Vec<PortableNativePlan>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableNativePlan {
    pub source_ordinal: usize,
    pub graph: u64,
    pub function: String,
    pub root: u64,
    pub consumer: u64,
    pub route: Vec<usize>,
    pub span: std::ops::Range<usize>,
    pub scope: String,
    pub plan: rvn_parser::Statement,
}

pub struct FrozenNativeSource {
    program: ParsedProgram,
    binding: ConsumerSourceBinding,
    compiler_map: SourceMap,
    sites: Vec<NativeSourceSite>,
    unmapped_sites: Vec<NativeSourceSite>,
    debug_origins: NativeDebugSourceMap,
}
impl FrozenNativeSource {
    pub fn from_workspace(workspace: &SourceWorkspace) -> Result<Self, String> {
        let program = workspace.freeze_native_program()?;
        Self::from_frozen_program(program)
    }
    pub fn from_frozen_program(program: ParsedProgram) -> Result<Self, String> {
        // Core reparses every frozen source and validates the entire AST,
        // inclusion/ordinal associations and spans before constructing hashes.
        let binding =
            ConsumerSourceBinding::from_program(&program).map_err(|error| error.to_string())?;
        let sites = rvn_graph::native_source_sites(&program)?;
        let mut compiler_map = SourceMap::from_program(&program)?;
        let debug_origins = NativeDebugSourceMap::from_program(&program)?;
        fn attach(
            actual: &mut rvn_core::program::InstructionOrigin,
            origins: &[NativeDebugOrigin],
        ) -> Result<(), String> {
            if let Some(step) = &actual.scheduled_step {
                let candidates: Vec<_> = origins.iter().filter(|origin|
                    matches!(origin.role, NativeOriginRole::Producer { step: index, .. } if index == step.index)
                    && actual.source.as_ref() == Some(&origin.source)
                    && actual.span.as_ref() == Some(&origin.span)
                    && step.source_ordinal == origin.source_ordinal
                    && step.definition_root_ordinal == origin.definition_root_ordinal
                    && origin.expression.as_ref() == Some(&step.expression)
                    && origin.scope == step.plan_scope).collect();
                let [origin] = candidates.as_slice() else {
                    return Err("Producer has no unique verified Graph/source site".into());
                };
                let endpoint = step.endpoint.as_ref().ok_or("Producer has no declared Graph endpoint")?;
                if endpoint.graph != origin.graph.get() || endpoint.node != origin.node.get()
                    || endpoint.ports.len() != origin.ports.len()
                    || endpoint.ports.iter().any(|port| !origin.ports.iter().any(|actual|
                        actual.role == port.role && actual.pin.get() == port.pin
                        && actual.incoming.map(|edge| edge.get()) == port.incoming))
                    || !matches!(&origin.role, NativeOriginRole::Producer { legacy_terminal,native_loop_output, .. } if legacy_terminal == &endpoint.legacy_terminal && native_loop_output == &endpoint.native_loop_output)
                {
                    return Err("Producer pins/roles differ from the reconstructed Graph".into());
                }
                actual.graph = Some(GraphInstructionOrigin { source: Some(origin.source.clone()),
                    graph: origin.graph.get(), node: Some(origin.node.get()) });
                return Ok(());
            }
            let candidates: Vec<_> = origins
                .iter()
                .filter(|origin| {
                    !matches!(origin.role, NativeOriginRole::Producer { .. })
                        && actual.source.as_ref() == Some(&origin.source)
                        && actual.span.as_ref() == Some(&origin.span)
                })
                .collect();
            if let Some(first) = candidates.first() {
                if candidates
                    .iter()
                    .any(|candidate| candidate.graph != first.graph || candidate.node != first.node)
                {
                    return Err(
                        "Native compiler origin is ambiguous at an actual source range".into(),
                    );
                }
                actual.graph = Some(GraphInstructionOrigin {
                    source: Some(first.source.clone()),
                    graph: first.graph.get(),
                    node: Some(first.node.get()),
                });
            }
            Ok(())
        }
        for actual in &mut compiler_map.origins {
            attach(actual, debug_origins.origins())?;
        }
        for actual in &mut compiler_map.sites {
            attach(&mut actual.origin, debug_origins.origins())?;
            attach(&mut actual.declaration, debug_origins.origins())?;
        }
        let unmapped_sites = sites
            .iter()
            .filter(|site| {
                !compiler_map
                    .origins
                    .iter()
                    .chain(compiler_map.sites.iter().map(|s| &s.origin))
                    .any(|actual| {
                        actual.source.as_ref() == Some(&site.source)
                            && actual.span.as_ref() == Some(&site.span.range)
                            && actual.graph.as_ref().is_some_and(|graph| {
                                graph.graph == site.graph.get()
                                    && graph.node == Some(site.consumer.get())
                            })
                    })
            })
            .cloned()
            .collect();
        Ok(Self {
            program,
            binding,
            compiler_map,
            sites,
            unmapped_sites,
            debug_origins,
        })
    }
    pub fn debug_origins(&self) -> &NativeDebugSourceMap {
        &self.debug_origins
    }
    /// Only genuine Core instructions/sites are returned. Producers select the
    /// lexer/reparse-verified Core site, never an invented emitted instruction.
    pub fn compiler_endpoint(
        &self,
        source: &std::path::Path,
        graph: rvn_graph::GraphId,
        node: rvn_graph::NodeId,
        pin: rvn_graph::PinId,
    ) -> Vec<CompilerEndpoint> {
        self.debug_origins
            .endpoint(source, graph, node, pin)
            .into_iter()
            .map(|origin| {
                let producer = matches!(origin.role, NativeOriginRole::Producer { .. });
                let emitted_instructions = if producer {
                    Vec::new()
                } else {
                    self.compiler_map
                        .origins
                        .iter()
                        .enumerate()
                        .filter(|(_, actual)| {
                            actual.source.as_ref() == Some(&origin.source)
                                && actual.span.as_ref() == Some(&origin.span)
                        })
                        .map(|(index, _)| index)
                        .collect()
                };
                let nested_sites: Vec<rvn_core::editor_debugger::DebugOrigin> = if producer {
                    self.compiler_map.producer_sites_for_endpoint(origin.source_ordinal,
                        graph.get(), node.get(), pin.get()).filter(|site|
                            site.origin.source.as_ref() == Some(&origin.source)
                            && site.origin.span.as_ref() == Some(&origin.span)).cloned().collect()
                } else {
                    self.compiler_map
                        .sites
                        .iter()
                        .filter(|actual| {
                            actual.origin.source.as_ref() == Some(&origin.source)
                                && actual.origin.span.as_ref() == Some(&origin.span)
                        })
                        .cloned()
                        .collect()
                };
                let requires_producer_admission = producer && nested_sites.is_empty();
                CompilerEndpoint {
                    origin: origin.clone(),
                    emitted_instructions,
                    nested_sites,
                    requires_producer_admission,
                }
            })
            .collect()
    }
    /// Select only breakpoints which the actual Engine has already admitted.
    /// This adapter never constructs a PC, ResumePosition or Site route.
    pub fn breakpoints_for_endpoint<R: rvn_core::Renderer>(
        &self,
        engine: &rvn_core::Engine<R>,
        attachment: &NativeDebuggerAttachment,
        source: &std::path::Path,
        graph: rvn_graph::GraphId,
        node: rvn_graph::NodeId,
        pin: rvn_graph::PinId,
    ) -> Result<Vec<rvn_core::editor_debugger::Breakpoint>, String> {
        if attachment.instance != engine.instance_id()
            || attachment.epoch != engine.runtime_epoch()
            || attachment.binding != self.binding
            || attachment.emitted_script != engine.script
        {
            return Err(
                "Graph debugger attachment belongs to a different actual Engine/program".into(),
            );
        }
        if ConsumerSourceBinding::from_program(&self.program).map_err(|e| e.to_string())?
            != self.binding
        {
            return Err("Graph source binding changed".into());
        }
        let expected = self.compiler_endpoint(source, graph, node, pin);
        if !expected
            .iter()
            .any(|endpoint| !endpoint.requires_producer_admission)
        {
            return Ok(Vec::new());
        }
        let mut offset = 0;
        let mut result = Vec::new();
        loop {
            let page = engine.debug_site_page(offset).map_err(|e| e.to_string())?;
            for entry in &page.entries {
                if entry.origin.as_ref().is_some_and(|origin| {
                    expected.iter().any(|endpoint| {
                        !endpoint.requires_producer_admission
                            && origin.span.as_ref() == Some(&endpoint.origin.span)
                            && origin.graph.as_ref().is_some_and(|actual| {
                                actual.source.as_deref() == Some(source)
                                    && actual.graph == graph.get()
                                    && actual.node == Some(node.get())
                            })
                    })
                }) {
                    if result.len() >= rvn_core::editor_debugger::MAX_BREAKPOINTS {
                        return Err("Graph endpoint exceeds the Core breakpoint budget".into());
                    }
                    result.push(entry.breakpoint.clone());
                }
            }
            offset += page.entries.len();
            if offset >= page.total {
                break;
            }
            if page.entries.is_empty() {
                return Err("Core debugger site page did not advance".into());
            }
        }
        engine
            .validate_debug_breakpoints(&result)
            .map_err(|e| e.to_string())?;
        Ok(result)
    }
    /// Core transports and independently compares the full source-derived step
    /// record. Graph supplies its catalogue/wiring proof without adding a PC.
    pub fn debugger_overlay(
        &self,
        staged: &ParsedProgram,
        nonce: u64,
    ) -> Result<rvn_core::debugger_provenance::Overlay, String> {
        SourceMap::from_program(staged)?.staged_overlay(&self.compiler_map, nonce)
    }
    pub fn attach_debugger<R: rvn_core::Renderer>(
        &self,
        engine: &mut rvn_core::Engine<R>,
        nonce: u64,
    ) -> Result<NativeDebuggerAttachment, String> {
        self.attach_staged_debugger(&self.program, engine, nonce)
    }
    pub fn attach_staged_debugger<R: rvn_core::Renderer>(
        &self, staged: &ParsedProgram, engine: &mut rvn_core::Engine<R>, nonce: u64,
    ) -> Result<NativeDebuggerAttachment, String> {
        let overlay = self.debugger_overlay(staged, nonce)?;
        let staged_binding = ConsumerSourceBinding::from_program(staged).map_err(|e|e.to_string())?;
        engine.install_debug_source_map(overlay, nonce)?;
        Ok(NativeDebuggerAttachment {
            instance: engine.instance_id(),
            epoch: engine.runtime_epoch(),
            binding: self.binding.clone(),
            staged_binding,
            emitted_script: engine.script.clone(),
        })
    }
    pub fn into_debugger_engine<R: rvn_core::Renderer>(
        &self,
        renderer: R,
        rollback_depth: usize,
        nonce: u64,
    ) -> Result<(rvn_core::Engine<R>, NativeDebuggerAttachment), String> {
        // Prepare before constructing/initializing the runtime. Installation
        // then compares against Core's independently frozen actual map.
        self.debugger_overlay(&self.program, nonce)?;
        let mut engine = rvn_core::Engine::from_program_with_debugger(
            self.program.clone(),
            renderer,
            rollback_depth,
        )
        .map_err(|e| e.to_string())?;
        let attachment = self.attach_debugger(&mut engine, nonce)?;
        Ok((engine, attachment))
    }
    pub fn program(&self) -> &ParsedProgram {
        &self.program
    }
    pub fn binding(&self) -> &ConsumerSourceBinding {
        &self.binding
    }
    pub fn compiler_map(&self) -> &SourceMap {
        &self.compiler_map
    }
    pub fn sites(&self) -> &[NativeSourceSite] {
        &self.sites
    }
    pub fn unmapped_compiler_sites(&self) -> &[NativeSourceSite] {
        &self.unmapped_sites
    }
    pub fn portable_receipt(&self) -> PortableNativeSourceReceipt {
        let ordinal = |path: &Option<std::path::PathBuf>| {
            self.program
                .sources
                .iter()
                .position(|source| &source.path == path)
                .expect("verified source ordinal")
        };
        PortableNativeSourceReceipt {
            version: 1,
            emitted: self.binding.emitted,
            content: self.binding.content,
            source_bytes_in_inclusion_order: self
                .program
                .sources
                .iter()
                .map(|source| source.source.clone())
                .collect(),
            root_source_ordinals: self
                .program
                .origins
                .iter()
                .map(|origin| ordinal(&origin.source))
                .collect(),
            plans: self
                .sites
                .iter()
                .map(|site| PortableNativePlan {
                    source_ordinal: ordinal(&Some(site.source.clone())),
                    graph: site.graph.get(),
                    function: site.function.clone(),
                    root: site.root.get(),
                    consumer: site.consumer.get(),
                    route: site.route.clone(),
                    span: site.span.range.clone(),
                    scope: site.scope.clone(),
                    plan: site.plan.clone(),
                })
                .collect(),
        }
    }
    pub fn accepts_portable_receipt(&self, receipt: &PortableNativeSourceReceipt) -> bool {
        // Compare the complete receipt derived from the current actual source
        // loader, not only hashes supplied by a save/client.
        receipt.version == 1 && *receipt == self.portable_receipt()
    }
    pub fn into_engine<R: rvn_core::Renderer>(
        self,
        renderer: R,
        rollback_depth: usize,
    ) -> Result<rvn_core::Engine<R>, rvn_core::RuntimeError> {
        // The actual Engine recalculates and compares its own source binding.
        // Derived compiler_map is not passed as an unverified replacement.
        rvn_core::Engine::from_program_with_consumer_binding(
            self.program,
            self.binding,
            renderer,
            rollback_depth,
        )
    }
}

#[derive(Debug, Clone)]
pub struct CompilerEndpoint {
    pub origin: NativeDebugOrigin,
    pub emitted_instructions: Vec<usize>,
    pub nested_sites: Vec<rvn_core::editor_debugger::DebugOrigin>,
    pub requires_producer_admission: bool,
}

/// Issued only after Core verifies and installs the real compiler overlay.
/// No serde/public constructor: a client cannot claim a different programme.
pub struct NativeDebuggerAttachment {
    instance: u64,
    epoch: u64,
    binding: ConsumerSourceBinding,
    staged_binding: ConsumerSourceBinding,
    emitted_script: rvn_parser::Script,
}
impl NativeDebuggerAttachment {
    pub fn staged_binding(&self) -> &ConsumerSourceBinding { &self.staged_binding }
}
