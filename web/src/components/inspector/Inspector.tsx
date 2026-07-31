/**
 * Inspector (SPEC §6). Title bar + one of four content states: marquee summary,
 * clip inspector (with Text/Video/Audio/AI Edit tabs), media-asset source, or project
 * metadata. Editable fields commit via SetClipProperties; a field whose
 * property already has an active keyframe track stays editable but commits
 * via UpsertKeyframe at the playhead instead (see `../../lib/keyframeValue`).
 * AI Edit proposals are reviewed here and accepted through the same undoable
 * command layer as direct Inspector edits.
 */

import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import {
  ChevronLeft,
  ChevronRight,
  CircleDashed,
  Crop as CropIcon,
  Diamond,
  Info,
  Palette,
  Pipette,
  RotateCcw,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { PanelHeaderBar } from "../ui/PanelShell";
import { Icon } from "../ui/Icon";
import { HoverButton } from "../ui/HoverButton";
import { ScrubbableNumberField } from "./ScrubbableNumberField";
import { TextTab } from "./TextTab";
import { AiEditTab } from "./AiEditTab";
import { KeyframesPanel } from "./KeyframesPanel";
import { SwapMediaSection } from "./SwapMediaSection";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { formatTimecode } from "../../lib/geometry";
import {
  cropAt,
  liveVolumeKfLinearAt,
  findLogicalSingleClip,
  mediaCanvasAspect,
  rawOpacityAt,
  resizeTransformKeepingSourceAspect,
  rotationAt,
  sizeAt,
  topLeftAt,
} from "../../lib/clip";
import {
  cropEdgeKeyframeValue,
  opacityKeyframeValue,
  positionXKeyframeValue,
  positionYKeyframeValue,
  rotationKeyframeValue,
  scaleKeyframeValue,
  volumeKeyframeValue,
} from "../../lib/keyframeValue";
import {
  clipContainsFrame,
  hasKeyframeAt,
  nextKeyframeFrame,
  previousKeyframeFrame,
} from "../../lib/keyframeNav";
import { CROP_ASPECT_LOCKS, cropForPreset, type CropAspectLock } from "../../lib/cropOverlay";
import { FS, RADIUS, SPACE } from "../../lib/theme";
import { useT, type TFunction } from "../../i18n";
import type {
  ChromaKey,
  Clip,
  ClipType,
  ColorGrade,
  Crop,
  GenerationInput,
  Interpolation,
  KeyframeProperty,
  Mask,
  MaskShape,
  MediaItem,
  Rgb,
  Timeline,
} from "../../lib/types";
import { formatFileSize, formatMediaDuration } from "../../lib/mediaFormat";

function gcd(a: number, b: number): number {
  return b === 0 ? a : gcd(b, a % b);
}

export function Inspector() {
  const t = useT();
  const rootTimeline = useProjectStore((s) => s.timeline);
  const activeNestedSequenceId = useEditorUiStore((s) => s.activeNestedSequenceId);
  const timeline =
    rootTimeline.nestedSequences?.find(
      (sequence) => sequence.id === activeNestedSequenceId,
    )?.timeline ?? rootTimeline;
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const inspectorTab = useEditorUiStore((s) => s.inspectorTab);
  const setInspectorTab = useEditorUiStore((s) => s.setInspectorTab);
  const keyframesPanelVisible = useEditorUiStore((s) => s.keyframesPanelVisible);
  const toggleKeyframesPanel = useEditorUiStore((s) => s.toggleKeyframesPanel);
  // The media-panel asset currently shown in the preview is upstream's
  // "selected media asset" for the Source inspector (upstream `selectMediaAsset`
  // opens the asset's preview tab AND selects it; OpenTake's single active
  // media asset is `previewMediaId`). Resolved from the catalog mirror.
  const previewMediaId = useEditorUiStore((s) => s.previewMediaId);
  const mediaAsset = useMediaStore((s) =>
    previewMediaId ? s.items.find((m) => m.id === previewMediaId) ?? null : null,
  );

  const selectedClips = collectSelected(timeline, selectedClipIds);
  const single = findLogicalSingleClip(timeline, selectedClipIds);
  const isMarquee = selectedClips.length > 0 && single === null;
  // State priority mirrors upstream `InspectorView.body` (:49-56):
  // marquee > clip(visual/audio) > mediaAsset > projectMetadata. Clip selection
  // is checked before the media asset, so a selected clip always wins.
  const showMediaAsset = !isMarquee && !single && mediaAsset !== null;

  const title =
    single || isMarquee
      ? t("inspector.title")
      : showMediaAsset
        ? t("inspector.source")
        : t("inspector.timeline");
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
            keyframesOpen={keyframesPanelVisible}
            onToggleKeyframes={toggleKeyframesPanel}
            t={t}
          />
        ) : showMediaAsset && mediaAsset ? (
          <MediaAssetSource asset={mediaAsset} t={t} />
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

/** Inline hint shown beside an animated field's editable control, signaling
 *  that committing a value here upserts a keyframe at the playhead rather
 *  than setting the clip's static property. */
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

/** Per-row keyframe cluster (SPEC §6.3; 1:1 port of upstream
 *  `InspectorView.keyframeControls` / `keyframeNavButton`,
 *  Inspector/InspectorView.swift:511-563). Left/right chevrons jump the playhead
 *  to the neighboring keyframe (disabled when there is none on that side); the
 *  center diamond is FILLED when a keyframe sits at the playhead and toggles it
 *  (stamp when empty, remove when filled). The diamond is disabled — and dimmed —
 *  while the playhead is outside the clip's span, matching upstream's `inRange`
 *  gate (`editor.clipFor(id:)?.contains(timelineFrame:)`). All frames are
 *  absolute; the pure helpers in `../../lib/keyframeNav` handle the clip-relative
 *  storage conversion. */
function KeyframeRowControls({
  clip,
  property,
  activeFrame,
  t,
}: {
  clip: Clip;
  property: KeyframeProperty;
  activeFrame: number;
  t: TFunction;
}) {
  const setActiveFrame = useEditorUiStore((s) => s.setActiveFrame);
  const inRange = clipContainsFrame(clip, activeFrame);
  const onKeyframe = hasKeyframeAt(clip, property, activeFrame);
  const prev = previousKeyframeFrame(clip, property, activeFrame);
  const next = nextKeyframeFrame(clip, property, activeFrame);

  const toggle = () => {
    if (!inRange) return;
    if (onKeyframe) {
      void edit.removeKeyframe(clip.id, property, activeFrame);
    } else {
      // Stamp the clip's current sampled value at the playhead (upstream's
      // `stampKeyframe`, which samples the property at `frame`).
      void edit.stampKeyframe(clip.id, property, activeFrame);
    }
  };

  const diamondTitle = !inRange
    ? t("inspector.keyframe.outsideClip")
    : onKeyframe
      ? t("inspector.keyframe.remove")
      : t("inspector.keyframe.add");

  return (
    <span style={{ display: "inline-flex", alignItems: "center" }}>
      <HoverButton
        title={t("inspector.keyframe.prev")}
        disabled={prev === null}
        onClick={() => prev !== null && setActiveFrame(prev)}
        size={18}
      >
        <Icon icon={ChevronLeft} size={12} />
      </HoverButton>
      <HoverButton title={diamondTitle} disabled={!inRange} onClick={toggle} size={18}>
        {/* currentColor drives both stroke and (when set) fill; filled = a
            keyframe sits at the playhead, in the timecode accent (upstream
            `diamond.fill` + `timecodeColor`). */}
        <span
          style={{
            display: "inline-flex",
            color: onKeyframe ? "var(--accent-timecode)" : "var(--text-tertiary)",
          }}
        >
          <Icon icon={Diamond} size={11} fill={onKeyframe ? "currentColor" : "none"} />
        </span>
      </HoverButton>
      <HoverButton
        title={t("inspector.keyframe.next")}
        disabled={next === null}
        onClick={() => next !== null && setActiveFrame(next)}
        size={18}
      >
        <Icon icon={ChevronRight} size={12} />
      </HoverButton>
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
  keyframesOpen,
  onToggleKeyframes,
  t,
}: {
  clip: Clip;
  tab: string;
  setTab: (t: "text" | "video" | "audio" | "aiEdit") => void;
  keyframesOpen: boolean;
  onToggleKeyframes: () => void;
  t: TFunction;
}) {
  // Live sampling: read the current playhead frame so every numeric field shows
  // the value at the playhead (upstream `InspectorView.livePreview`).
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const rootTimeline = useProjectStore((s) => s.timeline);
  const activeNestedSequenceId = useEditorUiStore((s) => s.activeNestedSequenceId);
  const timeline =
    rootTimeline.nestedSequences?.find(
      (sequence) => sequence.id === activeNestedSequenceId,
    )?.timeline ?? rootTimeline;
  const mediaItem = useMediaStore((s) => s.items.find((m) => m.id === clip.mediaRef) ?? null);
  // Available tabs depend on selection (SPEC §6.3). Visual source clips expose
  // AI Edit; video with an embedded audio stream also exposes Audio. Missing
  // source media keeps AI Edit visible and presents a typed unavailable state
  // inside the tab instead of silently changing the tab set.
  const tabs: Array<"text" | "video" | "audio" | "aiEdit"> = [];
  if (clip.mediaType === "text") tabs.push("text");
  else if (clip.mediaType === "audio") tabs.push("audio");
  else {
    tabs.push("video");
    if (mediaItem?.hasAudio) tabs.push("audio");
    tabs.push("aiEdit");
  }

  const activeTab = tabs.includes(tab as never) ? tab : tabs[0];
  const aspect = mediaCanvasAspect(
    mediaItem?.width,
    mediaItem?.height,
    timeline.width,
    timeline.height,
  );
  // Raw SOURCE pixel aspect (sourceWidth / sourceHeight) for the Crop aspect-lock
  // menu — distinct from `aspect` above (canvas-normalized). 1:1 with upstream
  // `sourcePixelAspect(for:)` (CropOverlayView.swift:207-212).
  const sourcePixelAspect =
    mediaItem?.width && mediaItem?.height && mediaItem.height > 0
      ? mediaItem.width / mediaItem.height
      : null;

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
  const sampledOpacity = rawOpacityAt(clip, activeFrame);
  const sampledVolume = liveVolumeKfLinearAt(clip, activeFrame) ?? clip.volume;
  const sampledRotation = rotationAt(clip, activeFrame);
  const sampledScale = sizeAt(clip, activeFrame)[0];
  const sampledTopLeft = topLeftAt(clip, activeFrame);
  const sampledCrop = cropAt(clip, activeFrame);

  return (
    <div>
      {tabs.length > 1 && (
        <div
          role="tablist"
          aria-label={t("inspector.title")}
          style={{
            display: "flex",
            gap: "var(--space-md)",
            padding: "var(--space-xs) var(--space-lg) 0",
          }}
        >
          {tabs.map((tabId) => (
            <button
              key={tabId}
              type="button"
              role="tab"
              aria-selected={activeTab === tabId}
              onClick={() => setTab(tabId)}
              style={{
                paddingBottom: 4,
                fontSize: "var(--fs-sm-md)",
                fontWeight: activeTab === tabId ? "var(--fw-medium)" : "var(--fw-regular)",
                color: activeTab === tabId ? "var(--text-primary)" : "var(--text-tertiary)",
                ...(tabId === "aiEdit"
                  ? {
                      background: "var(--ai-gradient)",
                      WebkitBackgroundClip: "text",
                      WebkitTextFillColor: "transparent",
                      opacity: activeTab === tabId ? 1 : 0.6,
                    }
                  : {}),
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
        ) : activeTab === "aiEdit" ? (
          <AiEditTab
            clip={clip}
            fps={timeline.fps}
            unavailableReason={mediaItem?.missing ? t("inspector.aiEdit.unavailable") : null}
          />
        ) : activeTab === "audio" ? (
          <section>
            <SectionHeader label={t("inspector.section.levels")} />
            <Row label={t("inspector.field.volume")}>
              <ScrubbableNumberField
                value={volumeAnimated ? sampledVolume : clip.volume}
                min={0}
                max={4}
                sensitivity={0.01}
                format={(v) => (20 * Math.log10(Math.max(1e-6, v))).toFixed(1)}
                suffix=" dB"
                width={56}
                displayTextOverride={(v) => (v <= 0 ? "-∞ dB" : null)}
                onCommit={(v) =>
                  volumeAnimated
                    ? edit.upsertKeyframe(clip.id, "volume", activeFrame, volumeKeyframeValue(v))
                    : commit({ volume: v })
                }
              />
              {volumeAnimated && <AnimatedHint t={t} />}
              <KeyframeRowControls clip={clip} property="volume" activeFrame={activeFrame} t={t} />
            </Row>
            <FadeSection clip={clip} commit={commit} t={t} />
          </section>
        ) : (
          <>
            <section>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <SectionHeader label={t("inspector.section.transform")} />
                <HoverButton
                  title={t("inspector.action.resetTransform")}
                  onClick={() => edit.resetTransform([clip.id])}
                  size={18}
                >
                  <Icon icon={RotateCcw} size={12} />
                </HoverButton>
              </div>
              <Row label={t("inspector.field.scale")}>
                <ScrubbableNumberField
                  value={sampledScale}
                  min={0.01}
                  max={10}
                  sensitivity={0.005}
                  format={(v) => Math.round(v * 100).toString()}
                  suffix="%"
                  width={56}
                  onCommit={(v) =>
                    scaleAnimated
                      ? edit.upsertKeyframe(
                          clip.id,
                          "scale",
                          activeFrame,
                          scaleKeyframeValue(clip.transform, v, aspect),
                        )
                      : commit({
                          transform: resizeTransformKeepingSourceAspect(clip.transform, v, aspect),
                        })
                  }
                />
                {scaleAnimated && <AnimatedHint t={t} />}
                <KeyframeRowControls clip={clip} property="scale" activeFrame={activeFrame} t={t} />
              </Row>
              <Row label={t("inspector.field.rotation")}>
                <ScrubbableNumberField
                  value={sampledRotation}
                  min={-3600}
                  max={3600}
                  sensitivity={0.5}
                  format={(v) => v.toFixed(0)}
                  suffix="°"
                  width={56}
                  onCommit={(v) =>
                    rotationAnimated
                      ? edit.upsertKeyframe(clip.id, "rotation", activeFrame, rotationKeyframeValue(v))
                      : commit({ transform: { ...clip.transform, rotation: v } })
                  }
                />
                {rotationAnimated && <AnimatedHint t={t} />}
                <KeyframeRowControls clip={clip} property="rotation" activeFrame={activeFrame} t={t} />
              </Row>
              <Row label={t("inspector.field.opacity")}>
                <ScrubbableNumberField
                  value={opacityAnimated ? sampledOpacity : clip.opacity}
                  min={0}
                  max={1}
                  sensitivity={0.005}
                  format={(v) => Math.round(v * 100).toString()}
                  suffix="%"
                  width={56}
                  onCommit={(v) =>
                    opacityAnimated
                      ? edit.upsertKeyframe(
                          clip.id,
                          "opacity",
                          activeFrame,
                          opacityKeyframeValue(v * 100),
                        )
                      : commit({ opacity: v })
                  }
                />
                {opacityAnimated && <AnimatedHint t={t} />}
                <KeyframeRowControls clip={clip} property="opacity" activeFrame={activeFrame} t={t} />
              </Row>
            </section>

            <PositionSection
              clip={clip}
              sampledTopLeft={sampledTopLeft}
              animated={positionAnimated}
              activeFrame={activeFrame}
              commit={commit}
              t={t}
            />

            <CropSection
              clip={clip}
              sampledCrop={sampledCrop}
              animated={cropAnimated}
              activeFrame={activeFrame}
              commit={commit}
              sourcePixelAspect={sourcePixelAspect}
              t={t}
            />

            <FlipSection clip={clip} commit={commit} t={t} />

            <FadeSection clip={clip} commit={commit} t={t} />

            {clip.mediaType === "video" && !clip.nestedSequenceId && (
              <StabilizationSection clip={clip} t={t} />
            )}

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
  activeFrame,
  commit,
  t,
}: {
  clip: Clip;
  sampledTopLeft: { x: number; y: number };
  animated: boolean;
  activeFrame: number;
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
        <ScrubbableNumberField
          value={sampledTopLeft.x}
          min={-2}
          max={2}
          sensitivity={0.005}
          format={(v) => v.toFixed(3)}
          width={56}
          onCommit={(v) =>
            animated
              ? edit.upsertKeyframe(
                  clip.id,
                  "position",
                  activeFrame,
                  positionXKeyframeValue(v, sampledTopLeft.y),
                )
              : commit({ transform: { ...clip.transform, centerX: v + w / 2 } })
          }
        />
        {animated && <AnimatedHint t={t} />}
        <KeyframeRowControls clip={clip} property="position" activeFrame={activeFrame} t={t} />
      </Row>
      <Row label={t("inspector.field.positionY")}>
        <ScrubbableNumberField
          value={sampledTopLeft.y}
          min={-2}
          max={2}
          sensitivity={0.005}
          format={(v) => v.toFixed(3)}
          width={56}
          onCommit={(v) =>
            animated
              ? edit.upsertKeyframe(
                  clip.id,
                  "position",
                  activeFrame,
                  positionYKeyframeValue(sampledTopLeft.x, v),
                )
              : commit({ transform: { ...clip.transform, centerY: v + h / 2 } })
          }
        />
        {animated && <AnimatedHint t={t} />}
      </Row>
    </section>
  );
}

// MARK: - Crop section (on-canvas toggle + aspect preset + 4 edge insets, 0–1)

const CROP_ASPECT_LABEL_KEY: Record<CropAspectLock, string> = {
  free: "inspector.cropAspect.free",
  original: "inspector.cropAspect.original",
  r16x9: "inspector.cropAspect.r16x9",
  r9x16: "inspector.cropAspect.r9x16",
  r1x1: "inspector.cropAspect.r1x1",
  r4x3: "inspector.cropAspect.r4x3",
  r3x4: "inspector.cropAspect.r3x4",
  r21x9: "inspector.cropAspect.r21x9",
};

function CropSection({
  clip,
  sampledCrop,
  animated,
  activeFrame,
  commit,
  sourcePixelAspect,
  t,
}: {
  clip: Clip;
  sampledCrop: Crop;
  animated: boolean;
  activeFrame: number;
  commit: (props: Parameters<typeof edit.setClipProperties>[1]) => void;
  sourcePixelAspect: number | null;
  t: TFunction;
}) {
  const cropEditingActive = useEditorUiStore((s) => s.cropEditingActive);
  const toggleCropEditingActive = useEditorUiStore((s) => s.toggleCropEditingActive);
  const cropAspectLock = useEditorUiStore((s) => s.cropAspectLock);
  const setCropAspectLock = useEditorUiStore((s) => s.setCropAspectLock);

  // 1:1 port of `applyCropPreset(_:on:)` (InspectorView.swift:851-863): `free`
  // only updates the lock state (no crop mutation — the user keeps the current
  // shape and drags freely); `original` resets to the identity crop; the sized
  // presets commit the largest centered crop matching that pixel aspect.
  const applyCropPreset = (preset: CropAspectLock) => {
    setCropAspectLock(preset);
    const next = cropForPreset(preset, sourcePixelAspect);
    if (next === null) return;
    if (animated) {
      void edit.upsertKeyframe(clip.id, "crop", activeFrame, { kind: "crop", value: next });
    } else {
      commit({ crop: next });
    }
  };

  const commitEdge = (edge: keyof Crop, v: number) => {
    const next: Crop = { ...clip.crop, [edge]: v };
    commit({ crop: next });
  };
  const renderEdge = (label: string, edge: keyof Crop, value: number) => (
    <Row label={label}>
      <ScrubbableNumberField
        value={value}
        min={0}
        max={1}
        sensitivity={0.005}
        format={(v) => v.toFixed(3)}
        width={56}
        onCommit={(v) =>
          animated
            ? edit.upsertKeyframe(
                clip.id,
                "crop",
                activeFrame,
                cropEdgeKeyframeValue(sampledCrop, edge, v),
              )
            : commitEdge(edge, v)
        }
      />
      {animated && <AnimatedHint t={t} />}
    </Row>
  );
  return (
    <section>
      <SectionHeader label={t("inspector.section.crop")} />
      <Row label={t("inspector.field.cropEditOnCanvas")}>
        <HoverButton
          title={t(
            cropEditingActive ? "inspector.action.cropEditStop" : "inspector.action.cropEditStart",
          )}
          active={cropEditingActive}
          onClick={toggleCropEditingActive}
          size={20}
        >
          <Icon icon={CropIcon} size={13} />
        </HoverButton>
        <select
          value={cropAspectLock}
          onChange={(e) => applyCropPreset(e.target.value as CropAspectLock)}
          title={t("inspector.field.cropAspect")}
          style={{
            fontSize: "var(--fs-sm)",
            color: "var(--accent-primary)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-xs)",
            padding: "1px 4px",
          }}
        >
          {CROP_ASPECT_LOCKS.map((preset) => (
            <option key={preset} value={preset}>
              {t(CROP_ASPECT_LABEL_KEY[preset])}
            </option>
          ))}
        </select>
        <KeyframeRowControls clip={clip} property="crop" activeFrame={activeFrame} t={t} />
      </Row>
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

function StabilizationSection({ clip, t }: { clip: Clip; t: TFunction }) {
  const [analyzing, setAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const solution = clip.stabilization;

  useEffect(() => {
    setAnalyzing(false);
    setError(null);
  }, [clip.id]);

  const analyze = async () => {
    setAnalyzing(true);
    setError(null);
    try {
      await edit.analyzeAndApplyStabilization(clip.id);
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason);
      if (!/\bcancell?ed\b/i.test(message)) setError(message);
    } finally {
      setAnalyzing(false);
    }
  };

  return (
    <section data-testid="stabilization-section">
      <SectionHeader label={t("inspector.section.stabilization")} />
      {!solution ? (
        <div style={{ padding: `0 ${SPACE.lg}px ${SPACE.sm}px` }}>
          <button type="button" style={controlStyle} disabled={analyzing} onClick={() => void analyze()}>
            {analyzing
              ? t("inspector.stabilization.analyzing")
              : t("inspector.stabilization.analyzeApply")}
          </button>
          {analyzing && (
            <button
              type="button"
              style={{ ...controlStyle, marginLeft: SPACE.xs }}
              onClick={() => void edit.cancelStabilizationAnalysis()}
            >
              {t("inspector.stabilization.cancel")}
            </button>
          )}
        </div>
      ) : (
        <>
          <Row label={t("inspector.stabilization.strength")}>
            <ScrubbableNumberField
              value={solution.strength * 100}
              min={0}
              max={100}
              sensitivity={1}
              format={(value) => value.toFixed(0)}
              suffix="%"
              width={56}
              onCommit={(value) =>
                void edit.adjustStabilization(clip.id, { strength: value / 100 })
              }
            />
          </Row>
          <Row label={t("inspector.stabilization.cropMargin")}>
            <ScrubbableNumberField
              value={solution.cropMargin * 100}
              min={0}
              max={50}
              sensitivity={0.5}
              format={(value) => value.toFixed(1)}
              suffix="%"
              width={56}
              onCommit={(value) =>
                void edit.adjustStabilization(clip.id, { cropMargin: value / 100 })
              }
            />
          </Row>
          <div
            style={{
              padding: `0 ${SPACE.lg}px ${SPACE.sm}px`,
              color: "var(--text-tertiary)",
              fontSize: FS.xs,
            }}
          >
            {solution.model} v{solution.modelVersion} · {solution.keyframes.length}{" "}
            {t("inspector.stabilization.samples")}
          </div>
          <div style={{ padding: `0 ${SPACE.lg}px ${SPACE.sm}px` }}>
            <button
              type="button"
              style={controlStyle}
              disabled={analyzing}
              onClick={() => void analyze()}
            >
              {analyzing
                ? t("inspector.stabilization.analyzing")
                : t("inspector.stabilization.reanalyze")}
            </button>
            <button
              type="button"
              style={{ ...controlStyle, marginLeft: SPACE.xs }}
              onClick={() =>
                void (analyzing
                  ? edit.cancelStabilizationAnalysis()
                  : edit.resetStabilization(clip.id))
              }
            >
              {analyzing
                ? t("inspector.stabilization.cancel")
                : t("inspector.stabilization.reset")}
            </button>
          </div>
        </>
      )}
      {error && (
        <div role="alert" style={{ padding: `0 ${SPACE.lg}px ${SPACE.sm}px`, color: "var(--danger)" }}>
          {error}
        </div>
      )}
    </section>
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
  const updateCommon = (field: "feather" | "invert", value: number | boolean) =>
    setDraft((m) => ({ ...m, [field]: value }));
  const commitCommon = (field: "feather" | "invert", value: number | boolean) =>
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
                else if (kind === "poly") setShape(defaultPolyShape());
              }}
              style={controlStyle}
            >
              <option value="circle">{t("inspector.mask.circle")}</option>
              <option value="linear">{t("inspector.mask.linear")}</option>
              <option value="poly">
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
          ) : (
            <PolyMaskFields
              shape={draft.shape}
              setShape={setShape}
              setDraftShape={(shape) => setDraft((m) => ({ ...m, shape }))}
              t={t}
            />
          )}
          <MaskTransformFields
            mask={draft}
            setDraft={setDraft}
            commitMask={commitMask}
            t={t}
          />
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
          <button
            type="button"
            onClick={() => setMaskEnabled(false)}
            style={{ ...controlStyle, width: "100%", marginTop: SPACE.xs, color: "#ff8f8f" }}
          >
            {t("inspector.mask.delete")}
          </button>
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

function PolyMaskFields({
  shape,
  setShape,
  setDraftShape,
  t,
}: {
  shape: Extract<MaskShape, { kind: "poly" }>;
  setShape: (shape: MaskShape) => void;
  setDraftShape: (shape: MaskShape) => void;
  t: TFunction;
}) {
  const updatePoint = (index: number, axis: keyof RgbPoint, value: number, commit: boolean) => {
    const points = shape.points.map((point, pointIndex) =>
      pointIndex === index ? { ...point, [axis]: value } : point,
    );
    if (commit) setShape({ ...shape, points });
    else setDraftShape({ ...shape, points });
  };
  const addPoint = () => {
    if (shape.points.length >= 16) return;
    const last = shape.points[shape.points.length - 1] ?? { x: 0.5, y: 0.5 };
    setShape({
      ...shape,
      points: [...shape.points, { x: Math.min(1, last.x + 0.05), y: Math.min(1, last.y + 0.05) }],
    });
  };
  const deletePoint = (index: number) => {
    if (shape.points.length <= 3) return;
    setShape({ ...shape, points: shape.points.filter((_, pointIndex) => pointIndex !== index) });
  };

  return (
    <>
      {shape.points.map((point, index) => (
        <div key={index} style={{ borderTop: "1px solid rgba(255,255,255,0.06)", paddingTop: SPACE.xs }}>
          <MaskNumberRow
            label={`P${index + 1} X`}
            value={point.x}
            min={0}
            max={1}
            onChange={(value) => updatePoint(index, "x", value, false)}
            onCommit={(value) => updatePoint(index, "x", value, true)}
          />
          <MaskNumberRow
            label={`P${index + 1} Y`}
            value={point.y}
            min={0}
            max={1}
            onChange={(value) => updatePoint(index, "y", value, false)}
            onCommit={(value) => updatePoint(index, "y", value, true)}
          />
          <button
            type="button"
            disabled={shape.points.length <= 3}
            onClick={() => deletePoint(index)}
            style={{ ...controlStyle, width: "100%", opacity: shape.points.length <= 3 ? 0.45 : 1 }}
          >
            {t("inspector.mask.deletePoint")} {index + 1}
          </button>
        </div>
      ))}
      <button
        type="button"
        disabled={shape.points.length >= 16}
        onClick={addPoint}
        style={{ ...controlStyle, width: "100%", marginTop: SPACE.xs }}
      >
        {t("inspector.mask.addPoint")}
      </button>
    </>
  );
}

function MaskTransformFields({
  mask,
  setDraft,
  commitMask,
  t,
}: {
  mask: Mask;
  setDraft: Dispatch<SetStateAction<Mask>>;
  commitMask: (mask: Mask) => void;
  t: TFunction;
}) {
  const transform = completeMaskTransform(mask.transform);
  const update = (next: typeof transform, commit: boolean) => {
    if (commit) commitMask({ ...mask, transform: next });
    else setDraft((current) => ({ ...current, transform: next }));
  };
  const pointField = (field: "offset" | "scale", axis: keyof RgbPoint, value: number, commit: boolean) =>
    update({ ...transform, [field]: { ...transform[field], [axis]: value } }, commit);

  return (
    <>
      <MaskNumberRow label={t("inspector.mask.offsetX")} value={transform.offset.x} onChange={(v) => pointField("offset", "x", v, false)} onCommit={(v) => pointField("offset", "x", v, true)} />
      <MaskNumberRow label={t("inspector.mask.offsetY")} value={transform.offset.y} onChange={(v) => pointField("offset", "y", v, false)} onCommit={(v) => pointField("offset", "y", v, true)} />
      <MaskNumberRow label={t("inspector.mask.scaleX")} value={transform.scale.x} min={0.05} max={4} onChange={(v) => pointField("scale", "x", v, false)} onCommit={(v) => pointField("scale", "x", v, true)} />
      <MaskNumberRow label={t("inspector.mask.scaleY")} value={transform.scale.y} min={0.05} max={4} onChange={(v) => pointField("scale", "y", v, false)} onCommit={(v) => pointField("scale", "y", v, true)} />
      <EffectNumberRow label={t("inspector.mask.rotation")} value={transform.rotationDegrees} min={-180} max={180} sensitivity={0.5} format={(v) => `${v.toFixed(1)}°`} onChange={(v) => update({ ...transform, rotationDegrees: v }, false)} onCommit={(v) => update({ ...transform, rotationDegrees: v }, true)} />
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
    transform: completeMaskTransform(mask?.transform),
  };
}

function completeMaskTransform(transform: Mask["transform"]) {
  return {
    offset: { x: transform?.offset.x ?? 0, y: transform?.offset.y ?? 0 },
    scale: { x: transform?.scale.x ?? 1, y: transform?.scale.y ?? 1 },
    rotationDegrees: transform?.rotationDegrees ?? 0,
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

function defaultPolyShape(): Extract<MaskShape, { kind: "poly" }> {
  return {
    kind: "poly",
    points: [
      { x: 0.25, y: 0.25 },
      { x: 0.75, y: 0.25 },
      { x: 0.75, y: 0.75 },
      { x: 0.25, y: 0.75 },
    ],
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

// MARK: - Media-asset Source inspector (upstream `mediaAssetInspectorContent`
// / `assetDetailsContent` / `fileSection`, InspectorView.swift:865-1006).

const CLIP_TYPE_LABEL_KEY: Record<ClipType, string> = {
  video: "clipType.video",
  audio: "clipType.audio",
  image: "clipType.image",
  text: "clipType.text",
  lottie: "clipType.lottie",
};

/** The "Source" inspector state shown when a media-panel asset is active (and no
 *  clip is selected). Renders the identity header + File section always, and the
 *  Generated / Prompt / References sections only when the asset carries a
 *  `generationInput` (AI-generated). Today nothing populates `generationInput`
 *  (generate_* is still stubbed), so those sections never render in practice —
 *  the gating is 1:1 with upstream's `if let gen = asset.generationInput` and
 *  lights up automatically once generation lands. */
function MediaAssetSource({ asset, t }: { asset: MediaItem; t: TFunction }) {
  const isAudio = asset.type === "audio";
  const isImage = asset.type === "image";
  const gen = asset.generationInput ?? null;
  return (
    <div
      style={{
        padding: "var(--space-lg)",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xl)",
      }}
    >
      {/* Identity header: name (+ AI badge when generated). */}
      <div style={{ display: "flex", alignItems: "baseline", gap: "var(--space-sm)" }}>
        <span
          style={{
            minWidth: 0,
            fontSize: "var(--fs-lg)",
            fontWeight: "var(--fw-semibold)",
            color: "var(--text-primary)",
            overflowWrap: "anywhere",
            userSelect: "text",
          }}
        >
          {asset.name}
        </span>
        {gen && <AiBadge t={t} />}
      </div>

      {/* File section (upstream `fileSection`). Dimensions skipped for audio;
          Duration skipped for stills; Size/Path shown when resolvable. */}
      <section>
        <SectionHeader label={t("inspector.source.file")} />
        <MetaRow label={t("inspector.source.type")} value={t(CLIP_TYPE_LABEL_KEY[asset.type])} />
        {!isAudio && asset.width != null && asset.height != null && (
          <MetaRow
            label={t("inspector.source.dimensions")}
            value={`${asset.width} × ${asset.height}`}
          />
        )}
        {asset.duration > 0 && !isImage && (
          <MetaRow
            label={t("inspector.source.duration")}
            value={formatMediaDuration(asset.duration)}
          />
        )}
        {asset.fileSize != null && (
          <MetaRow label={t("inspector.source.size")} value={formatFileSize(asset.fileSize)} />
        )}
        {asset.path && <MetaRow label={t("inspector.source.path")} value={asset.path} />}
      </section>

      {gen && <GenerationSections gen={gen} t={t} />}
    </div>
  );
}

/** Small "AI" pill shown beside a generated asset's name (upstream `aiBadge`). */
function AiBadge({ t }: { t: TFunction }) {
  return (
    <span
      style={{
        flex: "0 0 auto",
        fontSize: "var(--fs-xxs)",
        fontWeight: "var(--fw-bold)",
        letterSpacing: "var(--tracking-wide)",
        color: "var(--accent-primary)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-sm)",
        padding: "1px 6px",
      }}
    >
      {t("inspector.source.aiBadge")}
    </span>
  );
}

/** Generated / Prompt / References sections for an AI-generated asset (upstream
 *  `assetDetailsContent`'s `if let gen` block). References resolves reference
 *  asset ids to library names (upstream renders a thumbnail strip; the name list
 *  is the lightweight equivalent and only shows resolvable ids). */
function GenerationSections({ gen, t }: { gen: GenerationInput; t: TFunction }) {
  const referenceIds = [
    ...(gen.imageURLAssetIds ?? []),
    ...(gen.referenceImageAssetIds ?? []),
    ...(gen.referenceVideoAssetIds ?? []),
    ...(gen.referenceAudioAssetIds ?? []),
  ];
  const references = useMediaStore((s) =>
    referenceIds
      .map((id) => s.items.find((m) => m.id === id))
      .filter((m): m is MediaItem => m != null),
  );
  return (
    <>
      {references.length > 0 && (
        <section>
          <SectionHeader label={t("inspector.source.references")} />
          {references.map((ref) => (
            <MetaRow key={ref.id} label={t(CLIP_TYPE_LABEL_KEY[ref.type])} value={ref.name} />
          ))}
        </section>
      )}

      <section>
        <SectionHeader label={t("inspector.source.generated")} />
        <MetaRow label={t("inspector.source.model")} value={gen.model} />
        {gen.aspectRatio.length > 0 && (
          <MetaRow label={t("inspector.source.aspectRatio")} value={gen.aspectRatio} />
        )}
        {gen.resolution && (
          <MetaRow label={t("inspector.source.resolution")} value={gen.resolution} />
        )}
        {gen.duration > 0 && (
          <MetaRow label={t("inspector.source.durationField")} value={`${gen.duration}s`} />
        )}
      </section>

      {gen.prompt.length > 0 && (
        <section>
          <SectionHeader label={t("inspector.source.prompt")} />
          <div
            style={{
              fontSize: "var(--fs-sm)",
              color: "var(--text-secondary)",
              whiteSpace: "pre-wrap",
              overflowWrap: "anywhere",
              userSelect: "text",
            }}
          >
            {gen.prompt}
          </div>
        </section>
      )}
    </>
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
