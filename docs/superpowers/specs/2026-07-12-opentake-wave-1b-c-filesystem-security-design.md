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

Project media requirements:

- `MediaSource::Project.relativePath` is relative;
- it contains only normal components, begins with `media/`, and contains no
  root, prefix, `.` ambiguity, or `..`;
- every existing path component is opened without following a symlink;
- the leaf is a regular file under the canonical source `media/` root;
- valid but absent media remains a `MissingMedia` result; malformed, escaped,
  inaccessible, symlinked, or special-file paths are typed errors.

Auxiliary source requirements:

- `thumbnail.jpg`, when present, is a nofollow regular file;
- `chat-sessions/`, when present, is a real directory under the source bundle;
- traversal accepts only real directories and regular files and rejects every
  symlink, socket, FIFO, device, or other special entry;
- only `NotFound` means optional absent. Permission, metadata, and I/O errors
  fail the archive.

External media requirements:

- missing external paths retain the current missing-media report;
- readable external paths are enumerated before the save panel;
- symlink targets are resolved for the confirmation summary and opened once for
  copying; the existing lexical dedup key remains unchanged, so two separately
  listed symlink paths still produce separate collected entries as upstream
  expects;
- Rust shows a native warning that the export will read files outside the
  project and reports the count plus representative paths/targets. Cancel
  returns no output and performs no writes;
- no external-source approval is inferred from project JSON or WebView state.

If an external path resolves to a different target between native confirmation
and source-handle acquisition, export aborts without output. It does not silently
extend the confirmation to the replacement target.

The copy implementation uses source file handles and destination `create_new`
handles. A separate check followed by path-based `fs::copy` is not sufficient.
Directory traversal must be descriptor/capability-relative or must prove the
opened entry identity has not changed. Tests introduce swaps at the check/open
seams.

### B. Build and publish split

Archive becomes three explicit phases. Source-only analysis may run before the
save panel so Rust can obtain external-source consent, but the immutable
preflight plan is complete only after a destination has been selected and all
approved source handles have been acquired:

1. `prepare`: validate compatibility, paths, relationships, source kinds, and
   external-source summary without creating output;
2. `build_into`: create an unpredictable, exclusive sibling staging directory
   under `dest.parent`, copy media/extras through the preflight plan, and write
   the three JSON documents into that empty stage;
3. `publish_new`: open the staged bundle through `Project::open`, verify its
   core documents and rewritten media references, then rename the complete
   stage once to the still-nonexistent destination.

Any prepare/build/validation failure removes only the stage and leaves source
and destination untouched. If a competing process creates the destination,
publication fails rather than replacing it. Staging is in the destination
parent so the final rename cannot cross filesystems. Output byte accounting
uses the successful copy return value rather than a second metadata lookup.

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
) -> Result<Option<BundleReportDto>, String>
```

Flow:

1. capture a lock-consistent snapshot including project epoch, path, and document
   version; `BundleExportSnapshot` gains the currently missing `version` field;
2. perform source-only authorization analysis and, when necessary, show the
   native external-source confirmation;
3. open the Rust `tauri-plugin-dialog` save panel with the safe default name;
4. cancel returns `Ok(None)`;
5. compare `AppCore::project_revision()` with the captured epoch/version and
   recapture the current path; any epoch, version, or path change aborts before
   output so the dialogs cannot authorize one revision and export another;
6. complete destination relationship checks, acquire the approved source
   handles, and reject any external target that differs from the confirmed
   resolution;
7. run build/publish in a blocking worker;
8. return the report with the exact published path.

The main WebView cannot supply or override the destination. MCP and Agent do
not gain bundle export in this slice. Any future Agent flow must use the same
Rust-owned native confirmation and may not expose a raw-path mutation tool.

The Web API becomes `exportBundle(): Promise<BundleReport | null>`. The bundle
branch removes its JavaScript save-panel/path construction. `null` is neutral
cancel. Existing errors continue to render in the export dialog.

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
- stage validation/publication failure.

User-facing errors state what remains safe: source unchanged, destination
unchanged/not created, and whether a fresh name is required. Detailed paths stay
in the export dialog error, not in transient success toasts or release logs.

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
- external source confirmation accept/cancel and symlink-target presentation are
  covered without changing lexical dedup semantics.

### Publication

- build failure after copied media preserves source and creates no destination;
- JSON serialization/write, thumbnail, chat, validation, and final-rename
  failures remove stage and preserve source;
- existing destination is byte-identical and returns `DestinationExists`;
- successful output contains no stale media or staging names and reopens through
  `Project::open`;
- stage is a sibling on the same filesystem;
- concurrent destination creation causes refusal, not replacement;
- check/open and entry-type/copy race hooks cannot redirect a copy outside the
  approved source.

### Tauri/Web

- the command has no `outPath` input and cancellation creates nothing;
- a project identity/revision change while the native panel is open creates
  nothing;
- `BundleExportSnapshot.version` and `project_revision()` are compared before
  the first output write;
- external-source confirmation is required before the save panel;
- an external target swap after confirmation aborts rather than copying the new
  target;
- Web invokes `export_bundle` with no raw destination argument;
- saved/unsaved/collision defaults are safe;
- bundle export remains available on an empty timeline while video remains
  disabled;
- success, missing media, cancellation, typed path rejection, and generic I/O
  errors render distinctly.

### C2 Save-As

- failed media/extras copy leaves no new bundle and does not change
  `project_dir`;
- source/destination alias or nesting fails before writes;
- a complete new bundle is validated and published once, then becomes the
  active `project_dir`;
- existing different destination remains byte-identical;
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
all internal/archive-extra paths stay inside the approved source, existing
destinations are never changed, and a successful fresh destination appears only
after a complete validated stage.

Wave 1B-C is accepted only after C2 also proves that Save-As publishes a complete
new bundle or nothing, never changes project identity on failure, and reuses the
same path and nofollow controls.
