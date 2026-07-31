import { useEffect, useMemo, useState } from "react";
import { Blend, Trash2 } from "lucide-react";
import { useT } from "../../i18n";
import { findLogicalSingleClip } from "../../lib/clip";
import type { Clip, Timeline, TransitionKind } from "../../lib/types";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { setTransition } from "../../store/editActions";
import { Icon } from "../ui/Icon";

export interface TransitionPair {
  from: Clip;
  to: Clip;
  maximumDurationFrames: number;
}

export function resolveTransitionPair(
  timeline: Timeline,
  selectedClipIds: Set<string>,
): TransitionPair | null {
  const selected = findLogicalSingleClip(timeline, selectedClipIds);
  if (!selected || selected.mediaType === "audio" || selected.mediaType === "text") return null;
  const selectedId = selected.id;
  for (const track of timeline.tracks) {
    if (track.type === "audio") continue;
    const clips = track.clips
      .filter((clip) => clip.mediaType !== "audio" && clip.mediaType !== "text")
      .slice()
      .sort((a, b) => a.startFrame - b.startFrame || a.id.localeCompare(b.id));
    const selectedIndex = clips.findIndex((clip) => clip.id === selectedId);
    if (selectedIndex < 0) continue;
    const candidates: Array<[Clip | undefined, Clip | undefined]> = [
      [clips[selectedIndex], clips[selectedIndex + 1]],
      [clips[selectedIndex - 1], clips[selectedIndex]],
    ];
    for (const [from, to] of candidates) {
      if (!from || !to || from.startFrame + from.durationFrames !== to.startFrame) continue;
      return {
        from,
        to,
        maximumDurationFrames: Math.max(
          1,
          Math.floor(Math.min(from.durationFrames, to.durationFrames) / 2),
        ),
      };
    }
  }
  return null;
}

export function TransitionTab({
  onApply = setTransition,
}: {
  onApply?: (
    fromClipId: string,
    toClipId: string,
    kind: TransitionKind | null,
    durationFrames: number,
  ) => Promise<unknown>;
}) {
  const t = useT();
  const timeline = useProjectStore((state) => state.timeline);
  const selectedClipIds = useEditorUiStore((state) => state.selectedClipIds);
  const pair = useMemo(
    () => resolveTransitionPair(timeline, selectedClipIds),
    [selectedClipIds, timeline],
  );
  const [durationFrames, setDurationFrames] = useState(() => Math.max(1, Math.round(timeline.fps / 2)));
  const [pending, setPending] = useState(false);
  const [feedback, setFeedback] = useState<{
    transitionKind: TransitionKind | null;
    message: string;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!pair) return;
    const current = pair.from.transitionOut;
    setDurationFrames(
      Math.min(
        pair.maximumDurationFrames,
        Math.max(1, current?.toClipId === pair.to.id ? current.durationFrames : Math.round(timeline.fps / 2)),
      ),
    );
  }, [pair?.from.id, pair?.from.transitionOut?.durationFrames, pair?.to.id, pair?.maximumDurationFrames, timeline.fps]);

  useEffect(() => {
    setFeedback(null);
    setError(null);
  }, [pair?.from.id, pair?.to.id]);

  async function update(kind: TransitionKind | null) {
    if (!pair || pending) return;
    setPending(true);
    setFeedback(null);
    setError(null);
    try {
      await onApply(pair.from.id, pair.to.id, kind, durationFrames);
      setFeedback({
        transitionKind: kind,
        message:
          kind === null ? t("media.transition.removed") : t("media.transition.applied"),
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setPending(false);
    }
  }

  if (!pair) {
    return (
      <div
        data-testid="transition-tab"
        role="status"
        onMouseDown={(event) => event.stopPropagation()}
        style={{ flex: 1, display: "grid", placeItems: "center", padding: "var(--space-lg)", color: "var(--text-tertiary)", fontSize: "var(--fs-sm)", textAlign: "center" }}
      >
        {t("media.transition.selectCut")}
      </div>
    );
  }

  const current = pair.from.transitionOut?.toClipId === pair.to.id ? pair.from.transitionOut : null;
  const currentFeedback =
    feedback &&
    (feedback.transitionKind === null
      ? current === null
      : current?.kind === feedback.transitionKind)
      ? feedback.message
      : null;
  return (
    <div
      data-testid="transition-tab"
      onMouseDown={(event) => event.stopPropagation()}
      style={{ display: "flex", flexDirection: "column", gap: "var(--space-md)", padding: "var(--space-md)" }}
    >
      <div style={{ color: "var(--text-secondary)", fontSize: "var(--fs-xs)" }}>
        {pair.from.id} → {pair.to.id}
      </div>
      <button
        type="button"
        aria-pressed={current?.kind === "crossDissolve"}
        onClick={() => void update("crossDissolve")}
        disabled={pending}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-sm)",
          padding: "var(--space-sm-md)",
          borderRadius: "var(--radius-md)",
          border:
            current?.kind === "crossDissolve"
              ? "var(--bw-thin) solid var(--accent-primary)"
              : "var(--bw-thin) solid var(--border-primary)",
          background: "var(--bg-raised)",
          color: "var(--text-primary)",
          textAlign: "left",
        }}
      >
        <Icon icon={Blend} size={16} />
        <span>
          <strong style={{ display: "block", fontSize: "var(--fs-sm)" }}>
            {t("media.transition.crossDissolve")}
          </strong>
          <span style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
            {t("media.transition.crossDissolveDescription")}
          </span>
        </span>
      </button>

      <label style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <span style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
          {t("media.transition.duration", {
            frames: durationFrames,
            seconds: (durationFrames / Math.max(1, timeline.fps)).toFixed(2),
          })}
        </span>
        <input
          type="range"
          min={1}
          max={pair.maximumDurationFrames}
          value={durationFrames}
          onChange={(event) => setDurationFrames(Number(event.target.value))}
          disabled={pending}
          aria-label={t("media.transition.durationLabel")}
        />
      </label>

      <div style={{ display: "flex", gap: "var(--space-xs)" }}>
        <button
          type="button"
          data-action="apply-transition"
          disabled={pending}
          onClick={() => void update("crossDissolve")}
          style={primaryButtonStyle}
        >
          <Icon icon={Blend} size={12} />
          {pending ? t("media.transition.applying") : t("media.transition.apply")}
        </button>
        {current && (
          <button
            type="button"
            data-action="remove-transition"
            disabled={pending}
            onClick={() => void update(null)}
            style={secondaryButtonStyle}
          >
            <Icon icon={Trash2} size={12} />
            {t("media.transition.remove")}
          </button>
        )}
      </div>
      {currentFeedback && <div role="status" style={{ color: "var(--status-success)", fontSize: "var(--fs-xs)" }}>{currentFeedback}</div>}
      {error && <div role="alert" style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>{error}</div>}
    </div>
  );
}

const primaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  minHeight: 24,
  padding: "2px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  border: "none",
  background: "var(--accent-primary)",
  color: "#111",
  fontSize: "var(--fs-xs)",
  fontWeight: "var(--fw-semibold)",
} as const;

const secondaryButtonStyle = {
  ...primaryButtonStyle,
  border: "var(--bw-thin) solid var(--border-primary)",
  background: "var(--bg-prominent)",
  color: "var(--text-primary)",
} as const;
