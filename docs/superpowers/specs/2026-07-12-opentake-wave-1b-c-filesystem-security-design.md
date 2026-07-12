# OpenTake Wave 1B-C Filesystem Security Design

## Approval And Execution Authority

The user authorized the controller to approve designs, commit code, review work,
and continue without additional human confirmation. This design therefore uses
the same automated approval protocol as the full-convergence design:

1. the controller writes and self-reviews the artifact;
2. independent agents inspect it against the exact committed revision;
3. every actionable finding is fixed and the artifact is re-reviewed;
4. implementation begins only after the latest spec and plan reviews report
   Critical/Important/Minor 0/0/0.

## Goal

Make project-bundle export safe against source deletion, unauthorized destination
writes, path escape, symlink escape, partial output, and stale-project races.
Then apply the same filesystem invariants to Save-As in a separate implementation
plan so Wave 1B-C closes the whole project-bundle publication class rather than
moving the defect to another command.

## Confirmed Current Failures

The exact reviewed baseline is
`9a091379a31b960f39a7ec5a3617acfb52f63a21`.

The normal UI currently proposes the open project itself as the bundle-export
destination: `defaultBundleName(projectPath)` retains the source basename and
`onExportBundle()` combines it with the source parent. If the user accepts the
save panel's replacement prompt, `archive()` deletes `dest_bundle` before it
resolves internal media or copies thumbnail/chat data. With `dest ==
source_bundle`, the source is deleted and rebuilt from the in-memory JSON
snapshot without its internal media, thumbnail, or chat sessions. This is a
normal-workflow data-loss bug, not only an adversarial path.

The same implementation also:

- accepts an arbitrary renderer-supplied `outPath` at the Tauri command;
- deletes an existing destination before any source preflight;
- writes the destination directly, leaving a partial bundle on any later error;
- joins `.project.relativePath` without rejecting absolute or parent components;
- follows file symlinks in project media, thumbnail, and chat sessions;
- permits physical source/destination aliasing through `..`, case behavior, or
  symlinked ancestors;
- treats permission and metadata errors as ordinary missing media;
- can recurse into its own output when the destination is under source
  `chat-sessions/`;
- repeats related alias, symlink, and partial-publication behavior in
  `copy_media_dir()` and Save-As.

Compatibility-read-only refusal already runs before archive work and a single
core lock already produces a coherent bundle snapshot. Those controls remain.

## Threat Boundary

The repository-scoped threat model is cached at the Codex Security artifact
path for this revision. This design applies these narrower conclusions:

- Project JSON, media manifests, generation logs, thumbnail, chat sessions, and
  removable-volume layout are untrusted inputs.
- A native save panel is operator authorization only when Rust owns the panel
  result. A raw path returned to the WebView and later sent to another command
  is data, not proof of authorization.
- Project-internal paths and auxiliary project trees receive no ambient access
  outside the source bundle.
- External media paths are an intentional OpenTake feature. They may point
  anywhere the user can read, so bundle export requires a separate native
  confirmation before copying any external sources from a persisted manifest.
- The output is a directory package. Cross-platform `std::fs::rename` cannot
  atomically replace an arbitrary existing non-empty directory. Wave 1B-C must
  not describe backup/rollback as a single atomic replace.
- The renderer is inside the attack boundary. A separate hostile process already
  running as the same operating-system account is not; that process can read and
  rewrite all of the user's project data regardless of this command. Destination
  collision is nevertheless handled atomically so benign concurrent exporters
  cannot clobber one another.

## Alternatives Considered

### 1. Guard-only patch

Reject `source == destination` and change the default filename. This stops the
known normal-workflow deletion but leaves renderer path authority, traversal,
symlinks, direct partial writes, existing-destination destruction, and Save-As.
It is suitable only as the first independently reviewed implementation slice,
not as Wave 1B-C completion.

### 2. Rust-owned authorization plus new-destination-only publication

