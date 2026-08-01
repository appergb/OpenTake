import { useEffect, useRef, useState } from "react";
import { Focus, RotateCcw, Square } from "lucide-react";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import type { Clip, MotionTrackingRegion, MotionTrackingResult } from "../../lib/types";
import * as edit from "../../store/editActions";
import { useEditorUiStore } from "../../store/uiStore";
import { Icon } from "../ui/Icon";

type Phase = "idle" | "previewing" | "review" | "applying" | "applied";

const DEFAULT_REGION: MotionTrackingRegion = { x: 0.25, y: 0.25, width: 0.5, height: 0.5 };

export interface MotionTrackingDependencies {
  generate: (
    clipId: string,
    region: MotionTrackingRegion,
    range: { startFrame: number; endFrame: number },
    apply: boolean,
  ) => Promise<MotionTrackingResult>;
  cancel: () => Promise<boolean>;
  undo: () => Promise<unknown>;
}

const defaultDependencies: MotionTrackingDependencies = {
  generate: api.trackMotion,
  cancel: api.cancelAdvancedWorkflow,
  undo: edit.undo,
};

export function MotionTrackingSection({ clip, dependencies = defaultDependencies }: {
  clip: Clip;
  dependencies?: MotionTrackingDependencies;
}) {
  const t = useT();
  const clipEnd = clip.startFrame + clip.durationFrames;
  const selection = useEditorUiStore((state) => state.motionTrackingSelection);
  const setSelection = useEditorUiStore((state) => state.setMotionTrackingSelection);
  const [region, setRegion] = useState<MotionTrackingRegion>(DEFAULT_REGION);
  const [startFrame, setStartFrame] = useState(clip.startFrame);
  const [endFrame, setEndFrame] = useState(clipEnd);
  const [phase, setPhase] = useState<Phase>("idle");
  const [preview, setPreview] = useState<MotionTrackingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationRef = useRef(0);
  const compatible =
    clip.mediaType === "video" &&
    !clip.nestedSequenceId &&
    !clip.reversed &&
    Math.abs(clip.speed - 1) <= Number.EPSILON;
  const selecting = selection?.clipId === clip.id;

  useEffect(() => {
    operationRef.current += 1;
    setRegion(DEFAULT_REGION);
    setStartFrame(clip.startFrame);
    setEndFrame(clip.startFrame + clip.durationFrames);
    setPreview(null);
    setError(null);
    setPhase("idle");
  }, [clip.id, clip.startFrame, clip.durationFrames]);

  useEffect(() => {
    if (!selecting || !selection) return;
    setRegion(selection.region);
    operationRef.current += 1;
    setPreview(null);
    setError(null);
    setPhase("idle");
  }, [selecting, selection]);

  useEffect(
    () => () => {
      const active = useEditorUiStore.getState().motionTrackingSelection;
      if (active?.clipId === clip.id) setSelection(null);
    },
    [clip.id, setSelection],
  );

  function invalidatePreview() {
    operationRef.current += 1;
    setPreview(null);
    setError(null);
    setPhase("idle");
  }

  function updateRegion(next: MotionTrackingRegion) {
    const clamped = normalizeRegion(next);
    setRegion(clamped);
    if (selecting) setSelection({ clipId: clip.id, region: clamped });
    invalidatePreview();
  }

  function updateStartFrame(value: number) {
    setStartFrame(value);
    invalidatePreview();
  }

  function updateEndFrame(value: number) {
    setEndFrame(value);
    invalidatePreview();
  }

  async function run(apply: boolean) {
    if (!compatible || startFrame >= endFrame) return;
    const operation = operationRef.current + 1;
    operationRef.current = operation;
    setPhase(apply ? "applying" : "previewing");
    setError(null);
    try {
      const result = await dependencies.generate(
        clip.id,
        region,
        { startFrame, endFrame },
        apply,
      );
      if (operationRef.current !== operation) return;
      setPreview(result);
      setPhase(apply ? "applied" : "review");
      if (apply) setSelection(null);
    } catch (reason) {
      if (operationRef.current !== operation) return;
      setError(message(reason));
      setPhase(preview ? "review" : "idle");
    }
  }

  async function cancel() {
    operationRef.current += 1;
    await dependencies.cancel();
    setPhase(preview ? "review" : "idle");
  }

  async function undo() {
    setPhase("applying");
    try {
      await dependencies.undo();
      setPhase("review");
    } catch (reason) {
      setError(message(reason));
      setPhase("applied");
    }
  }

  const busy = phase === "previewing" || phase === "applying";
  return (
    <section data-testid="motion-tracking-section" style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)" }}>
      <div style={headingStyle}><Icon icon={Focus} size={11} />{t("inspector.motionTracking.heading")}</div>
      {!compatible && <div role="status" style={hintStyle}>{t("inspector.motionTracking.compatibility")}</div>}
      {compatible && (
        <>
          <div style={hintStyle}>{t("inspector.motionTracking.localPrivacy")}</div>
          <button
            type="button"
            disabled={busy || phase === "applied"}
            aria-pressed={selecting}
            onClick={() => setSelection(selecting ? null : { clipId: clip.id, region })}
            style={secondaryButtonStyle}
          >
            <Icon icon={Focus} size={12} />
            {selecting ? t("inspector.motionTracking.finishRegion") : t("inspector.motionTracking.selectRegion")}
          </button>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-xs)" }}>
            <PercentField label={t("inspector.motionTracking.x")} value={region.x} disabled={busy || phase === "applied"} onChange={(x) => updateRegion({ ...region, x })} />
            <PercentField label={t("inspector.motionTracking.y")} value={region.y} disabled={busy || phase === "applied"} onChange={(y) => updateRegion({ ...region, y })} />
            <PercentField label={t("inspector.motionTracking.width")} value={region.width} disabled={busy || phase === "applied"} onChange={(width) => updateRegion({ ...region, width })} />
            <PercentField label={t("inspector.motionTracking.height")} value={region.height} disabled={busy || phase === "applied"} onChange={(height) => updateRegion({ ...region, height })} />
            <FrameField label={t("inspector.motionTracking.startFrame")} value={startFrame} min={clip.startFrame} max={Math.max(clip.startFrame, endFrame - 2)} disabled={busy || phase === "applied"} onChange={updateStartFrame} />
            <FrameField label={t("inspector.motionTracking.endFrame")} value={endFrame} min={Math.min(clipEnd, startFrame + 2)} max={clipEnd} disabled={busy || phase === "applied"} onChange={updateEndFrame} />
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-xs)" }}>
            {phase === "applied" ? (
              <button type="button" onClick={() => void undo()} style={secondaryButtonStyle}><Icon icon={RotateCcw} size={12} />{t("inspector.motionTracking.undo")}</button>
            ) : (
              <>
                <button type="button" disabled={busy} onClick={() => void run(false)} style={secondaryButtonStyle}><Icon icon={Focus} size={12} />{phase === "previewing" ? t("inspector.motionTracking.processing") : error ? t("inspector.motionTracking.retry") : t("inspector.motionTracking.preview")}</button>
                <button type="button" disabled={!preview || busy} onClick={() => void run(true)} style={primaryButtonStyle}><Icon icon={Focus} size={12} />{phase === "applying" ? t("inspector.motionTracking.applying") : t("inspector.motionTracking.apply")}</button>
              </>
            )}
            {busy && <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}><Icon icon={Square} size={11} />{t("inspector.motionTracking.cancel")}</button>}
          </div>
        </>
      )}
      {preview && (
        <div role="status" style={{ ...hintStyle, color: "var(--status-success)" }}>
          {t("inspector.motionTracking.result", {
            confidence: Math.round(preview.result.minimumConfidence * 100),
            count: preview.result.keyframes.length,
          })}
        </div>
      )}
      {phase === "applied" && <div role="status" style={{ ...hintStyle, color: "var(--status-success)" }}>{t("inspector.motionTracking.applied")}</div>}
      {error && <div role="alert" style={{ ...hintStyle, color: "var(--status-error)" }}>{error}</div>}
    </section>
  );
}

