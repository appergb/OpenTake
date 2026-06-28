/**
 * Inspector (SPEC §6). Title bar + one of four content states: marquee summary,
 * clip inspector (with Video/Audio tabs), media-asset source, or project
 * metadata. Editable fields commit via SetClipProperties. The keyframe lane and
 * Text/AI-Edit tabs are scaffolded (TODO: full parity in a later pass).
 */

import { useEffect, useState } from "react";
import {
  CircleDashed,
  Diamond,
  Info,
  Palette,
  Pipette,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { PanelHeaderBar } from "../ui/PanelShell";
import { Icon } from "../ui/Icon";
import { ScrubbableNumberField } from "./ScrubbableNumberField";
import { TextTab } from "./TextTab";
import { KeyframesPanel } from "./KeyframesPanel";
import { SwapMediaSection } from "./SwapMediaSection";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { formatTimecode } from "../../lib/geometry";
import {
  cropAt,
  mediaCanvasAspect,
  opacityAt,
  resizeTransformKeepingSourceAspect,
  rotationAt,
  sizeAt,
  topLeftAt,
  volumeAt,
} from "../../lib/clip";
import { FS, RADIUS, SPACE } from "../../lib/theme";
import { useT, type TFunction } from "../../i18n";
import type {
  ChromaKey,
  Clip,
  ColorGrade,
  Crop,
  Interpolation,
  Mask,
  MaskShape,
  Rgb,
  Timeline,
} from "../../lib/types";

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}

export function Inspector() {
  const t = useT();
  const timeline = useProjectStore((s) => s.timeline);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const inspectorTab = useEditorUiStore((s) => s.inspectorTab);
  const setInspectorTab = useEditorUiStore((s) => s.setInspectorTab);
  const keyframesPanelVisible = useEditorUiStore((s) => s.keyframesPanelVisible);
  const toggleKeyframesPanel = useEditorUiStore((s) => s.toggleKeyframesPanel);

  const selectedClips = collectSelected(timeline, selectedClipIds);
  const isMarquee = selectedClips.length > 1;
  const single = selectedClips.length === 1 ? selectedClips[0] : null;

  const title = single || isMarquee ? t("inspector.title") : t("inspector.timeline");
  const TitleIcon = single || isMarquee ? SlidersHorizontal : Info;

  return (
    <>
      <PanelHeaderBar>
        <span style={{ display: "inline-flex", color: "var(--text-secondary)" }}>
          <Icon icon={TitleIcon} size={13} />
        </span>
        <span style={{ fontSize: "var(--fs-sm-md)", fontWeight: "var(--fw-medium)" }}>
          {title}
        </span>
      </PanelHeaderBar>

      <div style={{ flex: 1, overflowY: "auto", overflowX: "hidden" }}>
        {isMarquee ? (
          <MarqueeSummary count={selectedClips.length} t={t} />
        ) : single ? (
          <ClipInspector
            clip={single}
            tab={inspectorTab}
            setTab={setInspectorTab}
            hasAudio={single.mediaType === "audio"}
            keyframesOpen={keyframesPanelVisible}
            onToggleKeyframes={toggleKeyframesPanel}
            t={t}
          />
        ) : (
          <ProjectMetadata timeline={timeline} t={t} />
        )}
      </div>
    </>
  );
}

function collectSelected(timeline: Timeline, ids: Set<string>): Clip[] {
  const out: Clip[] = [];
  for (const t of timeline.tracks) for (const c of t.clips) if (ids.has(c.id)) out.push(c);
  return out;
}

function MarqueeSummary({ count, t }: { count: number; t: TFunction }) {
  return (
    <div
      style={{
        padding: "var(--space-xl)",
        textAlign: "center",
        color: "var(--text-tertiary)",
        fontSize: "var(--fs-sm-md)",
      }}
    >
      {t("inspector.selectedCount", { count })}
    </div>
  );
}

function SectionHeader({ label, icon }: { label: string; icon?: LucideIcon }) {
  const HeaderIcon = icon;
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-xs)",
        fontSize: "var(--fs-xxs)",
        fontWeight: "var(--fw-semibold)",
        letterSpacing: "var(--tracking-wide)",
        color: "var(--text-muted)",
        textTransform: "uppercase",
        marginBottom: "var(--space-sm)",
      }}
    >
      {HeaderIcon && <Icon icon={HeaderIcon} size={11} />}
      <span>{label}</span>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        height: 22,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "var(--space-sm)",
      }}
    >
      <span
        title={label}
        style={{
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          fontSize: "var(--fs-xs)",
          color: "var(--text-tertiary)",
        }}
      >
        {label}
      </span>
      <span
        style={{
          flexShrink: 0,
          display: "inline-flex",
          alignItems: "center",
          gap: "var(--space-xs)",
        }}
      >
        {children}
      </span>
    </div>
  );
}

/** A non-interactive numeric value shown when a property is keyframe-animated.
 *  Mirrors ScrubbableNumberField's typography but without drag/click handlers. */
