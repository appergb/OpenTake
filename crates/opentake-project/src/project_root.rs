use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use cap_fs_ext::{ambient_authority, DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use same_file::Handle;

use crate::error::{ProjectError, Result};

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
        let identity = Handle::from_file(
            dir.try_clone()
                .map_err(|error| ProjectError::io(path, error))?
                .into_std_file(),
        )
        .map_err(|error| ProjectError::io(path, error))?;
        Ok(Self {
            path: path.to_path_buf(),
            parent,
            name,
            dir,
            identity,
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn matches_identity(&self, other: &Handle) -> bool {
        &self.identity == other
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
        let source = match self.dir.open_dir_nofollow(crate::layout::MEDIA_DIR) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(ProjectError::io(
                    self.path.join(crate::layout::MEDIA_DIR),
                    error,
                ))
            }
        };
        let staging_name = unique_directory_name("media-copy");
        destination
            .dir
            .create_dir(&staging_name)
            .map_err(|error| ProjectError::io(destination.path.join(&staging_name), error))?;
        let staging = destination
            .dir
            .open_dir_nofollow(&staging_name)
            .map_err(|error| ProjectError::io(destination.path.join(&staging_name), error))?;
        if let Err(error) = copy_directory(&source, &staging) {
            let _ = destination.dir.remove_dir_all(&staging_name);
            return Err(ProjectError::io(
                destination.path.join(crate::layout::MEDIA_DIR),
                error,
            ));
        }
        match destination.dir.symlink_metadata(crate::layout::MEDIA_DIR) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                destination
                    .dir
                    .remove_dir_all(crate::layout::MEDIA_DIR)
                    .map_err(|error| {
                        ProjectError::io(destination.path.join(crate::layout::MEDIA_DIR), error)
                    })?;
            }
            Ok(_) => {
                let _ = destination.dir.remove_dir_all(&staging_name);
                return Err(ProjectError::io(
                    destination.path.join(crate::layout::MEDIA_DIR),
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "destination media is not a nofollow directory",
                    ),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                let _ = destination.dir.remove_dir_all(&staging_name);
                return Err(ProjectError::io(
                    destination.path.join(crate::layout::MEDIA_DIR),
                    error,
                ));
            }
        }
        destination
            .dir
            .rename(&staging_name, &destination.dir, crate::layout::MEDIA_DIR)
            .map_err(|error| {
                ProjectError::io(destination.path.join(crate::layout::MEDIA_DIR), error)
            })?;
        Ok(())
    }

    pub(crate) fn read_optional(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let mut file = match self.dir.open_with(name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(ProjectError::io(self.path.join(name), error)),
        };
        if !file
            .metadata()
            .map_err(|error| ProjectError::io(self.path.join(name), error))?
            .is_file()
        {
            return Err(ProjectError::io(
                self.path.join(name),
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "project component is not a nofollow regular file",
                ),
            ));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| ProjectError::io(self.path.join(name), error))?;
        Ok(Some(bytes))
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
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfo, SetFileInformationByHandle, FILE_RENAME_INFO, FILE_RENAME_INFO_0,
    };

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
    let info_size = std::mem::offset_of!(FILE_RENAME_INFO, FileName)
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rename allocation overflow",
            )
        })?
        .max(std::mem::size_of::<FILE_RENAME_INFO>());
    let word_size = std::mem::size_of::<usize>();
    let mut storage = vec![0_usize; info_size.div_ceil(word_size)];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    let info_size = u32::try_from(info_size).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "rename buffer too large")
    })?;
    // SAFETY: storage is aligned and sized for the header plus UTF-16 target;
    // both retained handles remain open throughout the synchronous call.
    let renamed = unsafe {
        (*info).Anonymous = FILE_RENAME_INFO_0 {
            ReplaceIfExists: true,
        };
        (*info).RootDirectory = root.as_raw_handle();
        (*info).FileNameLength = file_name_bytes;
        std::ptr::copy_nonoverlapping(
            wide.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            wide.len(),
        );
        SetFileInformationByHandle(
            leaf.handle.as_file().as_raw_handle(),
            FileRenameInfo,
            info.cast(),
            info_size,
        )
    };
    if renamed == 0 {
        return Err(std::io::Error::last_os_error());
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
}
