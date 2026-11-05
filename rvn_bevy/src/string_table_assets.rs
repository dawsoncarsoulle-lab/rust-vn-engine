//! Typed asynchronous assets supplement the validated pre-Init bootstrap.
//! Persistent VN UUIDs never become Bevy handle IDs or saved loading states.
use bevy::{asset::{io::Reader, AssetId, AssetLoader, AssetLoadFailedEvent, AsyncReadExt, LoadContext}, ecs::event::ManualEventReader, prelude::*};
use rvn_core::{string_table::StringTableAsset, string_table_file::{StringTableFile, MAX_STRING_TABLE_FILE_BYTES}, string_table_project::{StringTableDeclarations, MAX_PROJECT_TABLE_BYTES}};
use std::{collections::{BTreeMap, BTreeSet, HashMap}, sync::Arc};
use crate::{resources::VnEngine, vn_command::VnCommand};

// Match the existing checkpoint asset: derives need these aliases when
// Bevy dependencies are selected by target in Cargo.toml.
use bevy::asset as bevy_asset;
use bevy::reflect as bevy_reflect;

#[derive(Asset, TypePath, Clone)]
pub(crate) struct LoadedStringTable(pub(crate) Arc<StringTableAsset>);

#[derive(Default)]
struct StringTableLoader;
impl AssetLoader for StringTableLoader {
    type Asset = LoadedStringTable;
    type Settings = ();
    type Error = std::io::Error;
    async fn load<'a>(&'a self, reader: &'a mut Reader<'_>, _: &'a (), _: &'a mut LoadContext<'_>) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.take((MAX_STRING_TABLE_FILE_BYTES + 1) as u64).read_to_end(&mut bytes).await?;
        let file = StringTableFile::decode(&bytes).map_err(std::io::Error::other)?;
        Ok(LoadedStringTable(Arc::new(file.into_table())))
    }
    fn extensions(&self) -> &[&str] { &["rvnstrings"] }
}

pub(crate) struct StringTableAssetsPlugin { declarations: StringTableDeclarations }
impl StringTableAssetsPlugin {
    pub(crate) fn new(declarations: StringTableDeclarations) -> Self { Self { declarations } }
}
impl Plugin for StringTableAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LoadedStringTable>().init_asset_loader::<StringTableLoader>()
            .insert_resource(TableHost { declarations: self.declarations.clone(), ..default() })
            .init_resource::<StringTableDiagnostics>()
            .add_systems(Startup, bind)
            .add_systems(Update, synchronize
                .before(crate::scene_publication::HostClockSet)
                .before(crate::scene_publication::GameFrameSet)
                .before(crate::systems::stepping_system)
                .before(crate::systems::locale_reload_system)
                .before(crate::systems::locale_lang_watch_system));
    }
}

#[derive(Resource, Default)]
pub(crate) struct StringTableDiagnostics(pub(crate) BTreeMap<String, String>);
impl StringTableDiagnostics {
    fn record(&mut self, id: &str, mut diagnostic: String) {
        if diagnostic.len() > 4096 {
            let mut end = 4096;
            while !diagnostic.is_char_boundary(end) { end -= 1; }
            diagnostic.truncate(end);
        }
        warn!("[string table {id}] {diagnostic}");
        self.0.insert(id.into(), diagnostic);
    }
}

pub(super) struct BoundTable { pub(super) handle: Handle<LoadedStringTable> }
#[derive(Resource, Default)]
pub(super) struct TableHost {
    pub(super) declarations: StringTableDeclarations,
    pub(super) bound: BTreeMap<String, BoundTable>,
    pub(super) identities: HashMap<AssetId<LoadedStringTable>, String>,
    pub(super) events: ManualEventReader<AssetEvent<LoadedStringTable>>,
    pub(super) failures: ManualEventReader<AssetLoadFailedEvent<LoadedStringTable>>,
    pub(super) pending: BTreeSet<String>,
}

