/**
 * CaptionsTab — the 字幕 tab of the media panel. Port of upstream
 * `MediaPanel/CaptionsTab/CaptionTab.swift` (minus the Agent-mode section, which
 * depends on the agent chat and lands later).
 *
 * Source select (auto / a specific track), language (auto / manual code), caption
 * style (size / color / background / case / censor profanity), placement (X/Y),
 * and a Generate button whose states mirror upstream: needs-model → download
 * prompt (reusing transcribe_model_status / download_transcribe_model),
 * transcribing/placing spinner, then a note on the result ("no speech detected").
 *
 * The heavy lifting (transcribe → pack → place) all happens in Rust via
 * `generate_captions` (the SAME pipeline the add_captions agent tool uses); this
 * component only gathers the request and reports progress. Clip-scoped captioning
 * follows the live timeline selection, matching upstream's "selected clips when
 * available, otherwise all captionable audio".
 */

import { useEffect, useMemo, useState } from "react";
import { useT, type TFunction } from "../../i18n";
import { useProjectStore } from "../../store/projectStore";
import { useMediaStore } from "../../store/mediaStore";
import { useEditorUiStore } from "../../store/uiStore";
import { generateCaptions } from "../../store/editActions";
import {
  downloadTranscribeModel,
  isTauri,
  onTranscribeProgress,
  transcribeModelStatus,
} from "../../lib/api";
import { LAYOUT, SPACE, RADIUS } from "../../lib/theme";
import type {
  CaptionCase,
  CaptionRequest,
  CaptionSource,
  ModelStatus,
  Rgba,
  TextStyle,
  Timeline,
} from "../../lib/types";

/** Caption style/placement defaults, 1:1 with upstream `AppTheme.Caption`. */
const DEFAULT_FONT_SIZE = LAYOUT.captionDefaultFontSize;
const MIN_FONT_SIZE = LAYOUT.captionMinFontSize;
const MAX_FONT_SIZE = LAYOUT.captionMaxFontSize;
const DEFAULT_CENTER_X = 0.5;
const DEFAULT_CENTER_Y = LAYOUT.captionDefaultCenterY;
const CENTER_SNAP = 0.5;
const CENTER_SNAP_THRESHOLD = LAYOUT.captionCenterSnapThreshold;

const CASE_OPTIONS: ReadonlyArray<CaptionCase> = ["auto", "upper", "lower"];

/** The Generate flow's phase, driving the button label + progress overlay. */
type Phase =
  | { kind: "idle" }
  | { kind: "needsModel"; status: ModelStatus }
  | { kind: "downloading"; fraction: number }
  | { kind: "transcribing" };

