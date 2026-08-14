use super::*;

use std::fs;
use std::path::{Path, PathBuf};

use opentake_core::AppCore;
use serde_json::Value;
use tempfile::TempDir;

fn saved_core(name: &str) -> (TempDir, AppCore, PathBuf) {
    let temp = tempfile::tempdir().expect("create temp project parent");
    let path = temp.path().join(format!("{name}.opentake"));
    let core = AppCore::new();
    core.save_project(Some(path.clone()))
        .expect("save fixture project");
    (temp, core, path)
}

fn create_document(store: &MotionDocumentStore) -> MotionDocument {
    store
        .create(MotionDocumentCreateRequest {
            title: Some("片头标题".to_string()),
        })
        .expect("create motion document")
}

fn current_directory(project: &Path, document_id: &str) -> PathBuf {
    let catalog: Value = serde_json::from_slice(
        &fs::read(project.join(MOTION_DOCUMENTS_DIR).join(CATALOG_FILE))
            .expect("read motion catalog"),
    )
    .expect("decode motion catalog");
    let directory = catalog["documents"][document_id]["directory"]
        .as_str()
        .expect("catalog directory");
    project.join(MOTION_DOCUMENTS_DIR).join(directory)
}

fn replace_html(document: &MotionDocument, html: &str) -> MotionDocumentPatchRequest {
    MotionDocumentPatchRequest {
        document_id: document.summary.id.clone(),
        file: "index.html".to_string(),
        baseline_hash: document.summary.revision_hash.clone(),
        edits: vec![MotionTextReplacement {
            start: 0,
            end: document.html.len(),
            replacement: html.to_string(),
        }],
        expected_result_hash: revision_hash(html, &document.css, &document.parameters)
            .expect("hash replacement"),
    }
}

#[test]
fn creates_visible_bilingual_template_and_lists_it() {
    let (_temp, core, _project) = saved_core("template");
    let store = MotionDocumentStore::new(core);
    let document = create_document(&store);

    assert!(document.html.contains("让创意动起来"));
    assert!(document.html.contains("Motion Studio"));
    assert!(document.css.contains("@keyframes"));
    assert_eq!(document.summary.title, "片头标题");
    assert_eq!(
        store.list().expect("list documents"),
        vec![document.summary]
    );
}

#[test]
fn persists_across_a_fresh_core_and_store() {
    let (_temp, core, project) = saved_core("restart");
    let store = MotionDocumentStore::new(core);
    let document = create_document(&store);
    drop(store);

    let reopened_core = AppCore::new();
    reopened_core
        .open_project(project)
        .expect("reopen fixture project");
    let reopened = MotionDocumentStore::new(reopened_core)
        .read(&document.summary.id)
        .expect("read persisted document");
    assert_eq!(reopened, document);
}

#[test]
fn survives_complete_project_save_as_and_reopen() {
    let (temp, core, _project) = saved_core("save-as-source");
    let store = MotionDocumentStore::new(core.clone());
    let document = create_document(&store);
    let destination = temp.path().join("Save As.opentake");

    core.save_project(Some(destination.clone()))
        .expect("save project under a new name");
    drop(store);

    let reopened_core = AppCore::new();
    reopened_core
        .open_project(destination)
        .expect("reopen Save As destination");
    assert_eq!(
        MotionDocumentStore::new(reopened_core)
            .read(&document.summary.id)
            .expect("read document after Save As"),
        document
    );
}

#[test]
fn rejects_stale_hash_and_preserves_the_winner() {
    let (_temp, core, _project) = saved_core("stale");
    let store = MotionDocumentStore::new(core);
    let original = create_document(&store);
    let request = replace_html(&original, "<main>first writer</main>");
    let winner = store.save_patch(request.clone()).expect("first patch wins");

    let error = store
        .save_patch(request)
        .expect_err("stale baseline must fail");
    assert!(error.contains("revision conflict"), "{error}");
    assert_eq!(
        store.read(&original.summary.id).expect("read winner"),
        winner
    );
}

#[test]
fn normalizes_crlf_and_lone_cr_before_hashing_and_persistence() {
    let (_temp, core, _project) = saved_core("line-endings");
    let store = MotionDocumentStore::new(core);
    let original = create_document(&store);
    let normalized = "<main>first\nsecond\nthird</main>";
    let mut request = replace_html(&original, "<main>first\r\nsecond\rthird</main>");
    request.expected_result_hash = revision_hash(normalized, &original.css, &original.parameters)
        .expect("hash normalized replacement");

    let saved = store
        .save_patch(request)
        .expect("normalized replacement must save");

    assert_eq!(saved.html, normalized);
    assert!(!saved.html.contains('\r'));
    assert_eq!(store.read(&saved.summary.id).unwrap(), saved);
}