Rust opens the save panel, validates the selected path and all sources, builds a
complete sibling staging bundle, validates it, and renames it once to a
destination that must not exist. Existing destinations are never overwritten.
This is the selected design because it provides a real renderer boundary and a
cross-platform atomic publication point with a small, testable state machine.

### 3. Existing-directory replacement transaction

Build a sibling stage, rename the old destination to backup, rename stage into
place, roll back on failure, clean backup, and recover after crashes. This
preserves upstream overwrite behavior but is materially larger, has a visible
crash window without platform-specific directory exchange, and needs a durable
journal/recovery policy. It is explicitly excluded from this release slice.
Users export to a new name instead.

## Selected Architecture

### A. Path policy and immutable preflight

`opentake-project` gains one shared path-policy layer used by archive first and
Save-As next. It constructs an immutable preflight plan before the first output
write.

The implementation boundary is a new private `safe_fs` module rather than
scattered `std::fs` checks:

- on Linux and macOS, `rustix` supplies descriptor-relative `openat`, `statat`,
  directory iteration, unlink, and `renameat_with` operations;
- Unix traversal opens each directory with `O_DIRECTORY | O_NOFOLLOW |
  O_CLOEXEC`, opens leaves with `O_NOFOLLOW | O_CLOEXEC`, and verifies type and
  `(device, inode)` from the opened handle;
- on Windows, a target-specific `windows-sys` adapter opens directories with
  `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`, rejects every
  reparse point through `FileAttributeTagInfo`, retains file identity from
  `FileIdInfo`, and holds the authorized parent without `FILE_SHARE_DELETE`;
- unsupported operating systems or filesystems return typed
  `UnsupportedSecureFilesystem`/`UnsupportedAtomicPublish`; there is no
  path-based or ordinary-rename fallback.

Linux, macOS, and Windows compile gates are required. The path-policy and
publication race suites run natively on all three release platforms; the
existing Ubuntu-only CI is expanded before C1 is accepted.

Every destination carries a `NamespaceAnchor`, not only an open parent. Starting
at the filesystem/volume root, the adapter retains the bounded component chain
`(directory handle, child name, stable identity)` down to the authorized parent.
Unix re-walks the chain with descriptor-relative `statat(..., NOFOLLOW)` before
stage validation and final publication. Windows retains every ancestor handle
without `FILE_SHARE_DELETE` and also rechecks `FileIdInfo`. A renamed, rebound,
or remounted component returns `DestinationNamespaceChanged`; output is not
published through a still-open directory object whose authorized pathname has
changed.

Project media and auxiliary trees always reject symlink/reparse components.
External media uses a separate bounded resolver: Unix walks with `readlinkat`
relative to retained directory descriptors; Windows reads only supported
symlink/mount-point reparse data with `FSCTL_GET_REPARSE_POINT`. Both cap link
depth, reject loops and unsupported reparse tags, produce a canonical
component-only target, and then reopen that target with nofollow/reparse-point
flags for identity verification. Ambient `canonicalize()` is discovery only and
is never the copy authorization primitive.

Destination requirements:

- absolute path;
- literal lowercase final extension `.opentake` for a new destination;
- an existing, canonicalizable parent directory;
- final path itself does not exist, including a file, directory, or symlink;
- no lexical or physical equality/ancestor/descendant overlap with the source
  bundle;
- no overlap with a resolved media source;
- no source, destination, staging, or auxiliary-tree self-recursion.

The comparison form canonicalizes the source root and the destination's nearest
existing ancestor, then appends only validated normal components. Existence is
queried through the selected parent filesystem, so a case-folding volume treats
a differently cased alias as the existing source while a case-sensitive volume
may accept it as a distinct new entry. The implementation does not infer volume
semantics from the host platform and does not use the existing lexical
`standardize()` dedup key as a security decision.

The native selection is converted immediately into a
`DestinationCapability`: the canonical parent directory handle, its stable file
identity, its `NamespaceAnchor`, the validated single destination name, and an
exclusive random stage name. Stage creation, output `create_new`, validation,
cleanup, and publication remain relative to that same handle. Neither the
original renderer-visible path nor a freshly resolved parent path is used after
capability creation.

