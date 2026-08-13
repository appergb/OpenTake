//! Project-local low-resolution proxy creation.
//!
//! The source is retained once and hashed before and after transcoding. FFmpeg
//! reads that same capability and writes to a sibling partial file, which is
//! published without clobbering only after a successful retained-file probe.

use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use same_file::Handle as FileIdentity;
use sha2::{Digest, Sha256};

use crate::cancel::MediaCancelToken;
use crate::error::{MediaError, Result};
use crate::{ff, probe};

pub type ProxyProgressCallback = Arc<dyn Fn(usize, usize) + Send + Sync>;

const PROXY_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug)]
pub struct ProxyRequest<'a> {
    pub source: &'a Path,
    pub output: &'a Path,
    pub max_size: (u32, u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyResult {
    pub path: PathBuf,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
}

fn report(progress: &Option<ProxyProgressCallback>, done: usize) {
    if let Some(callback) = progress {
        callback(done, 1000);
    }
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let file = open_retained_regular_file(path)?;
    file_sha256_file_cancellable(&file, &MediaCancelToken::new())
}

pub fn file_sha256_file_cancellable(file: &File, cancel: &MediaCancelToken) -> Result<String> {
    let mut reader = BufReader::new(file.try_clone()?);
    reader.seek(SeekFrom::Start(0))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancel.checkpoint() {
            return Err(MediaError::Cancelled);
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
fn partial_path(output: &Path) -> PathBuf {
    match output.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => output.with_extension(format!("{extension}.partial")),
        None => output.with_extension("partial"),
    }
}

fn open_retained_regular_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
            FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        options
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_NO_RECALL | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            path.display().to_string(),
        ));
    }
    Ok(file)
}

struct PrivateStage {
    identity: FileIdentity,
    path: tempfile::TempPath,
    directory: tempfile::TempDir,
}