function ReadOnlyValue({ text, width = 56 }: { text: string; width?: number }) {
  return (
    <span
      className="tabular"
      style={{
        width,
        display: "inline-block",
        textAlign: "right",
        color: "var(--text-tertiary)",
        fontSize: "var(--fs-sm)",
        userSelect: "text",
      }}
    >
      {text}
    </span>
  );
}

/** Inline hint shown beside a read-only field when a property is animated. */
function AnimatedHint({ t }: { t: TFunction }) {
  return (
    <span
      style={{
        fontSize: "var(--fs-xxs)",
        color: "var(--text-muted)",
        fontStyle: "italic",
      }}
    >
      {t("inspector.animatedHint")}
    </span>
  );
}

const INTERPOLATION_KEYS: Record<Interpolation, string> = {
  linear: "inspector.interpolation.linear",
  hold: "inspector.interpolation.hold",
  smooth: "inspector.interpolation.smooth",
};

/** A compact native `<select>` for choosing an interpolation mode. */
function InterpolationSelect({
  value,
  onChange,
  t,
}: {
  value: Interpolation;
  onChange: (v: Interpolation) => void;
  t: TFunction;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as Interpolation)}
      style={{
        fontSize: "var(--fs-sm)",
        color: "var(--accent-primary)",
        background: "var(--bg-raised)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-xs)",
        padding: "1px 4px",
      }}
    >
      {(Object.keys(INTERPOLATION_KEYS) as Interpolation[]).map((k) => (
        <option key={k} value={k}>
          {t(INTERPOLATION_KEYS[k])}
        </option>
      ))}
    </select>
  );
}

const TAB_LABEL_KEY: Record<"text" | "video" | "audio" | "aiEdit", string> = {
  text: "inspector.tab.text",
  video: "inspector.tab.video",
  audio: "inspector.tab.audio",
  aiEdit: "inspector.tab.aiEdit",
};

