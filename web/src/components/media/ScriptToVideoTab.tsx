import { useEffect, useMemo, useRef, useState } from "react";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import type { ScriptToVideoResult, ScriptToVideoSegmentInput } from "../../lib/types";
import * as edit from "../../store/editActions";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";
import { RADIUS, SPACE } from "../../lib/theme";

type Phase = "idle" | "planning" | "review" | "applying" | "applied";

export interface ScriptToVideoDependencies {
  run: (segments: ScriptToVideoSegmentInput[], apply: boolean) => Promise<ScriptToVideoResult>;
  cancel: () => Promise<boolean>;
  undo: () => Promise<unknown>;
}

const defaultDependencies: ScriptToVideoDependencies = {
  run: api.scriptToVideo,
  cancel: api.cancelAdvancedWorkflow,
  undo: edit.undo,
};

function emptySegment(mediaRef = ""): ScriptToVideoSegmentInput {
  return { script: "", mediaRef, durationFrames: 90, transition: "crossDissolve" };
}

export function ScriptToVideoTab({ dependencies = defaultDependencies }: { dependencies?: ScriptToVideoDependencies }) {
  const t = useT();
  const items = useMediaStore((state) => state.items);
  const fps = useProjectStore((state) => state.timeline.fps);
  const visuals = useMemo(() => items.filter((item) => item.type === "image" || item.type === "video" || item.type === "lottie"), [items]);
  const narrations = useMemo(() => items.filter((item) => (item.type === "audio" || item.type === "video") && item.hasAudio), [items]);
  const [segments, setSegments] = useState<ScriptToVideoSegmentInput[]>(() => [emptySegment(), emptySegment(), { ...emptySegment(), transition: undefined }]);
  const [phase, setPhase] = useState<Phase>("idle");
  const [result, setResult] = useState<ScriptToVideoResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);

  useEffect(() => {
    const first = visuals[0]?.id ?? "";
    setSegments((current) => current.map((segment) => visuals.some((item) => item.id === segment.mediaRef) ? segment : { ...segment, mediaRef: first }));
  }, [visuals]);

  const revise = (index: number, patch: Partial<ScriptToVideoSegmentInput>) => {
    operation.current += 1;
    setSegments((current) => current.map((segment, cursor) => cursor === index ? { ...segment, ...patch } : segment));
    setResult(null);
    setError(null);
    setPhase("idle");
  };

  const run = async (apply: boolean) => {
    const request = operation.current + 1;
    operation.current = request;
    setPhase(apply ? "applying" : "planning");
    setError(null);
    try {
      const next = await dependencies.run(segments, apply);
      if (operation.current !== request) return;
      setResult(next);
      setPhase(apply ? "applied" : "review");
    } catch (reason) {
      if (operation.current !== request) return;
      setError(reason instanceof Error ? reason.message : String(reason));
      setPhase(result ? "review" : "idle");
    }
  };

  const cancel = async () => {
    operation.current += 1;
    await dependencies.cancel();
    setPhase(result ? "review" : "idle");
  };

  const undo = async () => {
    setPhase("applying");
    try {
      await dependencies.undo();
      setPhase("review");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setPhase("applied");
    }
  };

  const ready = segments.length > 0 && segments.every((segment) => segment.script.trim() && segment.mediaRef && segment.durationFrames > 0);
  const busy = phase === "planning" || phase === "applying";
  return (
    <div data-testid="script-to-video-tab" style={{ height: "100%", overflowY: "auto", padding: SPACE.mdLg, display: "flex", flexDirection: "column", gap: SPACE.md }}>
      <div style={{ color: "var(--text-secondary)", fontSize: "var(--fs-sm)" }}>{t("scriptVideo.description")}</div>
      {segments.map((segment, index) => (
        <section key={index} style={{ padding: SPACE.md, border: "var(--bw-thin) solid var(--border-subtle)", borderRadius: RADIUS.md, display: "flex", flexDirection: "column", gap: SPACE.sm }}>
          <div style={{ display: "flex", justifyContent: "space-between", color: "var(--text-secondary)", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" }}>
            <span>{t("scriptVideo.segment", { number: index + 1 })}</span>
            {segments.length > 1 && <button type="button" disabled={busy} onClick={() => { const next = segments.filter((_, cursor) => cursor !== index).map((item, cursor, all) => cursor === all.length - 1 ? { ...item, transition: undefined } : item); setSegments(next); setResult(null); setPhase("idle"); }}>{t("scriptVideo.remove")}</button>}
          </div>
          <textarea aria-label={t("scriptVideo.script", { number: index + 1 })} value={segment.script} disabled={busy || phase === "applied"} placeholder={t("scriptVideo.scriptPlaceholder")} onChange={(event) => revise(index, { script: event.currentTarget.value })} style={{ ...inputStyle, minHeight: 58, resize: "vertical" }} />
          <label style={labelStyle}>{t("scriptVideo.visual")}
            <select value={segment.mediaRef} disabled={busy || phase === "applied"} onChange={(event) => revise(index, { mediaRef: event.currentTarget.value })} style={inputStyle}>
              <option value="">{t("scriptVideo.chooseVisual")}</option>
              {visuals.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
            </select>
          </label>
          <label style={labelStyle}>{t("scriptVideo.narration")}
            <select value={segment.narrationMediaRef ?? ""} disabled={busy || phase === "applied"} onChange={(event) => { const narration = narrations.find((item) => item.id === event.currentTarget.value); revise(index, { narrationMediaRef: narration?.id || undefined, ...(narration && narration.duration > 0 ? { durationFrames: Math.max(1, Math.round(narration.duration * Math.max(1, fps))) } : {}) }); }} style={inputStyle}>
              <option value="">{t("scriptVideo.noNarration")}</option>
              {narrations.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
            </select>
          </label>
          <label style={labelStyle}>{t("scriptVideo.duration")}
            <input type="number" min={1} max={36000} value={segment.durationFrames} disabled={busy || phase === "applied"} onChange={(event) => revise(index, { durationFrames: Math.max(1, Number(event.currentTarget.value) || 1) })} style={inputStyle} />
          </label>
          {index < segments.length - 1 && <label style={{ ...labelStyle, flexDirection: "row", alignItems: "center" }}><input type="checkbox" checked={segment.transition === "crossDissolve"} disabled={busy || phase === "applied"} onChange={(event) => revise(index, { transition: event.currentTarget.checked ? "crossDissolve" : undefined })} />{t("scriptVideo.transition")}</label>}
        </section>
      ))}
      {phase !== "applied" && <button type="button" disabled={busy || visuals.length === 0} onClick={() => { setSegments((current) => [...current.map((segment) => ({ ...segment, transition: segment.transition ?? "crossDissolve" })), { ...emptySegment(visuals[0]?.id), transition: undefined }]); setResult(null); setPhase("idle"); }} style={secondaryButtonStyle}>{t("scriptVideo.addSegment")}</button>}
      {result && <div style={{ padding: SPACE.sm, borderRadius: RADIUS.sm, background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" }}>{t("scriptVideo.planSummary", { count: result.result.segments.length, start: result.result.startFrame, end: result.result.endFrame })}<br />{result.result.planId}</div>}
      <div style={{ display: "flex", flexWrap: "wrap", gap: SPACE.sm }}>
        {phase === "applied" ? <button type="button" onClick={() => void undo()} style={primaryButtonStyle}>{t("scriptVideo.undo")}</button> : <>
          <button type="button" disabled={!ready || busy || visuals.length === 0} onClick={() => void run(false)} style={secondaryButtonStyle}>{phase === "planning" ? t("scriptVideo.planning") : error ? t("scriptVideo.retry") : t("scriptVideo.review")}</button>
          <button type="button" disabled={!result || busy || phase !== "review"} onClick={() => void run(true)} style={primaryButtonStyle}>{phase === "applying" ? t("scriptVideo.applying") : t("scriptVideo.apply")}</button>
        </>}
        {busy && <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>{t("scriptVideo.cancel")}</button>}
      </div>
      {phase === "applied" && <div role="status" style={{ color: "var(--status-success)", fontSize: "var(--fs-xs)" }}>{t("scriptVideo.applied")}</div>}
      {visuals.length === 0 && <div role="status" style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>{t("scriptVideo.noVisuals")}</div>}
      {error && <div role="alert" style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>{error}</div>}
    </div>
  );
}

const labelStyle = { display: "flex", flexDirection: "column", gap: 3, color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const inputStyle = { width: "100%", boxSizing: "border-box", padding: "5px var(--space-sm)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "var(--fs-xs)" } as const;
const primaryButtonStyle = { minHeight: 28, padding: "4px var(--space-md)", borderRadius: "var(--radius-sm)", background: "var(--ai-gradient)", color: "#111", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" } as const;
const secondaryButtonStyle = { minHeight: 28, padding: "4px var(--space-md)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