export function CaptionsTab() {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const mediaItems = useMediaStore((s) => s.items);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);

  // Asset ids known to carry audio (audio assets, or video assets with audio) —
  // used to decide whether a video clip's track is captionable in the UI hint.
  const audioAssetIds = useMemo(() => {
    const set = new Set<string>();
    for (const item of mediaItems) {
      if (item.type === "audio" || (item.type === "video" && item.hasAudio)) set.add(item.id);
    }
    return set;
  }, [mediaItems]);

  // Style (caption font size default 48, not the generic text 96).
  const [fontSize, setFontSize] = useState<number>(DEFAULT_FONT_SIZE);
  const [color, setColor] = useState<Rgba>({ r: 1, g: 1, b: 1, a: 1 });
  const [background, setBackground] = useState<{ enabled: boolean; color: Rgba }>({
    enabled: false,
    color: { r: 0, g: 0, b: 0, a: 0.6 },
  });
  const [textCase, setTextCase] = useState<CaptionCase>("auto");
  const [censorProfanity, setCensorProfanity] = useState(false);

  // Placement (normalized canvas center; default bottom-center).
  const [centerX, setCenterX] = useState(DEFAULT_CENTER_X);
  const [centerY, setCenterY] = useState<number>(DEFAULT_CENTER_Y);

  // Source: null = auto (or selected clips), else a specific track id.
  const [trackId, setTrackId] = useState<string | null>(null);
  // Manual language code (empty = auto-detect).
  const [language, setLanguage] = useState("");

  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [note, setNote] = useState<string | null>(null);

  // Caption-eligible tracks (any audio track, or a video track that carries
  // audio). Mirrors upstream's track menu built from `captionTargets`.
  const captionTracks = useMemo(
    () => captionableTracks(timeline, audioAssetIds),
    [timeline, audioAssetIds],
  );

  // A track that no longer exists (deleted) falls back to auto.
  useEffect(() => {
    if (trackId && !captionTracks.some((tr) => tr.id === trackId)) setTrackId(null);
  }, [captionTracks, trackId]);

  const busy = phase.kind === "downloading" || phase.kind === "transcribing";
  const hasSelection = selectedClipIds.size > 0;

  /** The request source: a chosen track wins; else the live selection; else auto. */
  const requestSource = (): CaptionSource => {
    if (trackId) return { kind: "track", trackId };
    if (hasSelection) return { kind: "clips", clipIds: [...selectedClipIds] };
    return { kind: "auto" };
  };

  const buildStyle = (): TextStyle => ({
    fontName: "Helvetica-Bold",
    fontSize,
    fontScale: 1,
    color,
    alignment: "center",
    shadow: { enabled: true, color: { r: 0, g: 0, b: 0, a: 0.6 }, offsetX: 0, offsetY: -2, blur: 6 },
    background,
    border: { enabled: false, color: { r: 0, g: 0, b: 0, a: 1 } },
  });

  const runGenerate = async () => {
    setNote(null);
    setPhase({ kind: "transcribing" });
    const request: CaptionRequest = {
      source: requestSource(),
      style: buildStyle(),
      centerX,
      centerY,
      textCase,
      censorProfanity,
      language: language.trim() || undefined,
    };
    try {
      const result = await generateCaptions(request);
      if (result.captionCount === 0) setNote(t("captions.noSpeech"));
      else setNote(t("captions.added", { count: result.captionCount }));
    } catch (err) {
      setNote(t("captions.failed", { error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setPhase({ kind: "idle" });
    }
  };

  /** Generate click: gate on the model being installed first (upstream shows a
   *  download prompt when the on-device model isn't present). */
  const onGenerate = async () => {
    if (!isTauri) {
      setNote(t("captions.desktopOnly"));
      return;
    }
    setNote(null);
    try {
      const status = await transcribeModelStatus();
      if (!status.installed) {
        setPhase({ kind: "needsModel", status });
        return;
      }
    } catch {
      // If the status check fails, still attempt generation — it will surface a
      // clearer backend error than a status probe would.
    }
    await runGenerate();
  };

  const onDownloadModel = async () => {
    setNote(null);
    setPhase({ kind: "downloading", fraction: 0 });
    const unlisten = await onTranscribeProgress((fraction) =>
      setPhase({ kind: "downloading", fraction }),
    );
    try {
      await downloadTranscribeModel();
      unlisten();
      // Model ready → go straight into transcription (upstream flows through).
      await runGenerate();
    } catch (err) {
      unlisten();
      setPhase({ kind: "idle" });
      setNote(t("captions.failed", { error: err instanceof Error ? err.message : String(err) }));
    }
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", position: "relative" }}>
      <div style={{ flex: 1, overflowY: "auto", padding: `${SPACE.md}px ${SPACE.lgXl}px` }}>
        <Section title={t("captions.source")}>
          <Row label={t("captions.source")} help={t("captions.sourceHelp")}>
            <select
              value={trackId ?? "__auto__"}
              onChange={(e) => setTrackId(e.target.value === "__auto__" ? null : e.target.value)}
              style={selectStyle}
            >
              <option value="__auto__">{autoSourceLabel(t, hasSelection, selectedClipIds.size)}</option>
              {captionTracks.map((tr) => (
                <option key={tr.id} value={tr.id}>
                  {t("captions.source.track")} {tr.indexLabel} · {t("captions.clipCount", { count: tr.clipCount })}
                </option>
              ))}
            </select>
          </Row>
          <Row label={t("captions.language")}>
            <input
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              placeholder={t("captions.language.auto")}
              aria-label={t("captions.language")}
              style={{ ...inputStyle, width: 96 }}
            />
          </Row>
        </Section>

        <Section title={t("captions.style")}>
          <Row label={t("captions.style.size")}>
            <input
              type="number"
              min={MIN_FONT_SIZE}
              max={MAX_FONT_SIZE}
              value={Math.round(fontSize)}
              onChange={(e) => setFontSize(clampNumber(Number(e.target.value), MIN_FONT_SIZE, MAX_FONT_SIZE))}
              aria-label={t("captions.style.size")}
              style={{ ...inputStyle, width: 64 }}
            />
          </Row>
          <Row label={t("captions.style.color")}>
            <ColorSwatch label={t("captions.style.color")} color={color} onChange={setColor} />
          </Row>
          <Row label={t("captions.style.background")}>
            <div style={{ display: "flex", alignItems: "center", gap: SPACE.sm }}>
              <ColorSwatch
                label={t("captions.style.background")}
                color={background.color}
                disabled={!background.enabled}
                onChange={(c) => setBackground((b) => ({ ...b, color: c }))}
              />
              <input
                type="checkbox"
                checked={background.enabled}
                onChange={(e) => setBackground((b) => ({ ...b, enabled: e.target.checked }))}
                aria-label={t("captions.style.background")}
              />
            </div>
          </Row>
          <Row label={t("captions.style.case")}>
            <select
              value={textCase}
              onChange={(e) => setTextCase(e.target.value as CaptionCase)}
              aria-label={t("captions.style.case")}
              style={selectStyle}
            >
              {CASE_OPTIONS.map((c) => (
                <option key={c} value={c}>
                  {t(`captions.case.${c}`)}
                </option>
              ))}
            </select>
          </Row>
          <Row label={t("captions.censorProfanity")}>
            <input
              type="checkbox"
              checked={censorProfanity}
              onChange={(e) => setCensorProfanity(e.target.checked)}
              aria-label={t("captions.censorProfanity")}
            />
          </Row>
        </Section>

        <Section title={t("captions.placement")}>
          <CaptionPreview timeline={timeline} style={buildStyle()} centerX={centerX} centerY={centerY} previewText={t("captions.previewText")} />
          <div style={{ display: "flex", gap: SPACE.mdLg, marginTop: SPACE.sm }}>
            <PosField label="X" value={centerX} onChange={(v) => setCenterX(snapCenter(v))} />
            <PosField label="Y" value={centerY} onChange={(v) => setCenterY(snapCenter(v))} />
          </div>
        </Section>
      </div>

      {/* Generate bar (fixed at the bottom, like upstream). */}
      <div
        style={{
          flex: "0 0 auto",
          padding: `${SPACE.md}px ${SPACE.lgXl}px`,
          borderTop: "var(--bw-hairline) solid var(--border-subtle)",
          display: "flex",
          flexDirection: "column",
          gap: SPACE.sm,
        }}
      >
        {note && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>{note}</div>
        )}
        {phase.kind === "needsModel" ? (
          <>
            <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
              {t("captions.needsModel", {
                model: phase.status.model,
                size: formatBytes(phase.status.bytes),
              })}
            </div>
            <button type="button" onClick={onDownloadModel} style={primaryButtonStyle(false)}>
              {t("captions.downloadModel")}
            </button>
          </>
        ) : (
          <button
            type="button"
            onClick={onGenerate}
            disabled={busy}
            style={primaryButtonStyle(busy)}
          >
            {phase.kind === "downloading"
              ? t("captions.downloading", { percent: Math.round(phase.fraction * 100) })
              : phase.kind === "transcribing"
                ? t("captions.generating")
                : t("captions.generate")}
          </button>
        )}
      </div>

      {busy && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            background: "var(--bg-surface)",
            opacity: 0.72,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-secondary)",
            fontSize: "var(--fs-sm)",
          }}
        >
          {phase.kind === "downloading"
            ? t("captions.downloading", { percent: Math.round(phase.fraction * 100) })
            : t("captions.generating")}
        </div>
      )}
    </div>
  );
}