pub(crate) fn http_asset_root(project_root: &str, assets: &str) -> String {
    format!("{}/{}", project_root.trim_end_matches('/'), rvn_core::string_table_project::url_path(&assets.replace('\\', "/")))
}

pub(crate) fn http_asset_path(path: &str) -> String {
    rvn_core::string_table_project::url_path(path)
}

fn bind(mut host: ResMut<TableHost>, server: Res<AssetServer>, mut diagnostics: ResMut<StringTableDiagnostics>) {
    if let Err(error) = rvn_core::string_table_project::validate_declarations(&host.declarations) {
        diagnostics.record("project", error); return;
    }
    let declarations = host.declarations.clone();
    for (id, path) in declarations {
        #[cfg(target_arch = "wasm32")]
        let path = http_asset_path(&path);
        // A typed load selects this loader even for a mixed-case extension.
        let handle = server.load::<LoadedStringTable>(path);
        host.identities.insert(handle.id(), id.clone());
        host.bound.insert(id, BoundTable { handle });
    }
}

fn presentation_failure(world: &mut World, message: String) {
    world.resource_mut::<VnEngine>().0.renderer.scene_frame_failed = true;
    if let Some(mut diagnostic) = world.get_resource_mut::<crate::resources::ScriptErrorMessage>() { diagnostic.0 = message; }
    if let Some(mut next) = world.get_resource_mut::<NextState<crate::resources::VnState>>() { next.set(crate::resources::VnState::Error); }
}

