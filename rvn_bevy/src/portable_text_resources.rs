//! PRIVATE host preparation: receipts are rebuilt from actual AssetServer,
//! event readers, declared handles, decoded assets and published Core content.
use crate::{
    resources::VnEngine,
    string_table_assets::{LoadedStringTable, StringTableDiagnostics, TableHost},
};
use bevy::asset as bevy_asset;
use bevy::reflect as bevy_reflect;
use bevy::{
    asset::{io::Reader, AssetLoadFailedEvent, AssetLoader, AsyncReadExt, LoadContext},
    ecs::event::ManualEventReader,
    prelude::*,
};
use rvn_core::{
    locale::{LocaleCatalog, LocaleContentSnapshot, LocaleManager},
    string_table::{StringTableContentSnapshot, StringTableRevision},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PublishedLoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed,
}
fn table_load_state(
    server: &AssetServer,
    id: bevy::asset::AssetId<LoadedStringTable>,
) -> PublishedLoadState {
    match server.load_state(id) {
        bevy::asset::LoadState::NotLoaded => PublishedLoadState::NotLoaded,
        bevy::asset::LoadState::Loading => PublishedLoadState::Loading,
        bevy::asset::LoadState::Loaded => PublishedLoadState::Loaded,
        bevy::asset::LoadState::Failed(_) => PublishedLoadState::Failed,
    }
}

