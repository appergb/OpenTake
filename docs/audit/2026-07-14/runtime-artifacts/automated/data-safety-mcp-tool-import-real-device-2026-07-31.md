# Data safety MCP tool import real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 3 `DS-mcp-tool-import`.
- Requirement: `requirement-d317ca3e45fba737`.
- Boundary: strict tool arguments plus path, inline-byte, and HTTPS media import with fail-closed staging and publication.

## Exact code evidence

Three dispatcher tests and the production URL-import test passed:

- `import_media_requires_exactly_one_source`: PASS, 1/1.
- `import_media_rejects_unknown_nested_source_key`: PASS, 1/1.
- `import_media_bytes_rejects_oversized_base64_before_bridge`: PASS, 1/1.
- `https_url_import_enforces_scheme_mime_and_decoded_limit`: PASS, 1/1.

The initial command for `all_tool_schemas_reject_unknown_missing_wrong_type -- --exact` reported success but executed zero tests because the owning test had an extra `_and_nonfinite` suffix. The test was renamed to the exact plan-declared name without weakening its assertions; the corrected command then passed 1/1 and still covers non-finite numbers in addition to unknown, missing, wrong-root, wrong-type, and nested-field cases.

The URL test covers HTTPS/userinfo/redirect restrictions, response and override MIME resolution, extension/container conflicts, Content-Length and streamed decoded-byte ceilings, cancellation, probe failure, manifest-writer failure, retained staging identity, successful publication, and reopen persistence.

## Packaged application result

The rebuilt debug macOS application opened the retained Task 3 fixture and exposed the production MCP bridge at `127.0.0.1:19789`.

Invalid live calls used no source, two sources, an unknown nested source key, and a plain-HTTP URL. Every request returned an MCP error and the media manifest remained at one entry. The unknown nested key was reported as `MCP_INVALID_ARGUMENTS`; private operational failures were redacted with an error identifier.

The valid paths then succeeded through the same live server:

| Source | Runtime result |
|---|---|
| Absolute path WAV | Imported as `Task3 Path Music`; one-second waveform appeared in the media panel |
| HTTPS PNG | Downloaded from `raw.githubusercontent.com`, probed as `image/png`, retained 12,746 bytes in the project, and appeared as `Task3 HTTPS Rust` |
| Base64 PNG | Decoded 68 bytes, retained in the project, and appeared as `Task3 Inline PNG` |

An initial Wikimedia image request received a source-server HTTP 403. OpenTake returned a redacted failure and left the manifest unchanged. Repeating the acceptance run with an HTTPS source that permits programmatic downloads succeeded, distinguishing external rejection from an import defect.

After returning Home to save, `media.json` contained four entries: the original offline fixture, the external path WAV, and two project-retained images. The retained files were present at their declared relative paths:

| Artifact | SHA-256 |
|---|---|
| HTTPS PNG | `cf78cef9ba96a43bda7254c0ffb10b9faffd075997ca6b7bd89df1c64e3c5605` |
| Inline PNG | `e4bff65a73fef402fa8fbd4f4cc20d774df30ad45315617efec31455aa3fe1f0` |

## Outcome

Task 3 is complete: exact schemas, failure isolation, three real import modes, retained publication, disk persistence, and the visible media panel agree. This closes one data-safety record only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