function ClipInspector({
  clip,
  tab,
  setTab,
  hasAudio,
  keyframesOpen,
  onToggleKeyframes,
  t,
}: {
  clip: Clip;
  tab: string;
  setTab: (t: "text" | "video" | "audio" | "aiEdit") => void;
  hasAudio: boolean;
  keyframesOpen: boolean;
  onToggleKeyframes: () => void;
  t: TFunction;
}) {
  // Available tabs depend on selection (SPEC §6.3).
  const tabs: Array<"text" | "video" | "audio" | "aiEdit"> = [];
  if (clip.mediaType === "text") tabs.push("text");
  else tabs.push("video");
  if (hasAudio) tabs.push("audio");

  const activeTab = tabs.includes(tab as never) ? tab : tabs[0];

  // Live sampling: read the current playhead frame so every numeric field shows
  // the value at the playhead (upstream `InspectorView.livePreview`).
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const timeline = useProjectStore((s) => s.timeline);
  const mediaItem = useMediaStore((s) => s.items.find((m) => m.id === clip.mediaRef) ?? null);
  const aspect = mediaCanvasAspect(
    mediaItem?.width,
    mediaItem?.height,
    timeline.width,
    timeline.height,
  );

  const commit = (props: Parameters<typeof edit.setClipProperties>[1]) =>
    edit.setClipProperties([clip.id], props);

  // Track-active checks (a track is active iff it holds ≥1 keyframe).
  const opacityAnimated = !!clip.opacityTrack && clip.opacityTrack.keyframes.length > 0;
  const volumeAnimated = !!clip.volumeTrack && clip.volumeTrack.keyframes.length > 0;
  const rotationAnimated = !!clip.rotationTrack && clip.rotationTrack.keyframes.length > 0;
  const scaleAnimated = !!clip.scaleTrack && clip.scaleTrack.keyframes.length > 0;
  const positionAnimated = !!clip.positionTrack && clip.positionTrack.keyframes.length > 0;
  const cropAnimated = !!clip.cropTrack && clip.cropTrack.keyframes.length > 0;

  // Sampled values at the playhead.
  const sampledOpacity = opacityAt(clip, activeFrame);
  const sampledVolume = volumeAt(clip, activeFrame);
  const sampledRotation = rotationAt(clip, activeFrame);
  const sampledScale = sizeAt(clip, activeFrame)[0];
  const sampledTopLeft = topLeftAt(clip, activeFrame);
  const sampledCrop = cropAt(clip, activeFrame);

  return (
    <div>
      {tabs.length > 1 && (
        <div
          style={{
            display: "flex",
            gap: "var(--space-md)",
            padding: "var(--space-xs) var(--space-lg) 0",
          }}
        >
          {tabs.map((tabId) => (
            <button
              key={tabId}
              onClick={() => setTab(tabId)}
              style={{
                paddingBottom: 4,
                fontSize: "var(--fs-sm-md)",
                fontWeight: activeTab === tabId ? "var(--fw-medium)" : "var(--fw-regular)",
                color: activeTab === tabId ? "var(--text-primary)" : "var(--text-tertiary)",
                borderBottom:
                  activeTab === tabId ? "var(--bw-medium) solid var(--text-primary)" : "none",
              }}
            >
              {t(TAB_LABEL_KEY[tabId])}
            </button>
          ))}
        </div>
      )}

      <div style={{ padding: "var(--space-lg)", display: "flex", flexDirection: "column", gap: "var(--space-lg)" }}>
        {clip.mediaType !== "text" && <SwapMediaSection clip={clip} t={t} />}
        {activeTab === "text" ? (
          <TextTab clip={clip} t={t} />
        ) : activeTab === "audio" ? (
          <section>
            <SectionHeader label={t("inspector.section.levels")} />
            <Row label={t("inspector.field.volume")}>
              {volumeAnimated ? (
                <>
                  <ReadOnlyValue
                    text={(20 * Math.log10(Math.max(1e-6, sampledVolume))).toFixed(1) + " dB"}
                  />
                  <AnimatedHint t={t} />
                </>
              ) : (
                <ScrubbableNumberField
                  value={clip.volume}
                  min={0}
                  max={4}
                  sensitivity={0.01}
                  format={(v) => (20 * Math.log10(Math.max(1e-6, v))).toFixed(1)}
                  suffix=" dB"
                  width={56}
                  displayTextOverride={(v) => (v <= 0 ? "-∞ dB" : null)}
                  onCommit={(v) => commit({ volume: v })}
                />
              )}
            </Row>
            <FadeSection clip={clip} commit={commit} t={t} />
          </section>
        ) : (
          <>
            <section>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <SectionHeader label={t("inspector.section.transform")} />
              </div>
              <Row label={t("inspector.field.scale")}>
                {scaleAnimated ? (
                  <>
                    <ReadOnlyValue text={Math.round(sampledScale * 100) + "%"} />
                    <AnimatedHint t={t} />
                  </>
                ) : (
                  <ScrubbableNumberField
                    value={sampledScale}
                    min={0.01}
                    max={10}
                    sensitivity={0.005}
                    format={(v) => Math.round(v * 100).toString()}
                    suffix="%"
                    width={56}
                    onCommit={(v) =>
                      commit({
                        transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect),
                      })
                    }
                  />
                )}
              </Row>
              <Row label={t("inspector.field.rotation")}>
                {rotationAnimated ? (
                  <>
                    <ReadOnlyValue text={sampledRotation.toFixed(0) + "°"} />
                    <AnimatedHint t={t} />
                  </>
                ) : (
                  <ScrubbableNumberField
                    value={sampledRotation}
                    min={-3600}
                    max={3600}
                    sensitivity={0.5}
                    format={(v) => v.toFixed(0)}
                    suffix="°"
                    width={56}
                    onCommit={(v) => commit({ transform: { ...clip.transform, rotation: v } })}
                  />
                )}
              </Row>
              <Row label={t("inspector.field.opacity")}>
                {opacityAnimated ? (
                  <>
                    <ReadOnlyValue text={Math.round(sampledOpacity * 100) + "%"} />
                    <AnimatedHint t={t} />
                  </>
                ) : (
                  <ScrubbableNumberField
                    value={clip.opacity}
                    min={0}
                    max={1}
                    sensitivity={0.005}
                    format={(v) => Math.round(v * 100).toString()}
                    suffix="%"
                    width={56}
                    onCommit={(v) => commit({ opacity: v })}
                  />
                )}
              </Row>
            </section>

            <PositionSection
              clip={clip}
              sampledTopLeft={sampledTopLeft}
              animated={positionAnimated}
              commit={commit}
              t={t}
            />

            <CropSection
              clip={clip}
              sampledCrop={sampledCrop}
              animated={cropAnimated}
              commit={commit}
              t={t}
            />

            <FlipSection clip={clip} commit={commit} t={t} />

            <FadeSection clip={clip} commit={commit} t={t} />

            <section>
              <SectionHeader label={t("inspector.section.playback")} />
              <Row label={t("inspector.field.speed")}>
                <ScrubbableNumberField
                  value={clip.speed}
                  min={0.25}
                  max={4}
                  sensitivity={0.01}
                  format={(v) => v.toFixed(2)}
                  suffix="x"
                  width={56}
                  onCommit={(v) => commit({ speed: v })}
                />
              </Row>
            </section>

            {isVisualEffectClip(clip) && <ShaderEffectsSection clip={clip} t={t} />}
          </>
        )}
      </div>

      {/* Keyframes toggle bar (SPEC §6.4). */}
      <div
        style={{
          display: "flex",
          justifyContent: "flex-end",
          padding: "var(--space-sm) var(--space-lg)",
          borderTop: "var(--bw-thin) solid var(--border-primary)",
        }}
      >
        <button
          onClick={onToggleKeyframes}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "var(--space-xs)",
            color: keyframesOpen ? "var(--text-primary)" : "var(--text-tertiary)",
            fontSize: "var(--fs-sm)",
          }}
        >
          <Icon icon={Diamond} size={12} />
          {t("inspector.keyframes")}
        </button>
      </div>

      {keyframesOpen && <KeyframesPanel clip={clip} t={t} />}
    </div>
  );
}

// MARK: - Position section (top-left x/y)

