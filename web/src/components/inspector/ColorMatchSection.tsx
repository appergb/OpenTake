import { useEffect, useMemo, useRef, useState } from "react";
import { Palette, RotateCcw, Square } from "lucide-react";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import type { Clip, MatchColorResult, Rgb } from "../../lib/types";
import * as edit from "../../store/editActions";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import { Icon } from "../ui/Icon";

type Phase = "idle" | "previewing" | "review" | "applying" | "applied";

export interface ColorMatchDependencies {
  generate: (
    clipId: string,
    referenceMediaRef: string,
    referenceFrame: number,
    targetFrame: number,
    apply: boolean,
  ) => Promise<MatchColorResult>;
  cancel: () => Promise<boolean>;
  undo: () => Promise<unknown>;
}

const defaultDependencies: ColorMatchDependencies = {
  generate: api.matchColor,
  cancel: api.cancelAdvancedWorkflow,
  undo: edit.undo,
};

export function ColorMatchSection({ clip, dependencies = defaultDependencies }: {
  clip: Clip;
  dependencies?: ColorMatchDependencies;
}) {
  const t = useT();
  const activeFrame = useEditorUiStore((state) => state.activeFrame);
  const items = useMediaStore((state) => state.items);
  const references = useMemo(
    () => items.filter((item) => item.id !== clip.mediaRef && (item.type === "image" || item.type === "video")),
    [items, clip.mediaRef],
  );
  const [referenceId, setReferenceId] = useState("");
  const [referenceFrame, setReferenceFrame] = useState(0);
  const [targetFrame, setTargetFrame] = useState(
    activeFrame >= clip.startFrame && activeFrame < clip.startFrame + clip.durationFrames
      ? activeFrame
      : clip.startFrame,
  );
  const [phase, setPhase] = useState<Phase>("idle");
  const [preview, setPreview] = useState<MatchColorResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationRef = useRef(0);
  const previousColorMatchInputRef = useRef(clip.colorMatchInput);
  const compatible =
    (clip.mediaType === "image" || clip.mediaType === "video") &&
    !clip.nestedSequenceId &&
    !clip.reversed &&
    Math.abs(clip.speed - 1) <= Number.EPSILON;

  useEffect(() => {
    if (!references.some((item) => item.id === referenceId)) {
      setReferenceId(references[0]?.id ?? "");
    }
  }, [references, referenceId]);

  useEffect(() => {
    operationRef.current += 1;
    setPreview(null);
    setError(null);
    setPhase("idle");
  }, [clip.id, referenceId, referenceFrame, targetFrame]);

  useEffect(() => {
    const previous = previousColorMatchInputRef.current;
    previousColorMatchInputRef.current = clip.colorMatchInput;
    if (phase === "applied" && previous && !clip.colorMatchInput) {
      setPreview(null);
      setPhase("idle");
    }
    // Only a persisted-provenance transition invalidates the applied state;
    // phase itself must not make injected/test completions look stale.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clip.colorMatchInput]);

  async function run(apply: boolean) {
    if (!compatible || !referenceId) return;
    const operation = operationRef.current + 1;
    operationRef.current = operation;
    setPhase(apply ? "applying" : "previewing");
    setError(null);
    try {
      const result = await dependencies.generate(
        clip.id,
        referenceId,
        referenceFrame,
        targetFrame,
        apply,
      );
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
  const selectedReference = references.find((item) => item.id === referenceId);
  return (
    <section data-testid="color-match-section" style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)" }}>
      <div style={headingStyle}><Icon icon={Palette} size={11} />{t("inspector.colorMatch.heading")}</div>
      {!compatible && <div role="status" style={hintStyle}>{t("inspector.colorMatch.compatibility")}</div>}
      {compatible && references.length === 0 && <div role="status" style={hintStyle}>{t("inspector.colorMatch.noReference")}</div>}
      {compatible && references.length > 0 && (
        <>
          <label style={fieldLabelStyle}>
            {t("inspector.colorMatch.reference")}
            <select value={referenceId} disabled={busy || phase === "applied"} onChange={(event) => setReferenceId(event.currentTarget.value)} style={inputStyle}>
              {references.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
            </select>
          </label>
          {phase !== "applied" && (
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-xs)" }}>
              <NumberField label={t("inspector.colorMatch.referenceFrame")} value={referenceFrame} min={0} max={selectedReference?.type === "image" ? 0 : Number.MAX_SAFE_INTEGER} disabled={busy} onChange={setReferenceFrame} />
              <NumberField label={t("inspector.colorMatch.targetFrame")} value={targetFrame} min={clip.startFrame} max={clip.startFrame + clip.durationFrames - 1} disabled={busy} onChange={setTargetFrame} />
            </div>
          )}
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-xs)" }}>
            {phase === "applied" ? (
              <button type="button" onClick={() => void undo()} style={secondaryButtonStyle}><Icon icon={RotateCcw} size={12} />{t("inspector.colorMatch.undo")}</button>
            ) : (
              <>
                <button type="button" disabled={busy} onClick={() => void run(false)} style={secondaryButtonStyle}><Icon icon={Palette} size={12} />{phase === "previewing" ? t("inspector.colorMatch.processing") : error ? t("inspector.colorMatch.retry") : t("inspector.colorMatch.preview")}</button>
                <button type="button" disabled={!preview || busy} onClick={() => void run(true)} style={primaryButtonStyle}><Icon icon={Palette} size={12} />{phase === "applying" ? t("inspector.colorMatch.applying") : t("inspector.colorMatch.apply")}</button>
              </>
            )}
            {busy && <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}><Icon icon={Square} size={11} />{t("inspector.colorMatch.cancel")}</button>}
          </div>
        </>
      )}
      {preview && (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-xs)" }}>
            <Swatch label={t("inspector.colorMatch.targetSample")} color={preview.result.targetMeanLinear} />
            <Swatch label={t("inspector.colorMatch.referenceSample")} color={preview.result.matchedMeanLinear} />
          </div>
          <div style={hintStyle}>{t("inspector.colorMatch.delta", { before: preview.result.deltaEBefore.toFixed(2), after: preview.result.deltaEAfter.toFixed(2) })}</div>
          <div style={hintStyle}>{t("inspector.colorMatch.luma", { before: preview.result.targetLumaBefore.toFixed(3), after: preview.result.targetLumaAfter.toFixed(3) })}</div>
        </div>
      )}
      {phase === "applied" && <div role="status" style={{ ...hintStyle, color: "var(--status-success)" }}>{t("inspector.colorMatch.applied")}</div>}
      {error && <div role="alert" style={{ ...hintStyle, color: "var(--status-error)" }}>{error}</div>}
    </section>
  );
}

