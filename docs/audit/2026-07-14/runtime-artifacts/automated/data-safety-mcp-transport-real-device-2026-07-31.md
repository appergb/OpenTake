# Data safety MCP transport real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 2 `DS-mcp-transport`.
- Requirement: `requirement-473d4379da3bd4cc`.
- Boundary: loopback-only startup, DNS-rebinding guards, request-size limit, and MCP protocol-version validation.

## Exact code evidence

All four named HTTP integration tests existed at the initial baseline and passed independently:

- `non_local_origin_is_rejected`: PASS, 1/1.
- `oversized_request_body_is_rejected`: PASS, 1/1.
- `serve_rejects_non_loopback_bind`: PASS, 1/1.
- `unsupported_protocol_version_is_400`: PASS, 1/1.

The tests drive the actual axum/Streamable HTTP router. Their mutation counters prove rejected Origin, Host, protocol-version, and oversized-body requests never reach tool dispatch. The production implementation also rejects a caller-supplied non-loopback bind before creating a listener.

Because the reviewed production boundary was already complete, the initial baseline was GREEN; no artificial failing implementation was introduced merely to satisfy the generated RED wording.

## Packaged application result

The rebuilt debug macOS application exposed its MCP instructions in Settings, including the exact endpoint `http://127.0.0.1:19789/mcp`, client setup commands, and the visible statement that the server binds only to `127.0.0.1` while the application is running.

The running packaged process was then checked through its real TCP endpoint:

| Probe | Result |
|---|---|
| Listener inspection | `opentake` listening on `TCP 127.0.0.1:19789`, IPv4 loopback only |
| Valid MCP `initialize` with JSON + SSE accept types | HTTP 200, protocol `2025-06-18`, server `opentake` version `1.0.0` |
| Valid session `tools/list` | HTTP 200 and production tool catalog returned |
| `Origin: http://evil.example.com:19789` | HTTP 403 |
| `Host: 127.0.0.1.evil:19789` | HTTP 403 |
| `MCP-Protocol-Version: 1900-01-01` | HTTP 400 |
| Body larger than the 16 MiB request ceiling | HTTP 413 |

The live `tools/list` response included timeline reads, media inspection/search, clip/track edits, undo, text/captions, and beat-detection tools. This demonstrates that the valid path reached the packaged server while each invalid transport path was rejected at the boundary.

## Outcome

Task 2 is complete: the exact owning tests and the packaged TCP server agree on loopback binding and every required request guard. This closes one data-safety record only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
