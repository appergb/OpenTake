use super::capability::*;
use super::component::ComponentName;
use super::error::Result;
use super::platform;
use std::path::Path;

pub(crate) fn capture_absolute_directory(
    path: &Path,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    platform::capture_absolute_directory(path, access)
}
pub(crate) fn revalidate_namespace(directory: &DirectoryAuthority) -> Result<()> {
    platform::revalidate_namespace(directory)
}
pub(crate) fn query_child_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<ChildState> {
    platform::query_child_nofollow(parent, name)
}
pub(crate) fn open_dir_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    platform::open_dir_nofollow(parent, name, access)
}
pub(crate) fn open_file_nofollow(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    access: FileAccess,
) -> Result<FileCapability> {
    platform::open_file_nofollow(parent, name, access)
}
pub(crate) fn create_dir_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
    access: DirectoryAccess,
) -> Result<DirectoryAuthority> {
    platform::create_dir_new(parent, name, permissions, access)
}
pub(crate) fn create_stage_dir_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
) -> Result<StageCapability> {
    platform::create_stage_dir_new(parent, name, permissions)
}
pub(crate) fn create_file_new(
    parent: &DirectoryAuthority,
    name: &ComponentName,
    permissions: CreatePermissions,
) -> Result<FileCapability> {
    platform::create_file_new(parent, name, permissions)
}
// Name enumeration validates and returns every child component without following it or
// granting authority. Callers must query metadata or open an explicit nofollow capability.
pub(crate) fn enumerate(directory: &DirectoryAuthority) -> Result<Vec<ComponentName>> {
    platform::enumerate(directory)
}
pub(crate) fn read_link_component(
    parent: &DirectoryAuthority,
    name: &ComponentName,
) -> Result<RawLinkTarget> {
    platform::read_link_component(parent, name)
}
pub(crate) fn quarantine_stage(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    quarantine_name: ComponentName,
) -> Result<QuarantinedCapability> {
    platform::quarantine_stage(stage, parent, quarantine_name)
}
pub(crate) fn publish_stage_noreplace(
    stage: StageCapability,
    parent: &DirectoryAuthority,
    destination: ComponentName,
) -> Result<()> {
    platform::publish_stage_noreplace(stage, parent, destination)
}
pub(super) fn open_cleanup_child_nofollow(
    parent: &QuarantinedCapability,
    name: &ComponentName,
) -> Result<CleanupCapability> {
    platform::open_cleanup_child_nofollow(parent, name)
}
pub(super) fn delete_quarantined_entry(entry: CleanupCapability) -> Result<()> {
    platform::delete_quarantined_entry(entry)
}
pub(crate) fn delete_quarantined_empty_directory(directory: QuarantinedCapability) -> Result<()> {
    platform::delete_quarantined_empty_directory(directory)
}

pub(crate) fn cleanup_quarantined_tree(root: QuarantinedCapability) -> Result<()> {
    let names = enumerate(root.directory())?;
    for name in names {
        match open_cleanup_child_nofollow(&root, &name)? {
            CleanupCapability::Directory(child) => cleanup_quarantined_tree(*child)?,
            entry @ CleanupCapability::Entry(_) => delete_quarantined_entry(entry)?,
        }
    }
    delete_quarantined_empty_directory(root)
}
