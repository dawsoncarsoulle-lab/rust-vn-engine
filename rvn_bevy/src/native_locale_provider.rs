//! Native filesystem evidence only. No opaque/HTTP reader is blocked here.
use super::*;
use bevy::asset::io::{AssetReaderError, ErasedAssetReader};
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug)]
pub(super) struct NativeLocaleRoot {
    anchor: PathBuf,
    suffix: PathBuf,
}
fn reparse(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; metadata.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}
impl NativeLocaleRoot {
    pub(super) fn capture(root: &Path) -> Result<Self, String> {
        let root = if root.is_absolute() { root.to_owned() } else {
            std::env::current_dir().map_err(|e|e.to_string())?.join(root)
        };
        let mut cursor = root;
        let mut missing = Vec::new();
        loop {
            match std::fs::metadata(&cursor) {
                Ok(metadata) => {
                    if !metadata.is_dir() {return Err("Locale provider anchor is not a directory".into());}
                    let canonical=cursor.canonicalize().map_err(|e|e.to_string())?;
                    // Retain the existing parent as authority even when the
                    // locale directory initially exists; its later removal is
                    // a source transition, while parent replacement is not.
                    let (anchor,suffix)=if missing.is_empty(){
                        match (canonical.parent(),canonical.file_name()){
                            (Some(parent),Some(name))=>(parent.to_owned(),PathBuf::from(name)),
                            _=>(canonical,PathBuf::new()),
                        }
                    }else{(canonical,missing.into_iter().rev().collect::<PathBuf>())};
                    let result=Self{anchor,suffix};result.validate()?;return Ok(result);
                }
                Err(error) if error.kind()==std::io::ErrorKind::NotFound => {
                    let name=cursor.file_name().ok_or("Locale root has no existing directory anchor")?.to_owned();
                    if name=="."||name==".." {return Err("Unresolved locale root traversal".into());}
                    missing.push(name);
                    if !cursor.pop(){return Err("Locale root has no existing directory anchor".into());}
                }
                Err(error)=>return Err(error.to_string()),
            }
        }
    }
    pub(super) fn path(&self)->PathBuf {self.anchor.join(&self.suffix)}
    /// An absent suffix may become ordinary directories. A replacement of the
    /// anchored provider by a symlink/junction or a file never proves absence.
    pub(super) fn validate(&self)->Result<bool,String>{
        let meta=std::fs::symlink_metadata(&self.anchor).map_err(|e|e.to_string())?;
        if reparse(&meta)||!meta.is_dir()||self.anchor.canonicalize().map_err(|e|e.to_string())?!=self.anchor {
            return Err("Native locale provider anchor changed".into());
        }
        let mut path=self.anchor.clone();
        for part in self.suffix.components(){
            let Component::Normal(part)=part else{return Err("Invalid locale root suffix".into());};path.push(part);
            match std::fs::symlink_metadata(&path){
                Ok(meta) if !reparse(&meta)&&meta.is_dir()=>{},
                Ok(_)=>return Err("Locale provider suffix became a file or reparse point".into()),
                Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(false),
                Err(error)=>return Err(error.to_string()),
            }
        }
        Ok(true)
    }
    pub(super) fn validate_file(&self,relative:&str)->Result<(),String>{
        self.validate()?;
        let root=self.path();let mut current=root.clone();
        for part in Path::new(relative).components(){
            let Component::Normal(part)=part else{return Err("Invalid locale relative path".into());};current.push(part);
            match std::fs::symlink_metadata(&current){
                Ok(meta) if reparse(&meta)=>{
                    // Preserve existing all-present inside-root aliases. A
                    // dangling/escaping reparse point cannot prove absence.
                    let canonical=current.canonicalize().map_err(|e|e.to_string())?;
                    if !canonical.starts_with(&root){return Err("Locale declaration escapes its provider root".into());}
                },
                Ok(_)=>{},
                Err(error) if error.kind()==std::io::ErrorKind::NotFound=>return Ok(()),
                Err(error)=>return Err(error.to_string()),
            }
        }
        let canonical=current.canonicalize().map_err(|e|e.to_string())?;
        if !canonical.starts_with(&root){return Err("Locale declaration escapes its provider root".into());}Ok(())
    }
}