Project media requirements:

- `MediaSource::Project.relativePath` is relative;
- it contains only normal components, begins with `media/`, and contains no
  root, prefix, `.` ambiguity, or `..`;
- every existing path component is opened without following a symlink;
- the leaf is a regular file under the canonical source `media/` root;
- a file leaf must have exactly one hard link (`st_nlink == 1` on Unix,
  `NumberOfLinks == 1` on Windows), checked before and after copy;
- valid but absent media remains a `MissingMedia` result; malformed, escaped,
  inaccessible, symlinked, or special-file paths are typed errors.

Auxiliary source requirements:

- `thumbnail.jpg`, when present, is a nofollow regular file;
- `chat-sessions/`, when present, is a real directory under the source bundle;
- traversal accepts only real directories and regular files and rejects every
  symlink, socket, FIFO, device, or other special entry;
- every thumbnail/chat file leaf must have exactly one hard link, checked before
  and after copy; multi-link leaves are `UnsupportedSourceType::HardLink`;
- only `NotFound` means optional absent. Permission, metadata, and I/O errors
  fail the archive.

External media requirements:

- `absolutePath` must be absolute. Relative paths never resolve against the
  process working directory;
- a symlink is allowed only when complete resolution ends at a regular file;
  a directory, FIFO, socket, device, reparse point, symlink loop, or other
  special target is rejected;
- only `NotFound` is missing. Permission, metadata, resolution, and other I/O
  failures are typed errors rather than missing media;
- the analysis records every lexically distinct manifest path, status, resolved
  target, and stable file identity. The existing lexical dedup key remains
  unchanged, so two separately listed symlink paths still produce separate
  collected entries as upstream expects;
- `native_disclosure` is a non-WebView platform adapter whose Rust-owned model
  and return value never pass through renderer IPC. macOS uses an AppKit
  `NSPanel` with `NSTableView` inside `NSScrollView`; Windows uses a Win32 modal
  dialog with `SysListView32`; Linux uses a GTK modal window with a scrolled
  `TreeView`. All show one row per lexical entry with separate original,
  resolved-target, and status columns, plus search and cancel. `Approve all`
  stays disabled until the complete model is loaded, and the virtualized table
  supports scrolling back through every row;
- disclosure text uses a reversible ASCII encoding of operating-system path
  units, not Unicode scalar assumptions. Unix passes printable ASCII bytes
  except backslash and encodes every other raw byte as `\xNN`; Windows passes
  printable ASCII UTF-16 units except backslash and encodes every other code
  unit, including unpaired surrogates, as `\uNNNN`. U+2028/U+2029, bidi,
  zero-width/default-ignorable characters, C0/C1 controls, separators, and all
  non-ASCII therefore have no raw visual effect. Display columns are counted on
  this ASCII form;
- an encoded original or target over 4,096 columns, a manifest over 4,096
  external entries, or a total encoded model over 4 MiB returns typed
  `ExternalDisclosureTooLong`/`TooManyExternalSources` before opening the panel
  or writing output. No field is truncated or elided;
- native accessibility/UI QA enumerates every table row and column and compares
  it byte-for-byte with the Rust authorization model. Only the adapter's native
  approve result can continue; renderer events cannot synthesize it;
- successfully copied external entries are rewritten to project-relative
  `media/` paths. A missing external entry is rewritten to a deterministic,
  absent `media/missing/<sha256>-<sanitized-basename>` project reference and
  remains in the missing-media report. SHA-256 uses a domain-separated encoding
  of entry ordinal, raw asset id, and raw lexical source path; the full 64 hex
  characters are retained. The basename uses only ASCII alphanumeric, `.`, `_`,
  and `-`, is capped at 64 bytes, and falls back to `missing`. The resulting path
  is passed through the same normal-component validator, and collisions are a
  typed error. Raw asset ids never become path components, and no host absolute
  path from `MediaSource::External.absolutePath` is persisted in an exported
  media source field;
- no external-source approval is inferred from project JSON or WebView state.

