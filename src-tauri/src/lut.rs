//! Secure project-managed 3D LUT import and shared runtime resolution.

use std::io::Read;
use std::path::Path;
use std::rc::Rc;

use cap_fs_ext::{ambient_authority, FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, OpenOptions};
use opentake_core::AppCore;
use opentake_domain::{CubeLut, LutReference};
use opentake_project::ProjectRoot;
use opentake_render::gpu::texture::upload_lut_3d;
use opentake_render::{GpuLutTexture, RenderError};
use sha2::{Digest, Sha256};
use tauri::State;

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_source(path: &Path) -> Result<Vec<u8>, String> {
    if !path.is_absolute() {
        return Err("LUT source path must be absolute".to_string());
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some("cube")
    {
        return Err("LUT source must have a .cube extension".to_string());
    }
    let parent_path = path
        .parent()
        .ok_or_else(|| "LUT source has no parent".to_string())?;
    let name = path
        .file_name()
        .ok_or_else(|| "LUT source has no filename".to_string())?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
        .map_err(|error| format!("open LUT source directory: {error}"))?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK);
    let mut file = parent
        .open_with(name, &options)
        .map_err(|error| format!("open LUT source without following links: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("inspect LUT source: {error}"))?;
    if !metadata.is_file() {
        return Err("LUT source must be a regular file".to_string());
    }
    if metadata.len() > CubeLut::MAX_BYTES as u64 {
        return Err(format!("LUT source exceeds {} bytes", CubeLut::MAX_BYTES));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    Read::by_ref(&mut file)
        .take(CubeLut::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read LUT source: {error}"))?;
    if bytes.len() > CubeLut::MAX_BYTES {
        return Err(format!("LUT source exceeds {} bytes", CubeLut::MAX_BYTES));
    }
    Ok(bytes)
}

/// Validate an untrusted `.cube`, publish it by content hash inside the active
/// bundle, and return the path-free authored reference for a later `SetLut`.
#[tauri::command]
pub fn import_lut(core: State<'_, AppCore>, path: String) -> Result<LutReference, String> {
    import_lut_impl(&core, &path)
}

fn import_lut_impl(core: &AppCore, path: &str) -> Result<LutReference, String> {
    let source_path = Path::new(path);
    let bytes = read_source(source_path)?;
    CubeLut::parse(&bytes).map_err(|error| format!("invalid LUT: {error}"))?;
    let id = sha256_hex(&bytes);
    let display_name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| {
            value
                .chars()
                .filter(|character| !character.is_control())
                .take(128)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Imported LUT".to_string());
    let reference = LutReference::new(id, display_name, 1.0).map_err(|error| error.to_string())?;

    let _workflow = core.lock_project_identity_workflow();
    let snapshot = core.runtime_snapshot();
    let project_dir = snapshot
        .project_dir
        .ok_or_else(|| "save the project before importing a LUT".to_string())?;
    core.mutable_runtime_snapshot_for_project(snapshot.project_epoch, &project_dir)
        .map_err(|error| error.to_string())?;
    let root = ProjectRoot::open(&project_dir).map_err(|error| error.to_string())?;
    core.ensure_project_root_identity_for_project(
        snapshot.project_epoch,
        &project_dir,
        root.identity(),
    )
    .map_err(|error| error.to_string())?;
    root.write_lut_atomic(&format!("{}.cube", reference.id), &bytes)
        .map_err(|error| error.to_string())?;
    Ok(reference)
}

/// Read, hash-check, parse and upload one managed asset for preview/export/
/// playback. A tampered asset is a render error, never an identity fallback.
pub(crate) fn resolve_project_lut(
    root: Option<&ProjectRoot>,
    reference: &LutReference,
    device: &opentake_render::wgpu::Device,
    queue: &opentake_render::wgpu::Queue,
    label: &str,
) -> Result<Option<Rc<GpuLutTexture>>, RenderError> {
    reference.validate()?;
    let Some(root) = root else {
        return Ok(None);
    };
    let filename = format!("{}.cube", reference.id);
    let bytes = root
        .read_lut(&filename, CubeLut::MAX_BYTES)
        .map_err(|error| RenderError::InvalidLut(error.to_string()))?
        .ok_or_else(|| RenderError::MissingLut(reference.id.clone()))?;
    if sha256_hex(&bytes) != reference.id {
        return Err(RenderError::InvalidLut(format!(
            "managed LUT {} failed its content hash",
            reference.id
        )));
    }
    let lut = CubeLut::parse(&bytes).map_err(|error| RenderError::InvalidLut(error.to_string()))?;
    Ok(Some(Rc::new(upload_lut_3d(
        device,
        queue,
        &lut,
        Some(label),
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_reader_rejects_relative_paths_before_io() {
        assert_eq!(
            read_source(Path::new("relative.cube")).unwrap_err(),
            "LUT source path must be absolute"
        );
    }

    #[test]
    fn import_validates_then_copies_by_hash_into_the_active_bundle() {
        let temp = tempfile::tempdir().unwrap();
        let bundle = temp.path().join("LutImport.opentake");
        let source = temp.path().join("Known Transform.cube");
        let mut bytes = b"LUT_3D_SIZE 17\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\n".to_vec();
        for _ in 0..17_usize.pow(3) {
            bytes.extend_from_slice(b"0 0 0\n");
        }
        std::fs::write(&source, &bytes).unwrap();

        let core = AppCore::new();
        core.new_project();
        core.save_project(Some(bundle.clone())).unwrap();
        let reference = import_lut_impl(&core, source.to_str().unwrap()).unwrap();
        assert_eq!(reference.name, "Known Transform");
        assert_eq!(reference.id, sha256_hex(&bytes));
        let root = ProjectRoot::open(bundle).unwrap();
        assert_eq!(
            root.read_lut(&format!("{}.cube", reference.id), CubeLut::MAX_BYTES)
                .unwrap(),
            Some(bytes)
        );
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_a_symlink_source() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target.cube");
        let link = temp.path().join("link.cube");
        std::fs::write(&target, b"LUT_3D_SIZE 17\n").unwrap();
        symlink(target, &link).unwrap();
        let error = read_source(&link).unwrap_err();
        assert!(error.contains("without following links"));
    }
}