pub(super) fn synchronize(world: &mut World) {
    let Some(mut host) = world.remove_resource::<TableHost>() else { return; };
    let missed=host.events.missed_events(world.resource::<Events<AssetEvent<LoadedStringTable>>>())
        .saturating_add(host.failures.missed_events(world.resource::<Events<AssetLoadFailedEvent<LoadedStringTable>>>()));
    if missed>0 {crate::portable_text_resources::mark_missed_events(world,missed);}
    let TableHost{events,identities,pending,..}=&mut host;
    for event in events.read(world.resource::<Events<AssetEvent<LoadedStringTable>>>()) {
        let asset_id = match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::Removed { id } | AssetEvent::LoadedWithDependencies { id } => id,
            AssetEvent::Unused { .. } => continue,
        };
        if let Some(id) = identities.get(asset_id) { pending.insert(id.clone()); }
    }
    let TableHost{failures,identities,..}=&mut host;
    let failures: std::collections::BTreeMap<_,_> = failures.read(world.resource::<Events<AssetLoadFailedEvent<LoadedStringTable>>>())
        .filter_map(|event|identities.get(&event.id).filter(|_|matches!(world.resource::<AssetServer>().load_state(event.id),bevy::asset::LoadState::Failed(_)))
            .map(|id|(id.clone(),crate::portable_text_resources::bounded_diagnostic(event.error.to_string())))).collect();
    let mut failed_ids = BTreeSet::new();
    for (id, error) in failures {
            failed_ids.insert(id.clone());
            // Failed decoding does not replace the last successfully loaded
            // asset. Retain it with an explicit diagnostic; never claim success.
            world.resource_mut::<StringTableDiagnostics>().record(&id, error);
    }
    if world.contains_resource::<crate::debugger_startup::DebugBootstrap>() {
        // Native startup already froze these tables before Init. Drain only
        // notifications proving the same real decoded assets; a reload cannot
        // silently rewrite the suspended Core owner's library.
        let checked:Vec<_>=host.pending.iter().map(|id|{
            let result=(||{
                let owner=crate::text_runtime_owner::current(world)?;
                let binding=host.bound.get(id).ok_or("Undeclared startup table event")?;
                let candidate=world.resource::<Assets<LoadedStringTable>>().get(binding.handle.id()).ok_or("Startup table has no actual decoded asset")?;
                candidate.0.validate()?;
                if owner.resources.ready_table(id)!=Some(candidate.0.as_ref()){return Err("Startup table source changed after admission".into());}
                Ok::<_,String>(())
            })();(id.clone(),result)
        }).collect();
        for(id,result)in checked{match result{
            Ok(())=>{host.pending.remove(&id);let failed=host.bound.get(&id).is_some_and(|binding|matches!(world.resource::<AssetServer>().load_state(binding.handle.id()),bevy::asset::LoadState::Failed(_)));if !failed&&!failed_ids.contains(&id){world.resource_mut::<StringTableDiagnostics>().0.remove(&id);}},
            Err(error)=>world.resource_mut::<StringTableDiagnostics>().record(&id,error),
        }}
        world.insert_resource(host);crate::portable_text_resources::observe_publication(world);return;
    }
    if !world.contains_resource::<VnEngine>() || world.resource::<VnEngine>().0.renderer.scene_batch_pending() {
        world.insert_resource(host); return;
    }
    let pending = std::mem::take(&mut host.pending);
    let mut changed = false;
    for id in pending {
        let Some(binding) = host.bound.get(&id) else { continue; };
        let candidate = world.resource::<Assets<LoadedStringTable>>().get(binding.handle.id()).cloned();
        let result = (|| -> Result<bool, String> {
            if let Some(candidate) = candidate {
                if candidate.0.id != id { return Err("Loaded asset UUID differs from its project declaration".into()); }
                candidate.0.validate()?;
                let resources = world.resource::<VnEngine>().0.text_resources();
                let mut bytes = candidate.0.content_bytes()?;
                for other in host.bound.keys().filter(|other| *other != &id) {
                    if let Some(table) = resources.ready_table(other) { bytes = bytes.checked_add(table.content_bytes()?).ok_or("Project string-table content size overflow")?; }
                }
                if bytes > MAX_PROJECT_TABLE_BYTES { return Err("Project string-table content exceeds 64 MiB".into()); }
                if resources.ready_table(&id) == Some(candidate.0.as_ref()) { return Ok(false); }
                crate::display_reveal::flush_world(world)?;
                world.resource_mut::<VnEngine>().0.text_resources_mut().publish_if_changed(candidate.0).map(|revision| revision.is_some())
            } else {
                if world.resource::<VnEngine>().0.text_resources().ready_table(&id).is_none() { return Ok(false); }
                crate::display_reveal::flush_world(world)?;
                world.resource_mut::<VnEngine>().0.text_resources_mut().unload(&id)
            }
        })();
        match result {
            Ok(publication) => {
                changed |= publication;
                // A late Added/Loaded notification can still point at the
                // previous retained asset after a reload failed. Clear its
                // diagnostic only after an actually successful current load.
                let failed = matches!(world.resource::<AssetServer>().load_state(binding.handle.id()), bevy::asset::LoadState::Failed(_));
                if !failed && !failed_ids.contains(&id) { world.resource_mut::<StringTableDiagnostics>().0.remove(&id); }
            }
            Err(error) => world.resource_mut::<StringTableDiagnostics>().record(&id, error),
        }
    }
    world.insert_resource(host);
    crate::portable_text_resources::observe_publication(world);
    if !changed { return; }
    let refresh = (|| -> Result<Vec<VnCommand>, String> {
        let mut engine = world.resource_mut::<VnEngine>();
        crate::systems::locale::refresh_localized_interfaces(&mut engine).map_err(|error| error.to_string())?;
        let mut commands = engine.0.renderer.take_pending();
        if let Some(rvn_core::Interaction::Dialogue { character, text }) = engine.0.displayed_interaction().map_err(|error| error.to_string())? {
            commands.push(crate::display_reveal::command(&engine, character, text));
        }
        Ok(commands)
    })();
    match refresh {
        Ok(commands) => { if let Some(mut output) = world.get_resource_mut::<Events<VnCommand>>() { output.send_batch(commands); } }
        Err(error) => presentation_failure(world, format!("String table presentation failed: {error}")),
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "string_table_asset_tests.rs"]
mod tests;