#[test]
fn patch_ranges_are_utf8_byte_offsets_and_must_land_on_character_boundaries() {
    let source = "<h1>让创意动起来</h1>";
    let start = source.find('让').expect("Chinese text starts");
    let end = start + "让创意动起来".len();
    let patched = apply_replacements(
        source,
        vec![MotionTextReplacement {
            start,
            end,
            replacement: "Motion Studio".into(),
        }],
        MAX_SOURCE_BYTES,
    )
    .expect("UTF-8 byte range patches cleanly");
    assert_eq!(patched, "<h1>Motion Studio</h1>");

    let error = apply_replacements(
        source,
        vec![MotionTextReplacement {
            start: start + 1,
            end,
            replacement: "invalid".into(),
        }],
        MAX_SOURCE_BYTES,
    )
    .expect_err("mid-codepoint byte offset must fail");
    assert!(error.contains("range is invalid"), "{error}");
}

#[test]
fn rejects_absolute_traversal_and_overlapping_edits() {
    let (_temp, core, _project) = saved_core("paths");
    let store = MotionDocumentStore::new(core);
    let original = create_document(&store);

    for file in ["../styles.css", "/tmp/index.html", "nested/index.html"] {
        let mut request = replace_html(&original, "safe");
        request.file = file.to_string();
        let error = store
            .save_patch(request)
            .expect_err("unsafe file must fail");
        assert!(error.contains("editable file"), "{file}: {error}");
    }
    for id in ["../escape", "/absolute", "not-a-uuid"] {
        let error = store.read(id).expect_err("unsafe id must fail");
        assert!(error.contains("document id"), "{id}: {error}");
    }

    let mut request = replace_html(&original, "unused");
    request.edits = vec![
        MotionTextReplacement {
            start: 0,
            end: 4,
            replacement: "a".into(),
        },
        MotionTextReplacement {
            start: 3,
            end: 5,
            replacement: "b".into(),
        },
    ];
    request.expected_result_hash = "0".repeat(64);
    let error = store
        .save_patch(request)
        .expect_err("overlapping edits must fail");
    assert!(error.contains("overlap"), "{error}");
}

#[cfg(unix)]
#[test]
fn rejects_a_symlinked_motion_root() {
    use std::os::unix::fs::symlink;

    let (temp, core, project) = saved_core("symlink");
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).expect("create outside directory");
    symlink(&outside, project.join(MOTION_DOCUMENTS_DIR)).expect("create root symlink");

    let error = MotionDocumentStore::new(core)
        .create(MotionDocumentCreateRequest { title: None })
        .expect_err("symlinked root must fail");
    assert!(error.contains("no-follow directory"), "{error}");
    assert!(fs::read_dir(outside)
        .expect("read outside")
        .next()
        .is_none());
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_revision_sources_without_reading_outside_the_project() {
    use std::os::unix::fs::symlink;

    let (temp, core, project) = saved_core("source-symlink");
    let store = MotionDocumentStore::new(core);
    let document = create_document(&store);
    let revision = current_directory(&project, &document.summary.id);
    let outside = temp.path().join("outside.html");
    fs::write(&outside, b"outside secret").expect("write outside fixture");
    fs::remove_file(revision.join(HTML_FILE)).expect("remove managed HTML");
    symlink(&outside, revision.join(HTML_FILE)).expect("replace HTML with symlink");

    let error = store
        .read(&document.summary.id)
        .expect_err("symlinked source must fail closed");

    assert!(error.contains("no-follow regular file"), "{error}");
    assert_eq!(fs::read(outside).unwrap(), b"outside secret");
}

#[cfg(unix)]
#[test]
fn rejects_fifo_sources_without_blocking_the_store() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::mpsc;
    use std::time::Duration;

    let (_temp, core, project) = saved_core("source-fifo");
    let store = Arc::new(MotionDocumentStore::new(core));
    let document = create_document(&store);
    let revision = current_directory(&project, &document.summary.id);
    let fifo = revision.join(HTML_FILE);
    fs::remove_file(&fifo).expect("remove managed HTML");
    let fifo_path = CString::new(fifo.as_os_str().as_bytes()).expect("FIFO path");
    // SAFETY: the path is a valid, NUL-terminated filesystem path and mode is
    // restricted to the test process owner.
    assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);

    let reader = Arc::clone(&store);
    let document_id = document.summary.id.clone();
    let (sent, received) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        sent.send(reader.read(&document_id)).unwrap();
    });
    let error = received
        .recv_timeout(Duration::from_secs(2))
        .expect("FIFO read must not block")
        .expect_err("FIFO must fail closed");
    assert!(
        error.contains("byte limit") || error.contains("regular file"),
        "{error}"
    );
    worker.join().unwrap();
}

