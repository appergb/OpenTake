# OpenTake Motion Canvas Studio

Pinned browser bundle for OpenTake's first Motion Canvas template. The wrapper
uses Motion Canvas `v3.17.2`, builds one self-contained offline HTML document,
and lets OpenTake drive it with exact frame timestamps before encoding the
captured frames through the packaged FFmpeg.

Run `npm ci && npm audit --audit-level=moderate && npm run licenses && npm test
&& npm run build`. The release artifact is
`bundle/runner.html`; it intentionally contains the exact placeholder
`__OPENTAKE_MOTION_CONFIG_JSON__`, replaced by the Rust host with validated
template parameters before the document enters the network-disabled Chromium
sandbox.

This is a wrapper, not a source fork. See `LICENSE` and
`THIRD_PARTY_NOTICES.md` for upstream identity and modification notes.

The checked-in bundle is reproducible: CI rebuilds it from the lockfile and
fails if the resulting `bundle/runner.html` differs. Vite 7 is intentionally
used through a small target-compatibility shim because Motion Canvas 3.17.2's
declared Vite peer range predates the security-fixed Vite line.