function PositionSection({
  clip,
  sampledTopLeft,
  animated,
  commit,
  t,
}: {
  clip: Clip;
  sampledTopLeft: { x: number; y: number };
  animated: boolean;
  commit: (props: Parameters<typeof edit.setClipProperties>[1]) => void;
  t: TFunction;
}) {
  // Editing top-left x/y writes back through `transform.centerX/centerY`. The
  // size is preserved from the current transform (scale track writes via scale).
  const [w, h] = [clip.transform.width, clip.transform.height];
  return (
    <section>
      <SectionHeader label={t("inspector.section.position")} />
      <Row label={t("inspector.field.positionX")}>
        {animated ? (
          <>
            <ReadOnlyValue text={sampledTopLeft.x.toFixed(3)} />
            <AnimatedHint t={t} />
          </>
        ) : (
          <ScrubbableNumberField
            value={sampledTopLeft.x}
            min={-2}
            max={2}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            width={56}
            onCommit={(v) =>
              commit({ transform: { ...clip.transform, centerX: v + w / 2 } })
            }
          />
        )}
      </Row>
      <Row label={t("inspector.field.positionY")}>
        {animated ? (
          <>
            <ReadOnlyValue text={sampledTopLeft.y.toFixed(3)} />
            <AnimatedHint t={t} />
          </>
        ) : (
          <ScrubbableNumberField
            value={sampledTopLeft.y}
            min={-2}
            max={2}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            width={56}
            onCommit={(v) =>
              commit({ transform: { ...clip.transform, centerY: v + h / 2 } })
            }
          />
        )}
      </Row>
    </section>
  );
}

// MARK: - Crop section (4 edge insets, 0–1)

function CropSection({
  clip,
  sampledCrop,
  animated,
  commit,
  t,
}: {
  clip: Clip;
  sampledCrop: Crop;
  animated: boolean;
  commit: (props: Parameters<typeof edit.setClipProperties>[1]) => void;
  t: TFunction;
}) {
  const commitEdge = (edge: keyof Crop, v: number) => {
    const next: Crop = { ...clip.crop, [edge]: v };
    commit({ crop: next });
  };
  const renderEdge = (label: string, edge: keyof Crop, value: number) => (
    <Row label={label}>
      {animated ? (
        <>
          <ReadOnlyValue text={value.toFixed(3)} />
          <AnimatedHint t={t} />
        </>
      ) : (
        <ScrubbableNumberField
          value={value}
          min={0}
          max={1}
          sensitivity={0.005}
          format={(v) => v.toFixed(3)}
          width={56}
          onCommit={(v) => commitEdge(edge, v)}
        />
      )}
    </Row>
  );
  return (
    <section>
      <SectionHeader label={t("inspector.section.crop")} />
      {renderEdge(t("inspector.field.cropLeft"), "left", sampledCrop.left)}
      {renderEdge(t("inspector.field.cropTop"), "top", sampledCrop.top)}
      {renderEdge(t("inspector.field.cropRight"), "right", sampledCrop.right)}
      {renderEdge(t("inspector.field.cropBottom"), "bottom", sampledCrop.bottom)}
    </section>
  );
}

// MARK: - Flip section (horizontal / vertical checkboxes)

function FlipSection({
  clip,
  commit,
  t,
}: {
  clip: Clip;
  commit: (props: Parameters<typeof edit.setClipProperties>[1]) => void;
  t: TFunction;
}) {
  const checkboxStyle: React.CSSProperties = {
    accentColor: "var(--accent-primary)",
    cursor: "pointer",
  };
  return (
    <section>
      <SectionHeader label={t("inspector.section.flip")} />
      <Row label={t("inspector.field.flipHorizontal")}>
        <input
          type="checkbox"
          checked={clip.transform.flipHorizontal}
          style={checkboxStyle}
          onChange={(e) => commit({ flipHorizontal: e.target.checked })}
        />
      </Row>
      <Row label={t("inspector.field.flipVertical")}>
        <input
          type="checkbox"
          checked={clip.transform.flipVertical}
          style={checkboxStyle}
          onChange={(e) => commit({ flipVertical: e.target.checked })}
        />
      </Row>
    </section>
  );
}

// MARK: - Fade section (fade in/out frames + interpolation)

function FadeSection({
  clip,
  commit,
  t,
}: {
  clip: Clip;
  commit: (props: Parameters<typeof edit.setClipProperties>[1]) => void;
  t: TFunction;
}) {
  return (
    <section>
      <SectionHeader label={t("inspector.section.fade")} />
      <Row label={t("inspector.field.fadeInFrames")}>
        <ScrubbableNumberField
          value={clip.fadeInFrames}
          min={0}
          max={clip.durationFrames}
          sensitivity={1}
          format={(v) => v.toFixed(0)}
          width={56}
          onCommit={(v) => commit({ fadeInFrames: Math.round(v) })}
        />
      </Row>
      <Row label={t("inspector.field.fadeInInterpolation")}>
        <InterpolationSelect
          value={clip.fadeInInterpolation}
          onChange={(v) => commit({ fadeInInterpolation: v })}
          t={t}
        />
      </Row>
      <Row label={t("inspector.field.fadeOutFrames")}>
        <ScrubbableNumberField
          value={clip.fadeOutFrames}
          min={0}
          max={clip.durationFrames}
          sensitivity={1}
          format={(v) => v.toFixed(0)}
          width={56}
          onCommit={(v) => commit({ fadeOutFrames: Math.round(v) })}
        />
      </Row>
      <Row label={t("inspector.field.fadeOutInterpolation")}>
        <InterpolationSelect
          value={clip.fadeOutInterpolation}
          onChange={(v) => commit({ fadeOutInterpolation: v })}
          t={t}
        />
      </Row>
    </section>
  );
}

