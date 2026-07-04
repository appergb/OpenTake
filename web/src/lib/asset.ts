/**
 * Local-file → webview-loadable URL via Tauri's asset protocol (enabled in
 * `tauri.conf.json` → `assetProtocol`). Lets `<img>`/`<video>`/`<audio>` show
 * imported media straight from disk — the pragmatic preview/thumbnail path
 * (WebKit/WebView2 decodes the original file) that mirrors upstream feeding an
 * `AVPlayerItem` a file URL, without a separate Rust thumbnail pipeline.
 *
 * `convertFileSrc` only builds a string, so a static import is safe in the
 * browser shell; we still gate on `isTauri` since the asset scheme only resolves
 * inside the Tauri WebView (and `path` is `null` for browser-fallback media).
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import { isTauri } from "./api";

/** Asset URL for a local absolute `path`, or `null` when unavailable. */
export function assetUrl(path: string | null | undefined): string | null {
  if (!path || !isTauri) return null;
  try {
    return convertFileSrc(path);
  } catch {
    return null;
  }
}
