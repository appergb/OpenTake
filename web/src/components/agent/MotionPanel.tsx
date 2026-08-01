import { useEffect, useMemo, useState } from "react";
import { Clapperboard, Square } from "lucide-react";
import {
  addMotion,
  cancelMotion,
  isTauri,
  motionCapability,
  onMotionProgress,
  type MotionProgressPhase,
} from "../../lib/api";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { forceRefresh } from "../../store/sync";

const PHASE_KEYS: Record<MotionProgressPhase, string> = {
  validating: "motion.phaseValidating",
  rendering: "motion.phaseRendering",
  encoding: "motion.phaseEncoding",
  committing: "motion.phaseCommitting",
  complete: "motion.phaseComplete",
};

export function MotionPanel() {
  const t = useT();
  const projectPath = useProjectStore((state) => state.projectPath);
  const fps = useProjectStore((state) => state.timeline.fps);
  const activeFrame = useEditorUiStore((state) => state.activeFrame);
  const selectClips = useEditorUiStore((state) => state.selectClips);
  const [available, setAvailable] = useState<boolean | null>(null);
  const [templateId, setTemplateId] = useState<"title-card" | "lower-third.glass">(
    "title-card",
  );
  const [title, setTitle] = useState("OpenTake");
  const [subtitle, setSubtitle] = useState("");
  const [accent, setAccent] = useState("#7C5CFF");
  const [durationSeconds, setDurationSeconds] = useState(3);
  const [phase, setPhase] = useState<MotionProgressPhase | null>(null);
  const [error, setError] = useState<string | null>(null);
  const rendering = phase !== null && phase !== "complete";
  const durationFrames = useMemo(
    () => Math.max(1, Math.round(durationSeconds * Math.max(1, fps))),
    [durationSeconds, fps],
  );

  useEffect(() => {
    let disposed = false;
    void motionCapability().then((result) => {
      if (!disposed) setAvailable(result);
    });
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: () => void = () => {};
    void onMotionProgress(setPhase).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten();
    };
  }, []);

  async function add() {
    if (!projectPath || !available || rendering || !title.trim()) return;
    setError(null);
    setPhase("validating");
    try {
      const commit = await addMotion({
        templateId,
        params: {
          title: title.trim(),
          subtitle: subtitle.trim(),
          accent,
        },
        startFrame: Math.max(0, Math.round(activeFrame)),
        durationFrames,
      });
      await forceRefresh();
      selectClips(new Set([commit.clipId]));
      setPhase("complete");
    } catch (reason) {
      setPhase(null);
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }

  async function cancel() {
    await cancelMotion();
  }

  if (!isTauri) {
    return <MotionNotice>{t("motion.desktopOnly")}</MotionNotice>;
  }
  if (available === null) {
    return <MotionNotice>{t("motion.checking")}</MotionNotice>;
  }
  if (!available) {
    return <MotionNotice>{t("motion.unavailable")}</MotionNotice>;
  }

  return (
    <div
      style={{
        flex: 1,
        minHeight: 0,
        overflowY: "auto",
        padding: "var(--space-md)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-md)",
      }}
    >
      <div>
        <div style={{ fontSize: "var(--fs-sm)", fontWeight: 650, color: "var(--text-primary)" }}>
          {t("motion.heading")}
        </div>
        <div style={{ marginTop: 4, fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
          {t("motion.description")}
        </div>
      </div>

      <label style={fieldStyle}>
        <span>{t("motion.template")}</span>
        <select
          aria-label={t("motion.template")}
          value={templateId}
          disabled={rendering}
          onChange={(event) =>
            setTemplateId(event.target.value as "title-card" | "lower-third.glass")
          }
          style={inputStyle}
        >
          <option value="title-card">{t("motion.titleCard")}</option>
          <option value="lower-third.glass">{t("motion.lowerThird")}</option>
        </select>
      </label>
      <label style={fieldStyle}>
        <span>{t("motion.title")}</span>
        <input
          value={title}
          disabled={rendering}
          onChange={(event) => setTitle(event.target.value)}
          style={inputStyle}
        />
      </label>
      <label style={fieldStyle}>
        <span>{t("motion.subtitle")}</span>
        <input
          value={subtitle}
          disabled={rendering}
          onChange={(event) => setSubtitle(event.target.value)}
          style={inputStyle}
        />
      </label>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-sm)" }}>
        <label style={fieldStyle}>
          <span>{t("motion.duration")}</span>
          <input
            type="number"
            min={0.5}
            max={30}
            step={0.5}
            value={durationSeconds}
            disabled={rendering}
            onChange={(event) => setDurationSeconds(Number(event.target.value))}
            style={inputStyle}
          />
        </label>
        <label style={fieldStyle}>
          <span>{t("motion.accent")}</span>
          <input
            type="color"
            value={accent}
            disabled={rendering}
            onChange={(event) => setAccent(event.target.value)}
            style={{ ...inputStyle, padding: 3, minHeight: 34 }}
          />
        </label>
      </div>

      {phase && (
        <div role="status" style={{ fontSize: "var(--fs-xs)", color: "var(--text-secondary)" }}>
          {t(PHASE_KEYS[phase])}
        </div>
      )}
      {error && (
        <div role="alert" style={{ fontSize: "var(--fs-xs)", color: "var(--accent-danger)" }}>
          {error}
        </div>
      )}

      {rendering ? (
        <button type="button" onClick={() => void cancel()} style={primaryButtonStyle}>
          <Square size={13} /> {t("motion.cancel")}
        </button>
      ) : (
        <button
          type="button"
          onClick={() => void add()}
          disabled={!projectPath || !title.trim()}
          style={{
            ...primaryButtonStyle,
            opacity: projectPath && title.trim() ? 1 : 0.45,
          }}
        >
          <Clapperboard size={15} /> {t("motion.addAtPlayhead")}
        </button>
      )}
    </div>
  );
}

function MotionNotice({ children }: { children: string }) {
  return (
    <div style={{ padding: "var(--space-lg)", color: "var(--text-muted)", fontSize: "var(--fs-sm)" }}>
      {children}
    </div>
  );
}

const fieldStyle = {
  display: "flex",
  flexDirection: "column",
  gap: 5,
  color: "var(--text-secondary)",
  fontSize: "var(--fs-xs)",
} as const;

const inputStyle = {
  width: "100%",
  border: "var(--bw-thin) solid var(--border-subtle)",
  borderRadius: "var(--radius-sm)",
  background: "var(--bg-elevated)",
  color: "var(--text-primary)",
  padding: "7px 9px",
  fontFamily: "inherit",
  fontSize: "var(--fs-sm)",
} as const;

const primaryButtonStyle = {
  minHeight: 36,
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: 7,
  borderRadius: "var(--radius-sm)",
  background: "var(--accent-primary)",
  color: "#111",
  fontSize: "var(--fs-sm)",
  fontWeight: 650,
} as const;
