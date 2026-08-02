import { useEffect, useMemo, useRef, useState } from "react";
import { Download, RotateCcw, Sparkles, Square } from "lucide-react";
import { assetUrl } from "../../lib/asset";
import * as api from "../../lib/api";
import type { Clip, GenerateMatteResult, MattingModelStatus } from "../../lib/types";
import { useT } from "../../i18n";
import * as edit from "../../store/editActions";
import { Icon } from "../ui/Icon";

type Phase = "loading" | "idle" | "installing" | "previewing" | "review" | "applying" | "applied";

export interface MattingDependencies {
  status: () => Promise<MattingModelStatus>;
  install: () => Promise<MattingModelStatus>;
  cancelInstall: () => Promise<boolean>;
  onProgress: (
    handler: (progress: { fraction: number; downloadedBytes: number; totalBytes: number }) => void,
  ) => Promise<() => void>;
  generate: (clipId: string, apply: boolean) => Promise<GenerateMatteResult>;
  cancelWorkflow: () => Promise<boolean>;
  undo: () => Promise<unknown>;
}

const defaultDependencies: MattingDependencies = {
  status: api.mattingModelStatus,
  install: api.downloadMattingModel,
  cancelInstall: api.cancelMattingModelDownload,
  onProgress: api.onMattingProgress,
  generate: (clipId, apply) => api.generateMatte(clipId, apply),
  cancelWorkflow: api.cancelAdvancedWorkflow,
  undo: edit.undo,
};

