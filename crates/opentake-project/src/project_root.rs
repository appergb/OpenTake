use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use cap_fs_ext::{ambient_authority, DirExt, FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, OpenOptions};
use same_file::Handle;

use crate::error::{ProjectError, Result};

const TIMELINE_COMPONENT_MAX_BYTES: usize = 64 * 1024 * 1024;
const MANIFEST_COMPONENT_MAX_BYTES: usize = 32 * 1024 * 1024;
const GENERATION_LOG_COMPONENT_MAX_BYTES: usize = 16 * 1024 * 1024;
const THUMBNAIL_COMPONENT_MAX_BYTES: usize = 16 * 1024 * 1024;
const PUBLISH_MARKER_FILE: &str = ".opentake-publish-marker";
const PUBLISH_MARKER_MAX_BYTES: usize = 256;
const TRANSACTION_JOURNAL_MAX_BYTES: usize = 4 * 1024;

fn project_component_max_bytes(name: &str) -> Option<usize> {
    match name {
        crate::layout::TIMELINE_FILE => Some(TIMELINE_COMPONENT_MAX_BYTES),
        crate::layout::MANIFEST_FILE => Some(MANIFEST_COMPONENT_MAX_BYTES),
        crate::layout::GENERATION_LOG_FILE => Some(GENERATION_LOG_COMPONENT_MAX_BYTES),
        crate::layout::THUMBNAIL_FILE => Some(THUMBNAIL_COMPONENT_MAX_BYTES),
        PUBLISH_MARKER_FILE => Some(PUBLISH_MARKER_MAX_BYTES),
        _ => None,
    }
}

fn read_bounded_regular_file(
    file: &mut cap_std::fs::File,
    path: &Path,
    max_bytes: usize,
    description: &str,
) -> Result<Vec<u8>> {
    let metadata = file
        .metadata()
        .map_err(|error| ProjectError::io(path, error))?;
    if !metadata.is_file() {
        return Err(ProjectError::io(
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{description} is not a nofollow regular file"),
            ),
        ));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(ProjectError::io(
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{description} exceeds the {max_bytes}-byte limit"),
            ),
        ));
    }
    read_bounded_contents(file, path, metadata.len() as usize, max_bytes, description)
}

fn read_bounded_contents(
    reader: &mut impl Read,
    path: &Path,
    initial_capacity: usize,
    max_bytes: usize,
    description: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(initial_capacity.min(max_bytes));
    Read::by_ref(reader)
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ProjectError::io(path, error))?;
    if bytes.len() > max_bytes {
        return Err(ProjectError::io(
            path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{description} grew beyond the {max_bytes}-byte limit"),
            ),
        ));
    }
    Ok(bytes)
}

/// Retained authority for one concrete `.opentake` bundle directory.
///
/// The final bundle component is always opened no-follow. Consequently a path
/// whose final component is a symlink is rejected rather than canonicalized.
/// All component reads and same-project saves are relative to `dir`, so later
/// ambient A→B→A rebinding cannot redirect I/O.
pub struct ProjectRoot {
    path: PathBuf,
    parent: Dir,
    name: OsString,
    dir: Dir,
    identity: Handle,
    stable_identity: ProjectRootIdentity,
}

/// Cross-process identity of one retained project directory.
///
/// `volume`/`file` are `(st_dev, st_ino)` on Unix and
/// `(volume serial number, file index)` on Windows. They are obtained from an
/// already-open no-follow directory handle, never by trusting an ambient path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectRootIdentity {
    pub volume: u64,
    pub file: u64,
}

impl std::fmt::Debug for ProjectRoot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectRoot")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl ProjectRoot {
    /// Open an existing bundle directory without following its final component.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .ok_or_else(|| ProjectError::NotABundle(path.to_path_buf()))?;
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
            .map_err(|error| ProjectError::io(parent_path, error))?;
        Self::open_from_parent(path, parent, name.to_owned())
    }

    fn open_from_parent(path: &Path, parent: Dir, name: OsString) -> Result<Self> {
        let metadata = match parent.symlink_metadata(&name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ProjectError::NotABundle(path.to_path_buf()));
            }
            Err(error) => return Err(ProjectError::io(path, error)),
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ProjectError::NotABundle(path.to_path_buf()));
        }
        let dir = parent
            .open_dir_nofollow(&name)
            .map_err(|error| ProjectError::io(path, error))?;
        let identity_file = dir
            .try_clone()
            .map_err(|error| ProjectError::io(path, error))?
            .into_std_file();
        let stable_identity = stable_project_root_identity(&identity_file)
            .map_err(|error| ProjectError::io(path, error))?;
        let identity =
            Handle::from_file(identity_file).map_err(|error| ProjectError::io(path, error))?;
        Ok(Self {
            path: path.to_path_buf(),
            parent,
            name,
            dir,
            identity,
            stable_identity,
        })
    }

    /// Create the directory if needed, then retain the concrete no-follow root
    /// before any project component is written.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .ok_or_else(|| ProjectError::NotABundle(path.to_path_buf()))?
            .to_owned();
        let parent_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent_path).map_err(|error| ProjectError::io(parent_path, error))?;
        let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
            .map_err(|error| ProjectError::io(parent_path, error))?;
        match parent.create_dir(&name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ProjectError::io(path, error)),
        }
        Self::open_from_parent(path, parent, name)
    }

    pub(crate) fn open_optional(path: impl AsRef<Path>) -> Result<Option<Self>> {
        let path = path.as_ref();
        match fs::symlink_metadata(path) {
            Ok(_) => Self::open(path).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(ProjectError::io(path, error)),
        }
    }

    pub(crate) fn begin_replace(path: impl AsRef<Path>) -> Result<BundlePublisher> {
        BundlePublisher::begin(path.as_ref())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn matches_identity(&self, other: &Handle) -> bool {
        &self.identity == other
    }

    /// Stable no-follow identity retained for this concrete bundle. Callers
    /// may pass it back to the core before out-of-band project writes.
    pub fn identity(&self) -> &Handle {
        &self.identity
    }

    /// Serializable identity derived from the retained root handle. This is
    /// used to bind isolated asset-reader results back to the exact project
    /// session that authorized them.
    pub fn stable_identity(&self) -> ProjectRootIdentity {
        self.stable_identity
    }

    /// Open a project-local asset through this retained bundle capability.
    /// Every directory and the final leaf are opened no-follow, so an ambient
    /// rename/replacement of the `.opentake` pathname cannot redirect the read.
    pub fn open_asset_file(&self, relative: &Path) -> Result<fs::File> {
        if relative.is_absolute() {
            return Err(ProjectError::io(
                self.path.join(relative),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "project asset path must be relative",
                ),
            ));
        }
        let components = relative.components().collect::<Vec<_>>();
        if components.is_empty()
            || components
                .iter()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ProjectError::io(
                self.path.join(relative),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "project asset path contains an unsafe component",
                ),
            ));
        }

        let mut directory = self
            .dir
            .try_clone()
            .map_err(|error| ProjectError::io(&self.path, error))?;
        for component in &components[..components.len() - 1] {
            let Component::Normal(name) = component else {
                unreachable!("components were validated above");
            };
            directory = directory
                .open_dir_nofollow(name)
                .map_err(|error| ProjectError::io(self.path.join(relative), error))?;
        }

        let Component::Normal(name) = components[components.len() - 1] else {
            unreachable!("components were validated above");
        };
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_OPEN_NO_RECALL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .custom_flags(FILE_FLAG_OPEN_NO_RECALL);
        }
        directory
            .open_with(name, &options)
            .map(cap_std::fs::File::into_std)
            .map_err(|error| ProjectError::io(self.path.join(relative), error))
    }

    /// Diagnostic comparison for a caller-supplied path alias. Same-project
    /// saves continue through this root even when the logical spelling differs.
    pub fn matches_path(&self, path: impl AsRef<Path>) -> Result<bool> {
        Ok(Self::open_optional(path)?
            .as_ref()
            .is_some_and(|candidate| candidate.identity == self.identity))
    }

    /// Diagnostic namespace check. Authorization never depends on this path
    /// re-open; all I/O continues through the retained `dir` authority.
    pub fn is_current_namespace(&self) -> Result<bool> {
        let current = match self.parent.open_dir_nofollow(&self.name) {
            Ok(current) => current,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(ProjectError::io(&self.path, error)),
        };
        let current = Handle::from_file(current.into_std_file())
            .map_err(|error| ProjectError::io(&self.path, error))?;
        Ok(self.identity == current)
    }

    /// Copy the retained source bundle's `media/` tree into a retained
    /// destination bundle. Both traversal and publication are capability
    /// relative, so ambient source or destination rebinding cannot redirect it.
    pub fn copy_media_to(&self, destination: &ProjectRoot) -> Result<()> {
        self.copy_directory_component_to(destination, crate::layout::MEDIA_DIR, "media-copy")
    }

    /// Write one fresh media leaf into this retained bundle.
    ///
    /// Complete generation publication uses this only on an unpublished stage,
    /// after the existing media tree has been copied. The final leaf is created
    /// with `create_new`, streamed without an ambient destination path, checked
    /// against the downloader's exact byte count, synced, and kept only after
    /// every write succeeds.
    pub(crate) fn write_new_media_leaf(
        &self,
        name: &str,
        expected_bytes: u64,
        source: &mut dyn Read,
    ) -> Result<()> {
        validate_leaf(name).map_err(|error| {
            ProjectError::io(self.path.join(crate::layout::MEDIA_DIR).join(name), error)
        })?;
        let media_path = self.path.join(crate::layout::MEDIA_DIR);
        match self.dir.create_dir(crate::layout::MEDIA_DIR) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(ProjectError::io(&media_path, error)),
        }
        let media = self
            .dir
            .open_dir_nofollow(crate::layout::MEDIA_DIR)
            .map_err(|error| ProjectError::io(&media_path, error))?;
        let mut leaf = TransactionLeaf::create(&media, name)
            .map_err(|error| ProjectError::io(media_path.join(name), error))?;
        let copied = std::io::copy(
            &mut source.take(expected_bytes.saturating_add(1)),
            leaf.handle.as_file_mut(),
        )
        .map_err(|error| ProjectError::io(media_path.join(name), error))?;
        if copied != expected_bytes {
            return Err(ProjectError::io(
                media_path.join(name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "generated media size changed before publication",
                ),
            ));
        }
        leaf.handle
            .as_file_mut()
            .flush()
            .and_then(|()| leaf.handle.as_file().sync_all())
            .map_err(|error| ProjectError::io(media_path.join(name), error))?;
        leaf.cleanup_on_drop = false;
        Ok(())
    }

    /// Copy project-local Agent conversations during complete-bundle
    /// publication (Save As / archive) through retained no-follow roots.
    pub fn copy_chat_sessions_to(&self, destination: &ProjectRoot) -> Result<()> {
        self.copy_directory_component_to(
            destination,
            crate::layout::CHAT_SESSIONS_DIR,
            "chat-sessions-copy",
        )
    }

    /// Preserve the optional project cover across complete-bundle publication.
    pub(crate) fn copy_thumbnail_to(&self, destination: &ProjectRoot) -> Result<()> {
        if let Some(bytes) = self.read_optional(crate::layout::THUMBNAIL_FILE)? {
            destination.write_atomic(crate::layout::THUMBNAIL_FILE, &bytes)?;
        }
        Ok(())
    }

    fn copy_directory_component_to(
        &self,
        destination: &ProjectRoot,
        component: &str,
        staging_prefix: &str,
    ) -> Result<()> {
        let source = match self.dir.open_dir_nofollow(component) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(ProjectError::io(self.path.join(component), error)),
        };
        let staging_name = unique_directory_name(staging_prefix);
        destination
            .dir
            .create_dir(&staging_name)
            .map_err(|error| ProjectError::io(destination.path.join(&staging_name), error))?;
        let staging = destination
            .dir
            .open_dir_nofollow(&staging_name)
            .map_err(|error| ProjectError::io(destination.path.join(&staging_name), error))?;
        let copy_result = copy_directory(&source, &staging);
        // In particular on Windows, no directory handle which denied
        // FILE_SHARE_DELETE may remain live across cleanup or publication.
        // `open_retained_dir` does share deletion there, but close this
        // transient authority anyway so the rename commit point has the
        // smallest possible handle set.
        drop(staging);
        if let Err(error) = copy_result {
            let _ = remove_directory_artifact(&destination.dir, &destination.path, &staging_name);
            return Err(ProjectError::io(destination.path.join(component), error));
        }
        match destination.dir.symlink_metadata(component) {
            Ok(_) => {
                let _ =
                    remove_directory_artifact(&destination.dir, &destination.path, &staging_name);
                return Err(ProjectError::io(
                    destination.path.join(component),
                    std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("destination {component} already exists; complete bundle publication requires a fresh staging root"),
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                let _ =
                    remove_directory_artifact(&destination.dir, &destination.path, &staging_name);
                return Err(ProjectError::io(destination.path.join(component), error));
            }
        }
        destination
            .dir
            .rename(&staging_name, &destination.dir, component)
            .map_err(|error| ProjectError::io(destination.path.join(component), error))?;
        Ok(())
    }

    pub(crate) fn has_media_tree(&self) -> Result<bool> {
        match self.dir.open_dir_nofollow(crate::layout::MEDIA_DIR) {
            Ok(media) => {
                drop(media);
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(ProjectError::io(
                self.path.join(crate::layout::MEDIA_DIR),
                error,
            )),
        }
    }

    pub(crate) fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path.join(name);
        let max_bytes = project_component_max_bytes(name).ok_or_else(|| {
            ProjectError::io(
                &path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "project component has no configured byte limit",
                ),
            )
        })?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        let mut file = match self.dir.open_with(name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(&path, error)),
        };
        read_bounded_regular_file(&mut file, &path, max_bytes, "project component").map(Some)
    }

    pub(crate) fn write_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        validate_leaf(name).map_err(|error| ProjectError::io(self.path.join(name), error))?;
        let tmp_name = unique_temp_name(name);
        let mut tmp = TransactionLeaf::create(&self.dir, &tmp_name)
            .map_err(|error| ProjectError::io(self.path.join(&tmp_name), error))?;
        tmp.handle
            .as_file_mut()
            .write_all(bytes)
            .map_err(|error| ProjectError::io(self.path.join(&tmp_name), error))?;
        tmp.handle
            .as_file()
            .sync_all()
            .map_err(|error| ProjectError::io(self.path.join(&tmp_name), error))?;
        tmp.replace(&self.dir, Path::new(name))
            .map_err(|error| ProjectError::io(self.path.join(name), error))?;
        // The single replace syscall is the commit point; nothing fallible runs
        // after it before cleanup is disarmed.
        tmp.cleanup_on_drop = false;
        Ok(())
    }

    /// Read one no-follow regular file from `chat-sessions/`, bounded before
    /// allocation and while streaming in case the retained file grows.
    pub fn read_chat_session(&self, name: &str, max_bytes: usize) -> Result<Option<Vec<u8>>> {
        validate_leaf(name).map_err(|error| {
            ProjectError::io(
                self.path.join(crate::layout::CHAT_SESSIONS_DIR).join(name),
                error,
            )
        })?;
        let Some(directory) = self.chat_sessions_directory(false)? else {
            return Ok(None);
        };
        let path = self.path.join(crate::layout::CHAT_SESSIONS_DIR).join(name);
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        let mut file = match directory.open_with(name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(path, error)),
        };
        let metadata = file
            .metadata()
            .map_err(|error| ProjectError::io(&path, error))?;
        if !metadata.is_file() {
            return Err(ProjectError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "chat session is not a nofollow regular file",
                ),
            ));
        }
        if metadata.len() > max_bytes as u64 {
            return Err(ProjectError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "chat session exceeds the configured byte limit",
                ),
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| ProjectError::io(&path, error))?;
        if bytes.len() > max_bytes {
            return Err(ProjectError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "chat session grew beyond the configured byte limit",
                ),
            ));
        }
        Ok(Some(bytes))
    }

    /// Atomically replace one no-follow regular file in `chat-sessions/`.
    pub fn write_chat_session_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        validate_leaf(name).map_err(|error| {
            ProjectError::io(
                self.path.join(crate::layout::CHAT_SESSIONS_DIR).join(name),
                error,
            )
        })?;
        let directory = self
            .chat_sessions_directory(true)?
            .expect("create=true returns a directory");
        let directory_path = self.path.join(crate::layout::CHAT_SESSIONS_DIR);
        let tmp_name = unique_temp_name(name);
        let mut tmp = TransactionLeaf::create(&directory, &tmp_name)
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.handle
            .as_file_mut()
            .write_all(bytes)
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.handle
            .as_file()
            .sync_all()
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.replace(&directory, Path::new(name))
            .map_err(|error| ProjectError::io(directory_path.join(name), error))?;
        tmp.cleanup_on_drop = false;
        Ok(())
    }

    /// Read one project-managed LUT through retained no-follow directories.
    pub fn read_lut(&self, name: &str, max_bytes: usize) -> Result<Option<Vec<u8>>> {
        validate_leaf(name).map_err(|error| {
            ProjectError::io(
                self.path
                    .join(crate::layout::MEDIA_DIR)
                    .join(crate::layout::LUTS_DIR)
                    .join(name),
                error,
            )
        })?;
        let Some(directory) = self.luts_directory(false)? else {
            return Ok(None);
        };
        let path = self
            .path
            .join(crate::layout::MEDIA_DIR)
            .join(crate::layout::LUTS_DIR)
            .join(name);
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.custom_flags(libc::O_NONBLOCK);
        let mut file = match directory.open_with(name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(path, error)),
        };
        let metadata = file
            .metadata()
            .map_err(|error| ProjectError::io(&path, error))?;
        if !metadata.is_file() || metadata.len() > max_bytes as u64 {
            return Err(ProjectError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "LUT is not a bounded nofollow regular file",
                ),
            ));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        Read::by_ref(&mut file)
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| ProjectError::io(&path, error))?;
        if bytes.len() > max_bytes {
            return Err(ProjectError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "LUT grew beyond the configured byte limit",
                ),
            ));
        }
        Ok(Some(bytes))
    }

    /// Atomically publish one validated, content-addressed LUT under
    /// `media/luts/`. Callers validate both the bytes and digest before entry.
    pub fn write_lut_atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        validate_leaf(name).map_err(|error| {
            ProjectError::io(
                self.path
                    .join(crate::layout::MEDIA_DIR)
                    .join(crate::layout::LUTS_DIR)
                    .join(name),
                error,
            )
        })?;
        let directory = self
            .luts_directory(true)?
            .expect("create=true returns a directory");
        let directory_path = self
            .path
            .join(crate::layout::MEDIA_DIR)
            .join(crate::layout::LUTS_DIR);
        let tmp_name = unique_temp_name(name);
        let mut tmp = TransactionLeaf::create(&directory, &tmp_name)
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.handle
            .as_file_mut()
            .write_all(bytes)
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.handle
            .as_file()
            .sync_all()
            .map_err(|error| ProjectError::io(directory_path.join(&tmp_name), error))?;
        tmp.replace(&directory, Path::new(name))
            .map_err(|error| ProjectError::io(directory_path.join(name), error))?;
        tmp.cleanup_on_drop = false;
        Ok(())
    }

    /// List no-follow regular leaves in `chat-sessions/`. Callers own the
    /// filename policy (for example selecting only `<session>.json`).
    pub fn list_chat_session_files(&self, max_entries: usize) -> Result<Vec<OsString>> {
        let Some(directory) = self.chat_sessions_directory(false)? else {
            return Ok(Vec::new());
        };
        let directory_path = self.path.join(crate::layout::CHAT_SESSIONS_DIR);
        let entries = directory
            .entries()
            .map_err(|error| ProjectError::io(&directory_path, error))?;
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| ProjectError::io(&directory_path, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| ProjectError::io(&directory_path, error))?;
            if !file_type.is_file() {
                return Err(ProjectError::io(
                    directory_path.join(entry.file_name()),
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "chat session directory contains a non-regular entry",
                    ),
                ));
            }
            if file_type.is_file() {
                if files.len() == max_entries {
                    return Err(ProjectError::io(
                        &directory_path,
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "chat session directory exceeds the configured entry limit",
                        ),
                    ));
                }
                files.push(entry.file_name());
            }
        }
        files.sort();
        Ok(files)
    }

    fn chat_sessions_directory(&self, create: bool) -> Result<Option<Dir>> {
        let name = crate::layout::CHAT_SESSIONS_DIR;
        let path = self.path.join(name);
        match self.dir.symlink_metadata(name) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(ProjectError::io(
                        path,
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "chat-sessions must be a nofollow directory",
                        ),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                match self.dir.create_dir(name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(ProjectError::io(&path, error)),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(&path, error)),
        }
        self.dir
            .open_dir_nofollow(name)
            .map(Some)
            .map_err(|error| ProjectError::io(path, error))
    }

    fn luts_directory(&self, create: bool) -> Result<Option<Dir>> {
        let media_path = self.path.join(crate::layout::MEDIA_DIR);
        match self.dir.symlink_metadata(crate::layout::MEDIA_DIR) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(ProjectError::io(
                    media_path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "media must be a nofollow directory",
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                self.dir
                    .create_dir(crate::layout::MEDIA_DIR)
                    .or_else(|error| {
                        if error.kind() == std::io::ErrorKind::AlreadyExists {
                            Ok(())
                        } else {
                            Err(error)
                        }
                    })
                    .map_err(|error| ProjectError::io(&media_path, error))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(&media_path, error)),
        }
        let media = self
            .dir
            .open_dir_nofollow(crate::layout::MEDIA_DIR)
            .map_err(|error| ProjectError::io(&media_path, error))?;
        let luts_path = media_path.join(crate::layout::LUTS_DIR);
        match media.symlink_metadata(crate::layout::LUTS_DIR) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(ProjectError::io(
                    luts_path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "luts must be a nofollow directory",
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                media
                    .create_dir(crate::layout::LUTS_DIR)
                    .or_else(|error| {
                        if error.kind() == std::io::ErrorKind::AlreadyExists {
                            Ok(())
                        } else {
                            Err(error)
                        }
                    })
                    .map_err(|error| ProjectError::io(&luts_path, error))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(&luts_path, error)),
        }
        media
            .open_dir_nofollow(crate::layout::LUTS_DIR)
            .map(Some)
            .map_err(|error| ProjectError::io(luts_path, error))
    }
}

