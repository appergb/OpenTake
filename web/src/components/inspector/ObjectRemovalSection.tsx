import { useEffect, useMemo, useRef, useState } from "react";
import { Eraser, RotateCcw, Square } from "lucide-react";
import { assetUrl } from "../../lib/asset";
import * as api from "../../lib/api";
import type { Clip, RemoveObjectResult } from "../../lib/types";
import { useT } from "../../i18n";
import * as edit from "../../store/editActions";
import { Icon } from "../ui/Icon";

type Phase = "idle" | "previewing" | "review" | "applying" | "applied";

export interface ObjectRemovalDependencies {
  generate: (
    clipId: string,
    apply: boolean,
    range: { startFrame: number; endFrame: number },
  ) => Promise<RemoveObjectResult>;
  cancel: () => Promise<boolean>;
  undo: () => Promise<unknown>;
}

const defaultDependencies: ObjectRemovalDependencies = {
  generate: api.removeObject,
  cancel: api.cancelAdvancedWorkflow,
  undo: edit.undo,
};

export function ObjectRemovalSection({
  clip,
  dependencies = defaultDependencies,
}: {
  clip: Clip;
  dependencies?: ObjectRemovalDependencies;
}) {
  const t = useT();
  const clipEnd = clip.startFrame + clip.durationFrames;
  const [phase, setPhase] = useState<Phase>("idle");
  const [startFrame, setStartFrame] = useState(clip.startFrame);
  const [endFrame, setEndFrame] = useState(clipEnd);
  const [preview, setPreview] = useState<RemoveObjectResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationRef = useRef(0);
  const compatible =
    clip.mediaType === "video" &&
    !clip.nestedSequenceId &&
    !clip.reversed &&
    Math.abs(clip.speed - 1) <= Number.EPSILON;
  const hasMask = (clip.masks?.length ?? 0) > 0;
  const previewUrl = useMemo(
    () => assetUrl(preview?.result.previewPath),
    [preview?.result.previewPath],
  );
  const maskFingerprint = JSON.stringify(clip.masks?.[0] ?? null);

  useEffect(() => {
    operationRef.current += 1;
    setStartFrame(clip.startFrame);
    setEndFrame(clip.startFrame + clip.durationFrames);
    setPreview(null);
    setError(null);
    setPhase("idle");
  }, [clip.id, clip.startFrame, clip.durationFrames]);

  useEffect(() => {
    if (phase === "applying" || phase === "applied") return;
    operationRef.current += 1;
    setPreview(null);
    setError(null);
    setPhase("idle");
    // A mask edit invalidates the reviewed derivative. Apply must always use a
    // cache generated from the currently visible mask.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [maskFingerprint]);

  async function run(apply: boolean) {
    if (!compatible || !hasMask || startFrame >= endFrame) return;
    const operation = operationRef.current + 1;
    operationRef.current = operation;
    setPhase(apply ? "applying" : "previewing");
    setError(null);
    try {
      const result = await dependencies.generate(clip.id, apply, { startFrame, endFrame });
      if (operationRef.current !== operation) return;
      setPreview(result);
      setPhase(apply ? "applied" : "review");
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
    if (phase !== "applied") return;
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
    <section
      data-testid="object-removal-section"
      style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)" }}
    >
      <div style={headingStyle}>
        <Icon icon={Eraser} size={11} />
        {t("inspector.objectRemoval.heading")}
      </div>
      {!compatible && <div role="status" style={hintStyle}>{t("inspector.objectRemoval.compatibility")}</div>}
      {compatible && !hasMask && phase !== "applied" && (
        <div role="status" style={hintStyle}>{t("inspector.objectRemoval.maskNeeded")}</div>
      )}
      {compatible && (hasMask || phase === "applied") && (
        <>
          <div style={hintStyle}>{t("inspector.objectRemoval.localPrivacy")}</div>
          {phase !== "applied" && (
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-xs)" }}>
              <FrameField
                label={t("inspector.objectRemoval.startFrame")}
                value={startFrame}
                min={clip.startFrame}
                max={Math.max(clip.startFrame, endFrame - 1)}
                disabled={busy}
                onChange={setStartFrame}
              />
              <FrameField
                label={t("inspector.objectRemoval.endFrame")}
                value={endFrame}
                min={Math.min(clipEnd, startFrame + 1)}
                max={clipEnd}
                disabled={busy}
                onChange={setEndFrame}
              />
            </div>
          )}
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-xs)" }}>
            {phase === "applied" ? (
              <button type="button" onClick={() => void undo()} style={secondaryButtonStyle}>
                <Icon icon={RotateCcw} size={12} />
                {t("inspector.objectRemoval.undo")}
              </button>
            ) : (
              <>
                <button type="button" disabled={busy} onClick={() => void run(false)} style={secondaryButtonStyle}>
                  <Icon icon={Eraser} size={12} />
                  {phase === "previewing"
                    ? t("inspector.objectRemoval.processing")
                    : error
                      ? t("inspector.objectRemoval.retry")
                      : t("inspector.objectRemoval.preview")}
                </button>
                <button type="button" disabled={!preview || busy} onClick={() => void run(true)} style={primaryButtonStyle}>
                  <Icon icon={Eraser} size={12} />
                  {phase === "applying" ? t("inspector.objectRemoval.applying") : t("inspector.objectRemoval.apply")}
                </button>
              </>
            )}
            {busy && (
              <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>
                <Icon icon={Square} size={11} />
                {t("inspector.objectRemoval.cancel")}
              </button>
            )}
          </div>
        </>
      )}
      {previewUrl && phase !== "applied" && (
        <video
          key={previewUrl}
          src={previewUrl}
          controls
          playsInline
          aria-label={t("inspector.objectRemoval.previewLabel")}
          style={previewStyle}
        />
      )}
      {phase === "applied" && <div role="status" style={{ ...hintStyle, color: "var(--status-success)" }}>{t("inspector.objectRemoval.applied")}</div>}
      {error && <div role="alert" style={{ ...hintStyle, color: "var(--status-error)" }}>{error}</div>}
    </section>
  );
}

function FrameField({ label, value, min, max, disabled, onChange }: {
  label: string;
  value: number;
  min: number;
  max: number;
  disabled: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 2, color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
      {label}
      <input
        type="number"
        value={value}
        min={min}
        max={max}
        disabled={disabled}
        onChange={(event) => onChange(Number(event.currentTarget.value))}
        style={inputStyle}
      />
    </label>
  );
}

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

const headingStyle = { display: "flex", alignItems: "center", gap: "var(--space-xs)", fontSize: "var(--fs-xxs)", fontWeight: "var(--fw-semibold)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "var(--tracking-wide)" } as const;
const hintStyle = { color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const inputStyle = { minWidth: 0, padding: "3px var(--space-xs)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-primary)", fontSize: "var(--fs-xs)" } as const;
const primaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", background: "var(--ai-gradient)", color: "#111", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" } as const;
const secondaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
const previewStyle = { display: "block", width: "100%", maxHeight: 180, objectFit: "contain", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-prominent)" } as const;