If an external path resolves to a different target between native confirmation
and source-handle acquisition, export aborts without output. It does not silently
extend the confirmation to the replacement target.

The copy implementation is bounded and streaming. It retains source/destination
root capabilities plus each source's approved canonical target and stable file
identity, but does not pre-open an unbounded handle set. For each item it opens
at most one source and one destination leaf, verifies the opened identity and
regular-file metadata against the plan, copies through those handles, verifies
that size/identity did not change during the copy, and closes both before the
next item. A separate check followed by path-based `fs::copy` is not sufficient.
Tests introduce swaps at the check/open seams and run a large manifest under a
low file-descriptor limit.

### B. Build and publish split

Archive becomes three explicit phases. Source-only analysis may run before the
save panel so Rust can obtain external-source consent, but the immutable
preflight plan is complete only after a destination has been selected and all
approved source identities and root capabilities have been recorded:

1. `prepare`: validate compatibility, paths, relationships, source kinds, and
   external-source summary without creating output;
2. `build_into`: create an unpredictable, exclusive sibling staging directory
   under `dest.parent`, copy media/extras through the preflight plan, and write
   the three JSON documents into that empty stage, producing a `BuildReceipt`;
3. `validate_staged_bundle`: strictly read the three just-written JSON documents
   and rewritten media references relative to the retained stage handle, match
   the complete tree to `BuildReceipt`, then verify the namespace anchor and
   parent entry still name that stage identity;
4. `publish_new`: perform one atomic no-replace rename to the destination.

`validate_staged_bundle(stage: &StageCapability)` never calls the current
path-based `Project::open`. Core JSON is opened with `openat`/the Windows handle
adapter under the stage capability and decoded with strict, generated-output
rules; directory/media checks use the same retained root. `Project::open` is
reserved for post-publication QA, where path resolution no longer authorizes a
write.

Every directory and leaf created by `build_into` has a receipt. Directory rows
record relative normal path and stable identity. Leaf rows record relative path,
type, identity, exact length, and full SHA-256 measured through the writer's open
handle. Validation enumerates the stage capability, rejects any missing or extra
entry, reopens each leaf nofollow, matches identity/type/length/digest, and only
then performs strict JSON/media-reference checks. Replacing generated JSON with
different but valid JSON, replacing media, or adding a hidden extra entry cannot
pass. An identical-byte replacement is harmless content-equivalence but still
fails identity comparison.

An RAII `StageGuard` owns the parent capability, stage handle, name, and stable
identity. Cleanup recursively unlinks children relative to the retained stage
handle and removes the parent entry only after it still matches that identity.
If the name was rebound, cleanup never follows or deletes the replacement; it
empties only objects reachable through the retained capability where the OS
allows and returns `StageIdentityLost`. In that hostile same-account tamper case
an unreachable empty stage may remain, but no destination is published and no
unproven path is deleted. Ordinary build/validation/CAS failures remove the
stage completely.

`publish_new` is a platform adapter, not `std::fs::rename`:

- Linux calls descriptor-relative `renameat2(..., RENAME_NOREPLACE)` through
  `rustix::fs::renameat_with(..., RenameFlags::NOREPLACE)`;
- macOS calls descriptor-relative `renameatx_np(..., RENAME_EXCL)` through the
  same `rustix` API; a volume that rejects exclusive rename fails closed;
- Windows opens the stage with delete access and calls
  `SetFileInformationByHandle(FileRenameInfo)` with `ReplaceIfExists = FALSE`
  while the non-delete-shared parent handle remains open;
- an unsupported syscall, volume, remote-share behavior, or filesystem returns
  `UnsupportedAtomicPublish`. It never falls back to check-then-rename.

The stage is exclusively created with mode `0700` on Unix and equivalent
owner-only access on Windows. Its retained identity is checked before any
validation read and again immediately before publication; the full
`NamespaceAnchor` is revalidated at both points. Tests can swap/rebind an
ancestor or parent, swap the stage name, replace a stage child with valid data,
or add an extra child before those checks and must observe refusal without
changing the replacement. Arbitrary hostile same-account processes after the
final check remain outside the stated threat boundary; atomic destination
no-replace still holds against every concurrent destination creator.