// AssetServer ties its borrow to AssetSourceId's lifetime. Passing the raw
// static str would select Into<AssetSourceId<'static>> and require a static
// server borrow; the same static source name can inhabit any shorter id.
fn locale_source<'a>()->bevy::asset::io::AssetSourceId<'a>{
    bevy::asset::io::AssetSourceId::Name(bevy::utils::CowArc::Static(LOCALE_SOURCE))
}
pub(super) fn reader_identity(reader:&dyn ErasedAssetReader)->usize {
    reader as *const dyn ErasedAssetReader as *const () as usize
}
pub(super) fn actual_reader<'a>(sources:&KnownTextSources,server:&'a AssetServer)->Result<&'a dyn ErasedAssetReader,String>{
    use std::sync::atomic::Ordering;
    let retained=sources.server.as_ref().ok_or("Actual text AssetServer was not sealed")?;
    let actual=server.get_source(locale_source()).map_err(|e|e.to_string())?.reader();
    let held=retained.get_source(locale_source()).map_err(|e|e.to_string())?.reader();
    let registered=sources.locale_reader.load(Ordering::Acquire);
    if registered==0||reader_identity(actual)!=registered||!std::ptr::eq(actual,held){
        return Err("Actual native locale reader differs from the registered retained provider".into());
    }
    Ok(actual)
}
pub(super) fn failed_not_found(state:&bevy::asset::LoadState)->bool{
    matches!(state,bevy::asset::LoadState::Failed(error) if matches!(error.as_ref(),bevy::asset::AssetLoadError::AssetReaderError(AssetReaderError::NotFound(_))))
}
/// Only called after the exact native reader factory/retained server/root have
/// been authenticated. This opens a path; it does not allocate its contents.
pub(super) fn prove_absent(sources:&KnownTextSources,server:&AssetServer,path:&str)->Result<(),String>{
    let root=sources.native_root.as_ref().ok_or("No authenticated native locale root")?;
    root.validate_file(path)?;
    let reader=actual_reader(sources,server)?;
    match bevy::tasks::block_on(reader.read(Path::new(path))){
        Err(AssetReaderError::NotFound(_))=>{},
        Err(error)=>return Err(format!("Native locale absence probe failed: {error}")),
        Ok(_)=>return Err("Previously absent locale has appeared; actual reload is required".into()),
    }
    root.validate_file(path)?;Ok(())
}
#[derive(Clone,Copy,PartialEq,Eq)]
pub(super) struct Probe{pub content:Option<[u8;32]>,pub metadata:Option<[u8;32]>}
pub(super) struct ScanResult{
    pub present:bool,
    pub probes:BTreeMap<String,Probe>,
    pub metadata_errors:BTreeMap<String,String>,
}
pub(super) type Scan=Result<ScanResult,String>;
pub(super) struct PendingScan{
    pub task:bevy::tasks::Task<Scan>,
    pub authority:Arc<()>,
    pub reader:usize,
    pub files:BTreeMap<String,String>,
    pub handles:BTreeMap<String,bevy::asset::AssetId<LoadedLocaleSource>>,
}
// Mirrors Bevy's public AssetMetaCheck interpretation, including source ID.
fn checks_metadata(policy:&bevy::asset::AssetMetaCheck,path:&str)->bool{
    match policy{
        bevy::asset::AssetMetaCheck::Always=>true,
        bevy::asset::AssetMetaCheck::Never=>false,
        bevy::asset::AssetMetaCheck::Paths(paths)=>paths.contains(&bevy::asset::AssetPath::from(path.to_owned()).with_source(LOCALE_SOURCE)),
    }
}
fn meta_relative(path:&str)->Result<String,String>{
    // Bevy io::get_meta_path is private: reproduce only its path convention,
    // not its parser. The actual read still goes through read_meta(data_path).
    let mut meta=PathBuf::from(path);let mut ext=meta.extension().unwrap_or_default().to_os_string();ext.push(".meta");meta.set_extension(ext);
    meta.to_str().map(str::to_owned).ok_or_else(||"Non-UTF8 locale metadata path".into())
}
/// The public Bevy parser needs a complete byte slice. No invented size cap:
/// collect fallibly, off-thread. Parser allocations remain Bevy/RON's own and
/// are not claimed to have a fixed memory bound by this provider.
async fn collect_metadata(mut reader:Box<Reader<'_>>)->Result<Vec<u8>,String>{
    let mut bytes=Vec::new();let mut buffer=[0u8;8192];
    loop{let count=reader.read(&mut buffer).await.map_err(|e|format!("Metadata I/O: {e}"))?;if count==0{break;}
        bytes.try_reserve(count).map_err(|e|format!("Metadata allocation unavailable (retry required): {e}"))?;bytes.extend_from_slice(&buffer[..count]);}
    Ok(bytes)
}
async fn validate_metadata(server:&AssetServer,bytes:&[u8])->Result<(),String>{
    use bevy::asset::meta::{AssetActionMinimal,AssetMetaMinimal};
    let minimal:AssetMetaMinimal=bevy::asset::ron::de::from_bytes(bytes).map_err(|e|format!("Bevy metadata minimal parse: {e}"))?;
    let name=match minimal.asset{
        AssetActionMinimal::Load{loader}=>loader,
        AssetActionMinimal::Process{..}=>return Err("Bevy cannot load Process metadata as an asset".into()),
        AssetActionMinimal::Ignore=>return Err("Bevy cannot load Ignore metadata as an asset".into()),
    };
    let loader=server.get_asset_loader_with_type_name(&name).await.map_err(|e|e.to_string())?;
    if loader.asset_type_id()!=std::any::TypeId::of::<LoadedLocaleSource>()||loader.type_name()!=std::any::type_name::<LocaleSourceLoader>(){
        return Err("Explicit metadata loader is not the actual locale source loader".into());
    }
    let meta=loader.deserialize_meta(bytes).map_err(|e|format!("Bevy loader metadata parse: {e}"))?;
    if !meta.loader_settings().is_some_and(|settings|settings.downcast_ref::<()>().is_some()){
        return Err("Actual locale loader metadata does not have Unit settings".into());
    }
    // A successful parse is only compatible input, never a Loaded witness.
    Ok(())
}
/// Narrow native boundary read: hash only, no metadata Vec/parser on the main
/// thread. A changed fingerprint revokes the old async validation/receipt.
pub(super) fn current_metadata_fingerprint(sources:&KnownTextSources,server:&AssetServer,path:&str)->Result<Option<[u8;32]>,String>{
    let policy=sources.meta_check.as_ref().ok_or("Missing actual metadata policy")?;
    if !checks_metadata(policy,path){return Ok(None);}
    let root=sources.native_root.as_ref().ok_or("Missing native locale root")?;let meta=meta_relative(path)?;root.validate_file(&meta)?;
    let reader=actual_reader(sources,server)?;
    let digest=bevy::tasks::block_on(async{
        match reader.read_meta(Path::new(path)).await{
            Err(AssetReaderError::NotFound(_))=>Ok(None),Err(error)=>Err(error.to_string()),
            Ok(mut reader)=>{let mut hash=Sha256::new();let mut buffer=[0u8;8192];loop{let count=reader.read(&mut buffer).await.map_err(|e|e.to_string())?;if count==0{break;}hash.update(&buffer[..count]);}Ok(Some(hash.finalize().into()))}
        }
    })?;
    root.validate_file(&meta)?;actual_reader(sources,server)?;Ok(digest)
}
pub(super) async fn scan(server:AssetServer,root:NativeLocaleRoot,files:BTreeMap<String,String>,expected_reader:usize,policy:bevy::asset::AssetMetaCheck)->Scan{
    let present=root.validate()?;let mut result=BTreeMap::new();let mut total=0usize;let mut metadata_errors=BTreeMap::new();
    let reader=server.get_source(locale_source()).map_err(|e|e.to_string())?.reader();
    if reader_identity(reader)!=expected_reader{return Err("Native locale reader changed before scan".into());}
    for (lang,path) in files{
        root.validate_file(&path)?;
        let digest=match reader.read(Path::new(&path)).await{
            Err(AssetReaderError::NotFound(_))=>None,
            Err(error)=>return Err(error.to_string()),
            Ok(reader)=>{
                let mut bytes=Vec::new();reader.take(8*1024*1024+1).read_to_end(&mut bytes).await.map_err(|e|e.to_string())?;
                if bytes.len()>8*1024*1024{return Err("Locale source exceeds 8 MiB".into());}
                total=total.checked_add(bytes.len()).filter(|n|*n<=64*1024*1024).ok_or("Locale closure exceeds 64 MiB")?;
                Some(Sha256::digest(bytes).into())
            }
        };
        let metadata=if digest.is_some()&&checks_metadata(&policy,&path){
            let meta_path=meta_relative(&path)?;root.validate_file(&meta_path)?;
            let value=match reader.read_meta(Path::new(&path)).await{
                Err(AssetReaderError::NotFound(_))=>None,
                Err(error)=>return Err(error.to_string()),
                Ok(reader)=>{
                    let bytes=collect_metadata(reader).await?;let hash=Some(Sha256::digest(&bytes).into());
                    if let Err(error)=validate_metadata(&server,&bytes).await{metadata_errors.insert(lang.clone(),error);}
                    hash
                }
            };root.validate_file(&meta_path)?;value
        }else{None};
        root.validate_file(&path)?;result.insert(lang,Probe{content:digest,metadata});
    }
    if root.validate()?!=present||reader_identity(server.get_source(locale_source()).map_err(|e|e.to_string())?.reader())!=expected_reader{return Err("Locale provider changed during asynchronous probe".into());}
    Ok(ScanResult{present,probes:result,metadata_errors})
}