function NumberField({ label, value, min, max, disabled, onChange }: { label: string; value: number; min: number; max: number; disabled: boolean; onChange: (value: number) => void }) {
  return <label style={fieldLabelStyle}>{label}<input type="number" value={value} min={min} max={max} disabled={disabled} onChange={(event) => onChange(Number(event.currentTarget.value))} style={inputStyle} /></label>;
}

function Swatch({ label, color }: { label: string; color: Rgb }) {
  return <div style={{ ...fieldLabelStyle, padding: "var(--space-xs)", borderRadius: "var(--radius-sm)", background: linearRgbCss(color), minHeight: 42, justifyContent: "flex-end", color: "white", textShadow: "0 1px 2px black" }}>{label}</div>;
}

function linearRgbCss(color: Rgb): string {
  const encode = (value: number) => Math.round(255 * (value < 0.018 ? 4.5 * value : 1.099 * value ** 0.45 - 0.099));
  return `rgb(${encode(color.r)} ${encode(color.g)} ${encode(color.b)})`;
}

function message(reason: unknown): string { return reason instanceof Error ? reason.message : String(reason); }

const headingStyle = { display: "flex", alignItems: "center", gap: "var(--space-xs)", fontSize: "var(--fs-xxs)", fontWeight: "var(--fw-semibold)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "var(--tracking-wide)" } as const;
const hintStyle = { color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const fieldLabelStyle = { display: "flex", flexDirection: "column", gap: 2, color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const inputStyle = { minWidth: 0, padding: "3px var(--space-xs)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-primary)", fontSize: "var(--fs-xs)" } as const;
const primaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", background: "var(--ai-gradient)", color: "#111", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" } as const;
const secondaryButtonStyle = { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 4, minHeight: 24, padding: "2px var(--space-sm)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
