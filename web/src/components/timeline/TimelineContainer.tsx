/**
 * Timeline container (SPEC §5). Owns the scroll area, the content + ruler
 * canvases, the fixed track-header column, and the playhead/snap overlays, plus
 * the pointer-gesture decision tree (SPEC §5.8, §9): scrub, select, move, trim,
 * razor split, marquee, and Option/Cmd wheel zoom/pan.
 */

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { LAYOUT, ZOOM } from "../../lib/theme";
import {
  contentHeight,
  contentWidth,
  frameAt,
  totalFrames,
  trackAt,
} from "../../lib/geometry";
import { firstAudioIndex } from "../../lib/zones";
import { collectTargets, findSnap } from "../../lib/snap";
import { paintTimeline } from "./timelineCanvas";
import { paintRuler } from "./rulerCanvas";
import { TrackHeaderColumn } from "./TrackHeaderColumn";
import { Playhead } from "./Playhead";
import { SnapIndicator } from "./SnapIndicator";
import { hitTestClip, expandLinkGroup, clipsInRect, type ClipHit } from "./hitTest";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import * as edit from "../../store/editActions";

type DragState =
  | { kind: "scrub" }
  | { kind: "move"; hit: ClipHit; grabFrame: number; deltaFrames: number; startTrack: number; targetTrack: number; companions: string[] }
  | { kind: "trimLeft" | "trimRight"; hit: ClipHit; startTrim: number; deltaFrames: number }
  | { kind: "marquee"; startDocX: number; startDocY: number; curDocX: number; curDocY: number }
  | null;