Any ordinary prepare/build/validation failure removes only the stage and leaves
source and destination untouched; the explicitly described identity-lost tamper
case may leave an unreachable empty stage rather than delete an unproven name.
If a competing process creates the destination, publication fails rather than
replacing it. Staging is in the destination parent so the final rename cannot
cross filesystems. Output byte accounting uses the successful copy return value
rather than a second metadata lookup.

This design does not overwrite existing destinations. Rust returns typed
`DestinationExists`, and the native workflow asks the user to choose a fresh
name. The default is `<project-stem>-export.opentake`, with a numeric suffix when
needed. The source project itself is never a proposed default.

### C. Rust-owned destination authorization

The Tauri command no longer accepts `outPath`:

```rust
pub async fn export_bundle(
    app: tauri::AppHandle,
    core: tauri::State<'_, AppCore>,
) -> Result<Option<BundleReportDto>, BundleExportErrorDto>
```

Flow:

1. capture a lock-consistent snapshot including project epoch, path, and a new
   opaque `bundle_revision` covering every export input;
2. perform source-only authorization analysis and, when necessary, show the
   native external-source confirmation;
3. open the Rust `tauri-plugin-dialog` save panel with the safe default name;
4. cancel returns `Ok(None)`;
5. call one lock-scoped `compare_bundle_identity()` over project epoch,
   `bundle_revision`, and path; any change aborts before output so the dialogs
   cannot authorize one bundle state and export another;
6. complete destination relationship checks, acquire root capabilities and
   approved source identities, and reject any external target that differs from
   the confirmed resolution;
7. run prepare/build/validation in a blocking worker;
8. call `AppCore::publish_bundle_if_identity(expected, &mut stage_guard)`. This
   one finalizer acquires the core lock, compares epoch/revision/path, keeps the
   lock held for the single `publish_new` syscall, constructs the success report,
   then releases it. A same-project edit, media import/favorite/relink, Save-As,
   project open/new, or future generation-log mutation either commits before the
   lock and causes `ProjectChanged`, or waits until after publication;
9. on mismatch/publication failure, release the lock and clean through
   `StageGuard`; on success return the exact published path.

`CoreSessionSlot` owns a monotonic `bundle_revision` separate from the timeline
version. It increments on every successful change to timeline, media manifest,
generation log, compatibility state, or `project_dir`; all present and future
mutators must pass through wrappers that advance it. `BundleExportSnapshot`
contains the revision, and `bundle_identity()`/`compare_bundle_identity()` read
epoch, revision, and path together under one lock. Focused tests prove media-only
and generation-log-only changes invalidate an authorization even when the
timeline version is unchanged. `compare_bundle_identity()` is used for the
pre-write check; only `publish_bundle_if_identity()` may perform the final
compare-and-publish transition.

The main WebView cannot supply or override the destination. MCP and Agent do
not gain bundle export in this slice. Any future Agent flow must use the same
Rust-owned native confirmation and may not expose a raw-path mutation tool.

The Web API becomes `exportBundle(): Promise<BundleReport | null>`. The bundle
branch removes its JavaScript save-panel/path construction. `null` is neutral
cancel. Rejections carry a serializable tagged `BundleExportErrorDto` with a
stable code (`destination_exists`, `unsafe_source`, `unsafe_destination`,
`unsupported_source_type`, `project_changed`, `stage_failure`,
`stage_identity_lost`, `destination_namespace_changed`,
`unsupported_filesystem`, `too_many_external_sources`,
`external_disclosure_too_long`, or `io`), a user-safe message, and an optional
display path. The Tauri boundary maps `ProjectError` once without flattening it
to `String`; TypeScript narrows the DTO by `code` and never parses localized
messages.

### D. Bundle entry and empty-timeline behavior