export function MattingSection({
  clip,
  dependencies = defaultDependencies,
}: {
  clip: Clip;
  dependencies?: MattingDependencies;
}) {
  const t = useT();
  const [model, setModel] = useState<MattingModelStatus | null>(null);
  const [phase, setPhase] = useState<Phase>("loading");
  const [progress, setProgress] = useState(0);
  const [preview, setPreview] = useState<GenerateMatteResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const generationRef = useRef(0);
  const installRef = useRef(0);
  const compatible =
    clip.mediaType === "video" &&
    !clip.nestedSequenceId &&
    !clip.reversed &&
    Math.abs(clip.speed - 1) <= Number.EPSILON;
  const previewUrl = useMemo(
    () => assetUrl(preview?.result.previewPath),
    [preview?.result.previewPath],
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void dependencies
      .onProgress((event) => active && setProgress(event.fraction))
      .then((dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      });
    void dependencies.status().then(
      (status) => {
        if (!active) return;
        setModel(status);
        setPhase("idle");
      },
      (reason) => {
        if (!active) return;
        setError(message(reason));
        setPhase("idle");
      },
    );
    return () => {
      active = false;
      generationRef.current += 1;
      installRef.current += 1;
      unlisten?.();
    };
  }, [dependencies]);

  useEffect(() => {
    generationRef.current += 1;
    setPreview(null);
    setError(null);
    setPhase((current) => (current === "loading" || current === "installing" ? current : "idle"));
  }, [clip.id]);

  async function install() {
    if (phase !== "idle") return;
    const operation = installRef.current + 1;
    installRef.current = operation;
    setPhase("installing");
    setProgress(0);
    setError(null);
    try {
      const installed = await dependencies.install();
      if (installRef.current !== operation) return;
      setModel(installed);
      setProgress(1);
      setPhase("idle");
    } catch (reason) {
      if (installRef.current !== operation) return;
      setError(message(reason));
      setPhase("idle");
    }
  }

  async function run(apply: boolean) {
    if (!compatible || !model?.installed) return;
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    setPhase(apply ? "applying" : "previewing");
    setError(null);
    try {
      const result = await dependencies.generate(clip.id, apply);
      if (generationRef.current !== generation) return;
      setPreview(result);
      setPhase(apply ? "applied" : "review");
    } catch (reason) {
      if (generationRef.current !== generation) return;
      setError(message(reason));
      setPhase(preview ? "review" : "idle");
    }
  }

  async function cancel() {
    if (phase === "installing") {
      installRef.current += 1;
      await dependencies.cancelInstall();
    } else {
      generationRef.current += 1;
      await dependencies.cancelWorkflow();
    }
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

  return (
    <section data-testid="matting-section" style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-xs)", fontSize: "var(--fs-xxs)", fontWeight: "var(--fw-semibold)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "var(--tracking-wide)" }}>
        <Icon icon={Sparkles} size={11} />
        {t("inspector.matting.heading")}
      </div>

      {!compatible && (
        <div role="status" style={hintStyle}>{t("inspector.matting.compatibility")}</div>
      )}
      {compatible && model && !model.installed && (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
          <div style={hintStyle}>
            {t("inspector.matting.modelNeeded", { size: formatBytes(model.bytes) })}
          </div>
          <button type="button" disabled={phase !== "idle"} onClick={() => void install()} style={primaryButtonStyle}>
            <Icon icon={Download} size={12} />
            {phase === "installing"
              ? t("inspector.matting.installing", { percent: Math.round(progress * 100) })
              : t("inspector.matting.install")}
          </button>
        </div>
      )}
      {phase === "installing" && (
        <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>
          <Icon icon={Square} size={11} />
          {t("inspector.matting.cancel")}
        </button>
      )}

      {compatible && model?.installed && (
        <>
          <div style={hintStyle}>{t("inspector.matting.localPrivacy")}</div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-xs)" }}>
            {phase === "applied" ? (
              <button type="button" onClick={() => void undo()} style={secondaryButtonStyle}>
                <Icon icon={RotateCcw} size={12} />
                {t("inspector.matting.undo")}
              </button>
            ) : (
              <>
                <button
                  type="button"
                  disabled={phase === "previewing" || phase === "applying"}
                  onClick={() => void run(false)}
                  style={secondaryButtonStyle}
                >
                  <Icon icon={Sparkles} size={12} />
                  {phase === "previewing" ? t("inspector.matting.processing") : t("inspector.matting.preview")}
                </button>
                <button
                  type="button"
                  disabled={!preview || phase === "previewing" || phase === "applying"}
                  onClick={() => void run(true)}
                  style={primaryButtonStyle}
                >
                  <Icon icon={Sparkles} size={12} />
                  {phase === "applying" ? t("inspector.matting.applying") : t("inspector.matting.apply")}
                </button>
              </>
            )}
            {(phase === "previewing" || phase === "applying") && (
              <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>
                <Icon icon={Square} size={11} />
                {t("inspector.matting.cancel")}
              </button>
            )}
          </div>
        </>
      )}

      {previewUrl && phase !== "applied" && (
        <div
          style={{
            overflow: "hidden",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--border-primary)",
            background:
              "conic-gradient(var(--bg-prominent) 25%, var(--bg-raised) 0 50%, var(--bg-prominent) 0 75%, var(--bg-raised) 0) 0 0 / 12px 12px",
          }}
        >
          <video
            key={previewUrl}
            src={previewUrl}
            controls
            playsInline
            aria-label={t("inspector.matting.previewLabel")}
            style={{ display: "block", width: "100%", maxHeight: 180, objectFit: "contain" }}
          />
        </div>
      )}
      {phase === "applied" && <div role="status" style={{ ...hintStyle, color: "var(--status-success)" }}>{t("inspector.matting.applied")}</div>}
      {error && <div role="alert" style={{ ...hintStyle, color: "var(--status-error)" }}>{error}</div>}
    </section>
  );
}

function message(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}

function formatBytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const hintStyle = { color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;

const primaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 4,
  minHeight: 24,
  padding: "2px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  background: "var(--ai-gradient)",
  color: "#111",
  fontSize: "var(--fs-xs)",
  fontWeight: "var(--fw-semibold)",
} as const;

const secondaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 4,
  minHeight: 24,
  padding: "2px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-thin) solid var(--border-primary)",
  background: "var(--bg-prominent)",
  color: "var(--text-primary)",
  fontSize: "var(--fs-xs)",
} as const;
