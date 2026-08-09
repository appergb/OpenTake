/**
 * Local-file → webview-loadable URL via OpenTake's bounded native protocol.
 * The protocol opens a retained no-recall/non-blocking handle off the UI
 * thread, authorizes that handle's final path, and serves bounded byte ranges,
 * so File Provider placeholders or a path replaced by a FIFO/symlink cannot
 * freeze WebKit/WebView2 or escape the native scope.
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
    return convertFileSrc(path, "opentake-asset");
  } catch {
    return null;
  }
}