Project-bundle export is separate from rendered-video export. The UI must allow
opening bundle export for an empty or unsaved project even though video export
is disabled. The menu receives a distinct bundle item or opens the dialog with
bundle mode selected; video mode retains its empty-timeline disable rule.

The safe default name is locked by tests:

- saved `My Film.opentake` -> `My Film-export.opentake`;
- unsaved project -> localized `Untitled-export.opentake`;
- an existing candidate -> next unused numeric suffix;
- the proposed path never equals the open project path.

The backend is still authoritative for alias and existence rejection.

### E. Shared Save-As follow-up

`copy_media_dir()` and `EditorSession::save_project_with_thumbnail()` share the
same defect class but are not mixed into the first archive implementation plan.
Wave 1B-C therefore has two ordered subprojects:

1. **Wave 1B-C1:** secure self-contained bundle export described above;
2. **Wave 1B-C2:** stage a complete Save-As bundle (JSON, media, thumbnail, chat)
   at a new destination, apply the same path/nofollow policy, validate, publish
   once, and commit the new `project_dir` only after publication.

C2 reuses C1's path-policy, safe-tree-copy, staging, and `publish_new` modules.
Save-As to an existing different bundle is rejected. Normal save to the exact
current project remains a separate component-atomic operation and receives no
new overwrite semantics in C2.

Rust also owns every C2 destination authorization. `project_save()` becomes a
pathless command that may only save to the already-authorized active
`project_dir`. A separate `project_save_as(app, core)` opens the native panel and
returns `Result<Option<PathDto>, SaveProjectErrorDto>`; it accepts no renderer
path, and only its native result can create a `DestinationCapability`. The new
project UI uses `project_new_with_dialog(...)`, which opens the Rust panel first,
publishes a fresh empty bundle through the same transaction, and commits the new
session/playback transition only after publication; cancellation leaves the
current session unchanged. Web `projectSave(path)` and its JavaScript Save-As/new
project dialogs are removed. Browser-only fallback remains in-memory and has no
filesystem authority.

C2 uses an explicit short-lock CAS transaction:

1. capture an initial `BundleIdentity` for the native default, open the Rust save
   panel, and compare that identity after selection; a project change cancels
   authorization before thumbnail or output work;
2. under the core lock, capture `SaveCaptureSnapshot` containing the exact
   epoch, `bundle_revision`, path, timeline, and manifest used for thumbnail
   capture; release the lock and capture the optional JPEG;
3. call `prepare_save_snapshot(capture_identity, thumbnail)`, which compares the
   supplied identity under one lock before cloning timeline, manifest,
   generation log, compatibility, source path, and the identity. A mismatch
   returns `ProjectChanged` rather than attaching an old thumbnail to new state;
4. release the lock and build/validate a new-destination stage through the C1
   capabilities;
5. reacquire the lock, compare epoch/revision/source path, and on mismatch
   release the lock, delete the stage, and return `ProjectChanged`;
6. while that short lock remains held, call `publish_new`, update `project_dir`,
   advance `bundle_revision`, and capture the `ProjectSaved` payload; then
   release the lock and emit the event.

The core lock is never held during media/tree copying, but no edit can occur
between the final CAS, atomic publication, and adoption of the new project path.
A publication failure leaves the active path and revision unchanged. Thumbnail
precedence is deterministic: fresh captured JPEG bytes override everything;
otherwise a source `thumbnail.jpg` is copied through the nofollow policy; when
neither exists the output omits it. Generation log always comes from the save
snapshot, and chat sessions are safely copied from the source bundle. No output
component is inherited from a pre-existing destination because C2 only publishes
to a new path. The same capture-identity comparison protects thumbnail writes on
normal same-project save, without changing its destination semantics.

Fresh project creation has a distinct finalizer because it also changes playback
and prewarm epochs:

1. capture the current `BundleIdentity`, obtain the Rust-native destination, and
   build/validate a fully prepared empty-bundle stage plus an infallible fresh
   `EditorSession` carrying that future path; no transition reservation is held
   during dialogs or build;