#[derive(Resource)]
pub(crate) struct PublicationBook {
    authority: Arc<()>,
    epoch: u64,
    missed: usize,
    states: BTreeMap<String, PublishedLoadState>,
    registry: Option<StringTableRevision>,
    diagnostics: BTreeMap<String, String>,
    journal: VecDeque<PublicationEvent>,
    exhausted: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct PublicationEvent {
    pub epoch: u64,
    pub tables: usize,
    pub failed: usize,
    pub diagnostics: usize,
}
impl Default for PublicationBook {
    fn default() -> Self {
        Self {
            authority: Arc::new(()),
            epoch: 0,
            missed: 0,
            states: Default::default(),
            registry: None,
            diagnostics: Default::default(),
            journal: Default::default(),
            exhausted: false,
        }
    }
}
pub(crate) fn mark_missed_events(world: &mut World, count: usize) {
    if !world.contains_resource::<PublicationBook>() {
        world.insert_resource(PublicationBook::default());
    }
    let mut book = world.resource_mut::<PublicationBook>();
    book.missed = book.missed.saturating_add(count);
}
pub(crate) fn observe_publication(world: &mut World) {
    let Some(host) = world.get_resource::<TableHost>() else {
        return;
    };
    let Some(server) = world.get_resource::<AssetServer>() else {
        return;
    };
    let states = host
        .bound
        .iter()
        .map(|(id, binding)| (id.clone(), table_load_state(server, binding.handle.id())))
        .collect();
    let registry = crate::text_runtime_owner::current(world).ok().map(|owner|owner.resources.revision_stamp());
    if !world.contains_resource::<PublicationBook>() {
        world.insert_resource(PublicationBook::default());
    }
    let changed = {
        let book = world.resource::<PublicationBook>();
        book.states != states
            || book.registry != registry
            || book.diagnostics != world.resource::<StringTableDiagnostics>().0
    };
    if changed {
        let diagnostics = world.resource::<StringTableDiagnostics>().0.clone();
        let mut book = world.resource_mut::<PublicationBook>();
        let Some(next) = book.epoch.checked_add(1) else {
            book.exhausted = true;
            return;
        };
        book.epoch = next;
        book.states = states;
        book.registry = registry;
        book.diagnostics = diagnostics;
        if book.journal.len() == 256 {
            book.journal.pop_front();
        }
        let event = PublicationEvent {
            epoch: next,
            tables: book.states.len(),
            failed: book
                .states
                .values()
                .filter(|state| **state == PublishedLoadState::Failed)
                .count(),
            diagnostics: book.diagnostics.len(),
        };
        book.journal.push_back(event);
    }
}

/// Actual decoded locale source. Language comes from the verified manifest,
/// never from an AssetId, host pointer or an invented empty provider context.
#[derive(Asset, TypePath, Clone)]
pub(crate) struct LoadedLocaleSource(pub(crate) String, LocaleLoadedWitness);
/// Private evidence emitted only by the real registered loader. The decoded
/// String remains readable; replacing it cannot retain a valid live receipt.
#[derive(Clone)]
struct LocaleLoadedWitness { raw_digest:[u8;32], source_path:bevy::asset::AssetPath<'static>, source_authority:Arc<()> }
impl LoadedLocaleSource {
    fn authentic_raw(&self)->Result<&str,String>{
        if <[u8;32]>::from(Sha256::digest(self.0.as_bytes()))!=self.1.raw_digest {
            return Err("Decoded locale no longer matches its actual loader witness".into());
        }
        Ok(&self.0)
    }
}
struct LocaleSourceLoader { source_authority:Option<Arc<()>> }
impl FromWorld for LocaleSourceLoader{
    fn from_world(world:&mut World)->Self{
        // Borrow the existing registered host authority, not a new registry,
        // generation counter or ID. init_asset_loader remains the real route.
        Self{source_authority:world.get_resource::<KnownTextSources>().map(|sources|sources.authority.clone())}
    }
}
impl AssetLoader for LocaleSourceLoader {
    type Asset = LoadedLocaleSource;
    type Settings = ();
    type Error = std::io::Error;
    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _: &'a (),
        _load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let source_authority=self.source_authority.as_ref().ok_or_else(||std::io::Error::other("Actual registered locale loader has no retained provider authority"))?.clone();
        let mut bytes = Vec::new();
        reader
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(std::io::Error::other("Locale source exceeds 8 MiB"));
        }
        #[cfg(all(test,not(target_arch="wasm32"),not(target_os="android")))]
        native_absence_tests::pause_decode(_load_context.path()).await;
        let raw_digest=Sha256::digest(&bytes).into();
        let content = String::from_utf8(bytes).map_err(std::io::Error::other)?;
        LocaleCatalog::parse("validation", &content)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(LoadedLocaleSource(content,LocaleLoadedWitness{raw_digest,source_path:_load_context.asset_path().clone(),source_authority}))
    }
    fn extensions(&self) -> &[&str] {
        &["toml", "rvnlocale"]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocaleDeclarations {
    pub default_lang: String,
    pub current_lang: String,
    pub available: Vec<String>,
    pub files: BTreeMap<String, String>,
}
impl LocaleDeclarations {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let available: BTreeSet<_> = self.available.iter().collect();
        if self.files.len() > 4096
            || available.len() != self.available.len()
            || available.len() != self.files.len()
            || !available.contains(&self.default_lang)
            || !available.contains(&self.current_lang)
            || self.files.keys().any(|lang| !available.contains(lang))
        {
            return Err(
                "Locale asset declarations are not a complete selected language closure".into(),
            );
        }
        let mut paths = BTreeSet::new();
        let mut bytes = 0usize;
        for (lang, path) in &self.files {
            if lang.is_empty()
                || lang.len() > 256
                || !lang
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                || path.is_empty()
                || path.len() > 4096
                || path
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\\' | ':' | '#' | '?'))
                || path
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
                || !paths.insert(path)
            {
                return Err("Invalid or aliased locale asset declaration".into());
            }
            bytes = bytes
                .checked_add(lang.len() + path.len())
                .ok_or("Locale declaration size overflow")?;
            if bytes > 2 * 1024 * 1024 {
                return Err("Locale asset declarations exceed 2 MiB".into());
            }
        }
        Ok(())
    }
}

const LOCALE_SOURCE: &str = "rvn-closed-locales-v1";
#[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
#[path="native_locale_provider.rs"]
mod native_locale_provider;
/// Private provenance from the real launch, never a portable permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocaleSelectionMode { StrictDeclared, NormalCompatible }
#[derive(Clone)]
pub(crate) struct ClosedLocaleRoot{
    path:String,
    selection_mode:LocaleSelectionMode,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    witness:native_locale_provider::NativeLocaleRoot,
}
#[cfg(not(target_arch="wasm32"))]
pub(crate) fn closed_native_locale_root(root:&std::path::Path)->Result<ClosedLocaleRoot,String>{
    #[cfg(not(target_os="android"))]
    {let witness=native_locale_provider::NativeLocaleRoot::capture(root)?;Ok(ClosedLocaleRoot{path:witness.path().to_string_lossy().into_owned(),selection_mode:LocaleSelectionMode::StrictDeclared,witness})}
    #[cfg(target_os="android")]
    {Ok(ClosedLocaleRoot{path:root.canonicalize().map_err(|e|e.to_string())?.to_string_lossy().into_owned(),selection_mode:LocaleSelectionMode::StrictDeclared})}
}
impl ClosedLocaleRoot {
    pub(crate) fn selection_mode(&self)->LocaleSelectionMode { self.selection_mode }
}
/// Only the actual loader supplies its explicit-closed argument. The ordinary
/// root/registration helpers retain strict provenance for existing callers.
#[cfg(not(target_arch="wasm32"))]
pub(crate) fn closed_native_locale_root_for_launch(root:&std::path::Path,explicit_closed:bool)->Result<ClosedLocaleRoot,String>{
    let mut verified=closed_native_locale_root(root)?;
    if !explicit_closed { verified.selection_mode=LocaleSelectionMode::NormalCompatible; }
    Ok(verified)
}
#[cfg(not(target_arch="wasm32"))]
pub(crate) fn native_locale_root(root:&std::path::Path)->Result<String,String>{
    #[cfg(not(target_os="android"))]
    {Ok(native_locale_provider::NativeLocaleRoot::capture(root)?.path().to_string_lossy().into_owned())}
    #[cfg(target_os="android")]
    {root.canonicalize().map(|p|p.to_string_lossy().into_owned()).map_err(|e|e.to_string())}
}
#[cfg(not(target_arch="wasm32"))]
pub(crate) fn discover_native_locale_langs(root:&std::path::Path)->Result<Vec<String>,String>{
    let root=std::path::PathBuf::from(native_locale_root(root)?);
    let entries=match std::fs::read_dir(&root){Ok(entries)=>entries,Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),Err(error)=>return Err(error.to_string())};
    let mut langs=Vec::new();
    for entry in entries{let entry=entry.map_err(|e|e.to_string())?;let path=entry.path();let metadata=entry.metadata().map_err(|e|e.to_string())?;
        if metadata.is_file()&&path.extension().and_then(|e|e.to_str())==Some("toml"){if let Some(lang)=path.file_stem().and_then(|s|s.to_str()){langs.push(lang.to_owned());}}
    }langs.sort();Ok(langs)
}
#[derive(Resource)]
struct KnownTextSources {
    authority: Arc<()>,
    table_root: String,
    locale_root: String,
    policy: u32,
    selection_mode: LocaleSelectionMode,
    server: Option<AssetServer>,
    // Captured from the AssetPlugin actually installed, never inferred Default.
    meta_check: Option<bevy::asset::AssetMetaCheck>,
    table_reader: Arc<std::sync::atomic::AtomicUsize>,
    locale_reader: Arc<std::sync::atomic::AtomicUsize>,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    native_root: Option<native_locale_provider::NativeLocaleRoot>,
}
/// Observation of retained provenance only; this is not a live receipt and
/// grants no permission to read or dynamically select any language.
pub(crate) fn locale_selection_mode(world:&World)->Result<LocaleSelectionMode,String>{
    world.get_resource::<KnownTextSources>().map(|sources|sources.selection_mode)
        .ok_or_else(||"Missing actual registered text provider".into())
}
/// Register before AssetPlugin. The absolute root is live provider identity;
/// only bounded relative declaration paths enter the portable receipt.
pub(crate) fn register_text_sources(
    app: &mut App,
    table_root: String,
    root: String,
) -> Result<(), String> {
    #[cfg(not(target_arch="wasm32"))]
    let verified=closed_native_locale_root(std::path::Path::new(&root))?;
    #[cfg(target_arch="wasm32")]
    let verified=ClosedLocaleRoot{path:root,selection_mode:LocaleSelectionMode::StrictDeclared};
    register_verified_text_sources(app,table_root,verified)
}
pub(crate) fn register_verified_text_sources(app:&mut App,table_root:String,verified:ClosedLocaleRoot)->Result<(),String>{
    use bevy::asset::io::AssetSource;
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    verified.witness.validate()?;
    let selection_mode=verified.selection_mode;
    let root=verified.path.clone();
    if app.world().contains_resource::<AssetServer>() {
        return Err("Closed text sources must be installed before the actual AssetServer".into());
    }
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    let native_root=verified.witness;
    let table_root_identity = table_root.clone();
    let locale_root_identity = root.clone();
    let table_reader=Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let locale_reader=Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let table_identity=table_reader.clone();let locale_identity=locale_reader.clone();
    let mut table_factory=AssetSource::get_default_reader(table_root.clone());
    let mut locale_factory=AssetSource::get_default_reader(root.clone());
    app.register_asset_source(
        bevy::asset::io::AssetSourceId::Default,
        AssetSource::build()
            .with_reader(move||{let reader=table_factory();let _=table_identity.compare_exchange(0,reader.as_ref() as *const dyn bevy::asset::io::ErasedAssetReader as *const () as usize,std::sync::atomic::Ordering::AcqRel,std::sync::atomic::Ordering::Acquire);reader})
            .with_watcher(AssetSource::get_default_watcher(
                table_root,
                std::time::Duration::from_millis(300),
            )),
    );
    app.register_asset_source(
        LOCALE_SOURCE,
        AssetSource::build()
            .with_reader(move||{let reader=locale_factory();let _=locale_identity.compare_exchange(0,reader.as_ref() as *const dyn bevy::asset::io::ErasedAssetReader as *const () as usize,std::sync::atomic::Ordering::AcqRel,std::sync::atomic::Ordering::Acquire);reader})
            .with_watcher(AssetSource::get_default_watcher(
                root,
                std::time::Duration::from_millis(300),
            )),
    );
    app.insert_resource(KnownTextSources {
        authority: Arc::new(()),
        selection_mode,
        table_root: table_root_identity,
        locale_root: locale_root_identity,
        policy: 1,
        server:None,meta_check:None,table_reader,locale_reader,
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        native_root:Some(native_root),
    });
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_native_frozen_locales(
    root: &std::path::Path,
    declarations: &LocaleDeclarations,
) -> Result<LocaleManager, String> {
    let root=closed_native_locale_root(root)?;
    load_closed_native_frozen_locales(&root,declarations)
}
#[cfg(not(target_arch="wasm32"))]
pub(crate) fn load_closed_native_frozen_locales(root:&ClosedLocaleRoot,declarations:&LocaleDeclarations)->Result<LocaleManager,String>{
    use std::io::Read;
    declarations.validate()?;
    #[cfg(not(target_os="android"))]
    let witness=&root.witness;
    #[cfg(not(target_os="android"))]
    witness.validate()?;
    let allow_absence=root.selection_mode==LocaleSelectionMode::NormalCompatible;
    let root=std::path::PathBuf::from(&root.path);
    let mut catalogs=HashMap::new();let mut total=0usize;
    for (lang,path) in &declarations.files {
        #[cfg(not(target_os="android"))]
        witness.validate_file(path)?;
        let file=match std::fs::File::open(root.join(path)){
            Ok(file)=>file,
            #[cfg(not(target_os="android"))]
            Err(error) if allow_absence&&error.kind()==std::io::ErrorKind::NotFound=>{catalogs.insert(lang.clone(),empty_catalog(lang));continue;},
            Err(error)=>return Err(error.to_string()),
        };
        let mut bytes=Vec::new();file.take(8*1024*1024+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        if bytes.len()>8*1024*1024{return Err("Locale source exceeds 8 MiB".into());}
        total=total.checked_add(bytes.len()).filter(|n|*n<=64*1024*1024).ok_or("Locale source closure exceeds 64 MiB")?;
        let content=String::from_utf8(bytes).map_err(|e|e.to_string())?;
        catalogs.insert(lang.clone(),LocaleCatalog::parse(lang,&content).map_err(|e|e.to_string())?);
    }
    #[cfg(not(target_os="android"))]
    witness.validate()?;
    LocaleManager::from_frozen_catalogs(
        &declarations.default_lang,
        &declarations.current_lang,
        declarations.available.clone(),
        catalogs,
    )
}

#[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
struct LocaleRetryCompletion(Arc<std::sync::atomic::AtomicBool>);
#[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
impl Drop for LocaleRetryCompletion{fn drop(&mut self){self.0.store(true,std::sync::atomic::Ordering::Release);}}
// Explicit terminal refusal is distinct from in-flight I/O/retry diagnostics.
// Its identity is the genuine current owner instance, not a new source ID.
enum StartupLocaleRefusal { FrozenContentChanged{instance:u64} }
#[derive(Resource)]
pub(crate) struct LocaleAssetHost {
    declarations: LocaleDeclarations,
    bound: BTreeMap<String, Handle<LoadedLocaleSource>>,
    identities: HashMap<bevy::asset::AssetId<LoadedLocaleSource>, String>,
    events: ManualEventReader<AssetEvent<LoadedLocaleSource>>,
    failures: ManualEventReader<AssetLoadFailedEvent<LoadedLocaleSource>>,
    pending: bool,
    diagnostics: BTreeMap<String, String>,
    missed: usize,
    publication: u64,
    startup_refusal:Option<StartupLocaleRefusal>,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    scan: Option<native_locale_provider::PendingScan>,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    next_scan: std::time::Instant,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    root_present: Option<bool>,
    // Reload scheduling only; never evidence for admission or portable content.
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    observed_probe: BTreeMap<String,native_locale_provider::Probe>,
    // Latest authenticated observation, including during Loading. Negative
    // admission fence only: neither hashes nor metadata parsing prove Loaded.
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    latest_probe: BTreeMap<String,native_locale_provider::Probe>,
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    metadata_errors:BTreeMap<String,String>,
    // Coalesces only the actual public load_acquire call. Completion is not
    // source authority and cannot authenticate a successful/failed catalogue.
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    retry_in_flight:BTreeMap<String,Arc<std::sync::atomic::AtomicBool>>,
}
pub(crate) struct ClosedLocaleAssetsPlugin {
    declarations: LocaleDeclarations,
}
impl ClosedLocaleAssetsPlugin {
    pub(crate) fn new(declarations: LocaleDeclarations) -> Result<Self, String> {
        declarations.validate()?;
        Ok(Self { declarations })
    }
}
impl Plugin for ClosedLocaleAssetsPlugin {
    fn build(&self, app: &mut App) {
        // A different AssetServer or opaque reader cannot inherit this policy.
        // Processed servers are unsupported by this native unprocessed source.
        let meta_check={let plugins=app.get_added_plugins::<bevy::asset::AssetPlugin>();
            if plugins.len()==1 && matches!(plugins[0].mode,bevy::asset::AssetMode::Unprocessed){Some(plugins[0].meta_check.clone())}else{None}};
        if let Some(server)=app.world().get_resource::<AssetServer>().cloned(){
            if let Some(mut sources)=app.world_mut().get_resource_mut::<KnownTextSources>(){sources.server=Some(server);sources.meta_check=meta_check;}
        }
        app.init_asset::<LoadedLocaleSource>()
            .init_asset_loader::<LocaleSourceLoader>()
            .init_resource::<PublicationBook>()
            .insert_resource(LocaleAssetHost {
                declarations: self.declarations.clone(),
                bound: Default::default(),
                identities: Default::default(),
                events: Default::default(),
                failures: Default::default(),
                pending: true,
                diagnostics: Default::default(),
                missed: 0,
                publication: 0,
                startup_refusal:None,
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                scan:None,
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                next_scan:std::time::Instant::now(),
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                root_present:None,
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                observed_probe:Default::default(),
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                latest_probe:Default::default(),
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                metadata_errors:Default::default(),
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                retry_in_flight:Default::default(),
            })
            .add_systems(Startup, bind_locales)
            .add_systems(
                Update,
                publish_locales
                    .after(crate::string_table_assets::synchronize)
                    .before(crate::scene_publication::HostClockSet)
                    .before(crate::scene_publication::GameFrameSet)
                    .before(crate::systems::stepping_system)
                    .before(crate::systems::locale_lang_watch_system),
            );
    }
}
fn bind_locales(mut host: ResMut<LocaleAssetHost>, server: Res<AssetServer>) {
    for (lang, path) in host.declarations.files.clone() {
        #[cfg(target_arch = "wasm32")]
        let path = crate::string_table_assets::http_asset_path(&path);
        let handle = server.load::<LoadedLocaleSource>(
            bevy::asset::AssetPath::from(path).with_source(LOCALE_SOURCE),
        );
        host.identities.insert(handle.id(), lang.clone());
        host.bound.insert(lang, handle);
    }
}
fn diagnostic(message: String) -> String {
    if message.len() <= 4096 {
        return message;
    }
    let mut end = 4096;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].into()
}
pub(crate) fn bounded_diagnostic(message: String) -> String {
    diagnostic(message)
}
fn empty_catalog(lang:&str)->LocaleCatalog{
    rvn_core::locale::LocaleTable{lang:lang.to_owned(),strings:HashMap::new()}.into()
}
fn source_binding(world:&World)->Result<(),String>{
    use std::sync::atomic::Ordering;
    let sources=world.get_resource::<KnownTextSources>().ok_or("Opaque text asset provider")?;
    let server=world.get_resource::<AssetServer>().ok_or("Missing actual AssetServer")?;
    let retained=sources.server.as_ref().ok_or("Actual text AssetServer was not sealed")?;
    for (source,identity) in [(bevy::asset::io::AssetSourceId::Default,&sources.table_reader),(bevy::asset::io::AssetSourceId::from(LOCALE_SOURCE),&sources.locale_reader)]{
        let actual=server.get_source(source.clone()).map_err(|e|e.to_string())?.reader();
        let held=retained.get_source(source).map_err(|e|e.to_string())?.reader();
        let address=actual as *const dyn bevy::asset::io::ErasedAssetReader as *const () as usize;
        if identity.load(Ordering::Acquire)==0||address!=identity.load(Ordering::Acquire)||!std::ptr::eq(actual,held){return Err("Text AssetServer/source reader authority changed".into());}
    }
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    {
        sources.native_root.as_ref().ok_or("Missing native locale provider root")?.validate()?;
        if !matches!(server.mode(),bevy::asset::AssetServerMode::Unprocessed)||sources.meta_check.is_none(){return Err("Actual native locale metadata policy is not authenticated".into());}
    }
    Ok(())
}
fn locale_catalogs(world:&World,host:&LocaleAssetHost)->Result<(HashMap<String,LocaleCatalog>,BTreeSet<String>),String>{
    host.declarations.validate()?;source_binding(world)?;
    if host.bound.keys().ne(host.declarations.files.keys()){return Err("Locale handle closure is incomplete".into());}
    let server=world.resource::<AssetServer>();let assets=world.resource::<Assets<LoadedLocaleSource>>();
    let mut catalogs=HashMap::new();let mut absent=BTreeSet::new();let mut bytes=0usize;
    for (lang,handle) in &host.bound{
        let path=&host.declarations.files[lang];
        if host.identities.get(&handle.id())!=Some(lang)||server.get_path(handle.id())!=Some(bevy::asset::AssetPath::from(path.clone()).with_source(LOCALE_SOURCE)){
            return Err("Locale handle/source/path binding changed".into());
        }
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        world.resource::<KnownTextSources>().native_root.as_ref().ok_or("Missing native locale root")?.validate_file(path)?;
        match server.load_state(handle.id()){
            bevy::asset::LoadState::Loaded=>{
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                if !world.resource::<KnownTextSources>().native_root.as_ref().ok_or("Missing native root")?.validate()?{return Err("Loaded locale root disappeared; actual reload is required".into());}
                let loaded=assets.get(handle.id()).ok_or("Loaded locale has no retained decoded value")?;
                if !Arc::ptr_eq(&loaded.1.source_authority,&world.resource::<KnownTextSources>().authority){return Err("Actual loader provider authority differs from this retained text host".into());}
                if loaded.1.source_path!=bevy::asset::AssetPath::from(path.clone()).with_source(LOCALE_SOURCE){return Err("Actual loader source path differs from the bound locale source/path".into());}
                let raw=loaded.authentic_raw()?;
                #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
                {
                    let probe=host.latest_probe.get(lang).ok_or("Actual locale source observation is still pending")?;
                    if probe.content!=Some(Sha256::digest(raw.as_bytes()).into()){return Err("Observed locale source differs from the actual loaded source; reload is pending".into());}
                    if let Some(error)=host.metadata_errors.get(lang){return Err(format!("Actual locale metadata is not admissible: {error}"));}
                    let fresh=native_locale_provider::current_metadata_fingerprint(world.resource::<KnownTextSources>(),server,path)?;
                    if fresh!=probe.metadata{return Err("Locale metadata changed after asynchronous validation".into());}
                }
                if raw.len()>8*1024*1024{return Err("Decoded locale exceeds 8 MiB".into());}
                bytes=bytes.checked_add(raw.len()).filter(|n|*n<=64*1024*1024).ok_or("Locale closure exceeds 64 MiB")?;
                catalogs.insert(lang.clone(),LocaleCatalog::parse(lang,raw).map_err(|e|e.to_string())?);
            }
            #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
            state if native_locale_provider::failed_not_found(&state)=>{
                if locale_selection_mode(world)?!=LocaleSelectionMode::NormalCompatible{return Err("Strict closed locale source is absent".into());}
                if host.latest_probe.get(lang).is_some_and(|probe|probe.content.is_some()){return Err("Observed locale source is present; actual reload is pending".into());}
                native_locale_provider::prove_absent(world.resource::<KnownTextSources>(),server,path)?;
                absent.insert(lang.clone());catalogs.insert(lang.clone(),empty_catalog(lang));
            }
            bevy::asset::LoadState::Failed(error)=>return Err(format!("Actual locale source failed: {error}")),
            _=>return Err("Locale publication waits for actual source loads".into()),
        }
    }Ok((catalogs,absent))
}
#[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
fn poll_native_locales(world:&World,host:&mut LocaleAssetHost){
    let Some(sources)=world.get_resource::<KnownTextSources>()else{return;};
    let Some(server)=world.get_resource::<AssetServer>()else{return;};
    if let Err(error)=source_binding(world){host.diagnostics.insert("provider".into(),diagnostic(error));host.pending=true;return;}
    let reader=match native_locale_provider::actual_reader(sources,server){Ok(reader)=>native_locale_provider::reader_identity(reader),Err(error)=>{host.diagnostics.insert("provider".into(),diagnostic(error));host.pending=true;return;}};
    let handles=host.bound.iter().map(|(lang,handle)|(lang.clone(),handle.id())).collect::<BTreeMap<_,_>>();
    if host.scan.as_ref().is_some_and(|scan|!Arc::ptr_eq(&scan.authority,&sources.authority)||scan.reader!=reader||scan.files!=host.declarations.files||scan.handles!=handles){
        host.scan=None;host.pending=true;host.next_scan=std::time::Instant::now();
    }
    if let Some(scan)=host.scan.as_mut(){
        if let Some(result)=bevy::tasks::block_on(bevy::tasks::poll_once(&mut scan.task)){
            host.scan=None;host.next_scan=std::time::Instant::now()+std::time::Duration::from_millis(300);
            match result{
                Err(error)=>{host.diagnostics.insert("provider".into(),diagnostic(error));host.pending=true;},
                Ok(native_locale_provider::ScanResult{present,probes,metadata_errors})=>{
                    // Fence task results again on receipt, before using any
                    // observation. A worker argument is not provider authority.
                    if sources.native_root.as_ref().and_then(|root|root.validate().ok())!=Some(present)||native_locale_provider::actual_reader(sources,server).is_err(){
                        host.diagnostics.insert("provider".into(),"Locale provider changed before probe receipt".into());host.pending=true;return;
                    }
                    host.diagnostics.remove("provider");
                    if host.root_present.replace(present)!=Some(present){host.pending=true;}
                    if host.latest_probe!=probes||host.metadata_errors!=metadata_errors{host.pending=true;}
                    host.latest_probe=probes.clone();host.metadata_errors=metadata_errors;
                    let assets=world.resource::<Assets<LoadedLocaleSource>>();
                    for(lang,probe)in probes{let Some(handle)=host.bound.get(&lang)else{continue;};
                        #[cfg(test)]
                        native_absence_tests::received_probe(std::path::Path::new(&host.declarations.files[&lang]),probe);
                        if host.retry_in_flight.get(&lang).is_some_and(|done|!done.load(std::sync::atomic::Ordering::Acquire)){continue;}
                        host.retry_in_flight.remove(&lang);
                        // A scan during an actual load cannot consume the
                        // change that the next settled attempt must validate.
                        let state=server.load_state(handle.id());
                        if matches!(&state,bevy::asset::LoadState::Loading|bevy::asset::LoadState::NotLoaded){continue;}
                        let previous=host.observed_probe.insert(lang.clone(),probe);
                        let metadata_changed=previous.map(|old|old.metadata!=probe.metadata).unwrap_or(probe.metadata.is_some());
                        let matches=match probe.content{
                            None=>native_locale_provider::failed_not_found(&server.load_state(handle.id())),
                            Some(digest)=>matches!(server.load_state(handle.id()),bevy::asset::LoadState::Loaded)&&assets.get(handle.id()).is_some_and(|asset|<[u8;32]>::from(Sha256::digest(asset.0.as_bytes()))==digest),
                        };
                        if !matches||metadata_changed{
                            let path=bevy::asset::AssetPath::from(host.declarations.files[&lang].clone()).with_source(LOCALE_SOURCE);
                            if matches!(&state,bevy::asset::LoadState::Failed(_)){
                                let done=Arc::new(std::sync::atomic::AtomicBool::new(false));
                                let requested=server.load_acquire::<LoadedLocaleSource,_>(path,LocaleRetryCompletion(done.clone()));
                                if requested.id()!=handle.id(){host.diagnostics.insert("provider".into(),"Actual locale retry changed its bound handle".into());}
                                host.retry_in_flight.insert(lang.clone(),done);
                            }else{
                                // Force reload leaves the previous Loaded
                                // state exposed. Admission separately fences
                                // it with real raw/metadata observations.
                                server.reload(path);
                            }
                            host.pending=true;
                        }
                    }
                }
            }
        }
    }
    if host.scan.is_none()&&std::time::Instant::now()>=host.next_scan{
        if let Some(root)=sources.native_root.clone(){
            let files=host.declarations.files.clone();
            host.scan=Some(native_locale_provider::PendingScan{
                task:bevy::tasks::IoTaskPool::get().spawn(native_locale_provider::scan(server.clone(),root,files.clone(),reader,sources.meta_check.clone().expect("authenticated above"))),
                authority:sources.authority.clone(),reader,files,handles,
            });
        }
    }
}
fn terminal_startup_locale_refusal(world:&World,host:&LocaleAssetHost)->Option<&'static str>{
    let current=world.get_resource::<crate::debugger_startup::DebugBootstrap>()?.owner.snapshot().instance;
    match host.startup_refusal.as_ref(){
        Some(StartupLocaleRefusal::FrozenContentChanged{instance}) if *instance==current=>Some("Actual Startup locale source changed after admission"),
        _=>None,
    }
}
fn publish_locales(world: &mut World) {
    let Some(mut host) = world.remove_resource::<LocaleAssetHost>() else {
        return;
    };
    #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
    poll_native_locales(world,&mut host);
    host.missed = host.missed.saturating_add(
        host.events
            .missed_events(world.resource::<Events<AssetEvent<LoadedLocaleSource>>>())
            .saturating_add(host.failures.missed_events(
                world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>(),
            )),
    );
    let LocaleAssetHost {
        events,
        failures,
        identities,
        pending,
        diagnostics,
        ..
    } = &mut host;
    for event in events.read(world.resource::<Events<AssetEvent<LoadedLocaleSource>>>()) {
        let id = match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::Removed { id }
            | AssetEvent::LoadedWithDependencies { id } => id,
            AssetEvent::Unused { .. } => continue,
        };
        if identities.contains_key(id) {
            *pending = true;
        }
    }
    for event in failures.read(world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>())
    {
        if let Some(lang) = identities.get(&event.id) {
            diagnostics.insert(lang.clone(), diagnostic(event.error.to_string()));
            *pending = true;
        }
    }
    // A proven terminal rejection cannot be accidentally turned back into
    // transient Loading by a later scan. It applies only to this actual owner.
    if terminal_startup_locale_refusal(world,&host).is_some(){host.pending=true;world.insert_resource(host);return;}
    // Normal Game hotreload and a genuinely different owner retain their old
    // contract. Restoring Anchor does not install a permanent provider lock.
    host.startup_refusal=None;
    let blocked = crate::text_runtime_owner::current(world).map(|owner|owner.blocked).unwrap_or(true);
    if !host.pending || blocked {
        world.insert_resource(host);
        return;
    }
    let result = (|| -> Result<LocaleManager, String> {
        let (catalogs,_)=locale_catalogs(world,&host)?;
        if host.diagnostics.contains_key("provider"){return Err("Native provider probe has not recovered".into());}
        let owner=crate::text_runtime_owner::current(world)?;
        let current = owner.locale
            .filter(|locale| host.declarations.files.contains_key(locale.current_lang()))
            .map(|locale| locale.current_lang().to_owned())
            .unwrap_or_else(|| host.declarations.current_lang.clone());
        LocaleManager::from_frozen_catalogs(
            &host.declarations.default_lang,
            &current,
            host.declarations.available.clone(),
            catalogs,
        )
    })();
    if let Ok(candidate) = result {
        let equivalent = crate::text_runtime_owner::current(world).ok().and_then(|owner|owner.locale)
            .and_then(|locale| locale.portable_receipt().ok())
            == candidate.portable_receipt().ok();
        if !equivalent {
            if let Some(startup)=world.get_resource::<crate::debugger_startup::DebugBootstrap>(){
                host.startup_refusal=Some(StartupLocaleRefusal::FrozenContentChanged{instance:startup.owner.snapshot().instance});
                host.diagnostics.insert("publication".into(),"Startup locale source changed after admission".into());
                world.insert_resource(host);return;
            }
            if let Err(error) = crate::display_reveal::flush_world(world) {
                host.diagnostics
                    .insert("publication".into(), diagnostic(error));
                world.insert_resource(host);
                return;
            }
            let previous = world.resource_mut::<VnEngine>().0.locale.replace(candidate);
            if let Err(error) = crate::systems::locale::refresh_localized_interfaces(
                &mut world.resource_mut::<VnEngine>(),
            ) {
                host.diagnostics
                    .insert("publication".into(), diagnostic(error.to_string()));
                world.resource_mut::<VnEngine>().0.locale = previous;
                world
                    .resource_mut::<VnEngine>()
                    .0
                    .renderer
                    .scene_frame_failed = true;
                world.insert_resource(host);
                return;
            }
            let commands = world.resource_mut::<VnEngine>().0.renderer.take_pending();
            if let Some(mut output) =
                world.get_resource_mut::<Events<crate::vn_command::VnCommand>>()
            {
                output.send_batch(commands);
            }
        }
        host.publication = host.publication.checked_add(1).unwrap_or(u64::MAX);
        host.pending = false;
        host.diagnostics.clear();
    }
    world.insert_resource(host);
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortableTextHostReceipt {
    pub version: u32,
    pub string_loader_policy: u32,
    pub locale_loader_policy: u32,
    pub table_declarations: BTreeMap<String, String>,
    pub table_states: BTreeMap<String, PublishedLoadState>,
    pub table_diagnostics: BTreeMap<String, String>,
    pub locale_declarations: LocaleDeclarations,
    pub locale_diagnostics: BTreeMap<String, String>,
    pub registry: StringTableContentSnapshot,
    pub locales: LocaleContentSnapshot,
    #[serde(default,skip_serializing_if="BTreeSet::is_empty")]
    pub locale_absent: BTreeSet<String>,
}
#[derive(Clone)]
pub(crate) struct LiveTextHostReceipt {
    authority: Arc<()>,
    source_authority: Arc<()>,
    engine_instance: u64,
    locale_revision: rvn_core::locale::LocaleRevision,
    locale_source_digest: [u8; 32],
    selection_mode: LocaleSelectionMode,
    pub epoch: u64,
    pub locale_publication: u64,
    pub portable: PortableTextHostReceipt,
}
fn boundary(world: &World) -> Result<(), String> {
    source_binding(world)?;
    let sources = world
        .get_resource::<KnownTextSources>()
        .ok_or("Opaque text asset readers have no verified portable provider contract")?;
    if sources.policy != 1 || sources.table_root.is_empty() || sources.locale_root.is_empty() {
        return Err("Incomplete actual text reader policy".into());
    }
    #[cfg(not(target_arch = "wasm32"))]
    if !std::path::Path::new(&sources.table_root).is_dir()
    {
        return Err("Actual declared file provider root is unavailable".into());
    }
    let host = world
        .get_resource::<TableHost>()
        .ok_or("Missing actual string-table publication host")?;
    let locales = world
        .get_resource::<LocaleAssetHost>()
        .ok_or("Missing actual closed locale asset host")?;
    let book = world
        .get_resource::<PublicationBook>()
        .ok_or("Missing actual publication journal")?;
    let missed =
        host.events
            .missed_events(world.resource::<Events<AssetEvent<LoadedStringTable>>>())
            .saturating_add(
                host.failures.missed_events(
                    world.resource::<Events<AssetLoadFailedEvent<LoadedStringTable>>>(),
                ),
            )
            .saturating_add(
                locales
                    .events
                    .missed_events(world.resource::<Events<AssetEvent<LoadedLocaleSource>>>()),
            )
            .saturating_add(locales.failures.missed_events(
                world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>(),
            ));
    if missed > 0
        || book.missed > 0
        || book.exhausted
        || locales.missed > 0
        || locales.publication == u64::MAX
    {
        return Err("Missed events or exhausted publication journal cannot authenticate a portable boundary".into());
    }
    if !host.pending.is_empty()
        || locales.pending
        || host
            .events
            .len(world.resource::<Events<AssetEvent<LoadedStringTable>>>())
            > 0
        || host
            .failures
            .len(world.resource::<Events<AssetLoadFailedEvent<LoadedStringTable>>>())
            > 0
        || locales
            .events
            .len(world.resource::<Events<AssetEvent<LoadedLocaleSource>>>())
            > 0
        || locales
            .failures
            .len(world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>())
            > 0
    {
        return Err("Text resource event/publication queue is not drained".into());
    }
    let owner = crate::text_runtime_owner::current(world)?;
    if owner.blocked {
        return Err("Uncommitted or failed renderer publication blocks the text boundary".into());
    }
    rvn_core::string_table_project::validate_declarations(&host.declarations)?;
    locales.declarations.validate()?;
    if host.bound.keys().ne(host.declarations.keys())
        || locales.bound.keys().ne(locales.declarations.files.keys())
    {
        return Err("Declared assets are not all bound to actual handles".into());
    }
    let server = world.resource::<AssetServer>();
    for (id, binding) in &host.bound {
        if matches!(
            table_load_state(server, binding.handle.id()),
            PublishedLoadState::Loading | PublishedLoadState::NotLoaded
        ) {
            return Err("Actual string-table load is pending or unstarted".into());
        }
        if let bevy::asset::LoadState::Failed(error) = server.load_state(binding.handle.id()) {
            if world.resource::<StringTableDiagnostics>().0.get(id)
                != Some(&diagnostic(error.to_string()))
            {
                return Err("Actual failed load has no matching drained host diagnostic".into());
            }
        }
    }
    locale_catalogs(world,locales)?;
    if !locales.diagnostics.is_empty() {
        return Err("Locale host retains a failed publication diagnostic".into());
    }
    Ok(())
}
pub(crate) fn portable_receipt(world: &World) -> Result<PortableTextHostReceipt, String> {
    boundary(world)?;
    let host = world.resource::<TableHost>();
    let locale_host = world.resource::<LocaleAssetHost>();
    let owner = crate::text_runtime_owner::current(world)?;
    let server = world.resource::<AssetServer>();
    let registry = owner.resources.portable_receipt()?;
    if registry.tables.keys().ne(host.declarations.keys()) {
        return Err(
            "Registry contains missing or undeclared assets; provider closure is incomplete".into(),
        );
    }
    let assets = world.resource::<Assets<LoadedStringTable>>();
    for (id, binding) in &host.bound {
        if let Some(asset) = assets.get(binding.handle.id()) {
            if owner.resources.ready_table(id) != Some(asset.0.as_ref()) {
                return Err("Current decoded asset and published registry differ".into());
            }
        } else {
            return Err("Declared asset has no retained actual decoded value".into());
        }
    }
    let locales = owner.locale
        .ok_or("Missing actual published locale")?
        .portable_receipt()?;
    if locales.default_lang != locale_host.declarations.default_lang
        || locales.available_langs != locale_host.declarations.available
        || locales
            .cache
            .keys()
            .ne(locale_host.declarations.files.keys())
    {
        return Err(
            "Published locale closure differs from its actual provider declarations".into(),
        );
    }
    let (catalogs,locale_absent)=locale_catalogs(world,locale_host)?;
    let actual=LocaleManager::from_frozen_catalogs(&locale_host.declarations.default_lang,&locales.current_lang,locale_host.declarations.available.clone(),catalogs)?.portable_receipt()?;
    if actual!=locales{return Err("Current actual locale closure and published catalogue differ".into());}
    let receipt = PortableTextHostReceipt {
        version: 1,
        string_loader_policy: 1,
        locale_loader_policy: if locale_absent.is_empty(){1}else{2},
        table_declarations: host.declarations.clone(),
        table_states: host
            .bound
            .iter()
            .map(|(id, binding)| (id.clone(), table_load_state(server, binding.handle.id())))
            .collect(),
        table_diagnostics: world.resource::<StringTableDiagnostics>().0.clone(),
        locale_declarations: locale_host.declarations.clone(),
        locale_diagnostics: locale_host.diagnostics.clone(),
        registry,
        locales,
        locale_absent,
    };
    boundary(world)?;
    Ok(receipt)
}
pub(crate) fn accepts_portable_receipt(
    world: &World,
    expected: &PortableTextHostReceipt,
) -> Result<bool, String> {
    Ok(portable_receipt(world)? == *expected)
}
impl PortableTextHostReceipt {
    pub(crate) fn canonical_digest(&self) -> Result<String, String> {
        if self.version != 1 || self.string_loader_policy != 1
            || (self.locale_loader_policy!=1&&self.locale_loader_policy!=2)
            || (self.locale_loader_policy==1)!=self.locale_absent.is_empty() {
            return Err("Unsupported text host receipt policy".into());
        }
        rvn_core::string_table_project::validate_declarations(&self.table_declarations)?;
        self.locale_declarations.validate()?;
        self.registry.validate()?;
        self.registry.restore_digest()?;
        self.locales.validate()?;
        self.locales.restore_digest()?;
        if self.table_states.keys().ne(self.table_declarations.keys())
            || self
                .registry
                .tables
                .keys()
                .ne(self.table_declarations.keys())
            || self.table_states.values().any(|state| {
                matches!(
                    state,
                    PublishedLoadState::Loading | PublishedLoadState::NotLoaded
                )
            })
            || self.table_diagnostics.iter().any(|(id, message)| {
                !self.table_declarations.contains_key(id) || message.len() > 4096
            })
            || !self.locale_diagnostics.is_empty()
            || self.locales.default_lang != self.locale_declarations.default_lang
            || self.locales.available_langs != self.locale_declarations.available
            || self
                .locales
                .cache
                .keys()
                .ne(self.locale_declarations.files.keys())
            || self.table_states.iter().any(|(id, state)| {
                *state == PublishedLoadState::Failed && !self.table_diagnostics.contains_key(id)
            })
        {
            return Err("Incomplete, pending or inconsistent host receipt closure".into());
        }
        for lang in &self.locale_absent{
            let table=self.locales.cache.get(lang).ok_or("Absence is outside the declared locale closure")?;
            if !self.locale_declarations.files.contains_key(lang)||table.lang!=*lang||!table.strings.is_empty()||!table.text_sources.is_empty(){return Err("Absent locale has nonempty or undeclared semantic content".into());}
        }
        // Streaming bounded serialization: no Debug, roots, handles or epochs.
        struct Writer {
            hash: Sha256,
            bytes: usize,
        }
        impl std::io::Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.bytes = self
                    .bytes
                    .checked_add(bytes.len())
                    .filter(|n| *n <= 128 * 1024 * 1024)
                    .ok_or_else(|| {
                        std::io::Error::other("Host receipt exceeds 128 MiB wire budget")
                    })?;
                self.hash.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Writer {
            hash: Sha256::new(),
            bytes: 0,
        };
        writer.hash.update(b"rvn.bevy-text-host.v1\0");
        serde_json::to_writer(&mut writer, self).map_err(|error| error.to_string())?;
        Ok(format!("{:x}", writer.hash.finalize()))
    }
}
// The live receipt authenticates the actual retained source bytes as well as
// the published semantic catalogue. A comment-only edit before event delivery
// must revoke a prepared transaction. This digest never enters portable saves.
fn live_locale_source_digest(world: &World) -> Result<[u8; 32], String> {
    let host = world.resource::<LocaleAssetHost>();
    let assets = world.resource::<Assets<LoadedLocaleSource>>();
    let mut hash = Sha256::new();
    hash.update(b"rvn.bevy-live-locale-source.v1\0");
    let mut bytes = 0usize;
    let (_,absent)=locale_catalogs(world,host)?;
    if !absent.is_empty(){
        hash.update(b"native-absent-v1\0");
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        hash.update([u8::from(world.resource::<KnownTextSources>().native_root.as_ref().ok_or("Missing native root")?.validate()?)]);
    }
    for (lang, handle) in &host.bound {
        if absent.contains(lang){hash.update((lang.len() as u64).to_le_bytes());hash.update(lang.as_bytes());hash.update(b"absent\0");continue;}
        let raw = &assets.get(handle.id())
            .ok_or("Actual locale source vanished after preparation")?.0;
        if raw.len() > 8 * 1024 * 1024 {
            return Err("Actual locale source exceeds 8 MiB".into());
        }
        bytes = bytes.checked_add(raw.len())
            .filter(|n| *n <= 64 * 1024 * 1024)
            .ok_or("Actual locale source closure exceeds 64 MiB")?;
        hash.update((lang.len() as u64).to_le_bytes());
        hash.update(lang.as_bytes());
        hash.update((raw.len() as u64).to_le_bytes());
        hash.update(raw.as_bytes());
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        {
            match host.latest_probe.get(lang).ok_or("Missing actual metadata observation")?.metadata{
                None=>hash.update([0]),Some(digest)=>{hash.update([1]);hash.update(digest);}
            }
        }
    }
    Ok(hash.finalize().into())
}
pub(crate) fn live_receipt(world: &World) -> Result<LiveTextHostReceipt, String> {
    let portable = portable_receipt(world)?;
    let owner = crate::text_runtime_owner::current(world)?;
    let book = world.resource::<PublicationBook>();
    let locale = world.resource::<LocaleAssetHost>();
    Ok(LiveTextHostReceipt {
        authority: book.authority.clone(),
        source_authority: world.resource::<KnownTextSources>().authority.clone(),
        engine_instance: owner.instance,
        locale_revision: owner.locale.ok_or("Missing current locale authority")?.revision_stamp(),
        locale_source_digest: live_locale_source_digest(world)?,
        selection_mode: locale_selection_mode(world)?,
        epoch: book.epoch,
        locale_publication: locale.publication,
        portable,
    })
}
pub(crate) fn accepts_live_receipt(
    world: &World,
    expected: &LiveTextHostReceipt,
) -> Result<bool, String> {
    let actual = live_receipt(world)?;
    Ok(Arc::ptr_eq(&actual.authority, &expected.authority)
        && Arc::ptr_eq(&actual.source_authority, &expected.source_authority)
        && actual.engine_instance == expected.engine_instance
        && actual.locale_revision == expected.locale_revision
        && actual.locale_source_digest == expected.locale_source_digest
        && actual.selection_mode == expected.selection_mode
        && actual.epoch == expected.epoch
        && actual.locale_publication == expected.locale_publication
        && actual.portable == expected.portable)
}

