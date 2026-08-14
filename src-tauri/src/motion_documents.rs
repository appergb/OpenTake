//! Project-confined HTML/CSS documents for Motion Studio.

use std::collections::BTreeMap;
use std::path::Path;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use opentake_core::{AppCore, ProjectAssetAuthority};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

#[path = "motion_documents_fs.rs"]
mod document_fs;
use document_fs::{
    cleanup_revision_directory, read_bounded_file, read_bounded_utf8, sync_directory,
    write_catalog_atomic, write_new_file, CatalogWriteError,
};

#[path = "motion_documents_template.rs"]
mod template;
use template::{
    CATALOG_FILE, CATALOG_SCHEMA_VERSION, CSS_FILE, DOCUMENT_MANIFEST_FILE,
    DOCUMENT_SCHEMA_VERSION, HTML_FILE, MAX_CATALOG_BYTES, MAX_DOCUMENTS, MAX_MANIFEST_BYTES,
    MAX_PARAMETERS_BYTES, MAX_PATCH_EDITS, MAX_SOURCE_BYTES, MAX_TITLE_CHARS, MOTION_DOCUMENTS_DIR,
    STARTER_CSS, STARTER_HTML,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocumentSummary {
    pub id: String,
    pub title: String,
    pub revision_hash: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MotionDocument {
    pub summary: MotionDocumentSummary,
    pub html: String,
    pub css: String,
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionDocumentCreateRequest {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionTextReplacement {
    /// UTF-8 byte offset into the selected source file.
    pub start: usize,
    /// UTF-8 byte offset into the selected source file.
    pub end: usize,
    pub replacement: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MotionDocumentPatchRequest {
    pub document_id: String,
    pub file: String,
    pub baseline_hash: String,
    #[serde(default)]
    pub edits: Vec<MotionTextReplacement>,
    pub expected_result_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MotionCatalog {
    schema_version: u32,
    #[serde(default)]
    documents: BTreeMap<String, CatalogEntry>,
}

impl Default for MotionCatalog {
    fn default() -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            documents: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogEntry {
    directory: String,
    summary: MotionDocumentSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocumentManifest {
    schema_version: u32,
    summary: MotionDocumentSummary,
    #[serde(default)]
    parameters: BTreeMap<String, Value>,
}

pub struct MotionDocumentStore {
    core: AppCore,
    operation: Mutex<()>,
    #[cfg(test)]
    fail_next_catalog_replace: AtomicBool,
    #[cfg(test)]
    fail_next_catalog_sync: AtomicBool,
}

struct AuthorizedProjectRoot {
    root: Dir,
    identity: Handle,
    authority: ProjectAssetAuthority,
}

#[derive(Clone, Copy)]
enum EditableFile {
    Html,
    Css,
}

impl EditableFile {
    fn parse(file: &str) -> Result<Self, String> {
        match file {
            HTML_FILE => Ok(Self::Html),
            CSS_FILE => Ok(Self::Css),
            _ => Err("editable file must be exactly index.html or styles.css".into()),
        }
    }
}

impl MotionDocumentStore {
    pub fn new(core: AppCore) -> Self {
        Self {
            core,
            operation: Mutex::new(()),
            #[cfg(test)]
            fail_next_catalog_replace: AtomicBool::new(false),
            #[cfg(test)]
            fail_next_catalog_sync: AtomicBool::new(false),
        }
    }

    pub fn capture_authority(&self) -> Result<ProjectAssetAuthority, String> {
        self.core
            .project_asset_authority()
            .ok_or_else(|| "save the project before editing Motion Studio documents".to_string())
    }

    /// Synchronous embedding API. Tauri commands use the admitted-authority
    /// variant so queueing cannot move a request into a replacement project.
    #[allow(dead_code)]
    pub fn list(&self) -> Result<Vec<MotionDocumentSummary>, String> {
        let authority = self.capture_authority()?;
        self.list_for_authority(authority)
    }

    fn list_for_authority(
        &self,
        authority: ProjectAssetAuthority,
    ) -> Result<Vec<MotionDocumentSummary>, String> {
        let _operation = self.lock_operation()?;
        let _bundle_publication = self.core.lock_project_bundle_publication();
        let _identity_lease = self.core.lock_project_identity_workflow();
        let project = AuthorizedProjectRoot::open_expected(&self.core, authority)?;
        let Some(root) = motion_root(&project.root, false)? else {
            project.ensure_current(&self.core)?;
            return Ok(Vec::new());
        };
        let catalog = read_catalog(&root)?;
        project.ensure_current(&self.core)?;
        let mut summaries = catalog
            .documents
            .into_values()
            .map(|entry| entry.summary)
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(summaries)
    }

    /// Synchronous embedding API; see [`Self::list`].
    #[allow(dead_code)]
    pub fn create(&self, request: MotionDocumentCreateRequest) -> Result<MotionDocument, String> {
        let authority = self.capture_authority()?;
        self.create_for_authority(authority, request)
    }

    fn create_for_authority(
        &self,
        authority: ProjectAssetAuthority,
        request: MotionDocumentCreateRequest,
    ) -> Result<MotionDocument, String> {
        let _operation = self.lock_operation()?;
        let _bundle_publication = self.core.lock_project_bundle_publication();
        let _identity_lease = self.core.lock_project_identity_workflow();
        let project = AuthorizedProjectRoot::open_expected(&self.core, authority)?;
        let root = motion_root(&project.root, true)?.expect("create=true returns a root");
        let mut catalog = read_catalog(&root)?;
        if catalog.documents.len() >= MAX_DOCUMENTS {
            return Err("motion document limit reached".into());
        }
        let title = validated_title(request.title.as_deref().unwrap_or("Untitled Motion"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let parameters = BTreeMap::new();
        let document = document_with_content(id, title, STARTER_HTML, STARTER_CSS, parameters)?;
        let directory = write_revision_directory(&root, &document)?;
        catalog.documents.insert(
            document.summary.id.clone(),
            CatalogEntry {
                directory: directory.clone(),
                summary: document.summary.clone(),
            },
        );
        project.ensure_current(&self.core)?;
        if let Err(error) = self.write_catalog(&root, &catalog) {
            if !error.committed {
                cleanup_revision_directory(&root, &directory);
            }
            return Err(error.message);
        }
        Ok(document)
    }

    /// Synchronous embedding API; see [`Self::list`].
    #[allow(dead_code)]
    pub fn read(&self, document_id: &str) -> Result<MotionDocument, String> {
        let authority = self.capture_authority()?;
        self.read_for_authority(authority, document_id)
    }

    fn read_for_authority(
        &self,
        authority: ProjectAssetAuthority,
        document_id: &str,
    ) -> Result<MotionDocument, String> {
        validate_document_id(document_id)?;
        let _operation = self.lock_operation()?;
        let _bundle_publication = self.core.lock_project_bundle_publication();
        let _identity_lease = self.core.lock_project_identity_workflow();
        let project = AuthorizedProjectRoot::open_expected(&self.core, authority)?;
        let root = motion_root(&project.root, false)?
            .ok_or_else(|| "motion document was not found".to_string())?;
        let catalog = read_catalog(&root)?;
        let entry = catalog
            .documents
            .get(document_id)
            .ok_or_else(|| "motion document was not found".to_string())?;
        let document = read_document(&root, entry)?;
        project.ensure_current(&self.core)?;
        Ok(document)
    }

    /// Synchronous embedding API; see [`Self::list`].
    #[allow(dead_code)]
    pub fn save_patch(
        &self,
        request: MotionDocumentPatchRequest,
    ) -> Result<MotionDocument, String> {
        let authority = self.capture_authority()?;
        self.save_patch_for_authority(authority, request)
    }

    fn save_patch_for_authority(
        &self,
        authority: ProjectAssetAuthority,
        request: MotionDocumentPatchRequest,
    ) -> Result<MotionDocument, String> {
        validate_document_id(&request.document_id)?;
        let editable = EditableFile::parse(&request.file)?;
        if request.edits.is_empty() {
            return Err("motion document patch requires at least one edit".into());
        }
        if request.edits.len() > MAX_PATCH_EDITS {
            return Err("motion document patch has too many edits".into());
        }
        let _operation = self.lock_operation()?;
        let _bundle_publication = self.core.lock_project_bundle_publication();
        let _identity_lease = self.core.lock_project_identity_workflow();
        let project = AuthorizedProjectRoot::open_expected(&self.core, authority)?;
        let root = motion_root(&project.root, false)?
            .ok_or_else(|| "motion document was not found".to_string())?;
        let mut catalog = read_catalog(&root)?;
        let current_entry = catalog
            .documents
            .get(&request.document_id)
            .cloned()
            .ok_or_else(|| "motion document was not found".to_string())?;
        let current = read_document(&root, &current_entry)?;
        if current.summary.revision_hash != request.baseline_hash {
            return Err("motion document revision conflict".into());
        }

        let (html, css) = match editable {
            EditableFile::Html => (
                normalize_line_endings(&apply_replacements(
                    &current.html,
                    request.edits,
                    MAX_SOURCE_BYTES,
                )?),
                current.css.clone(),
            ),
            EditableFile::Css => (
                current.html.clone(),
                normalize_line_endings(&apply_replacements(
                    &current.css,
                    request.edits,
                    MAX_SOURCE_BYTES,
                )?),
            ),
        };
        let computed = revision_hash(&html, &css, &current.parameters)?;
        if computed != request.expected_result_hash {
            return Err("motion document expected result hash did not match".into());
        }
        let next = document_with_content(
            current.summary.id.clone(),
            current.summary.title.clone(),
            &html,
            &css,
            current.parameters.clone(),
        )?;
        if next.summary.revision_hash == current.summary.revision_hash {
            return Ok(current);
        }
        let directory = write_revision_directory(&root, &next)?;
        catalog.documents.insert(
            next.summary.id.clone(),
            CatalogEntry {
                directory: directory.clone(),
                summary: next.summary.clone(),
            },
        );
        project.ensure_current(&self.core)?;
        if let Err(error) = self.write_catalog(&root, &catalog) {
            if !error.committed {
                cleanup_revision_directory(&root, &directory);
            }
            return Err(error.message);
        }
        cleanup_revision_directory(&root, &current_entry.directory);
        Ok(next)
    }

    fn lock_operation(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        self.operation
            .lock()
            .map_err(|_| "motion document store is unavailable".to_string())
    }

    fn write_catalog(&self, root: &Dir, catalog: &MotionCatalog) -> Result<(), CatalogWriteError> {
        let bytes = serde_json::to_vec_pretty(catalog).map_err(|_| CatalogWriteError {
            message: "motion document catalog could not be encoded".to_string(),
            committed: false,
        })?;
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(CatalogWriteError {
                message: "motion document catalog exceeds its byte limit".into(),
                committed: false,
            });
        }
        #[cfg(test)]
        let inject_replace_failure = self.fail_next_catalog_replace.swap(false, Ordering::SeqCst);
        #[cfg(not(test))]
        let inject_replace_failure = false;
        #[cfg(test)]
        let inject_sync_failure = self.fail_next_catalog_sync.swap(false, Ordering::SeqCst);
        #[cfg(not(test))]
        let inject_sync_failure = false;
        write_catalog_atomic(root, &bytes, inject_replace_failure, inject_sync_failure)
    }

    #[cfg(test)]
    fn fail_next_catalog_replace_for_test(&self) {
        self.fail_next_catalog_replace.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn fail_next_catalog_sync_for_test(&self) {
        self.fail_next_catalog_sync.store(true, Ordering::SeqCst);
    }
}

impl AuthorizedProjectRoot {
    fn open_expected(core: &AppCore, authority: ProjectAssetAuthority) -> Result<Self, String> {
        if !core.project_asset_authority_matches(&authority) {
            return Err("current project changed before document access".to_string());
        }
        let path = &authority.project_path;
        let name = path
            .file_name()
            .ok_or_else(|| "current project path has no bundle name".to_string())?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
            .map_err(|_| "current project parent could not be opened".to_string())?;
        let metadata = parent
            .symlink_metadata(name)
            .map_err(|_| "current project bundle could not be inspected".to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("current project must be a no-follow directory".into());
        }
        let root = parent
            .open_dir_nofollow(name)
            .map_err(|_| "current project must be a no-follow directory".to_string())?;
        let identity = Handle::from_file(
            root.try_clone()
                .map_err(|_| "current project authority could not be cloned".to_string())?
                .into_std_file(),
        )
        .map_err(|_| "current project identity could not be retained".to_string())?;
        core.ensure_project_root_identity_for_project(
            authority.project_epoch,
            &authority.project_path,
            &identity,
        )
        .map_err(|_| "current project authority changed before document access".to_string())?;
        Ok(Self {
            root,
            identity,
            authority,
        })
    }

    fn ensure_current(&self, core: &AppCore) -> Result<(), String> {
        core.ensure_project_root_identity_for_project(
            self.authority.project_epoch,
            &self.authority.project_path,
            &self.identity,
        )
        .map_err(|_| "current project changed before document commit".to_string())
    }
}

fn motion_root(project: &Dir, create: bool) -> Result<Option<Dir>, String> {
    match project.symlink_metadata(MOTION_DOCUMENTS_DIR) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("motion document root must be a no-follow directory".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            project
                .create_dir(MOTION_DOCUMENTS_DIR)
                .map_err(|error| format!("motion document root could not be created: {error}"))?;
            sync_directory(project)?;
        }
        Err(error) => {
            return Err(format!(
                "motion document root could not be inspected: {error}"
            ))
        }
    }
    project
        .open_dir_nofollow(MOTION_DOCUMENTS_DIR)
        .map(Some)
        .map_err(|_| "motion document root must be a no-follow directory".to_string())
}

fn read_catalog(root: &Dir) -> Result<MotionCatalog, String> {
    let Some(bytes) = read_bounded_file(root, CATALOG_FILE, MAX_CATALOG_BYTES, "catalog")? else {
        return Ok(MotionCatalog::default());
    };
    let catalog: MotionCatalog = serde_json::from_slice(&bytes)
        .map_err(|_| "motion document catalog is invalid".to_string())?;
    if catalog.schema_version != CATALOG_SCHEMA_VERSION || catalog.documents.len() > MAX_DOCUMENTS {
        return Err("motion document catalog is invalid".into());
    }
    for (id, entry) in &catalog.documents {
        validate_document_id(id)?;
        validate_revision_directory(&entry.directory)?;
        validate_summary(&entry.summary)?;
        if entry.summary.id != *id {
            return Err("motion document catalog identity is invalid".into());
        }
    }
    Ok(catalog)
}

fn read_document(root: &Dir, entry: &CatalogEntry) -> Result<MotionDocument, String> {
    validate_revision_directory(&entry.directory)?;
    let directory = root
        .open_dir_nofollow(&entry.directory)
        .map_err(|_| "motion document revision must be a no-follow directory".to_string())?;
    let manifest_bytes = read_bounded_file(
        &directory,
        DOCUMENT_MANIFEST_FILE,
        MAX_MANIFEST_BYTES,
        "manifest",
    )?
    .ok_or_else(|| "motion document manifest is missing".to_string())?;
    let manifest: DocumentManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| "motion document manifest is invalid".to_string())?;
    if manifest.schema_version != DOCUMENT_SCHEMA_VERSION || manifest.summary != entry.summary {
        return Err("motion document manifest does not match the catalog".into());
    }
    validate_summary(&manifest.summary)?;
    validate_parameters(&manifest.parameters)?;
    let html = read_bounded_utf8(&directory, HTML_FILE, MAX_SOURCE_BYTES)?;
    let css = read_bounded_utf8(&directory, CSS_FILE, MAX_SOURCE_BYTES)?;
    if revision_hash(&html, &css, &manifest.parameters)? != manifest.summary.revision_hash {
        return Err("motion document revision hash is invalid".into());
    }
    Ok(MotionDocument {
        summary: manifest.summary,
        html,
        css,
        parameters: manifest.parameters,
    })
}

fn document_with_content(
    id: String,
    title: String,
    html: &str,
    css: &str,
    parameters: BTreeMap<String, Value>,
) -> Result<MotionDocument, String> {
    validate_document_id(&id)?;
    let title = validated_title(&title)?;
    validate_source(html)?;
    validate_source(css)?;
    let html = normalize_line_endings(html);
    let css = normalize_line_endings(css);
    validate_parameters(&parameters)?;
    let summary = MotionDocumentSummary {
        id,
        title,
        revision_hash: revision_hash(&html, &css, &parameters)?,
        updated_at: updated_at_millis(),
    };
    Ok(MotionDocument {
        summary,
        html,
        css,
        parameters,
    })
}

fn write_revision_directory(root: &Dir, document: &MotionDocument) -> Result<String, String> {
    let manifest = DocumentManifest {
        schema_version: DOCUMENT_SCHEMA_VERSION,
        summary: document.summary.clone(),
        parameters: document.parameters.clone(),
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| "motion document manifest could not be encoded".to_string())?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err("motion document manifest exceeds its byte limit".into());
    }
    let directory_name = format!(
        "rev-{}-{}-{}",
        document.summary.id,
        &document.summary.revision_hash[..16],
        uuid::Uuid::new_v4()
    );
    validate_revision_directory(&directory_name)?;
    root.create_dir(&directory_name)
        .map_err(|error| format!("motion document revision could not be created: {error}"))?;
    let directory = root
        .open_dir_nofollow(&directory_name)
        .map_err(|_| "motion document revision must be a no-follow directory".to_string())?;
    let result = (|| {
        drop(write_new_file(
            &directory,
            DOCUMENT_MANIFEST_FILE,
            &manifest_bytes,
        )?);
        drop(write_new_file(
            &directory,
            HTML_FILE,
            document.html.as_bytes(),
        )?);
        drop(write_new_file(
            &directory,
            CSS_FILE,
            document.css.as_bytes(),
        )?);
        sync_directory(&directory)
    })();
    if let Err(error) = result {
        cleanup_revision_directory(root, &directory_name);
        return Err(error);
    }
    Ok(directory_name)
}

fn apply_replacements(
    source: &str,
    mut edits: Vec<MotionTextReplacement>,
    max_bytes: usize,
) -> Result<String, String> {
    edits.sort_by_key(|edit| (edit.start, edit.end));
    let mut previous: Option<(usize, usize)> = None;
    for edit in &edits {
        if edit.start > edit.end
            || edit.end > source.len()
            || !source.is_char_boundary(edit.start)
            || !source.is_char_boundary(edit.end)
        {
            return Err("motion document edit range is invalid".into());
        }
        if previous.is_some_and(|(start, end)| edit.start < end || edit.start == start) {
            return Err("motion document edits overlap".into());
        }
        previous = Some((edit.start, edit.end));
    }
    let removed = edits
        .iter()
        .map(|edit| edit.end - edit.start)
        .sum::<usize>();
    let inserted = edits
        .iter()
        .try_fold(0usize, |total, edit| {
            total.checked_add(edit.replacement.len())
        })
        .ok_or_else(|| "motion document patch exceeds its byte limit".to_string())?;
    let result_len = source
        .len()
        .checked_sub(removed)
        .and_then(|length| length.checked_add(inserted))
        .ok_or_else(|| "motion document patch exceeds its byte limit".to_string())?;
    if result_len > max_bytes {
        return Err("motion document patch exceeds its byte limit".into());
    }
    let mut result = source.to_string();
    for edit in edits.into_iter().rev() {
        result.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(result)
}

fn normalize_line_endings(source: &str) -> String {
    source.replace("\r\n", "\n").replace('\r', "\n")
}

fn revision_hash(
    html: &str,
    css: &str,
    parameters: &BTreeMap<String, Value>,
) -> Result<String, String> {
    let parameters = serde_json::to_vec(parameters)
        .map_err(|_| "motion document parameters could not be encoded".to_string())?;
    let mut digest = Sha256::new();
    digest.update(b"opentake-motion-document-v1\0");
    for bytes in [html.as_bytes(), css.as_bytes(), parameters.as_slice()] {
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_document_id(id: &str) -> Result<(), String> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| "motion document id is invalid")?;
    if parsed.to_string() != id {
        return Err("motion document id is invalid".into());
    }
    Ok(())
}

fn validate_revision_directory(name: &str) -> Result<(), String> {
    if !name.starts_with("rev-")
        || name.len() > 160
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("motion document revision directory is invalid".into());
    }
    Ok(())
}

fn validated_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty()
        || title.chars().count() > MAX_TITLE_CHARS
        || title.chars().any(char::is_control)
    {
        return Err("motion document title is invalid".into());
    }
    Ok(title.to_string())
}

fn validate_summary(summary: &MotionDocumentSummary) -> Result<(), String> {
    validate_document_id(&summary.id)?;
    validated_title(&summary.title)?;
    if summary.revision_hash.len() != 64
        || !summary
            .revision_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || summary.updated_at == 0
    {
        return Err("motion document summary is invalid".into());
    }
    Ok(())
}

fn validate_source(source: &str) -> Result<(), String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("motion document source exceeds its byte limit".into());
    }
    Ok(())
}

fn validate_parameters(parameters: &BTreeMap<String, Value>) -> Result<(), String> {
    let bytes = serde_json::to_vec(parameters)
        .map_err(|_| "motion document parameters are invalid".to_string())?;
    if bytes.len() > MAX_PARAMETERS_BYTES {
        return Err("motion document parameters exceed their byte limit".into());
    }
    Ok(())
}

fn updated_at_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[tauri::command]
pub async fn motion_document_list(
    state: State<'_, Arc<MotionDocumentStore>>,
) -> Result<Vec<MotionDocumentSummary>, String> {
    let authority = state.capture_authority()?;
    let store = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || store.list_for_authority(authority))
        .await
        .map_err(|error| format!("motion document worker failed: {error}"))?
}

#[tauri::command]
pub async fn motion_document_create(
    state: State<'_, Arc<MotionDocumentStore>>,
    request: MotionDocumentCreateRequest,
) -> Result<MotionDocument, String> {
    let authority = state.capture_authority()?;
    let store = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || store.create_for_authority(authority, request))
        .await
        .map_err(|error| format!("motion document worker failed: {error}"))?
}

#[tauri::command]
pub async fn motion_document_read(
    state: State<'_, Arc<MotionDocumentStore>>,
    document_id: String,
) -> Result<MotionDocument, String> {
    let authority = state.capture_authority()?;
    let store = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || store.read_for_authority(authority, &document_id))
        .await
        .map_err(|error| format!("motion document worker failed: {error}"))?
}

#[tauri::command]
pub async fn motion_document_patch(
    state: State<'_, Arc<MotionDocumentStore>>,
    request: MotionDocumentPatchRequest,
) -> Result<MotionDocument, String> {
    let authority = state.capture_authority()?;
    let store = Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move || store.save_patch_for_authority(authority, request))
        .await
        .map_err(|error| format!("motion document worker failed: {error}"))?
}

#[cfg(test)]
#[path = "motion_documents_tests.rs"]
mod tests;