export function TimelineContainer() {
  const timeline = useProjectStore((s) => s.timeline);
  const zoomScale = useEditorUiStore((s) => s.zoomScale);
  const setZoomScale = useEditorUiStore((s) => s.setZoomScale);
  const setMinZoomScale = useEditorUiStore((s) => s.setMinZoomScale);
  const scrollLeft = useEditorUiStore((s) => s.scrollLeft);
  const scrollTop = useEditorUiStore((s) => s.scrollTop);
  const setScroll = useEditorUiStore((s) => s.setScroll);
  const setVisibleWidth = useEditorUiStore((s) => s.setVisibleWidth);
  const toolMode = useEditorUiStore((s) => s.toolMode);
  const activeFrame = useEditorUiStore((s) => s.activeFrame);
  const isPlaying = useEditorUiStore((s) => s.isPlaying);
  const setCurrentFrame = useEditorUiStore((s) => s.setCurrentFrame);
  const selectedClipIds = useEditorUiStore((s) => s.selectedClipIds);
  const selectClips = useEditorUiStore((s) => s.selectClips);
  const clearSelection = useEditorUiStore((s) => s.clearSelection);
  const trackHeights = useEditorUiStore((s) => s.trackDisplayHeights);

  const viewportRef = useRef<HTMLDivElement>(null);
  const contentCanvasRef = useRef<HTMLCanvasElement>(null);
  const rulerCanvasRef = useRef<HTMLCanvasElement>(null);
  const [viewport, setViewport] = useState({ width: 0, height: 0 });
  const dragRef = useRef<DragState>(null);
  const [snapFrame, setSnapFrame] = useState<number | null>(null);
  const [, forceTick] = useState(0);

  const total = useMemo(() => totalFrames(timeline), [timeline]);
  const docWidth = useMemo(
    () => contentWidth(total, zoomScale, viewport.width),
    [total, zoomScale, viewport.width],
  );
  const docHeight = useMemo(
    () => contentHeight(timeline, viewport.height, trackHeights),
    [timeline, viewport.height, trackHeights],
  );
  const firstAudio = useMemo(() => firstAudioIndex(timeline), [timeline]);

  // Observe viewport size.
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const update = () => {
      const w = el.clientWidth - LAYOUT.trackHeaderWidth;
      const h = el.clientHeight;
      setViewport({ width: Math.max(0, w), height: h });
      setVisibleWidth(Math.max(0, w));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [setVisibleWidth]);

  // minZoomScale = fit all frames into the visible width (lower bound).
  useEffect(() => {
    if (viewport.width > 0 && total > 0) {
      const fit = viewport.width / total;
      setMinZoomScale(Math.min(ZOOM.default, Math.max(0.01, fit)));
    }
  }, [viewport.width, total, setMinZoomScale]);

  // Auto-scroll to keep the playhead visible during playback (upstream follows
  // the playhead, but never auto-selects the clip under it). Gated on isPlaying
  // so it never fights manual scrolling while paused; when the playhead nears a
  // horizontal edge it recenters to a quarter from the left.
  useEffect(() => {
    if (!isPlaying || viewport.width <= 0) return;
    const playheadX = activeFrame * zoomScale;
    const margin = 60;
    if (playheadX < scrollLeft + margin || playheadX > scrollLeft + viewport.width - margin) {
      const maxScroll = Math.max(0, docWidth - viewport.width);
      const target = Math.min(maxScroll, Math.max(0, playheadX - viewport.width * 0.25));
      if (target !== scrollLeft) setScroll(target, scrollTop);
    }
  }, [isPlaying, activeFrame, zoomScale, viewport.width, scrollLeft, scrollTop, docWidth, setScroll]);

  // Paint content canvas.
  useEffect(() => {
    const canvas = contentCanvasRef.current;
    if (!canvas || viewport.width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(viewport.width * dpr);
    canvas.height = Math.ceil(viewport.height * dpr);
    canvas.style.width = `${viewport.width}px`;
    canvas.style.height = `${viewport.height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    paintTimeline(ctx, {
      timeline,
      pixelsPerFrame: zoomScale,
      trackHeights,
      selectedClipIds,
      dpr,
      width: docWidth,
      height: docHeight,
      firstAudioIndex: firstAudio,
      scrollLeft,
      scrollTop,
      viewWidth: viewport.width,
      viewHeight: viewport.height,
    });
  }, [
    timeline,
    zoomScale,
    trackHeights,
    selectedClipIds,
    scrollLeft,
    scrollTop,
    viewport,
    docWidth,
    docHeight,
    firstAudio,
  ]);

  // Paint ruler canvas (sticky top).
  useEffect(() => {
    const canvas = rulerCanvasRef.current;
    if (!canvas || viewport.width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.ceil(viewport.width * dpr);
    canvas.height = Math.ceil(LAYOUT.rulerHeight * dpr);
    canvas.style.width = `${viewport.width}px`;
    canvas.style.height = `${LAYOUT.rulerHeight}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    paintRuler(ctx, { fps: timeline.fps, pixelsPerFrame: zoomScale, scrollLeft, width: viewport.width, dpr });
  }, [timeline.fps, zoomScale, scrollLeft, viewport.width]);

  // --- Coordinate helpers (event -> document space) ---
  const toDoc = useCallback(
    (e: { clientX: number; clientY: number }) => {
      const el = viewportRef.current;
      if (!el) return { docX: 0, docY: 0, inRuler: false };
      const rect = el.getBoundingClientRect();
      const vx = e.clientX - rect.left - LAYOUT.trackHeaderWidth;
      const vy = e.clientY - rect.top;
      return { docX: vx + scrollLeft, docY: vy + scrollTop, inRuler: vy < LAYOUT.rulerHeight };
    },
    [scrollLeft, scrollTop],
  );

  // --- Wheel: Option=zoom (cursor-anchored), Cmd=pan, else scroll ---
  const onWheel = useCallback(
    (e: React.WheelEvent) => {
      if (e.altKey) {
        e.preventDefault();
        const { docX } = toDoc(e);
        const anchorFrame = docX / zoomScale;
        const factor = Math.exp(e.deltaY * ZOOM.scrollSensitivity);
        const newScale = Math.max(
          useEditorUiStore.getState().minZoomScale,
          Math.min(ZOOM.max, zoomScale * factor),
        );
        setZoomScale(newScale);
        // Keep the frame under the cursor stationary.
        const newDocX = anchorFrame * newScale;
        const viewX = docX - scrollLeft;
        setScroll(Math.max(0, newDocX - viewX), scrollTop);
      } else if (e.metaKey || e.ctrlKey) {
        e.preventDefault();
        // Upstream (TimelineInputController): delta = scrollingDeltaX * panSpeed,
        // with deltaX taking priority over deltaY. panSpeed (5) is applied 1:1.
        setScroll(Math.max(0, scrollLeft + (e.deltaX || e.deltaY) * ZOOM.panSpeed), scrollTop);
      } else {
        const maxLeft = Math.max(0, docWidth - viewport.width);
        const maxTop = Math.max(0, docHeight - viewport.height);
        setScroll(
          Math.max(0, Math.min(maxLeft, scrollLeft + e.deltaX)),
          Math.max(0, Math.min(maxTop, scrollTop + e.deltaY)),
        );
      }
    },
    [toDoc, zoomScale, scrollLeft, scrollTop, setZoomScale, setScroll, docWidth, docHeight, viewport],
  );

  // --- Pointer down: the decision tree (SPEC §5.8) ---
  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (e.button !== 0) return;
      const { docX, docY, inRuler } = toDoc(e);
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);

      // Ruler -> scrub playhead.
      if (inRuler) {
        dragRef.current = { kind: "scrub" };
        const f = frameAt(docX, zoomScale);
        setCurrentFrame(f);
        return;
      }

      const hit = hitTestClip(timeline, docX, docY, zoomScale, trackHeights);

      // Razor tool + clip -> split at click frame.
      if (toolMode === "razor" && hit) {
        const f = frameAt(docX, zoomScale);
        void edit.splitClip(hit.clip.id, f);
        dragRef.current = null;
        return;
      }

      if (hit) {
        // Selection logic (linkedOn = !Option).
        const linked = !e.altKey;
        const already = selectedClipIds.has(hit.clip.id);
        let nextSel: Set<string>;
        if (e.shiftKey) {
          nextSel = new Set(selectedClipIds);
          const group = linked
            ? expandLinkGroup(timeline, new Set([hit.clip.id]))
            : new Set([hit.clip.id]);
          if (already) group.forEach((id) => nextSel.delete(id));
          else group.forEach((id) => nextSel.add(id));
        } else if (e.altKey && !already) {
          nextSel = new Set([hit.clip.id]);
        } else if (!already) {
          nextSel = linked
            ? expandLinkGroup(timeline, new Set([hit.clip.id]))
            : new Set([hit.clip.id]);
        } else {
          nextSel = selectedClipIds;
        }
        selectClips(nextSel);

        // Sub-region: trim handles before body move.
        if (hit.region === "trimLeft" && !e.altKey) {
          dragRef.current = {
            kind: "trimLeft",
            hit,
            startTrim: hit.clip.trimStartFrame,
            deltaFrames: 0,
          };
        } else if (hit.region === "trimRight" && !e.altKey) {
          dragRef.current = {
            kind: "trimRight",
            hit,
            startTrim: hit.clip.trimEndFrame,
            deltaFrames: 0,
          };
        } else {
          const grabFrame = frameAt(docX, zoomScale);
          dragRef.current = {
            kind: "move",
            hit,
            grabFrame,
            deltaFrames: 0,
            startTrack: hit.trackIndex,
            targetTrack: hit.trackIndex,
            companions: [...nextSel],
          };
        }
        return;
      }

      // Empty space -> clear selection (non-shift) + start marquee.
      if (!e.shiftKey) clearSelection();
      dragRef.current = {
        kind: "marquee",
        startDocX: docX,
        startDocY: docY,
        curDocX: docX,
        curDocY: docY,
      };
    },
    [toDoc, timeline, zoomScale, trackHeights, toolMode, selectedClipIds, selectClips, clearSelection, setCurrentFrame],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const { docX, docY } = toDoc(e);

      if (d.kind === "scrub") {
        setCurrentFrame(frameAt(docX, zoomScale));
        return;
      }

      if (d.kind === "move") {
        const rawFrame = frameAt(docX, zoomScale);
        let deltaFrames = rawFrame - d.grabFrame;
        // Snap: probe the moved clip's edges.
        const excluded = new Set(d.companions);
        const targets = collectTargets(timeline, excluded, activeFrame);
        const movedStart = d.hit.clip.startFrame + deltaFrames;
        const movedEnd = movedStart + d.hit.clip.durationFrames;
        const snapStart = findSnap(movedStart, targets, zoomScale, null);
        const snapEnd = findSnap(movedEnd, targets, zoomScale, null);
        let snapped: number | null = null;
        if (snapStart && (!snapEnd || Math.abs(snapStart.frame - movedStart) <= Math.abs(snapEnd.frame - movedEnd))) {
          deltaFrames += snapStart.frame - movedStart;
          snapped = snapStart.frame;
        } else if (snapEnd) {
          deltaFrames += snapEnd.frame - movedEnd;
          snapped = snapEnd.frame;
        }
        // Clamp so the clip can't go before frame 0.
        if (d.hit.clip.startFrame + deltaFrames < 0) {
          deltaFrames = -d.hit.clip.startFrame;
          snapped = null;
        }
        const targetTrack = trackAt(timeline, docY, trackHeights) ?? d.startTrack;
        dragRef.current = { ...d, deltaFrames, targetTrack };
        setSnapFrame(snapped);
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "trimLeft" || d.kind === "trimRight") {
        const rawFrame = frameAt(docX, zoomScale);
        const edge = d.kind === "trimLeft" ? d.hit.clip.startFrame : d.hit.clip.startFrame + d.hit.clip.durationFrames;
        let deltaFrames = rawFrame - edge;
        const targets = collectTargets(timeline, new Set([d.hit.clip.id]), activeFrame);
        const snap = findSnap(rawFrame, targets, zoomScale, null);
        if (snap) {
          deltaFrames = snap.frame - edge;
          setSnapFrame(snap.frame);
        } else {
          setSnapFrame(null);
        }
        dragRef.current = { ...d, deltaFrames };
        forceTick((n) => n + 1);
        return;
      }

      if (d.kind === "marquee") {
        dragRef.current = { ...d, curDocX: docX, curDocY: docY };
        const ids = clipsInRect(timeline, d.startDocX, d.startDocY, docX, docY, zoomScale, trackHeights);
        const expanded = e.altKey ? ids : expandLinkGroup(timeline, ids);
        selectClips(expanded);
        forceTick((n) => n + 1);
      }
    },
    [toDoc, zoomScale, timeline, trackHeights, activeFrame, setCurrentFrame, selectClips],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      dragRef.current = null;
      setSnapFrame(null);
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      if (!d) return;

      if (d.kind === "move") {
        if (d.deltaFrames === 0 && d.targetTrack === d.startTrack) return; // no-op
        const moves = d.companions
          .map((id) => {
            const loc = findClipLoc(timeline, id);
            if (!loc) return null;
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            const trackDelta = d.targetTrack - d.startTrack;
            const toTrack = clamp(loc[0] + trackDelta, 0, timeline.tracks.length - 1);
            // Cross-track only when type-compatible; else stay.
            const compatible = compatibleTracks(timeline, loc[0], toTrack);
            return {
              clipId: id,
              toTrack: compatible ? toTrack : loc[0],
              toFrame: Math.max(0, clip.startFrame + d.deltaFrames),
            };
          })
          .filter((m): m is NonNullable<typeof m> => m !== null);
        void edit.moveClips(moves);
        return;
      }

      if (d.kind === "trimLeft") {
        if (d.deltaFrames === 0) return;
        const newTrim = Math.max(0, d.startTrim + d.deltaFrames);
        void edit.trimClips([
          { clipId: d.hit.clip.id, trimStartFrame: newTrim, trimEndFrame: d.hit.clip.trimEndFrame },
        ]);
        return;
      }
      if (d.kind === "trimRight") {
        if (d.deltaFrames === 0) return;
        const newTrim = Math.max(0, d.startTrim - d.deltaFrames);
        void edit.trimClips([
          { clipId: d.hit.clip.id, trimStartFrame: d.hit.clip.trimStartFrame, trimEndFrame: newTrim },
        ]);
      }
    },
    [timeline],
  );

  // Ghost preview offsets for the active drag (read from dragRef during render).
  const drag = dragRef.current;

  return (
    <div
      ref={viewportRef}
      style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden" }}
      onWheel={onWheel}
    >
      {/* Content canvas (clips + backgrounds), positioned right of header column. */}
      <canvas
        ref={contentCanvasRef}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        style={{
          position: "absolute",
          left: LAYOUT.trackHeaderWidth,
          top: 0,
          cursor: toolMode === "razor" ? "crosshair" : "default",
        }}
      />

      {/* Ruler canvas (sticky top, over content). */}
      <canvas
        ref={rulerCanvasRef}
        style={{
          position: "absolute",
          left: LAYOUT.trackHeaderWidth,
          top: 0,
          pointerEvents: "none",
          zIndex: 30,
        }}
      />

      {/* Fixed track header column. */}
      <TrackHeaderColumn timeline={timeline} scrollTop={scrollTop} totalHeight={docHeight} />

      {/* Overlays. */}
      <SnapIndicator
        frame={snapFrame}
        pixelsPerFrame={zoomScale}
        scrollLeft={scrollLeft}
        height={viewport.height}
      />
      <Playhead
        frame={activeFrame}
        pixelsPerFrame={zoomScale}
        scrollLeft={scrollLeft}
        height={viewport.height}
      />

      {/* Marquee box. */}
      {drag?.kind === "marquee" && (
        <MarqueeBox drag={drag} scrollLeft={scrollLeft} scrollTop={scrollTop} />
      )}

      {/* Horizontal scrollbar proxy (thin) — drag handled via wheel; kept minimal. */}
    </div>
  );
}

