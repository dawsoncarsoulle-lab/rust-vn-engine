//! Exact immutable Library inputs on the existing Source root/files boundary.
//! Transaction projections use journal before/after bytes, never latest disk.
use super::*;
use crate::library_manifest1027::{
    parse_library_manifest1027, LibraryManifest1027, MAX_LIBRARY_SOURCE_INPUT_BYTES1027,
};
use rvn_parser::function_library_asset1023::{FunctionLibraryAsset, MAX_FUNCTION_LIBRARY_BYTES};
use std::{collections::BTreeMap, io::Read, sync::Arc};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LibraryReadset1027 {
    version: u32,
    files: Vec<SourceRefactorFile>,
    assets_directory: Option<(String, bool)>,
}
impl LibraryReadset1027 {
    pub(super) fn capture(input: Option<&SourceLibraryInputs1027>) -> Self {
        match input {
            Some(input) => Self {
                version: 1,
                files: input.references.to_vec(),
                assets_directory: input
                    .assets_present
                    .map(|present| (input.manifest.assets_relative.clone(), present)),
            },
            None => Self {
                version: 1,
                files: vec![SourceRefactorFile {
                    path: "rvn.toml".into(),
                    before: None,
                    after: None,
                }],
                assets_directory: None,
            },
        }
    }
    pub(super) fn inputs(&self, root: &Path) -> Result<Option<SourceLibraryInputs1027>, String> {
        if self.version != 1 || self.files.iter().any(|f| f.before != f.after) {
            return Err("Unknown or writable Library resource readset in Source journal".into());
        }
        let Some(manifest) = self
            .files
            .iter()
            .find(|f| f.path == "rvn.toml")
            .ok_or("Source resource journal has no manifest observation")?
            .before
            .as_deref()
        else {
            if self.files.len() != 1 || self.assets_directory.is_some() {
                return Err("Absent manifest cannot claim descriptor or directory authority".into());
            }
            return Ok(None);
        };
        let parsed = parse_library_manifest1027(manifest)?;
        if self
            .assets_directory
            .as_ref()
            .is_some_and(|(path, _)| path != &parsed.assets_relative)
            || parsed.has_library_index && self.assets_directory.is_none()
        {
            return Err(
                "Source resource directory readset differs from its exact manifest namespace"
                    .into(),
            );
        }
        SourceLibraryInputs1027::from_exact1027(
            root.canonicalize().map_err(|e| e.to_string())?,
            self.files.clone(),
            self.assets_directory.as_ref().map(|(_, p)| *p),
        )
        .map(Some)
    }
    pub(super) fn validate(&self, root: &Path) -> Result<(), String> {
        if let Some(input) = self.inputs(root)? {
            input.validate_original()
        } else {
            if legacy_manifest1027(&root.join("rvn.toml"),None)?.is_some(){Err("Previously absent Source manifest appeared".into())}else{Ok(())}
        }
    }
    /// A journal may already have published some exact AFTER targets. Prove
    /// original observers against its BEFORE participants, and check readonly
    /// observers which are not write participants through their original policy.
    pub(super) fn validate_journal_before1027(&self,root:&Path,files:&[SourceRefactorFile])->Result<(),String>{
        let input=self.inputs(root)?;
        if let Some(input)=&input{input.validate_directory1027()?;}
        for reference in &self.files{
            if let Some(file)=files.iter().find(|f|path_key1027(&f.path)==path_key1027(&reference.path)){
                if file.before!=reference.before{return Err("Library journal lost an original resource BEFORE observation".into())}
            }else if reference.path=="rvn.toml"&&input.as_ref().is_none_or(|v|!v.manifest.has_library_index){
                let expected=reference.before.as_deref();let actual=legacy_manifest1027(&root.join("rvn.toml"),expected.map(str::len))?;
                if actual.as_deref()!=expected{return Err("Readonly legacy manifest changed during Source journal replay".into())}
            }else{
                refactor::check_files(root,std::slice::from_ref(reference),false)?;
                if bounded_optional1027(root,&root.join(&reference.path),MAX_LIBRARY_SOURCE_INPUT_BYTES1027)?.as_deref()!=reference.before.as_deref(){return Err("Original Library observer changed during Source replay".into())}
            }
        }Ok(())
    }
}
pub(super) fn journal_inputs1027(root:&Path,files:&[SourceRefactorFile],before:Option<&LibraryReadset1027>,after:Option<&LibraryReadset1027>)->Result<Option<Arc<SourceLibraryInputs1027>>,String>{
    match (before,after){
        (None,None)=>SourceLibraryInputs1027::from_transaction1027(root,files,true).map(|v|v.map(Arc::new)),
        (Some(before),Some(after))=>{
            before.validate_journal_before1027(root,files)?;
            if SourceLibraryInputs1027::has_manifest_participant1027(files){
                let candidate=SourceLibraryInputs1027::from_transaction1027(root,files,true)?;
                if LibraryReadset1027::capture(candidate.as_ref())!=*after{return Err("Source journal AFTER resources differ from actual file participants".into())}
                let baseline=SourceLibraryInputs1027::from_transaction1027(root,files,false)?;
                if LibraryReadset1027::capture(baseline.as_ref())!=*before{return Err("Source journal BEFORE resources differ from actual file participants".into())}
            }else if before!=after{return Err("A readonly resource observer cannot change without its actual Source file participant".into())}
            let inputs=after.inputs(root)?;if let Some(input)=&inputs{input.validate_directory1027()?;}Ok(inputs.map(Arc::new))
        }
        _=>Err("Scoped Source journal lost one original resource frontier".into()),
    }
}
impl SourceWorkspace {
    pub(super) fn append_library_readsets1027(
        &self,
        files: &mut Vec<SourceRefactorFile>,
    ) -> Result<(), String> {
        let Some(observed) = self.library_journal_readset1027() else {
            return Ok(());
        };
        observed.validate(&self.root)?;
        let indexed=observed.inputs(&self.root)?.is_some_and(|input|input.manifest.has_library_index);
        for reference in observed.files {
            if let Some(file) = files
                .iter()
                .find(|f| path_key1027(&f.path) == path_key1027(&reference.path))
            {
                if file.before != reference.before {
                    return Err("Library publication lost its original resource baseline".into());
                }
            } else if indexed {
                files.push(reference)
            }
        }
        Ok(())
    }
    pub(super) fn library_journal_readset1027(&self) -> Option<LibraryReadset1027> {
        project_readset1027(&self.project)
    }
}
pub(super) fn project_readset1027(project:&SourceProject)->Option<LibraryReadset1027>{project.observes_library_inputs1027().then(||LibraryReadset1027::capture(project.library_inputs1027().map(|v|v.as_ref())))}
pub(super) fn validate_refactor_snapshot1027(root:&Path,snapshot:&Snapshot,files:&[SourceRefactorFile],before:Option<&LibraryReadset1027>,after:Option<&LibraryReadset1027>,destination:bool)->Result<SourceProject,String>{
    if before.is_none()&&after.is_none(){return validate_published_snapshot_project(root,snapshot)}
    validate_snapshot_editorial(snapshot)?;refactor::check_files(root,files,true)?;
    let candidate=journal_inputs1027(root,files,before,after)?;
    let inputs=if destination{candidate}else{before.ok_or("Scoped Source journal has no original resource observation")?.inputs(root)?.map(Arc::new)};
    SourceProject::open_library_recovery1027(&source_path(root,&snapshot.source)?,snapshot.source_snapshot.clone(),&snapshot.graphs,snapshot.imports.clone(),inputs)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SourceLibraryInputs1027 {
    root: PathBuf,
    manifest: LibraryManifest1027,
    assets_present: Option<bool>,
    assets: Vec<FunctionLibraryAsset>,
    references: Arc<[SourceRefactorFile]>,
}
impl SourceLibraryInputs1027 {
    pub(crate) fn has_manifest_participant1027(files:&[SourceRefactorFile])->bool{files.iter().any(|file|path_key1027(&file.path)=="rvn.toml")}
    pub(crate) fn manifest_absent1027(root:&Path)->Result<bool,String>{legacy_manifest1027(&root.join("rvn.toml"),None).map(|value|value.is_none())}
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn assets(&self) -> &[FunctionLibraryAsset] {
        &self.assets
    }
    pub fn source_references(&self) -> &[SourceRefactorFile] {
        &self.references
    }
    pub fn manifest(&self) -> &LibraryManifest1027 {
        &self.manifest
    }
    pub(super) fn readset1027(&self) -> LibraryReadset1027 {
        LibraryReadset1027::capture(Some(self))
    }
    /// Called under the existing Source constructor/writer lock. A manifest
    /// without a Library table still has a real readset for later first birth.
    pub fn read(root: &Path) -> Result<Option<Self>, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        ordinary_chain1027(&root, &root)?;
        let Some(manifest) = legacy_manifest1027(&root.join("rvn.toml"),None)?
        else {
            return Ok(None);
        };
        let index = parse_library_manifest1027(&manifest)?;
        if index.has_library_index && bounded_optional1027(&root,&root.join("rvn.toml"),MAX_LIBRARY_SOURCE_INPUT_BYTES1027)?.as_deref()!=Some(manifest.as_str()){return Err("Indexed Library manifest changed during strict admission".into())}
        let assets_present = observe_assets1027(&root, &index)?;
        if assets_present != Some(true) && !index.libraries.is_empty() {
            return Err("Indexed Libraries require the original assets directory".into());
        }
        let mut total = manifest.len();
        let mut references = vec![SourceRefactorFile {
            path: "rvn.toml".into(),
            before: Some(manifest.clone()),
            after: Some(manifest),
        }];
        for locator in index.libraries.values() {
            let relative = format!("{}/{locator}", index.assets_relative);
            let remaining = MAX_LIBRARY_SOURCE_INPUT_BYTES1027
                .checked_sub(total)
                .ok_or("Library source batch overflow")?;
            let bytes = bounded_optional1027(
                &root,
                &root.join(&relative),
                remaining.min(MAX_FUNCTION_LIBRARY_BYTES),
            )?
            .ok_or("An indexed original Library descriptor is missing")?;
            total = total
                .checked_add(bytes.len())
                .ok_or("Library source batch overflow")?;
            references.push(SourceRefactorFile {
                path: relative,
                before: Some(bytes.clone()),
                after: Some(bytes),
            });
        }
        let value = Self::from_exact1027(root, references, assets_present)?;
        value.validate_original()?;
        Ok(Some(value))
    }
    fn from_exact1027(
        root: PathBuf,
        references: Vec<SourceRefactorFile>,
        assets_present: Option<bool>,
    ) -> Result<Self, String> {
        let original=references.iter().find(|f|f.path=="rvn.toml").and_then(|f|f.before.as_deref()).ok_or("Library source closure has no original manifest")?;
        let indexed=parse_library_manifest1027(original)?.has_library_index;
        let mut by_path = BTreeMap::new();
        let mut total = 0usize;
        for file in &references {
            if file.before != file.after {
                return Err(
                    "A loaded Library input must be an exact read-only resource participant".into(),
                );
            }
            let text = file
                .after
                .as_deref()
                .ok_or("A loaded Library input cannot be an absent file")?;
            total = total
                .checked_add(text.len())
                .ok_or("Library source byte count overflow")?;
            if indexed && total > MAX_LIBRARY_SOURCE_INPUT_BYTES1027 {
                return Err("Library source closure exceeds 64 MiB".into());
            }
            let key = path_key1027(&file.path);
            if by_path.insert(key, text).is_some() {
                return Err("Library source closure repeats a physical input".into());
            }
        }
        let manifest = parse_library_manifest1027(
            by_path
                .get("rvn.toml")
                .ok_or("Library source closure has no actual manifest")?,
        )?;
        if by_path.len() != manifest.libraries.len() + 1 {
            return Err("Library descriptor closure differs from the exact project index".into());
        }
        let mut assets = Vec::new();
        for (id, locator) in &manifest.libraries {
            let path = format!("{}/{locator}", manifest.assets_relative);
            let bytes = by_path
                .get(&path_key1027(&path))
                .ok_or("Original indexed Library descriptor is absent from its resource closure")?;
            let asset = FunctionLibraryAsset::decode(bytes.as_bytes())?;
            if asset.id() != id {
                return Err("Library descriptor UUID differs from the actual project index".into());
            }
            assets.push(asset);
        }
        Ok(Self {
            root,
            manifest,
            assets_present,
            assets,
            references: references.into(),
        })
    }
    pub fn validate_original(&self) -> Result<(), String> {
        self.validate_directory1027()?;
        if !self.manifest.has_library_index{
            let expected=self.references.iter().find(|f|f.path=="rvn.toml").and_then(|f|f.before.as_deref()).ok_or("Legacy manifest observation is absent")?;
            if legacy_manifest1027(&self.root.join("rvn.toml"),Some(expected.len()))?.as_deref()!=Some(expected){return Err("Original legacy Library manifest bytes changed".into())}return Ok(())
        }
        refactor::check_files(&self.root, &self.references, false)?;
        for file in self.references.iter() {
            let current = bounded_optional1027(&self.root,&self.root.join(&file.path),MAX_LIBRARY_SOURCE_INPUT_BYTES1027)?;
            if current != file.before {return Err("Original Library source bytes changed".into());}
        }
        Ok(())
    }
    fn validate_directory1027(&self)->Result<(),String>{
        ordinary_chain1027(&self.root, &self.root)?;
        if self.root.canonicalize().map_err(|e| e.to_string())? != self.root {
            return Err("Original Library source root changed".into());
        }
        if observe_assets1027(&self.root, &self.manifest)? != self.assets_present {
            return Err("The original assets directory presence changed".into());
        }
        Ok(())
    }
    /// The caller has already checked the same complete Source journal/CAS. This
    /// reconstructs the semantic candidate from its exact participants alone.
    pub(crate) fn from_transaction1027(
        root: &Path,
        files: &[SourceRefactorFile],
        after: bool,
    ) -> Result<Option<Self>, String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        ordinary_chain1027(&root, &root)?;
        let mut values = BTreeMap::new();
        for file in files {
            let key = path_key1027(&file.path);
            let value = if after { &file.after } else { &file.before };
            if values.insert(key, value).is_some() {
                return Err("Source resource journal repeats a physical path".into());
            }
        }
        let manifest = values.get("rvn.toml").ok_or(
            "Scoped Source replay requires its authentic manifest participant, not latest files",
        )?;
        let Some(manifest) = manifest.as_ref() else {
            return Ok(None);
        };
        let index = parse_library_manifest1027(manifest)?;
        let mut refs = vec![SourceRefactorFile {
            path: "rvn.toml".into(),
            before: Some(manifest.clone()),
            after: Some(manifest.clone()),
        }];
        for locator in index.libraries.values() {
            let path = format!("{}/{locator}", index.assets_relative);
            let value = values
                .get(&path_key1027(&path))
                .ok_or("Scoped Source journal lost an indexed descriptor fence")?
                .as_ref()
                .ok_or("Scoped Source journal deleted a still indexed descriptor")?
                .clone();
            refs.push(SourceRefactorFile {
                path,
                before: Some(value.clone()),
                after: Some(value),
            });
        }
        // A nonempty closure necessarily publishes under its declared real folder.
        // Empty legacy closures preserve the actual absence/presence, with no mkdir.
        let assets_present = if index.libraries.is_empty() {
            observe_assets1027(&root, &index)?
        } else {
            Some(true)
        };
        Self::from_exact1027(root, refs, assets_present).map(Some)
    }
}
fn path_key1027(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.into()
    }
}
/// The established no-index manifest policy follows redirects for READ only.
/// Capture its actual file-handle length, then read exactly that length plus
/// one to detect concurrent growth. An indexed manifest is subsequently
/// admitted again through the strict ordinary/root-contained bounded reader.
fn legacy_manifest1027(path:&Path,expected:Option<usize>)->Result<Option<String>,String>{
 let file=match fs::File::open(path){Ok(file)=>file,Err(error)if error.kind()==std::io::ErrorKind::NotFound=>return Ok(None),Err(error)=>return Err(error.to_string())};
 let metadata=file.metadata().map_err(|e|e.to_string())?;if !metadata.is_file(){return Err("Project manifest read target is not a file".into())}
 let length=usize::try_from(metadata.len()).map_err(|_|"Project manifest length is not addressable")?;
 if expected.is_some_and(|old|old!=length){return Err("Original legacy manifest length changed".into())}
 let limit=length.checked_add(1).ok_or("Project manifest length overflow")?;let mut bytes=Vec::new();file.take(limit as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len()!=length{return Err("Project manifest changed during its exact-length read".into())}String::from_utf8(bytes).map(Some).map_err(|e|e.to_string())
}
fn observe_assets1027(root: &Path, index: &LibraryManifest1027) -> Result<Option<bool>, String> {
    if index.has_library_index {
        return directory_presence1027(root, &root.join(&index.assets_relative)).map(Some);
    }
    // Absent Library index retains legacy absolute/external/link path policy.
    // It grants no editable Library namespace. Safe local absence is still a
    // captured fact, so a later first birth cannot silently replace that origin.
    if rvn_parser::widget_asset::validate_relative_locator(&index.assets_relative, None).is_err() {
        return Ok(None);
    }
    Ok(directory_presence1027(root, &root.join(&index.assets_relative)).ok())
}
fn directory_presence1027(root: &Path, path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
        Ok(m) => {
            ordinary_chain1027(root, path)?;
            if !m.is_dir() {
                return Err("Declared Library assets path is not an ordinary directory".into());
            }
            Ok(true)
        }
    }
}
fn ordinary_chain1027(root: &Path, path: &Path) -> Result<(), String> {
    if !path.starts_with(root) {
        return Err("Library input leaves its original Source root".into());
    }
    let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
    let mut current = root.to_path_buf();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            if !matches!(component, Component::Normal(_)) {
                return Err("Library input has a noncanonical path component".into());
            }
            current.push(component.as_os_str());
        }
        let metadata = fs::symlink_metadata(&current).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Source Library admission does not follow symbolic links".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err("Source Library admission does not follow reparse points".into());
            }
        }
    }
    Ok(())
}
fn bounded_optional1027(root: &Path, path: &Path, limit: usize) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(m) => {
            if !m.is_file() || m.len() > limit as u64 {
                return Err("Library source input is not an ordinary bounded file".into());
            }
        }
    }
    ordinary_chain1027(root, path)?;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit as u64 {
        return Err("Library source input changed before read".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Library source input exceeds its retained budget".into());
    }
    ordinary_chain1027(root, path)?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub(super) fn validate_published_snapshot_library1027(
    root: &Path,
    snapshot: &Snapshot,
    readset: &LibraryReadset1027,
) -> Result<SourceProject, String> {
    validate_snapshot_editorial(snapshot)?;
    readset.validate(root)?;
    let source = source_path(root, &snapshot.source)?;
    let native = rvn_parser::parse_for_context_discovery(&snapshot.source_snapshot)
        .map_err(|e| e.to_string())?;
    let imported_native = snapshot
        .imports
        .as_ref()
        .map(|v| v.contains_native_plan())
        .transpose()?
        .unwrap_or(false);
    let declared = crate::empty_native_init_0984::source_has_declarations(
        &snapshot.source_snapshot,
        rvn_parser::NarrativeContext::from_script(&native),
    )?;
    let native =
        crate::native_source_binding::contains_native(&native) || imported_native || declared;
    let inputs = readset.inputs(root)?.map(std::sync::Arc::new);
    SourceProject::open_library_at1027(
        &source,
        snapshot.source_snapshot.clone(),
        &snapshot.graphs,
        inputs.clone(),
        inputs,
        root,
        snapshot.imports.clone(),
        native,
    )
}
