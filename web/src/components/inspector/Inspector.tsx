/**
 * Inspector (SPEC §6). Title bar + one of four content states: marquee summary,
 * clip inspector (with Video/Audio tabs), media-asset source, or project
 * metadata. Editable fields commit via SetClipProperties. The keyframe lane and
 * Text/AI-Edit tabs are scaffolded (TODO: full parity in a later pass).
 */

import { useState } from "react";
import { Info, SlidersHorizontal, Diamond, RefreshCw } from "lucide-react";
import { PanelHeaderBar } from "../ui/PanelShell";
import { Icon } from "../ui/Icon";
import { ScrubbableNumberField } from "./ScrubbableNumberField";
import { TextTab } from "./TextTab";
import { KeyframesPanel } from "./KeyframesPanel";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { formatTimecode } from "../../lib/geometry";
import {
  cropAt,
  opacityAt,
  rotationAt,
  sizeAt,
  topLeftAt,
  volumeAt,
} from "../../lib/clip";
import { useT, type TFunction } from "../../i18n";
import type { Clip, Crop, Interpolation, MediaItem, Timeline } from "../../lib/types";

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

function SectionHeader({ label }: { label: string }) {
  return (
    <div
      style={{
        fontSize: "var(--fs-xxs)",
        fontWeight: "var(--fw-semibold)",
        letterSpacing: "var(--tracking-wide)",
        color: "var(--text-muted)",
        textTransform: "uppercase",
        marginBottom: "var(--space-sm)",
      }}
    >
      {label}
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
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{label}</span>
      <span style={{ display: "inline-flex", alignItems: "center", gap: "var(--space-xs)" }}>
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

/** A compact media-type badge label. */
function mediaTypeLabel(type: MediaItem["type"]): string {
  switch (type) {
    case "video":
      return "Video";
    case "audio":
      return "Audio";
    case "image":
      return "Image";
    case "text":
      return "Text";
    case "lottie":
      return "Lottie";
  }
}

/** "替换媒体" section: opens an inline media picker that lists every library
 *  asset except the clip's current `mediaRef`. Selecting one fires
 *  `edit.swapMedia`, which preserves all editing attributes and truncates the
 *  duration when the new media is shorter. Text clips don't render this section
 *  (they have no source media to swap). */
function SwapMediaSection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [open, setOpen] = useState(false);
  const items = useMediaStore((s) => s.items);

  // Exclude the current media source; text items aren't swappable targets.
  const candidates = items.filter((m) => m.id !== clip.mediaRef && m.type !== "text");

  const handlePick = (item: MediaItem) => {
    void edit.swapMedia(clip.id, item.id, { mediaType: item.type });
    setOpen(false);
  };

  return (
    <section>
      <button
        onClick={() => setOpen((v) => !v)}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          fontSize: "var(--fs-sm)",
          color: open ? "var(--text-primary)" : "var(--text-tertiary)",
          background: "none",
          border: "none",
          cursor: "pointer",
          padding: 0,
        }}
      >
        <Icon icon={RefreshCw} size={12} />
        {t("inspector.swapMedia")}
      </button>

      {open && (
        <div
          style={{
            marginTop: "var(--space-sm)",
            maxHeight: 200,
            overflowY: "auto",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--border-primary)",
            background: "var(--bg-secondary)",
          }}
        >
          <div
            style={{
              padding: "var(--space-xs) var(--space-sm)",
              fontSize: "var(--fs-xxs)",
              fontWeight: "var(--fw-semibold)",
              letterSpacing: "var(--tracking-wide)",
              color: "var(--text-muted)",
              textTransform: "uppercase",
              borderBottom: "var(--bw-thin) solid var(--border-primary)",
            }}
          >
            {t("inspector.swapMediaTitle")}
          </div>
          {candidates.length === 0 ? (
            <div
              style={{
                padding: "var(--space-sm)",
                fontSize: "var(--fs-xs)",
                color: "var(--text-tertiary)",
              }}
            >
              {t("inspector.swapMediaEmpty")}
            </div>
          ) : (
            candidates.map((item) => (
              <button
                key={item.id}
                onClick={() => handlePick(item)}
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  width: "100%",
                  padding: "var(--space-xs) var(--space-sm)",
                  fontSize: "var(--fs-xs)",
                  color: "var(--text-secondary)",
                  background: "none",
                  border: "none",
                  borderBottom: "var(--bw-thin) solid var(--border-primary)",
                  cursor: "pointer",
                  textAlign: "left",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "var(--bg-hover)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "none";
                }}
              >
                <span
                  style={{
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                    flex: 1,
                  }}
                >
                  {item.name || item.id}
                </span>
                <span style={{ color: "var(--text-muted)", marginLeft: "var(--space-sm)" }}>
                  {mediaTypeLabel(item.type)}
                </span>
              </button>
            ))
          )}
        </div>
      )}
    </section>
  );
}

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
                  value={sampledVolume}
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
                      commit({ transform: { ...clip.transform, width: v, height: v } })
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
                    value={sampledOpacity}
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