// MARK: - Shader effect sections (color grade / chroma key / masks)

const EFFECT_VALUE_WIDTH = 56;
const EFFECT_RGB_WIDTH = 42;
const COLOR_SWATCH_SIZE = SPACE.lgXl;

const controlStyle: React.CSSProperties = {
  fontSize: FS.sm,
  color: "var(--accent-primary)",
  background: "var(--bg-raised)",
  border: "var(--bw-thin) solid var(--border-primary)",
  borderRadius: RADIUS.xs,
  padding: `${SPACE.xxs}px ${SPACE.xs}px`,
};

const checkboxStyle: React.CSSProperties = {
  accentColor: "var(--accent-primary)",
  cursor: "pointer",
};

function ShaderEffectsSection({ clip, t }: { clip: Clip; t: TFunction }) {
  return (
    <>
      <ColorGradeSection clip={clip} t={t} />
      <ChromaKeySection clip={clip} t={t} />
      <MaskSection clip={clip} t={t} />
    </>
  );
}

function ColorGradeSection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [draft, setDraft] = useState<ColorGrade>(() => completeColorGrade(clip.colorGrade));

  useEffect(() => {
    setDraft(completeColorGrade(clip.colorGrade));
  }, [clip.id, clip.colorGrade]);

  const commitGrade = (next: ColorGrade) => {
    setDraft(next);
    void edit.setColorGrade([clip.id], next);
  };
  const updateField = (field: keyof Omit<ColorGrade, "liftGammaGain">, value: number) =>
    setDraft((g) => ({ ...g, [field]: value }));
  const commitField = (field: keyof Omit<ColorGrade, "liftGammaGain">, value: number) =>
    commitGrade({ ...draft, [field]: value });
  const updateLgg = (band: keyof ColorGrade["liftGammaGain"], channel: keyof Rgb, value: number) =>
    setDraft((g) => setLggChannel(g, band, channel, value));
  const commitLgg = (band: keyof ColorGrade["liftGammaGain"], channel: keyof Rgb, value: number) =>
    commitGrade(setLggChannel(draft, band, channel, value));

  return (
    <section>
      <SectionHeader label={t("inspector.section.colorGrade")} icon={Palette} />
      <EffectNumberRow
        label={t("inspector.field.exposure")}
        value={draft.exposure}
        min={-5}
        max={5}
        sensitivity={0.02}
        format={(v) => v.toFixed(2)}
        onChange={(v) => updateField("exposure", v)}
        onCommit={(v) => commitField("exposure", v)}
      />
      <EffectNumberRow
        label={t("inspector.field.temperature")}
        value={draft.temperature}
        min={-1}
        max={1}
        sensitivity={0.005}
        format={(v) => v.toFixed(2)}
        onChange={(v) => updateField("temperature", v)}
        onCommit={(v) => commitField("temperature", v)}
      />
      <EffectNumberRow
        label={t("inspector.field.tint")}
        value={draft.tint}
        min={-1}
        max={1}
        sensitivity={0.005}
        format={(v) => v.toFixed(2)}
        onChange={(v) => updateField("tint", v)}
        onCommit={(v) => commitField("tint", v)}
      />
      {(["lift", "gamma", "gain"] as Array<keyof ColorGrade["liftGammaGain"]>).map((band) =>
        (["r", "g", "b"] as Array<keyof Rgb>).map((channel) => (
          <EffectNumberRow
            key={`${band}-${channel}`}
            label={t(`inspector.field.${band}${channel.toUpperCase()}`)}
            value={draft.liftGammaGain[band][channel]}
            min={band === "lift" ? -1 : 0}
            max={band === "lift" ? 1 : 4}
            sensitivity={band === "lift" ? 0.005 : 0.01}
            format={(v) => v.toFixed(2)}
            width={EFFECT_RGB_WIDTH}
            onChange={(v) => updateLgg(band, channel, v)}
            onCommit={(v) => commitLgg(band, channel, v)}
          />
        )),
      )}
      <EffectNumberRow
        label={t("inspector.field.contrast")}
        value={draft.contrast}
        min={-1}
        max={2}
        sensitivity={0.01}
        format={(v) => v.toFixed(2)}
        onChange={(v) => updateField("contrast", v)}
        onCommit={(v) => commitField("contrast", v)}
      />
      <EffectNumberRow
        label={t("inspector.field.saturation")}
        value={draft.saturation}
        min={0}
        max={3}
        sensitivity={0.01}
        format={(v) => v.toFixed(2)}
        onChange={(v) => updateField("saturation", v)}
        onCommit={(v) => commitField("saturation", v)}
      />
    </section>
  );
}