// MARK: - Sub-views

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: SPACE.mdLg }}>
      <div
        style={{
          fontSize: "var(--fs-xs)",
          fontWeight: "var(--fw-semibold)",
          color: "var(--text-tertiary)",
          textTransform: "uppercase",
          letterSpacing: "0.04em",
          marginBottom: SPACE.sm,
        }}
      >
        {title}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: SPACE.sm }}>{children}</div>
    </div>
  );
}

function Row({
  label,
  help,
  children,
}: {
  label: string;
  help?: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: SPACE.sm }}>
      <span title={help} style={{ fontSize: "var(--fs-sm)", color: "var(--text-secondary)" }}>
        {label}
      </span>
      {children}
    </div>
  );
}

function ColorSwatch({
  label,
  color,
  disabled,
  onChange,
}: {
  label: string;
  color: Rgba;
  disabled?: boolean;
  onChange: (c: Rgba) => void;
}) {
  return (
    <input
      aria-label={label}
      type="color"
      disabled={disabled}
      value={rgbaToHex(color)}
      onChange={(e) => onChange({ ...hexToRgb(e.target.value), a: color.a })}
      style={{
        width: SPACE.lgXl,
        height: SPACE.lgXl,
        padding: 0,
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: RADIUS.xs,
        background: "transparent",
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
      }}
    />
  );
}

function PosField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: SPACE.xs }}>
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{label}</span>
      <input
        type="number"
        min={0}
        max={100}
        value={Math.round(value * 100)}
        onChange={(e) => onChange(clampNumber(Number(e.target.value), 0, 100) / 100)}
        aria-label={label}
        style={{ ...inputStyle, width: 56 }}
      />
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>%</span>
    </div>
  );
}

/** A live preview box sized to the project aspect, with the sample caption placed
 *  at the chosen center — a lightweight mirror of upstream's `previewBox`. */