function normalizeRegion(region: MotionTrackingRegion): MotionTrackingRegion {
  const width = Math.max(0.02, Math.min(1, region.width));
  const height = Math.max(0.02, Math.min(1, region.height));
  return {
    x: Math.max(0, Math.min(1 - width, region.x)),
    y: Math.max(0, Math.min(1 - height, region.y)),
    width,
    height,
  };
}

function PercentField({ label, value, disabled, onChange }: {
  label: string;
  value: number;
  disabled: boolean;
  onChange: (value: number) => void;
}) {
  return <FrameField label={label} value={Math.round(value * 100)} min={0} max={100} disabled={disabled} onChange={(next) => onChange(next / 100)} suffix="%" />;
}

function FrameField({ label, value, min, max, disabled, onChange, suffix }: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled: boolean;
  onChange: (value: number) => void;
  suffix?: string;
}) {
  return (
    <label style={fieldLabelStyle}>
      {label}
      <span style={{ display: "flex", alignItems: "center", gap: 3 }}>
        <input type="number" value={value} min={min} max={max} disabled={disabled} onChange={(event) => onChange(Number(event.currentTarget.value))} style={inputStyle} />
        {suffix && <span>{suffix}</span>}
      </span>
    </label>
  );
}

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

const headingStyle = { display: "flex", alignItems: "center", gap: "var(--space-xs)", fontSize: "var(--fs-xxs)", fontWeight: "var(--fw-semibold)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "var(--tracking-wide)" } as const;
const hintStyle = { color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const fieldLabelStyle = { display: "flex", flexDirection: "column", gap: 2, color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const inputStyle = { width: "100%", minWidth: 0, padding: "3px var(--space-xs)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-primary)", fontSize: "var(--fs-xs)" } as const;
const primaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", background: "var(--ai-gradient)", color: "#111", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" } as const;
const secondaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