function ChromaKeySection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [enabled, setEnabled] = useState(() => !!clip.chromaKey);
  const [draft, setDraft] = useState<ChromaKey>(() => completeChromaKey(clip.chromaKey));

  useEffect(() => {
    setEnabled(!!clip.chromaKey);
    setDraft(completeChromaKey(clip.chromaKey));
  }, [clip.id, clip.chromaKey]);

  const commitKey = (next: ChromaKey) => {
    setDraft(next);
    if (enabled) void edit.setChromaKey([clip.id], next);
  };
  const updateField = (field: keyof Omit<ChromaKey, "keyColor">, value: number) =>
    setDraft((k) => ({ ...k, [field]: value }));
  const commitField = (field: keyof Omit<ChromaKey, "keyColor">, value: number) =>
    commitKey({ ...draft, [field]: value });
  const setKeyEnabled = (nextEnabled: boolean) => {
    setEnabled(nextEnabled);
    if (nextEnabled) {
      const next = completeChromaKey(clip.chromaKey);
      setDraft(next);
      void edit.setChromaKey([clip.id], next);
    } else {
      void edit.setChromaKey([clip.id], null);
    }
  };

  return (
    <section>
      <SectionHeader label={t("inspector.section.chromaKey")} icon={Pipette} />
      <Row label={t("inspector.field.enabled")}>
        <input
          type="checkbox"
          checked={enabled}
          style={checkboxStyle}
          onChange={(e) => setKeyEnabled(e.target.checked)}
        />
      </Row>
      {enabled && (
        <>
          <Row label={t("inspector.field.keyColor")}>
            <input
              aria-label={t("inspector.field.keyColor")}
              type="color"
              value={rgbToHex(draft.keyColor)}
              onChange={(e) => setDraft((k) => ({ ...k, keyColor: hexToRgb(e.target.value) }))}
              onBlur={() => commitKey(draft)}
              style={{
                width: COLOR_SWATCH_SIZE,
                height: COLOR_SWATCH_SIZE,
                padding: 0,
                border: "var(--bw-thin) solid var(--border-primary)",
                borderRadius: RADIUS.xs,
                background: "transparent",
                cursor: "pointer",
              }}
            />
          </Row>
          <EffectNumberRow
            label={t("inspector.field.similarity")}
            value={draft.similarity}
            min={0}
            max={1}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            onChange={(v) => updateField("similarity", v)}
            onCommit={(v) => commitField("similarity", v)}
          />
          <EffectNumberRow
            label={t("inspector.field.smoothness")}
            value={draft.smoothness}
            min={0}
            max={1}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            onChange={(v) => updateField("smoothness", v)}
            onCommit={(v) => commitField("smoothness", v)}
          />
          <EffectNumberRow
            label={t("inspector.field.spill")}
            value={draft.spill}
            min={0}
            max={1}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            onChange={(v) => updateField("spill", v)}
            onCommit={(v) => commitField("spill", v)}
          />
        </>
      )}
    </section>
  );
}

function MaskSection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [enabled, setEnabled] = useState(() => (clip.masks?.length ?? 0) > 0);
  const [draft, setDraft] = useState<Mask>(() => completeMask(clip.masks?.[0]));

  useEffect(() => {
    setEnabled((clip.masks?.length ?? 0) > 0);
    setDraft(completeMask(clip.masks?.[0]));
  }, [clip.id, clip.masks]);

  const commitMask = (next: Mask) => {
    setDraft(next);
    void edit.setMasks([clip.id], [next, ...(clip.masks?.slice(1) ?? [])]);
  };
  const setMaskEnabled = (nextEnabled: boolean) => {
    setEnabled(nextEnabled);
    if (nextEnabled) {
      const next = completeMask(clip.masks?.[0]);
      setDraft(next);
      void edit.setMasks([clip.id], [next, ...(clip.masks?.slice(1) ?? [])]);
    } else {
      void edit.setMasks([clip.id], []);
    }
  };
  const setShape = (shape: MaskShape) => commitMask({ ...draft, shape });
  const updateCommon = (field: keyof Omit<Mask, "shape">, value: number | boolean) =>
    setDraft((m) => ({ ...m, [field]: value }));
  const commitCommon = (field: keyof Omit<Mask, "shape">, value: number | boolean) =>
    commitMask({ ...draft, [field]: value });

  return (
    <section>
      <SectionHeader label={t("inspector.section.mask")} icon={CircleDashed} />
      <Row label={t("inspector.field.enabled")}>
        <input
          type="checkbox"
          checked={enabled}
          style={checkboxStyle}
          onChange={(e) => setMaskEnabled(e.target.checked)}
        />
      </Row>
      {enabled && (
        <>
          <Row label={t("inspector.field.maskType")}>
            <select
              value={draft.shape.kind}
              onChange={(e) => {
                const kind = e.target.value;
                if (kind === "circle") setShape(defaultCircleShape());
                else if (kind === "linear") setShape(defaultLinearShape());
              }}
              style={controlStyle}
            >
              <option value="circle">{t("inspector.mask.circle")}</option>
              <option value="linear">{t("inspector.mask.linear")}</option>
              <option value="poly" disabled>
                {t("inspector.mask.polyPending")}
              </option>
            </select>
          </Row>
          {draft.shape.kind === "circle" ? (
            <CircleMaskFields
              shape={draft.shape}
              setShape={setShape}
              setDraftShape={(shape) => setDraft((m) => ({ ...m, shape }))}
              t={t}
            />
          ) : draft.shape.kind === "linear" ? (
            <LinearMaskFields
              shape={draft.shape}
              setShape={setShape}
              setDraftShape={(shape) => setDraft((m) => ({ ...m, shape }))}
              t={t}
            />
          ) : null}
          <EffectNumberRow
            label={t("inspector.field.feather")}
            value={draft.feather}
            min={0}
            max={1}
            sensitivity={0.005}
            format={(v) => v.toFixed(3)}
            onChange={(v) => updateCommon("feather", v)}
            onCommit={(v) => commitCommon("feather", v)}
          />
          <Row label={t("inspector.field.invert")}>
            <input
              type="checkbox"
              checked={draft.invert}
              style={checkboxStyle}
              onChange={(e) => commitCommon("invert", e.target.checked)}
            />
          </Row>
        </>
      )}
    </section>
  );
}