2. immediately before finalization, reserve playback transition first and
   prewarm transition second using the existing ordered rollback. If prewarm is
   busy, cancel playback; in no-default-feature builds reserve prewarm only;
3. under one short core lock, compare the original identity, call `publish_new`,
   and install the already prepared editor, advancing project epoch and
   `bundle_revision`. Every allocation/parse/fallible preparation occurs before
   this point, so after a successful rename the in-memory install is infallible;
4. on CAS/publication failure, release the lock, cancel every reservation, and
   clean the stage. On success, release the lock, activate playback then prewarm
   for the new epoch, and emit one `ProjectOpened` event carrying the published
   path/version; no intermediate empty-path project event is emitted.

Cancellation, playback-busy, prewarm-busy, CAS mismatch, and publication failure
leave the current session/epoch and both registries unchanged and create no
destination. Feature-on and no-default-feature tests prove every seam.

Wave 1B-C is not release-complete until both subprojects pass their exact-bundle
QA. C1 may merge first because it removes the current critical export path.

## Error Contract

`ProjectError` gains typed archive/path variants instead of flattening every
case into `Io` or `missing`:

- unsafe or escaped source;
- unsafe or overlapping destination;
- destination exists;
- unsupported source file type/symlink policy;
- project changed during authorization;
- stage validation/publication failure;
- too many external sources;
- external disclosure too long;
- stage identity lost;
- destination namespace changed;
- unsupported secure filesystem or atomic publication.

User-facing errors state what remains safe: source unchanged, destination
unchanged/not created, and whether a fresh name is required. Detailed paths stay
in the export dialog error, not in transient success toasts or release logs.
The serializable `BundleExportErrorDto` preserves those categories across IPC;
only cancellation uses `Ok(None)`.

## Test Design

### C0 emergency regression

- the current saved-project default name is not the source name;
- exact source == destination fails before mutation;
- lexical and symlink-parent aliases fail before mutation;
- source ancestor/destination descendant and destination ancestor/source
  descendant fail before mutation;
- source recursive manifest remains byte-identical after every rejection.

### Source policy

- project-relative parent, absolute, non-`media/`, symlink-file, symlink-dir,
  inaccessible, and special-file cases fail before destination creation;
- missing valid project media remains reported missing;
- thumbnail and chat root/entry symlink escapes fail;
- destination under chat sessions cannot recurse;
- only `NotFound` becomes optional absent/missing;
- external relative, directory, FIFO, socket, device, permission-error, symlink
  loop, and check/open-swap cases fail closed;
- every external entry appears in the Rust-owned native disclosure table,
  including a sensitive final entry in a large manifest, without changing
  lexical dedup semantics;
- native accessibility rows match the ASCII authorization model byte-for-byte;
  Unix non-UTF8 bytes, Windows unpaired surrogates, U+2028/U+2029, zero-width,
  default-ignorable, control, separator, backslash, and bidi inputs round-trip or
  fail the documented model limits before the panel;
- renderer attempts cannot alter the disclosure model or synthesize approval;
- missing external paths are sanitized to absent project-relative references and
  no raw external `absolutePath` survives in an output source field;
- hostile asset ids containing parent/root components, both separators,
  controls, bidi text, host paths, long strings, or collision inputs never enter
  placeholder paths; full digest paths remain unique normal components;
- project media, thumbnail, and chat hard-link leaves are rejected; link-count
  changes during copy fail closed on all three platforms;
- a low-FD-limit large manifest completes without unbounded open handles, while
  over-limit disclosure returns `TooManyExternalSources` before output.

### Publication

- build failure after copied media preserves source and creates no destination;
- JSON serialization/write, thumbnail, chat, validation, and final-rename
  failures remove stage and preserve source, except identity-lost tamper may
  leave only the capability-cleaned unreachable empty stage;
- existing destination is byte-identical and returns `DestinationExists`;
- successful output contains no stale media or staging names and reopens through
  `Project::open`;
- stage is a sibling on the same filesystem;
- concurrent creation of a destination file, empty directory, non-empty
  directory, or symlink causes atomic refusal and leaves it byte-identical;
