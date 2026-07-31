# Data safety cross-cutting security partial evidence — 2026-07-31

## Status

Task 10 `DS-cross-cutting-security-headings` remains **open**. This checkpoint closes the concrete CSP, asset-scope, packaged-sidecar supply, and offline WebView2 configuration defects discovered during audit, but does not claim the task's Windows installed-package or release-signing criteria.

## Defect found and corrected

The audited `src-tauri/tauri.conf.json` had `csp: null` and asset scope `['**']`. That allowed the WebView asset protocol to request arbitrary readable paths and left production content loading without an explicit CSP.

The corrected packaged boundary now has:

- production CSP with `default-src 'self'`, `object-src 'none'`, `frame-ancestors 'none'`, no remote HTTPS source, and only loopback HTTP for the native preview transport;
- separate development CSP for local Vite WebSocket traffic, so production does not retain a WebSocket allowance;
- asset protocol static scope limited to app cache, app-owned global-library data, and packaged resources;
- explicit deny precedence for `.ssh`, `.gnupg`, and `.aws` home trees;
- native dialog runtime grants persisted across restart via `tauri-plugin-persisted-scope` with its `protocol-asset` feature;
- no shell, filesystem, HTTP, or process plugin command permission exposed to the main WebView.
- the Windows NSIS/MSI bundle embeds the silent WebView2 offline installer, so a fresh installation does not depend on network access;
- each Windows CI job that compiles the Tauri crate provisions the checksum- and version-pinned FFmpeg/FFprobe sidecars before compilation.

## Automated evidence

`src-tauri/tests/security_config.rs` proves:

- packaged CSP is enabled, local-only, and carries the required deny directives;
- asset scope contains exactly the three application-owned allow patterns and no global/home wildcard;
- the main capability contains no shell/fs/http/process command permission;
- persisted-scope is enabled for the asset protocol and initializes after the required fs plugin.
- the Windows platform bundle selects `offlineInstaller` with silent installation;
- all three Windows Tauri CI jobs provision the pinned sidecars before their first Tauri compile/build step.

Result: PASS, 6/6. The four existing cross-cutting owning tests also pass 1/1 each, the Web suite passes 82 files / 774 tests, and the complete Rust workspace regression passed after the hardening patch.

## Packaged application evidence

The exact hardened tree built successfully with:

`./web/node_modules/.bin/tauri build --debug --bundles app`

The rebuilt `/target/debug/bundle/macos/OpenTake.app`:

1. launched to Home under the production CSP;
2. opened the legacy project through the native directory picker;
3. visibly rendered the persisted text, four media entries, a waveform cache, and the project-relative HTTPS-imported image;
4. exited and relaunched;
5. reopened the same project from Recents without another picker;
6. again rendered the same timeline and local-resource surfaces.

The persisted runtime files `.persisted-scope` and `.persisted-scope-asset` exist under the application's support directory after the picker/restart cycle.

## Remaining Task 10 blockers

- The macOS package contains and has executed its pinned FFmpeg/FFprobe sidecars without ambient `PATH`; the corresponding Windows installed-package receipt is still pending.
- Native Windows offline WebView2 installation and packaged-app smoke evidence is not yet attached on the final tree.
- macOS distribution identity signing and Apple notarization evidence is not yet available; the current debug `.app` is only a local QA artifact.

Accordingly, no Task 10 plan checkbox is marked complete and this checkpoint does not authorize Beta publication.