/// A read-only candidate reconstructed from actual verified locale assets.
/// The fields stay private: a serialized caller cannot replace its catalogue,
/// provider witness or source/game authority after preparation.
pub(crate) struct PreparedSelectedLanguage {
    base: LiveTextHostReceipt,
    locale: LocaleManager,
    receipt: PortableTextHostReceipt,
}
impl PreparedSelectedLanguage {
    pub(crate) fn locale(&self) -> &LocaleManager {
        &self.locale
    }
    pub(crate) fn receipt(&self) -> &PortableTextHostReceipt {
        &self.receipt
    }
    pub(crate) fn verify_host(&self, world: &World) -> Result<(), String> {
        if accepts_live_receipt(world, &self.base)? {
            Ok(())
        } else {
            Err("Actual text host changed after selected-language preparation".into())
        }
    }
}
/// Only the saved selection may differ from the actual published receipt.
/// Catalogue contents, declarations, diagnostics and policies still have to
/// equal a reconstruction from this actual host before any effect/publication.
pub(crate) fn prepare_selected_language(
    world: &World,
    expected: &PortableTextHostReceipt,
) -> Result<PreparedSelectedLanguage, String> {
    expected.canonical_digest()?;
    let base = live_receipt(world)?;
    let host = world.resource::<LocaleAssetHost>();
    let selected = &expected.locales.current_lang;
    if !host.declarations.files.contains_key(selected) {
        return Err("Saved selected language is outside the actual closed provider".into());
    }
    let (catalogs,_)=locale_catalogs(world,host)?;
    let locale = LocaleManager::from_frozen_catalogs(
        &host.declarations.default_lang,
        selected,
        host.declarations.available.clone(),
        catalogs,
    )?;
    let mut receipt = base.portable.clone();
    receipt.locales = locale.portable_receipt()?;
    if receipt != *expected {
        return Err("Saved text receipt differs from actual provider content/policy beyond selected language".into());
    }
    let prepared = PreparedSelectedLanguage {
        base,
        locale,
        receipt,
    };
    prepared.verify_host(world)?;
    Ok(prepared)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "portable_text_resource_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "portable_key_restore_tests.rs"]
mod key_restore_tests;

/// Initial host readiness is separate from authority. Pending I/O defers the
/// first computation; once drained, the complete existing boundary still runs.
pub(crate) fn startup_loads_ready(world:&World)->Result<bool,String>{
    let host=world.get_resource::<TableHost>().ok_or("Missing actual string-table publication host")?;
    let locales=world.get_resource::<LocaleAssetHost>().ok_or("Missing actual closed locale asset host")?;
    let server=world.get_resource::<AssetServer>().ok_or("Missing actual text AssetServer")?;
    let book=world.get_resource::<PublicationBook>().ok_or("Missing actual publication journal")?;
    if book.missed>0||book.exhausted||locales.missed>0||locales.publication==u64::MAX{return Err("Missed or exhausted text publication during Startup loading".into());}
    if let Some(error)=terminal_startup_locale_refusal(world,locales){return Err(error.into());}
    for binding in host.bound.values(){if let bevy::asset::LoadState::Failed(error)=server.load_state(binding.handle.id()){return Err(format!("Actual Startup string-table source failed: {error}"));}}
    if world.get_resource::<StringTableDiagnostics>().is_some_and(|diagnostics|!diagnostics.0.is_empty()){return Err("Actual Startup string-table publication failed".into());}
    for handle in locales.bound.values(){if let bevy::asset::LoadState::Failed(error)=server.load_state(handle.id()){
        #[cfg(all(not(target_arch="wasm32"),not(target_os="android")))]
        if locale_selection_mode(world)?==LocaleSelectionMode::NormalCompatible&&native_locale_provider::failed_not_found(&bevy::asset::LoadState::Failed(error.clone())){continue;}
        return Err(format!("Actual Startup locale source failed: {error}"));
    }}
    if let Some(error)=locales.diagnostics.get("provider"){return Err(format!("Actual Startup locale provider failed: {error}"));}
    if !locales.pending&&!locales.diagnostics.is_empty(){return Err("Actual Startup locale publication failed".into());}
    let events=world.resource::<Events<AssetEvent<LoadedStringTable>>>();let failed=world.resource::<Events<AssetLoadFailedEvent<LoadedStringTable>>>();
    let locale_events=world.resource::<Events<AssetEvent<LoadedLocaleSource>>>();let locale_failed=world.resource::<Events<AssetLoadFailedEvent<LoadedLocaleSource>>>();
    if host.events.missed_events(events)>0||host.failures.missed_events(failed)>0||locales.events.missed_events(locale_events)>0||locales.failures.missed_events(locale_failed)>0{return Err("Missed actual text events during Startup loading".into());}
    if !host.pending.is_empty()||locales.pending||host.bound.keys().ne(host.declarations.keys())||locales.bound.keys().ne(locales.declarations.files.keys())
        ||host.events.len(events)>0||host.failures.len(failed)>0||locales.events.len(locale_events)>0||locales.failures.len(locale_failed)>0
        ||host.bound.values().any(|binding|matches!(server.load_state(binding.handle.id()),bevy::asset::LoadState::NotLoaded|bevy::asset::LoadState::Loading))
        ||locales.bound.values().any(|handle|matches!(server.load_state(handle.id()),bevy::asset::LoadState::Loading|bevy::asset::LoadState::NotLoaded)){return Ok(false);}
    live_receipt(world)?;Ok(true)
}

#[cfg(all(test,not(target_arch="wasm32"),not(target_os="android")))]
#[path="native_locale_absence_tests.rs"]
pub(crate) mod native_absence_tests;