#[test]
fn rejects_invalid_utf8_oversized_files_and_invalid_manifest() {
    let (_temp, core, project) = saved_core("corrupt");
    let store = MotionDocumentStore::new(core);
    let first = create_document(&store);
    let first_dir = current_directory(&project, &first.summary.id);
    fs::write(first_dir.join(HTML_FILE), [0xff, 0xfe]).expect("write invalid UTF-8");
    let error = store
        .read(&first.summary.id)
        .expect_err("invalid UTF-8 must fail");
    assert!(error.contains("UTF-8"), "{error}");

    let second = store
        .create(MotionDocumentCreateRequest {
            title: Some("oversized".into()),
        })
        .expect("create second document");
    let second_dir = current_directory(&project, &second.summary.id);
    fs::write(second_dir.join(CSS_FILE), vec![b'a'; MAX_SOURCE_BYTES + 1])
        .expect("write oversized CSS");
    let error = store
        .read(&second.summary.id)
        .expect_err("oversized source must fail");
    assert!(error.contains("byte limit"), "{error}");

    let third = store
        .create(MotionDocumentCreateRequest {
            title: Some("manifest".into()),
        })
        .expect("create third document");
    let third_dir = current_directory(&project, &third.summary.id);
    fs::write(third_dir.join(DOCUMENT_MANIFEST_FILE), b"not-json").expect("write invalid manifest");
    let error = store
        .read(&third.summary.id)
        .expect_err("invalid manifest must fail");
    assert!(error.contains("manifest"), "{error}");
}

#[test]
fn failed_catalog_replace_preserves_the_prior_revision_after_restart() {
    let (_temp, core, project) = saved_core("rename-failure");
    let store = MotionDocumentStore::new(core.clone());
    let original = create_document(&store);
    store.fail_next_catalog_replace_for_test();
    let error = store
        .save_patch(replace_html(&original, "<main>must not publish</main>"))
        .expect_err("injected catalog replace must fail");
    assert!(
        error.contains("injected catalog replace failure"),
        "{error}"
    );
    assert_eq!(store.read(&original.summary.id).unwrap(), original);
    drop(store);

    let reopened_core = AppCore::new();
    reopened_core.open_project(project).expect("reopen project");
    assert_eq!(
        MotionDocumentStore::new(reopened_core)
            .read(&original.summary.id)
            .expect("read original after restart"),
        original
    );
}

#[test]
fn post_commit_sync_failure_reports_error_but_preserves_published_revision() {
    let (_temp, core, project) = saved_core("sync-failure");
    let store = MotionDocumentStore::new(core.clone());
    let original = create_document(&store);
    let replacement = "<main>published before directory sync failed</main>";
    let request = replace_html(&original, replacement);
    let expected_hash = request.expected_result_hash.clone();
    store.fail_next_catalog_sync_for_test();

    let error = store
        .save_patch(request)
        .expect_err("post-commit durability failure must not report success");
    assert!(error.contains("after commit"), "{error}");
    let published = store
        .read(&original.summary.id)
        .expect("committed catalog remains readable");
    assert_eq!(published.html, replacement);
    assert_eq!(published.summary.revision_hash, expected_hash);
    drop(store);

    let reopened_core = AppCore::new();
    reopened_core.open_project(project).expect("reopen project");
    let reopened = MotionDocumentStore::new(reopened_core)
        .read(&original.summary.id)
        .expect("committed revision survives restart");
    assert_eq!(reopened.html, replacement);
    assert_eq!(reopened.summary.revision_hash, expected_hash);
}

#[test]
fn queued_request_is_bound_to_authority_captured_at_admission() {
    let (temp, core, _source) = saved_core("admission-source");
    let store = MotionDocumentStore::new(core.clone());
    let original = create_document(&store);
    let authority = store.capture_authority().expect("capture source authority");
    let request = replace_html(&original, "<main>must not cross projects</main>");
    let destination = temp.path().join("admission-destination.opentake");
    core.save_project(Some(destination.clone()))
        .expect("Save As replacement project");

    let error = store
        .save_patch_for_authority(authority, request)
        .expect_err("queued old-project request must fail after project replacement");
    assert!(error.contains("current project changed"), "{error}");
    assert_eq!(
        store
            .read(&original.summary.id)
            .expect("Save As copy remains unchanged"),
        original
    );
}

#[test]
fn rejects_manifest_that_pretty_serialization_cannot_read_back() {
    let (_temp, _core, project) = saved_core("manifest-byte-limit");
    let project = Dir::open_ambient_dir(project, ambient_authority()).expect("open project root");
    let root = motion_root(&project, true)
        .expect("create motion root")
        .expect("motion root exists");
    let mut parameters = BTreeMap::new();
    for index in 0..3_500 {
        parameters.insert(format!("key-{index:04}"), Value::from(index));
    }
    assert!(serde_json::to_vec(&parameters).unwrap().len() <= MAX_PARAMETERS_BYTES);
    let document = document_with_content(
        uuid::Uuid::new_v4().to_string(),
        "Manifest bound".into(),
        STARTER_HTML,
        STARTER_CSS,
        parameters,
    )
    .expect("compact parameters fit their own bound");

    let error = write_revision_directory(&root, &document)
        .expect_err("unreadable oversized pretty manifest must be rejected before publication");
    assert!(error.contains("manifest exceeds its byte limit"), "{error}");
    assert!(root.read_dir(".").unwrap().all(|entry| {
        !entry
            .ok()
            .and_then(|entry| entry.file_name().into_string().ok())
            .is_some_and(|name| name.starts_with("rev-"))
    }));
}