- ancestor/parent rename, rebind, or remount and stage swaps before validation or
  final publication cause refusal and capability cleanup;
- strict stage validation opens no component through `Project::open(path)`;
  receipt mismatch, valid-JSON/media replacement, missing/extra entry, and
  stage-name/child replacement are rejected, and identity-lost cleanup never
  deletes an unproven name;
- unsupported no-replace syscalls/filesystems fail closed without ordinary
  rename fallback;
- Linux, macOS, and Windows native race suites exercise their platform adapter;
- check/open and entry-type/copy race hooks cannot redirect a copy outside the
  approved source.

### Tauri/Web

- the command has no `outPath` input and cancellation creates nothing;
- a project epoch/path/bundle-revision change while a native dialog is open or
  while the stage is building creates no destination;
- timeline, media-only, and generation-log-only mutations each advance
  `bundle_revision` and fail both pre-write and pre-publish CAS seams;
- a deterministic finalizer seam proves queued mutations cannot commit between
  the final comparison and `publish_new`;
- external-source confirmation is required before the save panel;
- an external target swap after confirmation aborts rather than copying the new
  target;
- Web invokes `export_bundle` with no raw destination argument;
- saved/unsaved/collision defaults are safe;
- bundle export remains available on an empty timeline while video remains
  disabled;
- DTO serialization and Web narrowing distinguish success, missing media,
  cancellation, every typed path/publication rejection, and generic I/O without
  parsing message text.

### C2 Save-As

- failed media/extras copy leaves no new bundle and does not change
  `project_dir`;
- source/destination alias or nesting fails before writes;
- a complete new bundle is validated and published once, then becomes the
  active `project_dir`;
- existing different destination remains byte-identical;
- edits, media-only changes, open/new, and another Save-As during stage build
  fail the final CAS with no destination and no path adoption;
- Rust Save-As/new-project commands accept no renderer destination, cancel
  without state change, and are the only source of `DestinationCapability`;
- feature-on and no-default-feature new-project tests cover playback busy,
  prewarm busy, CAS mismatch, publication failure, cancellation, and success,
  asserting destination, session/epoch, event, and registry state;
- timeline edit, media-only change, open/new, and Save-As between
  `SaveCaptureSnapshot` and `prepare_save_snapshot` reject the captured thumbnail
  and publish nothing;
- captured thumbnail overrides a source cover, `None` safely carries a source
  cover, and absent-on-both omits it; chat and generation log survive reopen;
- normal same-project save behavior and autosave remain unchanged.

Every code slice uses RED proof, focused tests, full relevant Rust/Web gates,
two independent 0/0/0 reviews, an exact detached bundle, and recursive before/
after manifests. Real UI QA covers the former self-overwrite default, cancel,
external confirmation, successful fresh export, existing-destination refusal,
and Save-As reopen.

## Non-Goals

- Existing non-empty bundle overwrite or crash-recovery journaling.
- General CSP and `assetProtocol.scope` hardening; those remain a separate
  WebView/security slice.
- Signed/notarized packaging, update/appcast signing, model integrity, or
  bundled FFmpeg; those remain later Wave 1B release work.
- MCP/Agent bundle export.
- Automatic upload or sharing of exported bundles.
- Rewriting the persisted media schema with bookmarks or provenance tokens.

## Acceptance

Wave 1B-C1 is accepted only when normal UI cannot propose or authorize the open
project as destination, renderer code cannot provide an arbitrary destination,
all internal/archive-extra paths stay inside the approved source, every external
read is fully disclosed, external source fields do not leak host absolute paths,
existing destinations are protected by a proven platform no-replace primitive,
and a successful fresh destination appears only after a complete validated stage
whose bundle identity still matches the authorized snapshot.

Wave 1B-C is accepted only after C2 also proves that Save-As publishes a complete
new bundle or nothing, never changes project identity on failure, accepts no
renderer-supplied Save-As/new-project destination, binds any thumbnail to the
same bundle identity, and reuses the same path and nofollow controls.