function CaptionPreview({
  timeline,
  style,
  centerX,
  centerY,
  previewText,
}: {
  timeline: Timeline;
  style: TextStyle;
  centerX: number;
  centerY: number;
  previewText: string;
}) {
  const aspect = timeline.width / Math.max(1, timeline.height);
  return (
    <div
      style={{
        position: "relative",
        width: "100%",
        aspectRatio: `${aspect}`,
        maxHeight: 160,
        background: "var(--bg-placeholder)",
        borderRadius: RADIUS.sm,
        border: "var(--bw-hairline) solid var(--border-subtle)",
        overflow: "hidden",
      }}
    >
      <span
        style={{
          position: "absolute",
          left: `${centerX * 100}%`,
          top: `${centerY * 100}%`,
          transform: "translate(-50%, -50%)",
          whiteSpace: "nowrap",
          color: rgbaToCss(style.color),
          background: style.background.enabled ? rgbaToCss(style.background.color) : "transparent",
          padding: "1px 4px",
          borderRadius: 2,
          // Scale the caption font (canvas points) into the preview's height.
          fontSize: `${(style.fontSize / Math.max(1, timeline.height)) * 160}px`,
          fontWeight: 700,
        }}
      >
        {previewText}
      </span>
    </div>
  );
}

// MARK: - Helpers

interface CaptionTrackInfo {
  id: string;
  indexLabel: number;
  clipCount: number;
}

/** Tracks that can be captioned: those holding an audio clip, or a video clip
 *  whose source asset carries audio (`audioAssetIds`). A lightweight UI mirror of
 *  `captionTargets` — the authoritative eligibility runs in Rust during
 *  generation, so this only needs to populate the source menu sensibly. */
function captionableTracks(timeline: Timeline, audioAssetIds: Set<string>): CaptionTrackInfo[] {
  const out: CaptionTrackInfo[] = [];
  timeline.tracks.forEach((track, index) => {
    const captionable = track.clips.filter(
      (c) => c.mediaType === "audio" || (c.mediaType === "video" && audioAssetIds.has(c.mediaRef)),
    );
    if (captionable.length > 0) {
      out.push({ id: track.id, indexLabel: index + 1, clipCount: captionable.length });
    }
  });
  return out;
}

function autoSourceLabel(t: TFunction, hasSelection: boolean, count: number): string {
  if (hasSelection) return t("captions.source.selectedClips", { count });
  return t("captions.source.auto");
}

/** Snap a center coordinate to 0.5 when close (upstream `snapCenter`). */
function snapCenter(v: number): number {
  return Math.abs(v - CENTER_SNAP) < CENTER_SNAP_THRESHOLD ? CENTER_SNAP : clampNumber(v, 0, 1);
}

function clampNumber(v: number, min: number, max: number): number {
  if (Number.isNaN(v)) return min;
  return Math.max(min, Math.min(max, v));
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "?";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

function channelHex(value: number): string {
  const clamped = Math.max(0, Math.min(255, Math.round(value * 255)));
  return clamped.toString(16).padStart(2, "0");
}

function rgbaToHex(color: Rgba): string {
  return `#${channelHex(color.r)}${channelHex(color.g)}${channelHex(color.b)}`;
}

function rgbaToCss(color: Rgba): string {
  return `rgba(${Math.round(color.r * 255)}, ${Math.round(color.g * 255)}, ${Math.round(color.b * 255)}, ${color.a})`;
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const raw = hex.replace("#", "");
  const expanded =
    raw.length === 3
      ? raw
          .split("")
          .map((ch) => ch + ch)
          .join("")
      : raw;
  const n = parseInt(expanded, 16);
  return { r: ((n >> 16) & 0xff) / 255, g: ((n >> 8) & 0xff) / 255, b: (n & 0xff) / 255 };
}

const inputStyle: React.CSSProperties = {
  height: 22,
  background: "var(--bg-raised)",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: RADIUS.sm,
  color: "var(--text-primary)",
  fontSize: "var(--fs-sm)",
  padding: "0 6px",
  textAlign: "right",
};

const selectStyle: React.CSSProperties = {
  height: 24,
  maxWidth: 180,
  background: "var(--bg-raised)",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: RADIUS.sm,
  color: "var(--text-primary)",
  fontSize: "var(--fs-sm)",
  padding: "0 6px",
};

function primaryButtonStyle(disabled: boolean): React.CSSProperties {
  return {
    width: "100%",
    padding: `${SPACE.smMd}px`,
    borderRadius: RADIUS.sm,
    border: "none",
    background: "var(--accent-primary)",
    color: "var(--bg-base)",
    fontSize: "var(--fs-sm)",
    fontWeight: "var(--fw-semibold)",
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.6 : 1,
  };
}