function MarqueeBox({
  drag,
  scrollLeft,
  scrollTop,
}: {
  drag: { startDocX: number; startDocY: number; curDocX: number; curDocY: number };
  scrollLeft: number;
  scrollTop: number;
}) {
  const x = Math.min(drag.startDocX, drag.curDocX) - scrollLeft + LAYOUT.trackHeaderWidth;
  const y = Math.min(drag.startDocY, drag.curDocY) - scrollTop;
  const w = Math.abs(drag.curDocX - drag.startDocX);
  const h = Math.abs(drag.curDocY - drag.startDocY);
  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left: x,
        top: y,
        width: w,
        height: h,
        background: "rgba(255,255,255,0.1)",
        border: "1px dashed rgba(255,255,255,0.6)",
        zIndex: 80,
        pointerEvents: "none",
      }}
    />
  );
}

function findClipLoc(timeline: { tracks: { clips: { id: string }[] }[] }, id: string): [number, number] | null {
  for (let ti = 0; ti < timeline.tracks.length; ti++) {
    const ci = timeline.tracks[ti].clips.findIndex((c) => c.id === id);
    if (ci >= 0) return [ti, ci];
  }
  return null;
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

function compatibleTracks(
  timeline: { tracks: { type: string }[] },
  a: number,
  b: number,
): boolean {
  const ta = timeline.tracks[a]?.type;
  const tb = timeline.tracks[b]?.type;
  if (!ta || !tb) return false;
  const visual = (t: string) => t === "video" || t === "image" || t === "text" || t === "lottie";
  return ta === tb || (visual(ta) && visual(tb));
}
