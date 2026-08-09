# Data safety MCP redaction real-device evidence — 2026-07-31

## Scope

- Plan: `data-safety-implementation.md`, Task 4 `DS-mcp-redaction`.
- Requirement: `requirement-2e9e6066655d5846`.
- Boundary: typed actionable MCP errors without paths, credentials, authorization headers, signed queries, provider bodies, nested source detail, or internal stack content.

## Exact code evidence

The exact owning test `llm_errors_redact_paths_credentials_headers_provider_bodies` passed 1/1. The complete `mcp_error_redaction` runner passed 2/2, including the multi-block test that proves text and image blocks cannot be recombined to recover private content.

The matrix independently injects:

- a full user-home media path;
- an API-key-shaped value;
- bearer and Basic authorization values;
- a signed URL query;
- a provider/customer response body;
- nested decoder and stack detail;
- split multi-block text and image content.

Every wire result retains a typed code and retry remediation while removing the injected content.

## Packaged application result

The rebuilt debug macOS application's production MCP server received two real `import_media` calls containing the same adversarial categories:

1. A nonexistent `/Users/alice/.../sk-live-.../secret.mp4` path with an `Authorization: Bearer ...` display name.
2. A signed HTTPS URL containing `token=signed-secret&expires=999999`, with a provider-style customer/quota body in the display name.

Both operations failed as intended and returned HTTP 200 MCP envelopes whose tool results were marked `isError=true`. Response bodies contained `MCP_TOOL_ERROR_REDACTED`, a unique `errorId`, and actionable retry guidance. A byte-level search across both retained responses found none of these strings: the home path, API key, bearer token, authorization label, signed token, expiry, quota text, or customer identity.

The same packaged session had immediately before this check returned non-redacted success content for valid path, HTTPS, and bytes imports. This proves the sanitizer is conditional at the LLM boundary rather than suppressing all useful tool output.

## Outcome

Task 4 is complete: adversarial automated coverage and the packaged MCP wire agree on typed, actionable, fail-closed redaction. This closes one data-safety record only; it does not reclassify the remaining data-safety tasks or authorize Beta publication.
