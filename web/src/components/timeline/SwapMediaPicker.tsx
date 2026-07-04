/**
 * SwapMediaPicker (SPEC §5.10). Modal media picker shown when the user invokes
 * Swap Media from the clip context menu. Lists library assets whose `type`
 * strictly equals the target clip's `mediaType` (1:1 with upstream
 * `isAssetCompatibleWithPendingSwap`), so type mismatch is prevented at the UI
 * layer; the backend re-validates as a safety net. On selection, fires
 * `edit.swapMedia` (which preserves trim/speed/keyframes/transform —
 * `resetTrim=false` semantics) and cascades to linked clips sharing the same
 * old mediaRef.
 */

import { useEffect, useMemo, useState } from "react";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";
import { clipDisplayName } from "../../lib/clip";
import { formatTimecode } from "../../lib/geometry";
import type { Clip, MediaItem } from "../../lib/types";

export function SwapMediaPicker() {
  const t = useT();
  const pendingSwapClipId = useEditorUiStore((s) => s.pendingSwapClipId);
  const setPendingSwapClipId = useEditorUiStore((s) => s.setPendingSwapClipId);
  const timeline = useProjectStore((s) => s.timeline);
  const items = useMediaStore((s) => s.items);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fps = timeline.fps;

  // Resolve the target clip from the pending id.
  const clip: Clip | null = useMemo(() => {
    if (!pendingSwapClipId) return null;
    for (const track of timeline.tracks) {
      const found = track.clips.find((c) => c.id === pendingSwapClipId);
      if (found) return found;
    }
    return null;
  }, [pendingSwapClipId, timeline]);

  // Close on Escape.
  useEffect(() => {
    if (!clip) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPendingSwapClipId(null);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [clip, setPendingSwapClipId]);

  if (!clip) return null;

  // Pre-filter candidates by strict type equality (backend re-validates).
  const candidates: MediaItem[] = items.filter(
    (m) => m.type === clip.mediaType && m.id !== clip.mediaRef,
  );

  async function pick(item: MediaItem) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await edit.swapMedia(clip!.id, item.id);
      setPendingSwapClipId(null);
    } catch (e) {
      // Backend refuses on type mismatch / missing clip (EditError::Refused).
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.5)",
      }}
      onClick={() => setPendingSwapClipId(null)}
    >
      <div
        style={{
          minWidth: 320,
          maxWidth: 480,
          maxHeight: "60vh",
          display: "flex",
          flexDirection: "column",
          background: "var(--bg-elevated)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 8,
          boxShadow: "0 12px 32px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          style={{
            padding: "10px 14px",
            borderBottom: "var(--bw-thin) solid var(--border-primary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <span style={{ fontSize: "var(--fs-sm)", fontWeight: 600 }}>
            {t("contextMenu.swapMedia")} · {clipDisplayName(clip)}
          </span>
          <button
            onClick={() => setPendingSwapClipId(null)}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-secondary)",
              cursor: "pointer",
              fontSize: 16,
              padding: "0 4px",
            }}
            aria-label="close"
          >
            ×
          </button>
        </div>

        {error && (
          <div
            style={{
              padding: "8px 14px",
              fontSize: "var(--fs-sm)",
              color: "var(--accent-danger, #ff6b6b)",
              background: "rgba(255,107,107,0.08)",
            }}
          >
            {error}
          </div>
        )}

        <div style={{ overflowY: "auto", flex: 1 }}>
          {candidates.length === 0 ? (
            <div
              style={{
                padding: "20px 14px",
                color: "var(--text-disabled, rgba(255,255,255,0.35))",
                fontSize: "var(--fs-sm)",
                textAlign: "center",
              }}
            >
              {t("swapMedia.noCandidates")}
            </div>
          ) : (
            candidates.map((m) => (
              <button
                key={m.id}
                disabled={busy}
                onClick={() => pick(m)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  width: "100%",
                  padding: "8px 14px",
                  textAlign: "left",
                  color: "var(--text-primary)",
                  background: "transparent",
                  border: "none",
                  borderBottom: "var(--bw-thin) solid var(--border-primary)",
                  cursor: busy ? "wait" : "pointer",
                  fontFamily: "var(--font-sans)",
                  fontSize: "var(--fs-sm)",
                }}
                onMouseEnter={(e) => {
                  if (!busy)
                    e.currentTarget.style.background =
                      "var(--bg-hover, rgba(255,255,255,0.08))";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                }}
              >
                <span
                  style={{
                    flex: 1,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {m.name}
                </span>
                <span
                  style={{
                    color: "var(--text-secondary)",
                    fontSize: "var(--fs-xs)",
                  }}
                >
                  {m.duration > 0
                    ? formatTimecode(Math.round(m.duration * fps), fps)
                    : ""}
                </span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