#[cfg(unix)]
fn stable_project_root_identity(file: &fs::File) -> std::io::Result<ProjectRootIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata()?;
    Ok(ProjectRootIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(target_os = "windows")]
fn stable_project_root_identity(file: &fs::File) -> std::io::Result<ProjectRootIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    // `std::os::windows::fs::MetadataExt::volume_serial_number/file_index` are
    // unstable (rust-lang/rust#63010); use the stable handle query instead,
    // mirroring src-tauri's retained_file_etag.
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` owns a live handle and `information` is writable.
    if unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            std::ptr::addr_of_mut!(information),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok(ProjectRootIdentity {
        volume: u64::from(information.dwVolumeSerialNumber),
        file: file_index,
    })
}

/// One complete-bundle sibling publication. The persistent lock leaf
/// coordinates well-behaved processes; it does not defend against an attacker
/// that can mutate the parent directory outside this protocol. Every artifact
/// operation still uses the retained parent and no-follow directory opens.
pub(crate) struct BundlePublisher {
    target_path: PathBuf,
    parent_path: PathBuf,
    parent: Dir,
    target_name: OsString,
    stage_name: OsString,
    backup_name: OsString,
    journal_name: OsString,
    journal: PublishJournal,
    target_identity: Option<ProjectRootIdentity>,
    stage_identity: ProjectRootIdentity,
    stage: Option<ProjectRoot>,
    _lock: std::fs::File,
    _lock_identity: Handle,
}

impl BundlePublisher {
    fn begin(target_path: &Path) -> Result<Self> {
        let target_name = target_path
            .file_name()
            .ok_or_else(|| ProjectError::NotABundle(target_path.to_path_buf()))?
            .to_owned();
        let parent_path = target_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        fs::create_dir_all(&parent_path).map_err(|error| ProjectError::io(&parent_path, error))?;
        let parent = Dir::open_ambient_dir(&parent_path, ambient_authority())
            .map_err(|error| ProjectError::io(&parent_path, error))?;
        let backup_name = artifact_name(&target_name, ".opentake-backup");
        let journal_name = artifact_name(&target_name, ".opentake-journal");
        let lock_name = artifact_name(&target_name, ".opentake-lock");
        let lock_path = parent_path.join(&lock_name);
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        let lock = parent
            .open_with(&lock_name, &options)
            .map_err(|error| ProjectError::io(&lock_path, error))?;
        if !lock
            .metadata()
            .map_err(|error| ProjectError::io(&lock_path, error))?
            .is_file()
        {
            return Err(ProjectError::io(
                lock_path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle transaction lock is not a nofollow regular file",
                ),
            ));
        }
        let lock = lock.into_std();
        let lock_identity = Handle::from_file(
            lock.try_clone()
                .map_err(|error| ProjectError::io(&lock_path, error))?,
        )
        .map_err(|error| ProjectError::io(&lock_path, error))?;
        lock.try_lock()
            .map_err(|error| ProjectError::io(&lock_path, error.into()))?;
        let current_lock = open_leaf_identity(&parent, &lock_name)
            .map_err(|error| ProjectError::io(&lock_path, error))?;
        if current_lock != lock_identity {
            return Err(ProjectError::io(
                lock_path,
                std::io::Error::other("bundle transaction lock identity changed during lock"),
            ));
        }

        recover_bundle_transaction(
            &parent,
            &parent_path,
            &target_name,
            &backup_name,
            &journal_name,
        )?;
        clear_idle_publish_marker(&parent, &parent_path, &target_name)?;
        let target_identity = directory_identity(&parent, &parent_path, &target_name, false)?;
        let journal = PublishJournal::new(target_identity);
        write_new_file_artifact(&parent, &parent_path, &journal_name, &journal.encode())?;
        let stage_name = stage_artifact_name(&target_name, &journal.nonce);
        parent
            .create_dir(&stage_name)
            .map_err(|error| ProjectError::io(parent_path.join(&stage_name), error))?;
        let stage_path = parent_path.join(&stage_name);
        let stage = ProjectRoot::open_from_parent(
            &stage_path,
            parent
                .try_clone()
                .map_err(|error| ProjectError::io(&parent_path, error))?,
            stage_name.clone(),
        )?;
        let stage_identity = stage.stable_identity();
        let publisher = Self {
            target_path: target_path.to_path_buf(),
            parent_path,
            parent,
            target_name,
            stage_name,
            backup_name,
            journal_name,
            journal,
            target_identity,
            stage_identity,
            stage: Some(stage),
            _lock: lock,
            _lock_identity: lock_identity,
        };
        publisher
            .stage()
            .write_atomic(PUBLISH_MARKER_FILE, publisher.journal.nonce.as_bytes())?;
        Ok(publisher)
    }

    pub(crate) fn stage(&self) -> &ProjectRoot {
        self.stage
            .as_ref()
            .expect("bundle publisher always owns a stage before publication")
    }

    pub(crate) fn publish(mut self) -> Result<ProjectRoot> {
        let result = {
            #[cfg(test)]
            if FAIL_PUBLISH_AFTER_BACKUP.with(|fail| fail.replace(false)) {
                self.publish_with_hook(|| {
                    Err(std::io::Error::other(
                        "injected publication failure after backup",
                    ))
                })
            } else {
                self.publish_with_hook(|| Ok(()))
            }

            #[cfg(not(test))]
            self.publish_with_hook(|| Ok(()))
        };

        // A caller may immediately start the next complete-bundle save while
        // retaining the returned ProjectRoot. Make that successful handoff
        // explicit instead of depending on the two cloned lock handles being
        // dropped at the end of this function. Error paths retain the lock
        // through Drop's staged-artifact cleanup. Closing the handles remains
        // the fallback; an unlock error cannot safely turn an already
        // committed publication into a reported failure.
        if result.is_ok() {
            let _ = self._lock.unlock();
        }
        result
    }

    fn publish_with_hook(
        &mut self,
        after_backup: impl FnOnce() -> std::io::Result<()>,
    ) -> Result<ProjectRoot> {
        let stage = self
            .stage
            .as_ref()
            .expect("bundle publisher owns its stage before publication");
        if stage.stable_identity() != self.stage_identity
            || !stage.is_current_namespace()?
            || !marker_matches(stage, &self.journal.nonce)?
        {
            return Err(ProjectError::io(
                self.parent_path.join(&self.stage_name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle stage identity or transaction nonce changed before publication",
                ),
            ));
        }
        // Close the staged ProjectRoot (and its identity clone) before any
        // sibling rename. This is required by Windows sharing semantics and
        // also makes the namespace rename the sole publication authority.
        drop(
            self.stage
                .take()
                .expect("validated bundle stage remains owned before publication"),
        );

        let current_target_identity =
            directory_identity(&self.parent, &self.parent_path, &self.target_name, false)?;
        if current_target_identity != self.target_identity {
            let message = if current_target_identity.is_some() != self.target_identity.is_some() {
                "bundle target existence changed after transaction preparation"
            } else {
                "bundle target identity changed after transaction preparation"
            };
            return Err(ProjectError::io(
                &self.target_path,
                std::io::Error::new(std::io::ErrorKind::InvalidData, message),
            ));
        }
        let target_exists = current_target_identity.is_some();
        if inspect_directory(&self.parent, &self.parent_path, &self.backup_name, true)? {
            return Err(ProjectError::io(
                self.parent_path.join(&self.backup_name),
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "bundle backup unexpectedly exists after recovery",
                ),
            ));
        }
        if target_exists {
            self.parent
                .rename(&self.target_name, &self.parent, &self.backup_name)
                .map_err(|error| ProjectError::io(&self.target_path, error))?;
            self.journal.phase = PublishPhase::BackedUp;
            if let Err(journal_error) = write_file_artifact_atomic(
                &self.parent,
                &self.parent_path,
                &self.journal_name,
                &self.journal.encode(),
            ) {
                if let Err(restore_error) =
                    self.parent
                        .rename(&self.backup_name, &self.parent, &self.target_name)
                {
                    return Err(ProjectError::RecoveryRequired {
                        backup: self.parent_path.join(&self.backup_name),
                        publish: journal_error.to_string(),
                        restore: restore_error.to_string(),
                    });
                }
                if let Err(cleanup_error) = self.cleanup_aborted_publish() {
                    return Err(ProjectError::RecoveryRequired {
                        backup: self.parent_path.join(&self.backup_name),
                        publish: journal_error.to_string(),
                        restore: format!(
                            "old target restored but transaction cleanup failed: {cleanup_error}"
                        ),
                    });
                }
                return Err(journal_error);
            }
        }
        let publish = after_backup().and_then(|()| {
            self.parent
                .rename(&self.stage_name, &self.parent, &self.target_name)
        });
        if let Err(publish_error) = publish {
            if target_exists {
                if let Err(restore_error) =
                    self.parent
                        .rename(&self.backup_name, &self.parent, &self.target_name)
                {
                    return Err(ProjectError::RecoveryRequired {
                        backup: self.parent_path.join(&self.backup_name),
                        publish: publish_error.to_string(),
                        restore: restore_error.to_string(),
                    });
                }
            }
            if let Err(cleanup_error) = self.cleanup_aborted_publish() {
                return Err(ProjectError::RecoveryRequired {
                    backup: self.parent_path.join(&self.backup_name),
                    publish: publish_error.to_string(),
                    restore: format!(
                        "old target restored but transaction cleanup failed: {cleanup_error}"
                    ),
                });
            }
            return Err(ProjectError::io(&self.target_path, publish_error));
        }

        // The rename committed the new target. Reopen it from the retained
        // parent and verify the journal nonce before returning authority to
        // the caller. A failure here is post-commit ambiguity, so preserve the
        // backup/journal and require recovery instead of returning an ordinary
        // error that might invite a blind retry.
        let root = ProjectRoot::open_from_parent(
            &self.target_path,
            self.parent
                .try_clone()
                .map_err(|error| ProjectError::RecoveryRequired {
                    backup: self.parent_path.join(&self.backup_name),
                    publish: format!("new target committed but parent clone failed: {error}"),
                    restore: "automatic recovery refused after commit".to_string(),
                })?,
            self.target_name.clone(),
        )
        .map_err(|error| ProjectError::RecoveryRequired {
            backup: self.parent_path.join(&self.backup_name),
            publish: format!("new target committed but could not be reopened: {error}"),
            restore: "automatic recovery refused after commit".to_string(),
        })?;
        if !marker_matches(&root, &self.journal.nonce).map_err(|error| {
            ProjectError::RecoveryRequired {
                backup: self.parent_path.join(&self.backup_name),
                publish: format!("new target committed but nonce verification failed: {error}"),
                restore: "automatic recovery refused after commit".to_string(),
            }
        })? {
            return Err(ProjectError::RecoveryRequired {
                backup: self.parent_path.join(&self.backup_name),
                publish: "new target committed without the journal nonce".to_string(),
                restore: "automatic recovery refused after commit".to_string(),
            });
        }
        let backup_cleaned = if target_exists {
            // The backup name must still denote the exact original target
            // directory that this transaction backed up. A hook or ambient
            // actor that rebound a foreign object at the backup name must
            // never be deleted: fail closed and preserve it for recovery
            // instead of silently destroying foreign data.
            let backup_identity =
                directory_identity(&self.parent, &self.parent_path, &self.backup_name, true)?;
            if backup_identity != self.target_identity {
                return Err(ProjectError::io(
                    self.parent_path.join(&self.backup_name),
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "bundle backup identity changed after transaction preparation",
                    ),
                ));
            }
            #[cfg(test)]
            if FAIL_BACKUP_CLEANUP.with(|fail| fail.replace(false)) {
                return Ok(root);
            }
            remove_directory_artifact(&self.parent, &self.parent_path, &self.backup_name).is_ok()
        } else {
            true
        };
        if backup_cleaned {
            #[cfg(test)]
            let skip_journal_cleanup =
                FAIL_JOURNAL_CLEANUP_AFTER_BACKUP.with(|fail| fail.replace(false));
            #[cfg(not(test))]
            let skip_journal_cleanup = false;
            let journal_cleaned = !skip_journal_cleanup
                && remove_file_artifact(&self.parent, &self.parent_path, &self.journal_name)
                    .is_ok();
            if journal_cleaned {
                #[cfg(test)]
                let skip_marker_cleanup =
                    FAIL_COMMITTED_MARKER_CLEANUP.with(|fail| fail.replace(false));
                #[cfg(not(test))]
                let skip_marker_cleanup = false;
                if !skip_marker_cleanup {
                    let _ = remove_file_artifact(
                        &root.dir,
                        &root.path,
                        OsStr::new(PUBLISH_MARKER_FILE),
                    );
                }
            }
        }
        Ok(root)
    }

    fn cleanup_aborted_publish(&mut self) -> Result<()> {
        if self.journal.had_target {
            self.journal.phase = PublishPhase::AbortedRestored;
            write_file_artifact_atomic(
                &self.parent,
                &self.parent_path,
                &self.journal_name,
                &self.journal.encode(),
            )?;
        }
        finish_aborted_publish_cleanup(
            &self.parent,
            &self.parent_path,
            Some(&self.stage_name),
            &self.journal_name,
        )
    }
}

impl Drop for BundlePublisher {
    fn drop(&mut self) {
        let Some(stage) = self.stage.take() else {
            return;
        };
        if !stage.is_current_namespace().unwrap_or(false) {
            return;
        }
        drop(stage);
        let stage_removed =
            remove_directory_artifact(&self.parent, &self.parent_path, &self.stage_name).is_ok()
                && !inspect_directory(&self.parent, &self.parent_path, &self.stage_name, true)
                    .unwrap_or(true);
        if stage_removed
            && !inspect_directory(&self.parent, &self.parent_path, &self.backup_name, true)
                .unwrap_or(true)
        {
            let _ = remove_file_artifact(&self.parent, &self.parent_path, &self.journal_name);
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static FAIL_BACKUP_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_JOURNAL_CLEANUP_AFTER_BACKUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_ABORT_CLEANUP_BEFORE_STAGE_REMOVAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_ABORT_CLEANUP_BEFORE_JOURNAL_REMOVAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_STAGE_CLEANUP_AFTER_MARKER_REMOVAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_COMMITTED_MARKER_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_IDLE_MARKER_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAIL_PUBLISH_AFTER_BACKUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn artifact_name(target: &OsStr, suffix: &str) -> OsString {
    let mut name = OsString::from(".");
    name.push(target);
    name.push(suffix);
    name
}

fn stage_artifact_prefix(target: &OsStr) -> OsString {
    artifact_name(target, ".opentake-stage-")
}

fn stage_artifact_name(target: &OsStr, nonce: &str) -> OsString {
    let mut name = stage_artifact_prefix(target);
    name.push(nonce);
    name
}

fn valid_publish_nonce(nonce: &str) -> bool {
    let mut parts = nonce.split('-');
    let valid_part = |part: &str| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(first), Some(second), Some(third), None)
            if valid_part(first) && valid_part(second) && valid_part(third)
    )
}

fn os_starts_with(value: &OsStr, prefix: &OsStr) -> bool {
    value
        .as_encoded_bytes()
        .starts_with(prefix.as_encoded_bytes())
}

fn stage_artifacts(parent: &Dir, parent_path: &Path, target_name: &OsStr) -> Result<Vec<OsString>> {
    let legacy_name = artifact_name(target_name, ".opentake-stage");
    let prefix = stage_artifact_prefix(target_name);
    let mut artifacts = Vec::new();
    let entries = parent
        .entries()
        .map_err(|error| ProjectError::io(parent_path, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| ProjectError::io(parent_path, error))?;
        let name = entry.file_name();
        if name == legacy_name || os_starts_with(&name, &prefix) {
            artifacts.push(name);
        }
    }
    artifacts.sort();
    Ok(artifacts)
}

fn finish_aborted_publish_cleanup(
    parent: &Dir,
    parent_path: &Path,
    stage_name: Option<&OsStr>,
    journal_name: &OsStr,
) -> Result<()> {
    #[cfg(test)]
    if FAIL_ABORT_CLEANUP_BEFORE_STAGE_REMOVAL.with(|fail| fail.replace(false)) {
        return Err(ProjectError::io(
            parent_path.join(journal_name),
            std::io::Error::other("injected abort cleanup failure before stage removal"),
        ));
    }
    if let Some(stage_name) = stage_name {
        remove_directory_artifact(parent, parent_path, stage_name)?;
    }
    #[cfg(test)]
    if FAIL_ABORT_CLEANUP_BEFORE_JOURNAL_REMOVAL.with(|fail| fail.replace(false)) {
        return Err(ProjectError::io(
            parent_path.join(journal_name),
            std::io::Error::other("injected abort cleanup failure before journal removal"),
        ));
    }
    remove_file_artifact(parent, parent_path, journal_name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublishPhase {
    Prepared,
    BackedUp,
    AbortedRestored,
}

#[derive(Debug)]
struct PublishJournal {
    nonce: String,
    had_target: bool,
    target_identity: Option<ProjectRootIdentity>,
    phase: PublishPhase,
}

impl PublishJournal {
    fn new(target_identity: Option<ProjectRootIdentity>) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            nonce: format!("{:x}-{:x}-{sequence:x}", std::process::id(), nanos),
            had_target: target_identity.is_some(),
            target_identity,
            phase: PublishPhase::Prepared,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let (target_volume, target_file) = match self.target_identity {
            Some(identity) => (identity.volume.to_string(), identity.file.to_string()),
            None => ("none".to_string(), "none".to_string()),
        };
        format!(
            "version=2\nnonce={}\nhad_target={}\ntarget_volume={}\ntarget_file={}\nphase={}\n",
            self.nonce,
            u8::from(self.had_target),
            target_volume,
            target_file,
            match self.phase {
                PublishPhase::Prepared => "prepared",
                PublishPhase::BackedUp => "backed_up",
                PublishPhase::AbortedRestored => "aborted_restored",
            }
        )
        .into_bytes()
    }

    fn decode(bytes: &[u8]) -> std::io::Result<Self> {
        let document = std::str::from_utf8(bytes).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "journal is not UTF-8")
        })?;
        let mut version = None;
        let mut nonce = None;
        let mut had_target = None;
        let mut target_volume = None;
        let mut target_file = None;
        let mut phase = None;
        for line in document.lines() {
            let (key, value) = line.split_once('=').ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid journal line")
            })?;
            match key {
                "version" if version.replace(value).is_none() => {}
                "nonce" if nonce.replace(value).is_none() => {}
                "had_target" if had_target.replace(value).is_none() => {}
                "target_volume" if target_volume.replace(value).is_none() => {}
                "target_file" if target_file.replace(value).is_none() => {}
                "phase" if phase.replace(value).is_none() => {}
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "duplicate or unknown journal field",
                    ))
                }
            }
        }
        if !matches!(version, Some("1" | "2")) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported journal version",
            ));
        }
        let nonce = nonce
            .filter(|nonce| valid_publish_nonce(nonce))
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid journal nonce")
            })?
            .to_string();
        let had_target = match had_target {
            Some("0") => false,
            Some("1") => true,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid journal target state",
                ))
            }
        };
        let target_identity = match version {
            Some("1") if target_volume.is_none() && target_file.is_none() => None,
            Some("2") => match (target_volume, target_file) {
                (Some("none"), Some("none")) if !had_target => None,
                (Some(volume), Some(file)) if had_target => {
                    let volume = volume.parse::<u64>().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid journal target volume identity",
                        )
                    })?;
                    let file = file.parse::<u64>().map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid journal target file identity",
                        )
                    })?;
                    Some(ProjectRootIdentity { volume, file })
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "journal target existence and identity disagree",
                    ))
                }
            },
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "legacy journal contains unexpected identity fields",
                ))
            }
        };
        let phase = match phase {
            Some("prepared") => PublishPhase::Prepared,
            Some("backed_up") => PublishPhase::BackedUp,
            Some("aborted_restored") => PublishPhase::AbortedRestored,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid journal phase",
                ))
            }
        };
        Ok(Self {
            nonce,
            had_target,
            target_identity,
            phase,
        })
    }
}

fn open_leaf_identity(parent: &Dir, name: &OsStr) -> std::io::Result<Handle> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let file = parent.open_with(name, &options)?;
    Handle::from_file(file.into_std())
}

fn inspect_directory(
    parent: &Dir,
    parent_path: &Path,
    name: &OsStr,
    artifact: bool,
) -> Result<bool> {
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(if artifact {
            ProjectError::io(
                parent_path.join(name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle transaction artifact is not a nofollow directory",
                ),
            )
        } else {
            ProjectError::NotABundle(parent_path.join(name))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProjectError::io(parent_path.join(name), error)),
    }
}

fn directory_identity(
    parent: &Dir,
    parent_path: &Path,
    name: &OsStr,
    artifact: bool,
) -> Result<Option<ProjectRootIdentity>> {
    if !inspect_directory(parent, parent_path, name, artifact)? {
        return Ok(None);
    }
    let root = ProjectRoot::open_from_parent(
        &parent_path.join(name),
        parent
            .try_clone()
            .map_err(|error| ProjectError::io(parent_path, error))?,
        name.to_owned(),
    )?;
    if !root.is_current_namespace()? {
        return Err(ProjectError::io(
            parent_path.join(name),
            std::io::Error::other("bundle directory identity changed during inspection"),
        ));
    }
    Ok(Some(root.stable_identity()))
}

fn remove_directory_artifact(parent: &Dir, parent_path: &Path, name: &OsStr) -> Result<()> {
    if !inspect_directory(parent, parent_path, name, true)? {
        return Ok(());
    }
    let retained = parent
        .open_dir_nofollow(name)
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    let retained_identity = Handle::from_file(
        retained
            .try_clone()
            .map_err(|error| ProjectError::io(parent_path.join(name), error))?
            .into_std_file(),
    )
    .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    let current = parent
        .open_dir_nofollow(name)
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    let current_identity = Handle::from_file(current.into_std_file())
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    if retained_identity != current_identity {
        return Err(ProjectError::io(
            parent_path.join(name),
            std::io::Error::other("bundle transaction artifact identity changed before cleanup"),
        ));
    }
    drop(current_identity);
    drop(retained_identity);
    #[cfg(test)]
    if FAIL_STAGE_CLEANUP_AFTER_MARKER_REMOVAL.with(|fail| fail.replace(false)) {
        match retained.remove_file(PUBLISH_MARKER_FILE) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(ProjectError::io(parent_path.join(name), error)),
        }
        return Err(ProjectError::io(
            parent_path.join(name),
            std::io::Error::other("injected partial stage cleanup failure"),
        ));
    }
    retained
        .remove_open_dir_all()
        .map_err(|error| ProjectError::io(parent_path.join(name), error))
}

fn read_file_artifact(parent: &Dir, parent_path: &Path, name: &OsStr) -> Result<Vec<u8>> {
    let path = parent_path.join(name);
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK);
    let mut file = parent
        .open_with(name, &options)
        .map_err(|error| ProjectError::io(&path, error))?;
    read_bounded_regular_file(
        &mut file,
        &path,
        TRANSACTION_JOURNAL_MAX_BYTES,
        "bundle transaction journal",
    )
}

fn write_new_file_artifact(
    parent: &Dir,
    parent_path: &Path,
    name: &OsStr,
    bytes: &[u8],
) -> Result<()> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut file = parent
        .open_with(name, &options)
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| ProjectError::io(parent_path.join(name), error))
}

fn write_file_artifact_atomic(
    parent: &Dir,
    parent_path: &Path,
    name: &OsStr,
    bytes: &[u8],
) -> Result<()> {
    let temp_name = unique_temp_name(&name.to_string_lossy());
    let mut leaf = TransactionLeaf::create(parent, &temp_name)
        .map_err(|error| ProjectError::io(parent_path.join(&temp_name), error))?;
    leaf.handle
        .as_file_mut()
        .write_all(bytes)
        .and_then(|()| leaf.handle.as_file().sync_all())
        .map_err(|error| ProjectError::io(parent_path.join(&temp_name), error))?;
    leaf.replace(parent, Path::new(name))
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    leaf.cleanup_on_drop = false;
    Ok(())
}

fn remove_file_artifact(parent: &Dir, parent_path: &Path, name: &OsStr) -> Result<()> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }
    let retained = match parent.open_with(name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(ProjectError::io(parent_path.join(name), error)),
    };
    if !retained
        .metadata()
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?
        .is_file()
    {
        return Err(ProjectError::io(
            parent_path.join(name),
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bundle transaction file artifact is not a nofollow regular file",
            ),
        ));
    }
    let retained_identity = Handle::from_file(retained.into_std())
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    let current_identity = open_leaf_identity(parent, name)
        .map_err(|error| ProjectError::io(parent_path.join(name), error))?;
    if retained_identity != current_identity {
        return Err(ProjectError::io(
            parent_path.join(name),
            std::io::Error::other("bundle transaction file identity changed before cleanup"),
        ));
    }
    parent
        .remove_file(name)
        .map_err(|error| ProjectError::io(parent_path.join(name), error))
}

fn marker_matches(root: &ProjectRoot, nonce: &str) -> Result<bool> {
    match root.read_optional(PUBLISH_MARKER_FILE)? {
        Some(bytes) => Ok(bytes == nonce.as_bytes()),
        None => Ok(false),
    }
}

fn clear_idle_publish_marker(parent: &Dir, parent_path: &Path, target_name: &OsStr) -> Result<()> {
    if !inspect_directory(parent, parent_path, target_name, false)? {
        return Ok(());
    }
    let target = ProjectRoot::open_from_parent(
        &parent_path.join(target_name),
        parent
            .try_clone()
            .map_err(|error| ProjectError::io(parent_path, error))?,
        target_name.to_owned(),
    )?;
    if target.read_optional(PUBLISH_MARKER_FILE)?.is_none() {
        return Ok(());
    }
    #[cfg(test)]
    if FAIL_IDLE_MARKER_CLEANUP.with(|fail| fail.replace(false)) {
        return Err(ProjectError::io(
            target.path.join(PUBLISH_MARKER_FILE),
            std::io::Error::other("injected idle publish marker cleanup failure"),
        ));
    }
    remove_file_artifact(&target.dir, &target.path, OsStr::new(PUBLISH_MARKER_FILE))?;
    if target.read_optional(PUBLISH_MARKER_FILE)?.is_some() {
        return Err(ProjectError::io(
            target.path.join(PUBLISH_MARKER_FILE),
            std::io::Error::other("idle publish marker remained after cleanup"),
        ));
    }
    Ok(())
}

fn recover_bundle_transaction(
    parent: &Dir,
    parent_path: &Path,
    target_name: &OsStr,
    backup_name: &OsStr,
    journal_name: &OsStr,
) -> Result<()> {
    let target_exists = inspect_directory(parent, parent_path, target_name, false)?;
    let backup_exists = inspect_directory(parent, parent_path, backup_name, true)?;
    let stages = stage_artifacts(parent, parent_path, target_name)?;
    let journal_exists = match parent.symlink_metadata(journal_name) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(ProjectError::io(
                parent_path.join(journal_name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "bundle transaction journal is not a nofollow regular file",
                ),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(ProjectError::io(parent_path.join(journal_name), error)),
    };
    if !journal_exists {
        if backup_exists {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "backup exists without a matching transaction journal".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        if let Some(stage_name) = stages.first() {
            return Err(ProjectError::io(
                parent_path.join(stage_name),
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "stage exists without a matching transaction journal",
                ),
            ));
        }
        return Ok(());
    }

    let journal_bytes = read_file_artifact(parent, parent_path, journal_name)?;
    let mut journal = PublishJournal::decode(&journal_bytes)
        .map_err(|error| ProjectError::io(parent_path.join(journal_name), error))?;
    let expected_stage_name = stage_artifact_name(target_name, &journal.nonce);
    let legacy_stage_name = artifact_name(target_name, ".opentake-stage");
    if stages.len() > 1
        || stages
            .first()
            .is_some_and(|name| name != &expected_stage_name && name != &legacy_stage_name)
    {
        return Err(ProjectError::io(
            parent_path.join(
                stages
                    .first()
                    .cloned()
                    .unwrap_or_else(|| expected_stage_name.clone()),
            ),
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "bundle transaction has unknown or multiple stage artifacts",
            ),
        ));
    }
    let stage_name = stages.first();
    let (stage_matches, stage_unmarked) = if let Some(stage_name) = stage_name {
        inspect_directory(parent, parent_path, stage_name, true)?;
        let stage = ProjectRoot::open_from_parent(
            &parent_path.join(stage_name),
            parent
                .try_clone()
                .map_err(|error| ProjectError::io(parent_path, error))?,
            stage_name.to_owned(),
        )?;
        match stage.read_optional(PUBLISH_MARKER_FILE)? {
            Some(marker) => (marker == journal.nonce.as_bytes(), false),
            None => (false, true),
        }
    } else {
        (false, false)
    };
    let (target_matches, target_unmarked) = if target_exists {
        let target = ProjectRoot::open_from_parent(
            &parent_path.join(target_name),
            parent
                .try_clone()
                .map_err(|error| ProjectError::io(parent_path, error))?,
            target_name.to_owned(),
        )?;
        match target.read_optional(PUBLISH_MARKER_FILE)? {
            Some(marker) => (marker == journal.nonce.as_bytes(), false),
            None => (false, true),
        }
    } else {
        (false, false)
    };

    if backup_exists {
        if target_exists {
            if stage_name.is_some()
                || !journal.had_target
                || journal.phase != PublishPhase::BackedUp
                || !target_matches
            {
                return Err(ProjectError::RecoveryRequired {
                    backup: parent_path.join(backup_name),
                    publish: "target plus backup does not match a committed replacement journal"
                        .to_string(),
                    restore: "automatic recovery refused".to_string(),
                });
            }
            remove_directory_artifact(parent, parent_path, backup_name)?;
            remove_file_artifact(parent, parent_path, journal_name)?;
            let target = ProjectRoot::open_from_parent(
                &parent_path.join(target_name),
                parent
                    .try_clone()
                    .map_err(|error| ProjectError::io(parent_path, error))?,
                target_name.to_owned(),
            )?;
            let _ =
                remove_file_artifact(&target.dir, &target.path, OsStr::new(PUBLISH_MARKER_FILE));
            return Ok(());
        }
        if !journal.had_target
            || !matches!(
                journal.phase,
                PublishPhase::Prepared | PublishPhase::BackedUp
            )
        {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "backup exists for a journal that records no prior target".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        if stage_name.is_some() && !stage_matches {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "stage does not carry the journal nonce".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        parent
            .rename(backup_name, parent, target_name)
            .map_err(|restore| ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "prior bundle publication was interrupted".to_string(),
                restore: restore.to_string(),
            })?;
        journal.phase = PublishPhase::AbortedRestored;
        write_file_artifact_atomic(parent, parent_path, journal_name, &journal.encode())?;
        return finish_aborted_publish_cleanup(
            parent,
            parent_path,
            stage_name.map(OsString::as_os_str),
            journal_name,
        );
    }

    if target_matches {
        if stage_name.is_some()
            || !matches!(
                (journal.had_target, journal.phase),
                (false, PublishPhase::Prepared) | (true, PublishPhase::BackedUp)
            )
        {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "marked target does not match a committed journal state".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        remove_file_artifact(parent, parent_path, journal_name)?;
        let target = ProjectRoot::open_from_parent(
            &parent_path.join(target_name),
            parent
                .try_clone()
                .map_err(|error| ProjectError::io(parent_path, error))?,
            target_name.to_owned(),
        )?;
        let _ = remove_file_artifact(&target.dir, &target.path, OsStr::new(PUBLISH_MARKER_FILE));
        return Ok(());
    }

    if let Some(stage_name) = stage_name {
        let nonce_named_unmarked_stage = stage_name == &expected_stage_name && stage_unmarked;
        let recoverable_unmarked_stage = nonce_named_unmarked_stage
            && (journal.phase == PublishPhase::Prepared && target_exists == journal.had_target
                || journal.phase == PublishPhase::AbortedRestored
                    && journal.had_target
                    && target_exists
                    && target_unmarked);
        if !(stage_matches || recoverable_unmarked_stage) {
            return Err(ProjectError::io(
                parent_path.join(stage_name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "stage does not carry the journal nonce",
                ),
            ));
        }
        if !target_exists && journal.had_target {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "prior target is missing while its stage exists without a backup"
                    .to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        if target_exists && !journal.had_target {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "unexpected target appeared during first-save staging".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        if journal.phase == PublishPhase::BackedUp {
            if !journal.had_target || !target_exists || !stage_matches {
                return Err(ProjectError::RecoveryRequired {
                    backup: parent_path.join(backup_name),
                    publish: "backed-up journal does not prove a restored target".to_string(),
                    restore: "automatic recovery refused".to_string(),
                });
            }
            journal.phase = PublishPhase::AbortedRestored;
            write_file_artifact_atomic(parent, parent_path, journal_name, &journal.encode())?;
        } else if journal.phase == PublishPhase::AbortedRestored
            && (!journal.had_target || !target_exists || !target_unmarked)
        {
            return Err(ProjectError::RecoveryRequired {
                backup: parent_path.join(backup_name),
                publish: "restored-abort journal does not match its target and stage".to_string(),
                restore: "automatic recovery refused".to_string(),
            });
        }
        return finish_aborted_publish_cleanup(
            parent,
            parent_path,
            Some(stage_name.as_os_str()),
            journal_name,
        );
    }

    let prepared_without_stage =
        journal.phase == PublishPhase::Prepared && target_exists == journal.had_target;
    let restored_without_stage = journal.phase == PublishPhase::AbortedRestored
        && journal.had_target
        && target_exists
        && target_unmarked;
    if prepared_without_stage || restored_without_stage {
        return remove_file_artifact(parent, parent_path, journal_name);
    }
    Err(ProjectError::RecoveryRequired {
        backup: parent_path.join(backup_name),
        publish: "journal state does not match target or stage".to_string(),
        restore: "automatic recovery refused".to_string(),
    })
}

fn validate_leaf(name: &str) -> std::io::Result<()> {
    if matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    ) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project component must be one relative leaf",
        ))
    }
}

fn unique_temp_name(target: &str) -> OsString {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    OsString::from(format!(
        ".{target}.{}.{sequence:020}.tmp",
        std::process::id()
    ))
}

fn unique_directory_name(tag: &str) -> OsString {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    OsString::from(format!(".{tag}.{}.{sequence:020}.tmp", std::process::id()))
}

fn copy_directory(source: &Dir, destination: &Dir) -> std::io::Result<()> {
    for entry in source.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "project media copy refuses symlinks",
            ));
        }
        if file_type.is_dir() {
            destination.create_dir(&name)?;
            let child_source = source.open_dir_nofollow(&name)?;
            let child_destination = destination.open_dir_nofollow(&name)?;
            copy_directory(&child_source, &child_destination)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "project media copy supports only regular files and directories",
            ));
        }
        let mut read_options = OpenOptions::new();
        read_options.read(true).follow(FollowSymlinks::No);
        let mut source_file = source.open_with(&name, &read_options)?;
        if !source_file.metadata()?.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "project media source changed during copy",
            ));
        }
        let mut write_options = OpenOptions::new();
        write_options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut destination_file = destination.open_with(&name, &write_options)?;
        std::io::copy(&mut source_file, &mut destination_file)?;
        destination_file.flush()?;
        destination_file.sync_all()?;
    }
    Ok(())
}

struct TransactionLeaf {
    name: OsString,
    root: Dir,
    handle: Handle,
    cleanup_on_drop: bool,
}

impl TransactionLeaf {
    fn create(root: &Dir, name: impl AsRef<Path>) -> std::io::Result<Self> {
        let name = name.as_ref().as_os_str().to_owned();
        let retained_root = root.try_clone()?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
            use windows_sys::Win32::Storage::FileSystem::{
                DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options
                .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        }
        let file = root.open_with(&name, &options)?;
        Ok(Self {
            name,
            root: retained_root,
            handle: Handle::from_file(file.into_std())?,
            cleanup_on_drop: true,
        })
    }

    fn matches_name(&self, root: &Dir) -> std::io::Result<bool> {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        let file = root.open_with(&self.name, &options)?;
        Ok(self.handle == Handle::from_file(file.into_std())?)
    }

    fn replace(&mut self, root: &Dir, target: &Path) -> std::io::Result<()> {
        if !self.matches_name(root)? {
            return Err(std::io::Error::other(
                "project transaction identity changed before replacement",
            ));
        }
        #[cfg(not(windows))]
        root.rename(&self.name, root, target)?;
        #[cfg(windows)]
        rename_by_handle(root, self, target)?;
        self.name = target.as_os_str().to_owned();
        Ok(())
    }
}

impl Drop for TransactionLeaf {
    fn drop(&mut self) {
        if !self.cleanup_on_drop || !self.matches_name(&self.root).unwrap_or(false) {
            return;
        }
        let _ = self.handle.as_file().set_len(0);
        let _ = self.handle.as_file().sync_all();
        #[cfg(windows)]
        if delete_by_handle(self).is_ok() {
            return;
        }
        if self.matches_name(&self.root).unwrap_or(false) {
            let _ = self.root.remove_file(&self.name);
        }
    }
}

#[cfg(windows)]
fn delete_by_handle(leaf: &TransactionLeaf) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };
    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let deleted = unsafe {
        SetFileInformationByHandle(
            leaf.handle.as_file().as_raw_handle(),
            FileDispositionInfo,
            std::ptr::addr_of!(disposition).cast(),
            u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>())
                .expect("FILE_DISPOSITION_INFO size fits u32"),
        )
    };
    if deleted == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn rename_by_handle(root: &Dir, leaf: &TransactionLeaf, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Wdk::Storage::FileSystem::{
        FileRenameInformation, NtSetInformationFile, FILE_RENAME_INFORMATION,
        FILE_RENAME_INFORMATION_0,
    };
    use windows_sys::Win32::Foundation::RtlNtStatusToDosError;
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

    let target = match target.components().collect::<Vec<_>>().as_slice() {
        [Component::Normal(name)] => *name,
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "project transaction target must be one relative leaf",
            ))
        }
    };
    let wide: Vec<u16> = target.encode_wide().collect();
    if wide.is_empty() || wide.contains(&0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "project transaction target is empty or contains NUL",
        ));
    }
    let file_name_bytes = wide
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|size| u32::try_from(size).ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "target too long"))?;
    let info_size = std::mem::size_of::<FILE_RENAME_INFORMATION>()
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rename allocation overflow",
            )
        })?;
    const _: () =
        assert!(std::mem::align_of::<usize>() >= std::mem::align_of::<FILE_RENAME_INFORMATION>());
    let word_size = std::mem::size_of::<usize>();
    let mut storage = vec![0_usize; info_size.div_ceil(word_size)];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFORMATION>();
    let info_size = u32::try_from(info_size).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename buffer too large")
    })?;
    // SAFETY: storage is aligned and sized for the complete native header plus
    // the UTF-16 target. Both retained handles remain open throughout the call.
    // TransactionLeaf is opened without FILE_FLAG_OVERLAPPED, so the operation
    // completes synchronously and cannot outlive the stack IO_STATUS_BLOCK.
    let status = unsafe {
        (*info).Anonymous = FILE_RENAME_INFORMATION_0 {
            ReplaceIfExists: true,
        };
        (*info).RootDirectory = root.as_raw_handle();
        (*info).FileNameLength = file_name_bytes;
        std::ptr::copy_nonoverlapping(
            wide.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            wide.len(),
        );
        let mut io_status = IO_STATUS_BLOCK::default();
        NtSetInformationFile(
            leaf.handle.as_file().as_raw_handle(),
            &mut io_status,
            info.cast(),
            info_size,
            FileRenameInformation,
        )
    };
    if status < 0 {
        // SAFETY: this converts only the NTSTATUS value returned above.
        let error = unsafe { RtlNtStatusToDosError(status) };
        return Err(std::io::Error::from_raw_os_error(error as i32));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "opentake-project-root-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_asset_open_stays_bound_to_retained_root_after_namespace_rebind() {
        let tmp = TmpDir::new("asset-root-rebind");
        let selected = tmp.path().join("Selected.opentake");
        let retained = tmp.path().join("Retained-A.opentake");
        let relative = Path::new("media/nested/clip.mp4");
        fs::create_dir_all(selected.join("media/nested")).unwrap();
        fs::write(selected.join(relative), b"project-a").unwrap();
        let root = ProjectRoot::open(&selected).unwrap();

        fs::rename(&selected, &retained).unwrap();
        fs::create_dir_all(selected.join("media/nested")).unwrap();
        fs::write(selected.join(relative), b"project-b").unwrap();

        let mut asset = root.open_asset_file(relative).unwrap();
        let mut bytes = Vec::new();
        asset.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"project-a");
        assert!(!root.is_current_namespace().unwrap());
    }

    #[test]
    fn project_asset_open_rejects_non_relative_components() {
        let tmp = TmpDir::new("asset-invalid-relative");
        let bundle = tmp.path().join("Selected.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();

        assert!(root.open_asset_file(Path::new("../outside.mp4")).is_err());
        assert!(root.open_asset_file(tmp.path()).is_err());
        assert!(root.open_asset_file(Path::new(".")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn project_asset_open_rejects_symlinked_directories_and_leaves() {
        use std::os::unix::fs::symlink;

        let tmp = TmpDir::new("asset-symlinks");
        let bundle = tmp.path().join("Selected.opentake");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&bundle).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.mp4"), b"secret").unwrap();
        symlink(&outside, bundle.join("linked-dir")).unwrap();
        symlink(outside.join("secret.mp4"), bundle.join("linked-file.mp4")).unwrap();
        let root = ProjectRoot::open(&bundle).unwrap();

        assert!(root
            .open_asset_file(Path::new("linked-dir/secret.mp4"))
            .is_err());
        assert!(root.open_asset_file(Path::new("linked-file.mp4")).is_err());
    }

    #[test]
    fn project_component_reads_enforce_each_configured_byte_limit() {
        let tmp = TmpDir::new("component-read-limits");
        let bundle = tmp.path().join("Bounded.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        let cases = [
            (crate::layout::TIMELINE_FILE, TIMELINE_COMPONENT_MAX_BYTES),
            (crate::layout::MANIFEST_FILE, MANIFEST_COMPONENT_MAX_BYTES),
            (
                crate::layout::GENERATION_LOG_FILE,
                GENERATION_LOG_COMPONENT_MAX_BYTES,
            ),
            (crate::layout::THUMBNAIL_FILE, THUMBNAIL_COMPONENT_MAX_BYTES),
            (PUBLISH_MARKER_FILE, PUBLISH_MARKER_MAX_BYTES),
        ];

        for (name, max_bytes) in cases {
            let file = fs::File::create(bundle.join(name)).unwrap();
            file.set_len(max_bytes as u64 + 1).unwrap();

            let error = match root.read_optional(name) {
                Err(error) => error,
                Ok(_) => panic!("metadata above the configured limit was accepted for {name}"),
            };
            assert!(error.to_string().contains(name), "{error}");
            assert!(error.to_string().contains("byte limit"), "{error}");
        }
    }

    #[test]
    fn project_component_read_accepts_the_exact_marker_limit() {
        let tmp = TmpDir::new("component-read-boundary");
        let bundle = tmp.path().join("Boundary.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        let marker = vec![b'a'; PUBLISH_MARKER_MAX_BYTES];
        fs::write(bundle.join(PUBLISH_MARKER_FILE), &marker).unwrap();

        assert_eq!(
            root.read_optional(PUBLISH_MARKER_FILE).unwrap(),
            Some(marker)
        );
    }

    #[test]
    fn bounded_stream_read_rejects_growth_after_the_metadata_boundary() {
        let path = Path::new("growing-project-component");
        let mut exact = std::io::Cursor::new(b"1234".to_vec());
        assert_eq!(
            read_bounded_contents(&mut exact, path, 4, 4, "project component").unwrap(),
            b"1234"
        );

        let mut grown = std::io::Cursor::new(b"12345".to_vec());
        let error = read_bounded_contents(&mut grown, path, 4, 4, "project component")
            .expect_err("MAX+1 must identify a file that grew after metadata inspection");
        assert!(error.to_string().contains("grew beyond"), "{error}");
    }

    #[test]
    fn transaction_journal_reads_enforce_the_exact_and_over_limit_boundaries() {
        let tmp = TmpDir::new("journal-read-boundary");
        let parent = Dir::open_ambient_dir(tmp.path(), ambient_authority()).unwrap();
        let name = OsStr::new(".Bounded.opentake.opentake-journal");
        let path = tmp.path().join(name);
        let bytes = vec![b'a'; TRANSACTION_JOURNAL_MAX_BYTES];
        fs::write(&path, &bytes).unwrap();
        assert_eq!(
            read_file_artifact(&parent, tmp.path(), name).unwrap(),
            bytes
        );

        fs::File::create(&path)
            .unwrap()
            .set_len(TRANSACTION_JOURNAL_MAX_BYTES as u64 + 1)
            .unwrap();
        let error = read_file_artifact(&parent, tmp.path(), name)
            .expect_err("an oversized journal must be rejected before allocation");
        assert!(error.to_string().contains("byte limit"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn transaction_journal_read_rejects_a_fifo_without_blocking() {
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;

        let tmp = TmpDir::new("journal-fifo");
        let name = OsString::from(".Blocked.opentake.opentake-journal");
        let fifo = tmp.path().join(&name);
        assert!(Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        let parent_path = tmp.path().to_path_buf();
        let parent = Dir::open_ambient_dir(&parent_path, ambient_authority()).unwrap();
        let (sent, received) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            sent.send(read_file_artifact(&parent, &parent_path, &name))
                .unwrap();
        });
        let result = match received.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => result,
            Err(_) => {
                let _writer = fs::OpenOptions::new().write(true).open(&fifo).unwrap();
                let _ = received.recv_timeout(Duration::from_secs(1));
                reader.join().unwrap();
                panic!("transaction journal FIFO open blocked instead of failing closed");
            }
        };
        reader.join().unwrap();
        assert!(result.is_err(), "a FIFO must never be parsed as a journal");
    }

    #[test]
    fn failed_atomic_replace_removes_the_capability_relative_temp_leaf() {
        let tmp = TmpDir::new("failed-replace-cleanup");
        let bundle = tmp.path().join("Cleanup.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        fs::create_dir(bundle.join(crate::layout::MANIFEST_FILE)).unwrap();

        root.write_atomic(crate::layout::MANIFEST_FILE, b"not published")
            .expect_err("a regular file must not replace a directory");

        let prefix = format!(".{}.", crate::layout::MANIFEST_FILE);
        let leaked = fs::read_dir(&bundle)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
        assert!(!leaked, "failed transaction leaked its temporary leaf");
    }

    #[test]
    fn chat_session_reads_are_bounded_and_listing_rejects_directories() {
        let tmp = TmpDir::new("chat-session-bounds");
        let bundle = tmp.path().join("Chat.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        root.write_chat_session_atomic("bounded.json", b"12345")
            .unwrap();

        let error = root
            .read_chat_session("bounded.json", 4)
            .expect_err("metadata larger than the caller's limit must be rejected");
        assert!(error.to_string().contains("byte limit"), "{error}");

        fs::create_dir(bundle.join("chat-sessions/not-a-session")).unwrap();
        let error = root
            .list_chat_session_files(16)
            .expect_err("non-regular directory entries must fail closed");
        assert!(error.to_string().contains("non-regular"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn direct_chat_session_read_rejects_a_fifo_without_blocking() {
        use std::process::Command;
        use std::sync::mpsc;
        use std::time::Duration;

        let tmp = TmpDir::new("chat-session-fifo");
        let bundle = tmp.path().join("Chat.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        fs::create_dir(bundle.join("chat-sessions")).unwrap();
        let fifo = bundle.join("chat-sessions/blocked.json");
        assert!(Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());

        let (sent, received) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            sent.send(root.read_chat_session("blocked.json", 1024))
                .unwrap();
        });
        let result = match received.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => result,
            Err(_) => {
                // Unblock the buggy blocking-open path before failing so this
                // regression never strands a test worker.
                let _writer = fs::OpenOptions::new().write(true).open(&fifo).unwrap();
                let _ = received.recv_timeout(Duration::from_secs(1));
                reader.join().unwrap();
                panic!("direct FIFO read blocked before it could fail closed");
            }
        };
        reader.join().unwrap();
        assert!(result.is_err(), "a FIFO must never be parsed as chat JSON");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_preserves_a_temp_handle_renamed_away_from_its_original_leaf() {
        let tmp = TmpDir::new("renamed-temp-cleanup");
        let bundle = tmp.path().join("Cleanup.opentake");
        let root = ProjectRoot::create(&bundle).unwrap();
        let temp_name = OsString::from(".manifest.transaction.tmp");
        let safe_name = OsString::from("recovered-manifest.json");
        let mut transaction = TransactionLeaf::create(&root.dir, &temp_name).unwrap();
        transaction
            .handle
            .as_file_mut()
            .write_all(b"recovered data")
            .unwrap();
        transaction.handle.as_file().sync_all().unwrap();
        root.dir.rename(&temp_name, &root.dir, &safe_name).unwrap();
        fs::write(bundle.join(&temp_name), b"replacement data").unwrap();

        drop(transaction);

        assert_eq!(fs::read(bundle.join(safe_name)).unwrap(), b"recovered data");
        assert_eq!(
            fs::read(bundle.join(temp_name)).unwrap(),
            b"replacement data"
        );
    }

    #[cfg(unix)]
    #[test]
    fn media_copy_uses_retained_source_and_destination_roots_after_rebinding() {
        let tmp = TmpDir::new("copy-rebinding");
        let source_parent = tmp.path().join("sources");
        let source_retained = tmp.path().join("sources-retained");
        let destination_parent = tmp.path().join("destinations");
        let destination_retained = tmp.path().join("destinations-retained");
        let source_bundle = source_parent.join("Source.opentake");
        let destination_bundle = destination_parent.join("Destination.opentake");
        fs::create_dir_all(source_bundle.join(crate::layout::MEDIA_DIR)).unwrap();
        fs::write(
            source_bundle
                .join(crate::layout::MEDIA_DIR)
                .join("clip.bin"),
            b"retained-source",
        )
        .unwrap();
        fs::create_dir_all(&destination_bundle).unwrap();
        let source = ProjectRoot::open(&source_bundle).unwrap();
        let destination = ProjectRoot::open(&destination_bundle).unwrap();

        fs::rename(&source_parent, &source_retained).unwrap();
        fs::create_dir_all(source_bundle.join(crate::layout::MEDIA_DIR)).unwrap();
        fs::write(
            source_bundle
                .join(crate::layout::MEDIA_DIR)
                .join("clip.bin"),
            b"replacement-source",
        )
        .unwrap();
        fs::rename(&destination_parent, &destination_retained).unwrap();
        fs::create_dir_all(&destination_bundle).unwrap();

        source.copy_media_to(&destination).unwrap();

        assert_eq!(
            fs::read(
                destination_retained
                    .join("Destination.opentake")
                    .join(crate::layout::MEDIA_DIR)
                    .join("clip.bin")
            )
            .unwrap(),
            b"retained-source"
        );
        assert!(
            !destination_bundle.join(crate::layout::MEDIA_DIR).exists(),
            "ambient replacement destination must remain untouched"
        );
    }

    fn tree_receipt(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn visit(base: &Path, path: &Path, receipt: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let mut entries = fs::read_dir(path)
                .unwrap()
                .map(std::result::Result::unwrap)
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(base).unwrap().to_path_buf();
                if entry.file_type().unwrap().is_dir() {
                    receipt.push((relative, None));
                    visit(base, &path, receipt);
                } else {
                    receipt.push((relative, Some(fs::read(path).unwrap())));
                }
            }
        }

        let mut receipt = Vec::new();
        visit(root, root, &mut receipt);
        receipt
    }

    fn stage_paths(parent: &Path, target_name: &str) -> Vec<PathBuf> {
        let prefix = format!(".{target_name}.opentake-stage");
        let mut paths = fs::read_dir(parent)
            .unwrap()
            .map(std::result::Result::unwrap)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    #[derive(Clone, Copy)]
    enum AbortCleanupWindow {
        BeforeStageRemoval,
        BeforeJournalRemoval,
    }

    impl AbortCleanupWindow {
        fn tag(self) -> &'static str {
            match self {
                Self::BeforeStageRemoval => "before-stage",
                Self::BeforeJournalRemoval => "before-journal",
            }
        }

        fn inject(self) {
            match self {
                Self::BeforeStageRemoval => {
                    FAIL_ABORT_CLEANUP_BEFORE_STAGE_REMOVAL.with(|fail| fail.set(true));
                }
                Self::BeforeJournalRemoval => {
                    FAIL_ABORT_CLEANUP_BEFORE_JOURNAL_REMOVAL.with(|fail| fail.set(true));
                }
            }
        }

        fn stage_remains(self) -> bool {
            matches!(self, Self::BeforeStageRemoval)
        }
    }

    fn leave_backed_up_transaction(target: &Path) {
        let mut interrupted = ProjectRoot::begin_replace(target).unwrap();
        interrupted
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        interrupted
            .parent
            .rename(
                &interrupted.target_name,
                &interrupted.parent,
                &interrupted.backup_name,
            )
            .unwrap();
        interrupted.journal.phase = PublishPhase::BackedUp;
        write_file_artifact_atomic(
            &interrupted.parent,
            &interrupted.parent_path,
            &interrupted.journal_name,
            &interrupted.journal.encode(),
        )
        .unwrap();
        drop(interrupted.stage.take().unwrap());
        drop(interrupted);
    }

    fn create_recovery_stage(parent: &Path, name: &OsStr, marker: Option<&str>) -> PathBuf {
        let path = parent.join(name);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("remaining-child.bin"), b"preserve me").unwrap();
        if let Some(marker) = marker {
            fs::write(path.join(PUBLISH_MARKER_FILE), marker).unwrap();
        }
        path
    }

    fn write_recovery_journal(parent: &Path, target_name: &OsStr, journal: &PublishJournal) {
        fs::write(
            parent.join(artifact_name(target_name, ".opentake-journal")),
            journal.encode(),
        )
        .unwrap();
    }

    fn assert_recovery_refused_without_mutation(target: &Path) {
        let parent = target.parent().unwrap();
        fs::write(
            parent.join(artifact_name(target.file_name().unwrap(), ".opentake-lock")),
            b"",
        )
        .unwrap();
        let before = tree_receipt(parent);
        match ProjectRoot::begin_replace(target) {
            Ok(_) => panic!("invalid recovery evidence must remain fail closed"),
            Err(error) => assert!(!error.to_string().is_empty()),
        }
        assert_eq!(tree_receipt(parent), before);
    }

    #[test]
    fn publish_failure_after_backup_restores_the_old_target_tree() {
        let tmp = TmpDir::new("publish-restore");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(target.join("media/nested")).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        fs::write(target.join("media/nested/clip.bin"), b"old media").unwrap();
        let before = tree_receipt(&target);

        let mut publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        let error = publisher
            .publish_with_hook(|| Err(std::io::Error::other("injected publish failure")))
            .expect_err("the injected post-backup failure must abort publication");

        assert!(error.to_string().contains("injected publish failure"));
        assert_eq!(tree_receipt(&target), before);
        assert!(!tmp
            .path()
            .join(".Existing.opentake.opentake-backup")
            .exists());
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
        assert!(!tmp
            .path()
            .join(".Existing.opentake.opentake-journal")
            .exists());
    }

    #[test]
    fn publish_releases_the_transaction_lock_before_returning() {
        let tmp = TmpDir::new("publish-lock-handoff");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"initial timeline").unwrap();
        let lock_path = tmp.path().join(".Existing.opentake.opentake-lock");

        for generation in 0..32 {
            let publisher = ProjectRoot::begin_replace(&target).unwrap();
            let expected = format!("timeline generation {generation}");
            publisher
                .stage()
                .write_atomic("project.json", expected.as_bytes())
                .unwrap();
            let root = publisher.publish().unwrap();

            let lock = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&lock_path)
                .unwrap();
            lock.try_lock()
                .expect("publish must hand off its transaction lock before returning");
            lock.unlock().unwrap();
            assert_eq!(
                root.read_optional("project.json").unwrap().unwrap(),
                expected.as_bytes()
            );
        }
    }

    #[test]
    fn postcommit_backup_cleanup_failure_returns_success_and_recovers_on_retry() {
        let tmp = TmpDir::new("postcommit-cleanup");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        let backup = tmp.path().join(".Existing.opentake.opentake-backup");
        let journal = tmp.path().join(".Existing.opentake.opentake-journal");

        let publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        FAIL_BACKUP_CLEANUP.with(|fail| fail.set(true));
        let root = publisher
            .publish()
            .expect("cleanup after the commit is best effort");
        assert_eq!(
            root.read_optional("project.json").unwrap().unwrap(),
            b"new timeline"
        );
        assert!(backup.is_dir());
        assert!(journal.is_file());
        assert!(target.join(PUBLISH_MARKER_FILE).is_file());
        drop(root);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("the next locked transaction reconciles committed cleanup artifacts");
        assert!(!backup.exists());
        assert!(!target.join(PUBLISH_MARKER_FILE).exists());
        drop(retry);
        assert!(!journal.exists());
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"new timeline"
        );
    }

    #[test]
    fn postcommit_journal_cleanup_failure_recovers_without_the_backup() {
        let tmp = TmpDir::new("postcommit-journal-cleanup");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        let backup = tmp.path().join(".Existing.opentake.opentake-backup");
        let journal = tmp.path().join(".Existing.opentake.opentake-journal");

        let publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        FAIL_JOURNAL_CLEANUP_AFTER_BACKUP.with(|fail| fail.set(true));
        let root = publisher
            .publish()
            .expect("post-commit journal cleanup remains best effort");
        assert_eq!(
            root.read_optional("project.json").unwrap().unwrap(),
            b"new timeline"
        );
        assert!(!backup.exists());
        assert!(journal.is_file());
        assert!(target.join(PUBLISH_MARKER_FILE).is_file());
        drop(root);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("the committed marker and journal reconcile without a backup");
        assert!(!target.join(PUBLISH_MARKER_FILE).exists());
        drop(retry);
        assert!(!journal.exists());
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"new timeline"
        );
    }

    #[test]
    fn public_save_to_normalizes_a_prior_marker_before_partial_abort_recovery() {
        let tmp = TmpDir::new("two-generation-stale-marker");
        let target = tmp.path().join("Existing.opentake");
        let journal = tmp.path().join(".Existing.opentake.opentake-journal");
        let backup = tmp.path().join(".Existing.opentake.opentake-backup");
        let mut generation_a = crate::Project::new(tmp.path().join("GenerationA.opentake"));
        generation_a.timeline.fps = 24;
        FAIL_COMMITTED_MARKER_CLEANUP.with(|fail| fail.set(true));
        generation_a.save_to(&target).unwrap();
        assert!(target.join(PUBLISH_MARKER_FILE).is_file());
        let expected_timeline = fs::read(target.join(crate::layout::TIMELINE_FILE)).unwrap();
        let expected_manifest = fs::read(target.join(crate::layout::MANIFEST_FILE)).unwrap();

        let mut generation_b = crate::Project::new(tmp.path().join("GenerationB.opentake"));
        generation_b.timeline.fps = 60;
        FAIL_PUBLISH_AFTER_BACKUP.with(|fail| fail.set(true));
        FAIL_STAGE_CLEANUP_AFTER_MARKER_REMOVAL.with(|fail| fail.set(true));
        generation_b
            .save_to(&target)
            .expect_err("generation B must fail after restoring generation A");

        assert!(!target.join(PUBLISH_MARKER_FILE).exists());
        assert_eq!(
            fs::read(target.join(crate::layout::TIMELINE_FILE)).unwrap(),
            expected_timeline
        );
        assert_eq!(
            fs::read(target.join(crate::layout::MANIFEST_FILE)).unwrap(),
            expected_manifest
        );
        assert!(journal.is_file());
        assert!(!backup.exists());
        assert_eq!(stage_paths(tmp.path(), "Existing.opentake").len(), 1);

        generation_a
            .save_to(&target)
            .expect("the public retry must recover B and republish generation A");
        assert_eq!(crate::Project::open(&target).unwrap().timeline.fps, 24);
        assert!(!journal.exists());
        assert!(!backup.exists());
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
        assert!(!target.join(PUBLISH_MARKER_FILE).exists());
    }

    #[test]
    fn public_save_to_creates_no_transaction_when_idle_marker_cleanup_fails() {
        let tmp = TmpDir::new("stale-marker-cleanup-failure");
        let target = tmp.path().join("Existing.opentake");
        let mut generation_a = crate::Project::new(tmp.path().join("GenerationA.opentake"));
        generation_a.timeline.fps = 24;
        FAIL_COMMITTED_MARKER_CLEANUP.with(|fail| fail.set(true));
        generation_a.save_to(&target).unwrap();
        assert!(target.join(PUBLISH_MARKER_FILE).is_file());
        let before = tree_receipt(tmp.path());

        let mut generation_b = crate::Project::new(tmp.path().join("GenerationB.opentake"));
        generation_b.timeline.fps = 60;
        FAIL_IDLE_MARKER_CLEANUP.with(|fail| fail.set(true));
        let error = generation_b
            .save_to(&target)
            .expect_err("generation B must not start without removing the idle marker");

        assert!(error
            .to_string()
            .contains("injected idle publish marker cleanup failure"));
        assert_eq!(tree_receipt(tmp.path()), before);
        assert!(!tmp
            .path()
            .join(".Existing.opentake.opentake-journal")
            .exists());
        assert!(!tmp
            .path()
            .join(".Existing.opentake.opentake-backup")
            .exists());
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
    }

    #[test]
    fn retry_removes_a_nonce_named_stage_created_before_its_marker() {
        let tmp = TmpDir::new("premarker-stage");
        let target = tmp.path().join("New.opentake");
        let target_name = target.file_name().unwrap();
        let journal_name = artifact_name(target_name, ".opentake-journal");
        let journal = PublishJournal::new(None);
        let stage_name = stage_artifact_name(target_name, &journal.nonce);
        let parent = Dir::open_ambient_dir(tmp.path(), ambient_authority()).unwrap();
        write_new_file_artifact(&parent, tmp.path(), &journal_name, &journal.encode()).unwrap();
        parent.create_dir(&stage_name).unwrap();
        let interrupted_stage = tmp.path().join(&stage_name);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("the nonce-derived name proves ownership before the marker write");
        assert!(!interrupted_stage.exists());
        drop(retry);
        assert!(stage_paths(tmp.path(), "New.opentake").is_empty());
        assert!(!tmp.path().join(journal_name).exists());
    }

    #[test]
    fn retry_removes_a_journal_created_before_its_stage() {
        let tmp = TmpDir::new("prestage-journal");
        let target = tmp.path().join("New.opentake");
        let target_name = target.file_name().unwrap();
        let journal_name = artifact_name(target_name, ".opentake-journal");
        let journal = PublishJournal::new(None);
        let parent = Dir::open_ambient_dir(tmp.path(), ambient_authority()).unwrap();
        write_new_file_artifact(&parent, tmp.path(), &journal_name, &journal.encode()).unwrap();

        let retry = ProjectRoot::begin_replace(&target)
            .expect("a prepared journal without its not-yet-created stage is safe to discard");
        drop(retry);
        assert!(stage_paths(tmp.path(), "New.opentake").is_empty());
        assert!(!tmp.path().join(journal_name).exists());
    }

    #[test]
    fn drop_preserves_the_journal_when_stage_cleanup_is_partial() {
        for marker_removed_before_drop in [false, true] {
            let tmp = TmpDir::new(if marker_removed_before_drop {
                "drop-partial-stage-unmarked"
            } else {
                "drop-partial-stage-marked"
            });
            let target = tmp.path().join("New.opentake");
            let publisher = ProjectRoot::begin_replace(&target).unwrap();
            publisher
                .stage()
                .write_atomic("remaining-child.bin", b"staged data")
                .unwrap();
            let stage_path = tmp.path().join(&publisher.stage_name);
            let journal_path = tmp.path().join(&publisher.journal_name);
            if marker_removed_before_drop {
                publisher
                    .stage()
                    .dir
                    .remove_file(PUBLISH_MARKER_FILE)
                    .unwrap();
            }
            FAIL_STAGE_CLEANUP_AFTER_MARKER_REMOVAL.with(|fail| fail.set(true));

            drop(publisher);

            assert!(stage_path.join("remaining-child.bin").is_file());
            assert!(!stage_path.join(PUBLISH_MARKER_FILE).exists());
            assert!(journal_path.is_file());
            let retry = ProjectRoot::begin_replace(&target)
                .expect("the retained journal lets retry clean its partially removed stage");
            assert!(!stage_path.exists());
            drop(retry);
            assert!(!journal_path.exists());
            assert!(stage_paths(tmp.path(), "New.opentake").is_empty());
        }
    }

    #[test]
    fn restored_abort_recovers_after_marker_only_partial_stage_cleanup() {
        let tmp = TmpDir::new("restored-abort-partial-stage");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        let before = tree_receipt(&target);
        let journal_path = tmp.path().join(".Existing.opentake.opentake-journal");

        let mut publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("remaining-child.bin", b"staged data")
            .unwrap();
        let stage_path = tmp.path().join(&publisher.stage_name);
        FAIL_STAGE_CLEANUP_AFTER_MARKER_REMOVAL.with(|fail| fail.set(true));
        let error = publisher
            .publish_with_hook(|| Err(std::io::Error::other("injected publish failure")))
            .expect_err("partial recursive stage cleanup must preserve recovery state");
        assert!(matches!(error, ProjectError::RecoveryRequired { .. }));
        assert_eq!(
            PublishJournal::decode(&fs::read(&journal_path).unwrap())
                .unwrap()
                .phase,
            PublishPhase::AbortedRestored
        );
        assert!(stage_path.join("remaining-child.bin").is_file());
        assert!(!stage_path.join(PUBLISH_MARKER_FILE).exists());
        drop(publisher);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("the exact nonce stage and restored phase prove partial cleanup ownership");
        assert!(!stage_path.exists());
        drop(retry);
        assert_eq!(tree_receipt(&target), before);
        assert!(!journal_path.exists());
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
    }

    #[test]
    fn prepared_recovery_refuses_a_stage_with_an_unknown_transaction_nonce() {
        let tmp = TmpDir::new("prepared-unknown-stage-nonce");
        let target = tmp.path().join("New.opentake");
        let target_name = target.file_name().unwrap();
        let journal_name = artifact_name(target_name, ".opentake-journal");
        let journal = PublishJournal::new(None);
        let unknown_stage = stage_artifact_name(target_name, "dead-beef-0");
        let parent = Dir::open_ambient_dir(tmp.path(), ambient_authority()).unwrap();
        write_new_file_artifact(&parent, tmp.path(), &journal_name, &journal.encode()).unwrap();
        parent.create_dir(&unknown_stage).unwrap();

        let error = match ProjectRoot::begin_replace(&target) {
            Ok(_) => panic!("a stage from another transaction must remain fail closed"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("unknown or multiple stage"));
        assert!(tmp.path().join(unknown_stage).is_dir());
        assert!(tmp.path().join(journal_name).is_file());
    }

    #[test]
    fn restored_abort_refuses_a_stage_with_an_unknown_transaction_nonce() {
        let tmp = TmpDir::new("unknown-stage-nonce");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        let target_name = target.file_name().unwrap();
        let journal_name = artifact_name(target_name, ".opentake-journal");
        let mut journal =
            PublishJournal::new(Some(ProjectRoot::open(&target).unwrap().stable_identity()));
        journal.phase = PublishPhase::AbortedRestored;
        let unknown_stage = stage_artifact_name(target_name, "dead-beef-0");
        let parent = Dir::open_ambient_dir(tmp.path(), ambient_authority()).unwrap();
        write_new_file_artifact(&parent, tmp.path(), &journal_name, &journal.encode()).unwrap();
        parent.create_dir(&unknown_stage).unwrap();

        let error = match ProjectRoot::begin_replace(&target) {
            Ok(_) => panic!("a stage from another transaction must remain fail closed"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("unknown or multiple stage"));
        assert!(tmp.path().join(unknown_stage).is_dir());
        assert!(tmp.path().join(journal_name).is_file());
    }

    #[test]
    fn legacy_marked_stage_remains_recoverable() {
        let tmp = TmpDir::new("legacy-marked-stage");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();
        let target_name = target.file_name().unwrap();
        let journal =
            PublishJournal::new(Some(ProjectRoot::open(&target).unwrap().stable_identity()));
        let legacy_stage = artifact_name(target_name, ".opentake-stage");
        let legacy_path = create_recovery_stage(tmp.path(), &legacy_stage, Some(&journal.nonce));
        write_recovery_journal(tmp.path(), target_name, &journal);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("a marked legacy stage retains its journal ownership evidence");
        assert!(!legacy_path.exists());
        drop(retry);
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"old timeline"
        );
        assert!(!tmp
            .path()
            .join(artifact_name(target_name, ".opentake-journal"))
            .exists());
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
    }

    #[test]
    fn ambiguous_stage_evidence_is_refused_and_preserved() {
        enum Case {
            LegacyUnmarked,
            ExactWrongMarker,
            ExactPlusLegacy,
        }

        for (tag, case) in [
            ("legacy-unmarked", Case::LegacyUnmarked),
            ("exact-wrong-marker", Case::ExactWrongMarker),
            ("exact-plus-legacy", Case::ExactPlusLegacy),
        ] {
            let tmp = TmpDir::new(tag);
            let target = tmp.path().join("Existing.opentake");
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("project.json"), b"old timeline").unwrap();
            let target_name = target.file_name().unwrap();
            let journal =
                PublishJournal::new(Some(ProjectRoot::open(&target).unwrap().stable_identity()));
            let exact_stage = stage_artifact_name(target_name, &journal.nonce);
            let legacy_stage = artifact_name(target_name, ".opentake-stage");
            match case {
                Case::LegacyUnmarked => {
                    create_recovery_stage(tmp.path(), &legacy_stage, None);
                }
                Case::ExactWrongMarker => {
                    create_recovery_stage(tmp.path(), &exact_stage, Some("wrong-marker"));
                }
                Case::ExactPlusLegacy => {
                    create_recovery_stage(tmp.path(), &exact_stage, Some(&journal.nonce));
                    create_recovery_stage(tmp.path(), &legacy_stage, Some(&journal.nonce));
                }
            }
            write_recovery_journal(tmp.path(), target_name, &journal);

            assert_recovery_refused_without_mutation(&target);
        }
    }

    #[test]
    fn invalid_or_foreign_restored_abort_states_are_refused_and_preserved() {
        #[derive(Clone, Copy)]
        enum Case {
            NoPriorTarget,
            MissingTarget,
            ForeignTargetMarker,
            TransactionMarkedTarget,
            BackupReappeared,
        }

        for (tag, case) in [
            ("abort-no-prior-target", Case::NoPriorTarget),
            ("abort-missing-target", Case::MissingTarget),
            ("abort-foreign-target-marker", Case::ForeignTargetMarker),
            (
                "abort-transaction-marked-target",
                Case::TransactionMarkedTarget,
            ),
            ("abort-backup-reappeared", Case::BackupReappeared),
        ] {
            let tmp = TmpDir::new(tag);
            let target = tmp.path().join("Existing.opentake");
            let target_name = target.file_name().unwrap();
            let mut journal = PublishJournal::new(if matches!(case, Case::NoPriorTarget) {
                None
            } else {
                Some(ProjectRootIdentity {
                    volume: u64::MAX,
                    file: u64::MAX - 1,
                })
            });
            journal.phase = PublishPhase::AbortedRestored;
            let stage_name = stage_artifact_name(target_name, &journal.nonce);
            create_recovery_stage(tmp.path(), &stage_name, Some(&journal.nonce));
            match case {
                Case::NoPriorTarget | Case::MissingTarget => {}
                // Normal transactions clear prior-generation markers in begin;
                // a different nonce here is therefore foreign/corrupt evidence.
                Case::ForeignTargetMarker => {
                    fs::create_dir_all(&target).unwrap();
                    fs::write(target.join(PUBLISH_MARKER_FILE), b"wrong-marker").unwrap();
                }
                Case::TransactionMarkedTarget => {
                    fs::create_dir_all(&target).unwrap();
                    fs::write(target.join(PUBLISH_MARKER_FILE), journal.nonce.as_bytes()).unwrap();
                }
                Case::BackupReappeared => {
                    fs::create_dir_all(&target).unwrap();
                    fs::create_dir_all(
                        tmp.path()
                            .join(artifact_name(target_name, ".opentake-backup")),
                    )
                    .unwrap();
                }
            }
            if target.is_dir() {
                fs::write(target.join("project.json"), b"preserve target").unwrap();
            }
            write_recovery_journal(tmp.path(), target_name, &journal);

            assert_recovery_refused_without_mutation(&target);
        }
    }

    #[test]
    fn direct_abort_recovers_across_both_post_restore_cleanup_windows() {
        for window in [
            AbortCleanupWindow::BeforeStageRemoval,
            AbortCleanupWindow::BeforeJournalRemoval,
        ] {
            let tmp = TmpDir::new(&format!("direct-abort-{}", window.tag()));
            let target = tmp.path().join("Existing.opentake");
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("project.json"), b"old timeline").unwrap();
            let journal = tmp.path().join(".Existing.opentake.opentake-journal");

            let mut publisher = ProjectRoot::begin_replace(&target).unwrap();
            publisher
                .stage()
                .write_atomic("project.json", b"new timeline")
                .unwrap();
            window.inject();
            let error = publisher
                .publish_with_hook(|| Err(std::io::Error::other("injected publish failure")))
                .expect_err("the injected abort cleanup interruption must be reported");
            assert!(matches!(error, ProjectError::RecoveryRequired { .. }));
            assert_eq!(
                PublishJournal::decode(&fs::read(&journal).unwrap())
                    .unwrap()
                    .phase,
                PublishPhase::AbortedRestored
            );
            assert_eq!(
                !stage_paths(tmp.path(), "Existing.opentake").is_empty(),
                window.stage_remains()
            );
            drop(publisher);

            let retry = ProjectRoot::begin_replace(&target)
                .expect("the next transaction finishes direct-abort cleanup");
            drop(retry);
            assert_eq!(
                fs::read(target.join("project.json")).unwrap(),
                b"old timeline"
            );
            assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
            assert!(!journal.exists());
        }
    }

    #[test]
    fn restart_recovery_survives_both_post_restore_cleanup_windows() {
        for window in [
            AbortCleanupWindow::BeforeStageRemoval,
            AbortCleanupWindow::BeforeJournalRemoval,
        ] {
            let tmp = TmpDir::new(&format!("restart-recovery-{}", window.tag()));
            let target = tmp.path().join("Existing.opentake");
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("project.json"), b"old timeline").unwrap();
            let journal = tmp.path().join(".Existing.opentake.opentake-journal");
            let backup = tmp.path().join(".Existing.opentake.opentake-backup");
            leave_backed_up_transaction(&target);

            window.inject();
            let error = match ProjectRoot::begin_replace(&target) {
                Ok(_) => panic!("the injected restart cleanup interruption must be reported"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("injected abort cleanup failure"));
            assert!(!backup.exists());
            assert_eq!(
                PublishJournal::decode(&fs::read(&journal).unwrap())
                    .unwrap()
                    .phase,
                PublishPhase::AbortedRestored
            );
            assert_eq!(
                !stage_paths(tmp.path(), "Existing.opentake").is_empty(),
                window.stage_remains()
            );

            let retry = ProjectRoot::begin_replace(&target)
                .expect("the next transaction finishes restart recovery cleanup");
            drop(retry);
            assert_eq!(
                fs::read(target.join("project.json")).unwrap(),
                b"old timeline"
            );
            assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
            assert!(!journal.exists());
        }
    }

    #[test]
    fn retry_restores_backup_after_an_interrupted_publish() {
        let tmp = TmpDir::new("interrupted-restore");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();

        let mut interrupted = ProjectRoot::begin_replace(&target).unwrap();
        interrupted
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        interrupted
            .parent
            .rename(
                &interrupted.target_name,
                &interrupted.parent,
                &interrupted.backup_name,
            )
            .unwrap();
        interrupted.journal.phase = PublishPhase::BackedUp;
        write_file_artifact_atomic(
            &interrupted.parent,
            &interrupted.parent_path,
            &interrupted.journal_name,
            &interrupted.journal.encode(),
        )
        .unwrap();
        drop(interrupted);
        assert!(!target.exists());

        let retry = ProjectRoot::begin_replace(&target)
            .expect("a locked retry restores the authoritative backup");
        drop(retry);
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"old timeline"
        );
    }

    #[test]
    fn recovery_refuses_an_unmarked_target_and_preserves_the_backup() {
        let tmp = TmpDir::new("ambiguous-recovery");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();

        let mut interrupted = ProjectRoot::begin_replace(&target).unwrap();
        interrupted
            .parent
            .rename(
                &interrupted.target_name,
                &interrupted.parent,
                &interrupted.backup_name,
            )
            .unwrap();
        interrupted.journal.phase = PublishPhase::BackedUp;
        write_file_artifact_atomic(
            &interrupted.parent,
            &interrupted.parent_path,
            &interrupted.journal_name,
            &interrupted.journal.encode(),
        )
        .unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"foreign target").unwrap();
        drop(interrupted);

        let error = match ProjectRoot::begin_replace(&target) {
            Ok(_) => panic!("an unmarked target plus backup is recovery ambiguity"),
            Err(error) => error,
        };
        assert!(matches!(error, ProjectError::RecoveryRequired { .. }));
        assert!(tmp
            .path()
            .join(".Existing.opentake.opentake-backup")
            .is_dir());
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"foreign target"
        );
    }

    #[test]
    fn recovery_refuses_a_prior_target_transaction_with_only_its_stage_left() {
        let tmp = TmpDir::new("missing-target-and-backup");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();

        let mut interrupted = ProjectRoot::begin_replace(&target).unwrap();
        drop(interrupted.stage.take().unwrap());
        fs::remove_dir_all(&target).unwrap();
        drop(interrupted);

        let error = match ProjectRoot::begin_replace(&target) {
            Ok(_) => panic!("a prior target cannot disappear without a retained backup"),
            Err(error) => error,
        };
        assert!(matches!(error, ProjectError::RecoveryRequired { .. }));
        assert_eq!(stage_paths(tmp.path(), "Existing.opentake").len(), 1);
        assert!(tmp
            .path()
            .join(".Existing.opentake.opentake-journal")
            .is_file());
    }

    #[test]
    fn recovery_aborts_a_restored_stage_when_the_prior_target_is_still_visible() {
        let tmp = TmpDir::new("restored-stage-abort");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"old timeline").unwrap();

        let mut interrupted = ProjectRoot::begin_replace(&target).unwrap();
        interrupted
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        interrupted.journal.phase = PublishPhase::BackedUp;
        write_file_artifact_atomic(
            &interrupted.parent,
            &interrupted.parent_path,
            &interrupted.journal_name,
            &interrupted.journal.encode(),
        )
        .unwrap();
        drop(interrupted.stage.take().unwrap());
        drop(interrupted);

        let retry = ProjectRoot::begin_replace(&target)
            .expect("visible restored target makes its staged sibling safe to abort");
        drop(retry);
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"old timeline"
        );
        assert!(stage_paths(tmp.path(), "Existing.opentake").is_empty());
        assert!(!tmp
            .path()
            .join(".Existing.opentake.opentake-journal")
            .exists());
    }

    #[test]
    fn bundle_transaction_lock_fails_fast_for_a_second_writer() {
        let tmp = TmpDir::new("transaction-lock");
        let target = tmp.path().join("Locked.opentake");
        let first = ProjectRoot::begin_replace(&target).unwrap();

        let error = match ProjectRoot::begin_replace(&target) {
            Ok(_) => panic!("the persistent target lock must reject a concurrent writer"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("lock"), "{error}");
        drop(first);
    }

    #[test]
    fn publish_refuses_a_different_target_that_rebinds_after_begin() {
        let tmp = TmpDir::new("publish-target-rebound-after-begin");
        let target = tmp.path().join("Existing.opentake");
        let original = tmp.path().join("Original.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"original timeline").unwrap();

        let publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"staged timeline")
            .unwrap();
        fs::rename(&target, &original).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"replacement timeline").unwrap();

        let error = publisher
            .publish()
            .expect_err("a rebound target must fail closed before publication");

        assert!(error.to_string().contains("identity"), "{error}");
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"replacement timeline"
        );
        assert_eq!(
            fs::read(original.join("project.json")).unwrap(),
            b"original timeline"
        );
    }

    #[test]
    fn publish_never_deletes_a_foreign_bundle_rebound_at_backup_name() {
        let tmp = TmpDir::new("publish-backup-rebound-before-cleanup");
        let target = tmp.path().join("Existing.opentake");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"original timeline").unwrap();

        let mut publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"new timeline")
            .unwrap();
        let backup = tmp.path().join(&publisher.backup_name);
        let preserved_original = tmp.path().join("preserved-original.opentake");

        let result = publisher.publish_with_hook(|| {
            fs::rename(&backup, &preserved_original)?;
            fs::create_dir_all(&backup)?;
            fs::write(backup.join("project.json"), b"foreign replacement")?;
            Ok(())
        });

        assert!(result.is_err(), "backup identity mismatch must fail closed");
        assert_eq!(
            fs::read(backup.join("project.json")).unwrap(),
            b"foreign replacement",
            "a foreign object rebound at the backup name must survive"
        );
        assert_eq!(
            fs::read(preserved_original.join("project.json")).unwrap(),
            b"original timeline"
        );
    }

    #[test]
    fn first_save_refuses_a_target_that_appears_after_staging() {
        let tmp = TmpDir::new("first-save-target-appeared");
        let target = tmp.path().join("Appeared.opentake");
        let publisher = ProjectRoot::begin_replace(&target).unwrap();
        publisher
            .stage()
            .write_atomic("project.json", b"staged timeline")
            .unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("project.json"), b"foreign timeline").unwrap();

        let error = publisher
            .publish()
            .expect_err("a newly appeared target must never be backed up and replaced");

        assert!(error.to_string().contains("existence changed"), "{error}");
        assert_eq!(
            fs::read(target.join("project.json")).unwrap(),
            b"foreign timeline"
        );
    }
}