impl PrivateStage {
    fn create(output: &Path) -> Result<Self> {
        let parent = output.parent().unwrap_or_else(|| Path::new("."));
        let mut builder = tempfile::Builder::new();
        builder.prefix(".opentake-proxy-stage-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o700));
        }
        let directory = builder.tempdir_in(parent)?;
        let path = directory.path().join("proxy.mp4");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };
            options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
        }
        let file = options.open(&path)?;
        Ok(Self {
            identity: FileIdentity::from_file(file)?,
            path: tempfile::TempPath::try_from_path(path)?,
            directory,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn file(&self) -> &File {
        self.identity.as_file()
    }

    fn verify_path_identity(&self) -> Result<()> {
        let current = FileIdentity::from_file(open_retained_regular_file(self.path())?)?;
        if current != self.identity {
            return Err(MediaError::Checksum(
                "proxy staging identity changed during transcode".to_string(),
            ));
        }
        Ok(())
    }

    fn persist_noclobber(self, output: &Path) -> Result<()> {
        let Self {
            identity,
            path,
            directory,
        } = self;
        let result = match path.persist_noclobber(output) {
            Ok(()) => Ok(()),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Err(
                MediaError::Ffmpeg("proxy destination appeared during transcode".to_string()),
            ),
            Err(error) => Err(error.error.into()),
        };
        drop(identity);
        drop(directory);
        result
    }
}

pub fn create_proxy(
    request: ProxyRequest<'_>,
    cancel: &MediaCancelToken,
    progress: Option<ProxyProgressCallback>,
) -> Result<ProxyResult> {
    report(&progress, 0);
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }
    if request.max_size.0 == 0 || request.max_size.1 == 0 {
        return Err(MediaError::Ffmpeg(
            "proxy dimensions must be positive".to_string(),
        ));
    }
    let source = open_retained_regular_file(request.source)?;
    if request.output.exists() {
        return Err(MediaError::Ffmpeg(
            "proxy destination already exists".to_string(),
        ));
    }

    if let Some(parent) = request.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let stage = PrivateStage::create(request.output)?;

    let source_sha256 = file_sha256_file_cancellable(&source, cancel)?;
    report(&progress, 100);
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }

    let mut ffmpeg_source = source.try_clone()?;
    ffmpeg_source.seek(SeekFrom::Start(0))?;

    let scale = format!(
        "scale=w={}:h={}:force_original_aspect_ratio=decrease:force_divisible_by=2",
        request.max_size.0, request.max_size.1
    );
    let mut child = Command::new(ff::ffmpeg_path())
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"])
        .arg("-i")
        .arg("fd:")
        .args(["-map", "0:v:0", "-map", "0:a?", "-vf"])
        .arg(scale)
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ])
        .arg(stage.path())
        .stdin(Stdio::from(ffmpeg_source))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| MediaError::Ffmpeg(format!("proxy spawn: {error}")))?;
    cancel.child_spawned();

    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MediaError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(MediaError::Ffmpeg(
                        "proxy transcode did not complete".to_string(),
                    ));
                }
                break;
            }
            Ok(None) => {
                report(&progress, 500);
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    }

    stage.verify_path_identity()?;

    report(&progress, 900);
    if cancel.is_cancelled() {
        return Err(MediaError::Cancelled);
    }
    report(&progress, 910);
    let verified_source_sha256 = file_sha256_file_cancellable(&source, cancel)?;
    if verified_source_sha256 != source_sha256 {
        return Err(MediaError::Checksum(
            "source changed while proxy was being created".to_string(),
        ));
    }

    report(&progress, 950);
    let metadata = match probe::probe_file_cancellable(stage.file(), cancel, PROXY_PROBE_TIMEOUT) {
        Ok(metadata) if metadata.has_video => metadata,
        Ok(_) => {
            return Err(MediaError::no_track("video", stage.path()));
        }
        Err(error) => return Err(error),
    };
    let (width, height) = match (metadata.width, metadata.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => (width, height),
        _ => {
            return Err(MediaError::Decode(
                "proxy has no usable dimensions".to_string(),
            ));
        }
    };
    if request.output.exists() {
        return Err(MediaError::Ffmpeg(
            "proxy destination appeared during transcode".to_string(),
        ));
    }
    report(&progress, 990);
    if cancel.checkpoint() {
        return Err(MediaError::Cancelled);
    }
    stage.persist_noclobber(request.output)?;
    report(&progress, 1000);

    Ok(ProxyResult {
        path: request.output.to_path_buf(),
        source_sha256,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    fn assert_proxy_tools_available() {
        assert!(ff::ffmpeg_available(), "ffmpeg is required for proxy tests");
        assert!(
            ff::ffprobe_available(),
            "ffprobe is required for proxy tests"
        );
    }

    fn make_video(path: &Path, duration_seconds: u32, size: &str) {
        let video_source = format!("testsrc2=size={size}:rate=30");
        let duration = duration_seconds.to_string();
        let status = Command::new(ff::ffmpeg_path())
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &video_source,
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
                "-t",
                &duration,
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(path)
            .status()
            .expect("spawn ffmpeg fixture");
        assert!(status.success(), "ffmpeg fixture creation failed");
    }

    fn stream_count(path: &Path, selector: &str) -> usize {
        let output = Command::new(ff::ffprobe_path())
            .args([
                "-v",
                "error",
                "-select_streams",
                selector,
                "-show_entries",
                "stream=index",
                "-of",
                "csv=p=0",
            ])
            .arg(path)
            .output()
            .expect("run ffprobe stream count");
        assert!(output.status.success(), "ffprobe stream count failed");
        String::from_utf8(output.stdout)
            .expect("ffprobe stream count is UTF-8")
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    }

    #[test]
    fn successful_proxy_is_bounded_probed_and_source_preserving() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        let source_before = fs::read(&source).expect("read source fixture");
        let source_sha256 = file_sha256(&source).expect("hash source fixture");
        let progress_values = Arc::new(Mutex::new(Vec::new()));
        let captured_progress = Arc::clone(&progress_values);
        let progress: ProxyProgressCallback = Arc::new(move |done, total| {
            captured_progress
                .lock()
                .expect("capture proxy progress")
                .push((done, total));
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 240),
            },
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("create proxy");

        assert_eq!(result.path, output);
        assert_eq!(result.source_sha256, source_sha256);
        assert_eq!((result.width, result.height), (320, 180));
        assert_eq!(fs::read(&source).expect("reread source"), source_before);
        let metadata = probe(&output).expect("probe completed proxy");
        assert!(metadata.has_video && metadata.has_audio);
        assert_eq!((metadata.width, metadata.height), (Some(320), Some(180)));
        assert_eq!(stream_count(&output, "v"), 1);
        assert!(stream_count(&output, "a") <= 1);
        let progress = progress_values.lock().expect("read proxy progress");
        assert_eq!(progress.first().copied(), Some((0, 1000)));
        assert!(progress.contains(&(100, 1000)));
        assert!(progress.contains(&(900, 1000)));
        assert_eq!(progress.last().copied(), Some((1000, 1000)));
        assert!(!partial_path(&output).exists());
    }

    #[test]
    fn cancellation_during_transcode_kills_child_and_cleans_partial() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/cancelled.mp4");
        make_video(&source, 8, "1920x1080");
        let cancel = MediaCancelToken::new();
        let cancel_from_progress = cancel.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 500 {
                cancel_from_progress.cancel();
            }
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 180),
            },
            &cancel,
            Some(progress),
        );

        assert!(matches!(result, Err(MediaError::Cancelled)));
        assert_eq!(cancel.spawned_child_count(), 1);
        assert!(!output.exists());
        assert!(!partial_path(&output).exists());
    }

    #[test]
    fn changed_source_fails_identity_check_and_cleans_partial() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        let source_to_change = source.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 900 {
                use std::io::Write;
                let mut source = fs::OpenOptions::new()
                    .append(true)
                    .open(&source_to_change)
                    .expect("open source for identity change");
                source
                    .write_all(b"changed-after-transcode")
                    .expect("change source identity");
            }
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 180),
            },
            &MediaCancelToken::new(),
            Some(progress),
        );

        assert!(matches!(result, Err(MediaError::Checksum(_))));
        assert!(!output.exists());
        assert!(!partial_path(&output).exists());
    }

    #[test]
    fn pathname_replacement_a_to_b_to_a_cannot_change_the_retained_source() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let replacement = temp.path().join("replacement.mp4");
        let parked_source = temp.path().join("retained-source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        make_video(&replacement, 1, "360x640");
        let source_for_callback = source.clone();
        let replacement_for_callback = replacement.clone();
        let parked_for_callback = parked_source.clone();
        let replaced = Arc::new(AtomicBool::new(false));
        let restored = Arc::new(AtomicBool::new(false));
        let callback_replaced = Arc::clone(&replaced);
        let callback_restored = Arc::clone(&restored);
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 100 && !callback_replaced.swap(true, Ordering::AcqRel) {
                fs::rename(&source_for_callback, &parked_for_callback)
                    .expect("park original source pathname");
                fs::rename(&replacement_for_callback, &source_for_callback)
                    .expect("replace source pathname with B");
            }
            if done == 900 && !callback_restored.swap(true, Ordering::AcqRel) {
                fs::rename(&source_for_callback, &replacement_for_callback)
                    .expect("restore replacement pathname");
                fs::rename(&parked_for_callback, &source_for_callback)
                    .expect("restore original source pathname A");
            }
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 320),
            },
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("create proxy from retained source A");

        assert!(replaced.load(Ordering::Acquire));
        assert!(restored.load(Ordering::Acquire));
        assert_eq!((result.width, result.height), (320, 180));
        let metadata = probe(&output).expect("probe retained-source proxy");
        assert_eq!((metadata.width, metadata.height), (Some(320), Some(180)));
    }

    #[test]
    fn cancellation_at_verification_probe_and_prepublication_cleans_partial() {
        assert_proxy_tools_available();

        for cancellation_progress in [910, 950, 990] {
            let temp = tempfile::tempdir().expect("proxy test directory");
            let source = temp.path().join("source.mp4");
            let output = temp.path().join("media/proxies/source-proxy.mp4");
            make_video(&source, 1, "640x360");
            let cancel = MediaCancelToken::new();
            let cancel_from_progress = cancel.clone();
            let progress: ProxyProgressCallback = Arc::new(move |done, _| {
                if done == cancellation_progress {
                    cancel_from_progress.cancel();
                }
            });

            let result = create_proxy(
                ProxyRequest {
                    source: &source,
                    output: &output,
                    max_size: (320, 180),
                },
                &cancel,
                Some(progress),
            );

            assert!(
                matches!(result, Err(MediaError::Cancelled)),
                "phase {cancellation_progress} did not return Cancelled"
            );
            assert!(!output.exists());
            assert!(!partial_path(&output).exists());
        }
    }

    #[test]
    fn destination_appearing_after_final_check_is_preserved_and_partial_is_cleaned() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        let output_to_claim = output.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 990 {
                fs::write(&output_to_claim, b"competing-writer").expect("claim proxy destination");
            }
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 180),
            },
            &MediaCancelToken::new(),
            Some(progress),
        );

        assert!(matches!(result, Err(MediaError::Ffmpeg(_))));
        assert_eq!(
            fs::read(&output).expect("read competing output"),
            b"competing-writer"
        );
        assert!(!partial_path(&output).exists());
    }

    #[test]
    fn verified_partial_rebound_at_prepublication_cannot_change_output_or_delete_replacement() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let replacement = temp.path().join("replacement.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        make_video(&replacement, 1, "360x640");
        let partial = partial_path(&output);
        let replacement_bytes = fs::read(&replacement).expect("read replacement fixture");
        let rebound = Arc::new(AtomicBool::new(false));
        let rebound_from_callback = Arc::clone(&rebound);
        let partial_for_callback = partial.clone();
        let replacement_for_callback = replacement.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 990 && !rebound_from_callback.swap(true, Ordering::AcqRel) {
                fs::copy(&replacement_for_callback, &partial_for_callback)
                    .expect("introduce an attacker-controlled ambient partial");
            }
        });

        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 320),
            },
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("publish the retained verified artifact");

        assert!(rebound.load(Ordering::Acquire));
        assert_eq!((result.width, result.height), (320, 180));
        let published = probe(&output).expect("probe published retained artifact");
        assert_eq!((published.width, published.height), (Some(320), Some(180)));
        assert_eq!(
            fs::read(&partial).expect("attacker replacement must remain untouched"),
            replacement_bytes
        );
    }

    #[test]
    fn ffmpeg_output_is_never_exposed_at_the_ambient_partial_path() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        let public_partial = partial_path(&output);
        let observed_public_partial = Arc::new(AtomicBool::new(false));
        let observed_from_callback = Arc::clone(&observed_public_partial);
        let partial_from_callback = public_partial.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 900 && partial_from_callback.exists() {
                observed_from_callback.store(true, Ordering::Release);
            }
        });

        create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 180),
            },
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("create proxy entirely through private staging");

        assert!(output.is_file());
        assert!(
            !observed_public_partial.load(Ordering::Acquire),
            "FFmpeg output was exposed in the ambient output directory"
        );
        assert!(!public_partial.exists());
    }

    #[cfg(unix)]
    #[test]
    fn rebound_fifo_does_not_block_or_get_deleted_on_publication_error() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::FileTypeExt;
        use std::time::Instant;

        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");
        let partial = partial_path(&output);
        let rebound = Arc::new(AtomicBool::new(false));
        let rebound_from_callback = Arc::clone(&rebound);
        let partial_for_callback = partial.clone();
        let output_for_callback = output.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 990 && !rebound_from_callback.swap(true, Ordering::AcqRel) {
                let fifo = CString::new(partial_for_callback.as_os_str().as_bytes())
                    .expect("FIFO path has no NUL");
                assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
                fs::write(&output_for_callback, b"competing-writer")
                    .expect("claim proxy destination");
            }
        });

        let started = Instant::now();
        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 320),
            },
            &MediaCancelToken::new(),
            Some(progress),
        );

        assert!(matches!(result, Err(MediaError::Ffmpeg(_))));
        assert!(rebound.load(Ordering::Acquire));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cleanup followed or blocked on the rebound FIFO"
        );
        assert_eq!(
            fs::read(&output).expect("read competing output"),
            b"competing-writer"
        );
        assert!(fs::symlink_metadata(&partial)
            .expect("rebound FIFO must remain")
            .file_type()
            .is_fifo());
    }

    #[cfg(unix)]
    #[test]
    fn initial_source_fifo_and_symlink_are_rejected_without_blocking() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::symlink;
        use std::time::Instant;

        let temp = tempfile::tempdir().expect("proxy test directory");
        let regular = temp.path().join("regular.mp4");
        let fifo = temp.path().join("source.fifo");
        let link = temp.path().join("source-link.mp4");
        fs::write(&regular, b"not opened through aliases").expect("write regular fixture");
        symlink(&regular, &link).expect("create source symlink");
        let fifo_name = CString::new(fifo.as_os_str().as_bytes()).expect("FIFO path has no NUL");
        assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);

        for source in [&fifo, &link] {
            let output = temp.path().join(format!(
                "{}.proxy.mp4",
                source.file_name().expect("source leaf").to_string_lossy()
            ));
            let started = Instant::now();
            let result = create_proxy(
                ProxyRequest {
                    source,
                    output: &output,
                    max_size: (320, 180),
                },
                &MediaCancelToken::new(),
                None,
            );

            assert!(matches!(result, Err(MediaError::Io(_))));
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "initial source open followed or blocked on {}",
                source.display()
            );
            assert!(!output.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn unlinked_source_namespace_does_not_break_the_retained_source() {
        assert_proxy_tools_available();

        let temp = tempfile::tempdir().expect("proxy test directory");
        let source = temp.path().join("source.mp4");
        let output = temp.path().join("media/proxies/source-proxy.mp4");
        make_video(&source, 1, "640x360");

        let source_to_remove = source.clone();
        let progress: ProxyProgressCallback = Arc::new(move |done, _| {
            if done == 900 {
                fs::remove_file(&source_to_remove).expect("remove source before identity recheck");
            }
        });
        let result = create_proxy(
            ProxyRequest {
                source: &source,
                output: &output,
                max_size: (320, 180),
            },
            &MediaCancelToken::new(),
            Some(progress),
        )
        .expect("retained source remains readable after unlink");

        assert_eq!((result.width, result.height), (320, 180));
        assert!(!source.exists());
        assert!(output.is_file());
        assert!(!partial_path(&output).exists());
    }
}