function CircleMaskFields({
  shape,
  setShape,
  setDraftShape,
  t,
}: {
  shape: Extract<MaskShape, { kind: "circle" }>;
  setShape: (shape: MaskShape) => void;
  setDraftShape: (shape: MaskShape) => void;
  t: TFunction;
}) {
  const updatePoint = (field: "center" | "radius", axis: keyof RgbPoint, value: number) =>
    setDraftShape({ ...shape, [field]: { ...shape[field], [axis]: value } });
  const commitPoint = (field: "center" | "radius", axis: keyof RgbPoint, value: number) =>
    setShape({ ...shape, [field]: { ...shape[field], [axis]: value } });

  return (
    <>
      <MaskNumberRow
        label={t("inspector.field.centerX")}
        value={shape.center.x}
        onChange={(v) => updatePoint("center", "x", v)}
        onCommit={(v) => commitPoint("center", "x", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.centerY")}
        value={shape.center.y}
        onChange={(v) => updatePoint("center", "y", v)}
        onCommit={(v) => commitPoint("center", "y", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.radiusX")}
        value={shape.radius.x}
        min={0.01}
        max={3}
        onChange={(v) => updatePoint("radius", "x", v)}
        onCommit={(v) => commitPoint("radius", "x", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.radiusY")}
        value={shape.radius.y}
        min={0.01}
        max={3}
        onChange={(v) => updatePoint("radius", "y", v)}
        onCommit={(v) => commitPoint("radius", "y", v)}
      />
    </>
  );
}

function LinearMaskFields({
  shape,
  setShape,
  setDraftShape,
  t,
}: {
  shape: Extract<MaskShape, { kind: "linear" }>;
  setShape: (shape: MaskShape) => void;
  setDraftShape: (shape: MaskShape) => void;
  t: TFunction;
}) {
  const updatePoint = (field: "point" | "normal", axis: keyof RgbPoint, value: number) =>
    setDraftShape({ ...shape, [field]: { ...shape[field], [axis]: value } });
  const commitPoint = (field: "point" | "normal", axis: keyof RgbPoint, value: number) =>
    setShape({ ...shape, [field]: { ...shape[field], [axis]: value } });

  return (
    <>
      <MaskNumberRow
        label={t("inspector.field.pointX")}
        value={shape.point.x}
        onChange={(v) => updatePoint("point", "x", v)}
        onCommit={(v) => commitPoint("point", "x", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.pointY")}
        value={shape.point.y}
        onChange={(v) => updatePoint("point", "y", v)}
        onCommit={(v) => commitPoint("point", "y", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.normalX")}
        value={shape.normal.x}
        min={-1}
        max={1}
        onChange={(v) => updatePoint("normal", "x", v)}
        onCommit={(v) => commitPoint("normal", "x", v)}
      />
      <MaskNumberRow
        label={t("inspector.field.normalY")}
        value={shape.normal.y}
        min={-1}
        max={1}
        onChange={(v) => updatePoint("normal", "y", v)}
        onCommit={(v) => commitPoint("normal", "y", v)}
      />
    </>
  );
}

function EffectNumberRow({
  label,
  value,
  min,
  max,
  sensitivity,
  format,
  width = EFFECT_VALUE_WIDTH,
  onChange,
  onCommit,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  sensitivity: number;
  format: (v: number) => string;
  width?: number;
  onChange: (v: number) => void;
  onCommit: (v: number) => void;
}) {
  return (
    <Row label={label}>
      <ScrubbableNumberField
        value={value}
        min={min}
        max={max}
        sensitivity={sensitivity}
        format={format}
        width={width}
        onChange={onChange}
        onCommit={onCommit}
      />
    </Row>
  );
}

function MaskNumberRow({
  label,
  value,
  min = -1,
  max = 2,
  onChange,
  onCommit,
}: {
  label: string;
  value: number;
  min?: number;
  max?: number;
  onChange: (v: number) => void;
  onCommit: (v: number) => void;
}) {
  return (
    <EffectNumberRow
      label={label}
      value={value}
      min={min}
      max={max}
      sensitivity={0.005}
      format={(v) => v.toFixed(3)}
      onChange={onChange}
      onCommit={onCommit}
    />
  );
}

type RgbPoint = { x: number; y: number };

function isVisualEffectClip(clip: Clip): boolean {
  return clip.mediaType === "video" || clip.mediaType === "image" || clip.mediaType === "lottie";
}

function defaultRgb(value = 0): Rgb {
  return { r: value, g: value, b: value };
}

function completeColorGrade(grade: ColorGrade | undefined): ColorGrade {
  return {
    exposure: grade?.exposure ?? 0,
    temperature: grade?.temperature ?? 0,
    tint: grade?.tint ?? 0,
    liftGammaGain: {
      lift: { ...defaultRgb(0), ...grade?.liftGammaGain?.lift },
      gamma: { ...defaultRgb(1), ...grade?.liftGammaGain?.gamma },
      gain: { ...defaultRgb(1), ...grade?.liftGammaGain?.gain },
    },
    contrast: grade?.contrast ?? 0,
    saturation: grade?.saturation ?? 1,
  };
}

function completeChromaKey(chromaKey: ChromaKey | undefined): ChromaKey {
  return {
    keyColor: { r: 0, g: 1, b: 0, ...chromaKey?.keyColor },
    similarity: chromaKey?.similarity ?? 0.15,
    smoothness: chromaKey?.smoothness ?? 0.35,
    spill: chromaKey?.spill ?? 0.5,
  };
}

function completeMask(mask: Mask | undefined): Mask {
  return {
    shape: mask?.shape ?? defaultCircleShape(),
    feather: mask?.feather ?? 0,
    invert: mask?.invert ?? false,
  };
}

function defaultCircleShape(): Extract<MaskShape, { kind: "circle" }> {
  return {
    kind: "circle",
    center: { x: 0.5, y: 0.5 },
    radius: { x: 1.5, y: 1.5 },
  };
}

function defaultLinearShape(): Extract<MaskShape, { kind: "linear" }> {
  return {
    kind: "linear",
    point: { x: 0.5, y: 0.5 },
    normal: { x: 1, y: 0 },
  };
}

function setLggChannel(
  grade: ColorGrade,
  band: keyof ColorGrade["liftGammaGain"],
  channel: keyof Rgb,
  value: number,
): ColorGrade {
  return {
    ...grade,
    liftGammaGain: {
      ...grade.liftGammaGain,
      [band]: {
        ...grade.liftGammaGain[band],
        [channel]: value,
      },
    },
  };
}

function rgbToHex(rgb: Rgb): string {
  const channel = (value: number) => {
    const clamped = Math.max(0, Math.min(255, Math.round(value * 255)));
    return clamped.toString(16).padStart(2, "0");
  };
  return `#${channel(rgb.r)}${channel(rgb.g)}${channel(rgb.b)}`;
}

function hexToRgb(hex: string): Rgb {
  const raw = hex.replace("#", "");
  const expanded =
    raw.length === 3
      ? raw
          .split("")
          .map((ch) => ch + ch)
          .join("")
      : raw;
  const parsed = Number.parseInt(expanded, 16);
  if (!Number.isFinite(parsed)) return { r: 0, g: 1, b: 0 };
  return {
    r: ((parsed >> 16) & 0xff) / 255,
    g: ((parsed >> 8) & 0xff) / 255,
    b: (parsed & 0xff) / 255,
  };
}

function ProjectMetadata({ timeline, t }: { timeline: Timeline; t: TFunction }) {
  const g = gcd(timeline.width, timeline.height) || 1;
  const total = timeline.tracks.reduce(
    (m, track) =>
      Math.max(m, track.clips.reduce((mm, c) => Math.max(mm, c.startFrame + c.durationFrames), 0)),
    0,
  );
  return (
    <div style={{ padding: "var(--space-lg)", display: "flex", flexDirection: "column", gap: "var(--space-xl)" }}>
      <section>
        <SectionHeader label={t("inspector.section.format")} />
        <MetaRow label={t("inspector.field.resolution")} value={`${timeline.width} × ${timeline.height}`} />
        <MetaRow label={t("inspector.field.frameRate")} value={`${timeline.fps} fps`} />
        <MetaRow label={t("inspector.field.aspectRatio")} value={`${timeline.width / g}:${timeline.height / g}`} />
        <MetaRow label={t("inspector.field.duration")} value={formatTimecode(total, timeline.fps)} />
      </section>
    </div>
  );
}

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        gap: "var(--space-sm)",
        padding: "2px 0",
      }}
    >
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{label}</span>
      <span
        className="tabular"
        style={{
          fontSize: "var(--fs-xs)",
          color: "var(--text-secondary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
          userSelect: "text",
        }}
      >
        {value}
      </span>
    </div>
  );
}
