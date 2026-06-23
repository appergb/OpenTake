/**
 * Timeline container (SPEC §5). Owns the scroll area, the content + ruler
 * canvases, the fixed track-header column, and the playhead/snap overlays, plus
 * the pointer-gesture decision tree (SPEC §5.8, §9): scrub, select, move, trim,
 * razor split, marquee, and the CapCut/剪映 wheel model (pinch or Cmd/Ctrl
 * zoom, Option horizontal scroll, bare/two-finger pan).
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
import { clampTrimDeltaFrames, trimSourceValues } from "../../lib/clip";
import { collectTargets, findSnap } from "../../lib/snap";
import { paintTimeline, type DragPaint } from "./timelineCanvas";
import { useT } from "../../i18n";
import { paintRuler } from "./rulerCanvas";
import { TrackHeaderColumn } from "./TrackHeaderColumn";
import { Playhead } from "./Playhead";
import { SnapIndicator } from "./SnapIndicator";
import { hitTestClip, expandLinkGroup, clipsInRect, type ClipHit } from "./hitTest";
import { ClipContextMenu } from "./ClipContextMenu";
import { useProjectStore } from "../../store/projectStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useMediaStore } from "../../store/mediaStore";
import * as edit from "../../store/editActions";
import { getWaveform } from "../../lib/api";

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
  const mediaItems = useMediaStore((s) => s.items);

  // Asset ids whose source file is offline → clips referencing them get the
  // error wash. Recomputed when the catalog changes (so a relink clears it).
  const missingMediaRefs = useMemo(
    () => new Set(mediaItems.filter((m) => m.missing).map((m) => m.id)),
    [mediaItems],
  );

  const viewportRef = useRef<HTMLDivElement>(null);
  const contentCanvasRef = useRef<HTMLCanvasElement>(null);
  const rulerCanvasRef = useRef<HTMLCanvasElement>(null);
  const [viewport, setViewport] = useState({ width: 0, height: 0 });
  const dragRef = useRef<DragState>(null);
  const [snapFrame, setSnapFrame] = useState<number | null>(null);
  const [dragTick, forceTick] = useState(0);
  const t = useT();
  // Waveform sample cache (media id → buckets), loaded on demand from Rust.
  const waveformsRef = useRef<Map<string, number[]>>(new Map());
  // Refs of media whose waveform fetch is currently in flight — kept separate from
  // the resolved-cache `waveformsRef` so a failed/empty fetch can be retried on a
  // later effect run instead of being permanently suppressed by a placeholder (#127).
  const inFlightRef = useRef<Set<string>>(new Set());
  // Guards `setWaveformVersion` against firing after unmount (the cache write itself
  // is mount-independent and must NOT be discarded on re-render — see #127).
  const mountedRef = useRef(true);
  useEffect(() => () => { mountedRef.current = false; }, []);
  const [waveformVersion, setWaveformVersion] = useState(0);
  // Right-click context menu state. `x/y` are viewport coords (clientX/clientY)
  // so ClipContextMenu can position itself with position:fixed (#93 review #108).
  const [menu, setMenu] = useState<{ clipId: string; x: number; y: number } | null>(null);

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
    // Project the active drag so dragged clips render at their live position
    // (ghost) — `dragTick` (bumped each pointer-move) re-runs this effect.
    const d = dragRef.current;
    let drag: DragPaint | undefined;
    if (d?.kind === "move") {
      drag = {
        kind: "move",
        ids: new Set(d.companions),
        deltaFrames: d.deltaFrames,
        trackDelta: d.targetTrack - d.startTrack,
      };
    } else if (d?.kind === "trimLeft" || d?.kind === "trimRight") {
      drag = {
        kind: "trim",
        clipId: d.hit.clip.id,
        edge: d.kind === "trimLeft" ? "left" : "right",
        deltaFrames: d.deltaFrames,
      };
    }
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
      waveforms: waveformsRef.current,
      missingMediaRefs,
      emptyLabel: t("timeline.dropHint"),
      drag,
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
    waveformVersion,
    missingMediaRefs,
    dragTick,
    t,
  ]);

  // Load waveform samples for every audio clip's source on demand (cached by
  // media id), then trigger a repaint. The real bars replace the faint band
  // once the Rust `get_waveform` cache resolves.
  useEffect(() => {
    const wanted = new Set<string>();
    for (const track of timeline.tracks) {
      for (const clip of track.clips) {
        if (clip.mediaType === "audio") wanted.add(clip.mediaRef);
      }
    }
    for (const ref of wanted) {
      // Skip only if already resolved (cached) or a fetch is in flight. A failed or
      // empty fetch leaves no placeholder, so a later effect run retries it.
      if (waveformsRef.current.has(ref) || inFlightRef.current.has(ref)) continue;
      inFlightRef.current.add(ref);
      void getWaveform(ref)
        .then((samples) => {
          // Write the cache even if `timeline` changed meanwhile — a ref write is
          // idempotent and mount-independent. Discarding valid results on every edit
          // was exactly what dropped waveforms intermittently (#127). Only the
          // repaint bump is guarded against unmount.
          if (samples && samples.length > 0) {
            waveformsRef.current.set(ref, samples);
            if (mountedRef.current) setWaveformVersion((v) => v + 1);
          }
        })
        .finally(() => {
          // Clear in-flight so a failed/empty ref is retried on the next effect run.
          inFlightRef.current.delete(ref);
        });
    }
  }, [timeline]);

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

  // --- Wheel: 1:1 with CapCut/剪映's scroll-wheel & trackpad model ---
  //   • pinch (ctrlKey, set by the browser on a trackpad pinch) OR Cmd (Mac) /
  //     Ctrl (Win) + scroll → cursor-anchored ZOOM (剪映: "Ctrl/Cmd + 滚轮 缩放，
  //     以当前位置为原点").
  //   • Option (altKey) + scroll → HORIZONTAL scroll (剪映: "Alt + 滚轮 = 左右").
  //   • bare scroll / two-finger swipe → pan (剪映: "滚轮 = 上下"); on a trackpad
  //     deltaX also pans horizontally, so a two-finger swipe moves the timeline
  //     in any direction.
  const onWheel = useCallback(
    (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
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
      } else if (e.altKey) {
        e.preventDefault();
        // Option + scroll = horizontal (剪映 Alt+滚轮). A mouse wheel only has
        // deltaY, so fall back to it when there's no deltaX.
        const maxLeft = Math.max(0, docWidth - viewport.width);
        const dx = (e.deltaX || e.deltaY) * ZOOM.panSpeed;
        setScroll(Math.max(0, Math.min(maxLeft, scrollLeft + dx)), scrollTop);
      } else {
        // Bare scroll / two-finger swipe pans the timeline: vertical (剪映 上下)
        // plus horizontal on a trackpad. preventDefault stops the macOS
        // two-finger swipe from triggering browser back/forward navigation.
        e.preventDefault();
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

  // Attach the wheel handler natively with { passive: false }. React's onWheel
  // is passive, so preventDefault() there silently no-ops — but a trackpad pinch
  // is Ctrl+wheel, which the webview would otherwise turn into a PAGE zoom, and a
  // two-finger swipe could trigger back/forward navigation. A latest-ref keeps
  // the listener stable while always running the current closure.
  const onWheelRef = useRef(onWheel);
  onWheelRef.current = onWheel;
  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => onWheelRef.current(e);
    el.addEventListener("wheel", handler, { passive: false });
    return () => el.removeEventListener("wheel", handler);
  }, []);

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

      // Razor tool + clip -> split at the (snapped) click frame. Snapping to
      // clip edges / playhead matches upstream's razor (a cut landing on the
      // clip's own edge is a backend no-op, which is fine).
      if (toolMode === "razor" && hit) {
        const raw = frameAt(docX, zoomScale);
        const targets = collectTargets(timeline, new Set(), activeFrame);
        const snap = findSnap(raw, targets, zoomScale, null);
        void edit.splitClip(hit.clip.id, snap ? snap.frame : raw);
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
        // Clamp so the clip keeps a ≥1-frame duration and can't run past the
        // available source (upstream's mouseDragged trim clamp).
        deltaFrames = clampTrimDeltaFrames(d.hit.clip, d.kind === "trimLeft" ? "left" : "right", deltaFrames);
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

  // Abandon an in-progress drag WITHOUT committing — fires on pointercancel (a
  // touch/trackpad gesture, or an HTML5 DnD started over the canvas) and on
  // lostpointercapture (capture stolen by a reflow, e.g. importing a second media
  // item triggers insertTrack→refresh→addClips→refresh mid-gesture). Without these
  // the gesture never reaches pointerup, so dragRef and the pointer capture stay
  // stuck and the whole timeline becomes undraggable (#126).
  const endDrag = useCallback((e: React.PointerEvent) => {
    dragRef.current = null;
    setSnapFrame(null);
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
  }, []);

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      const d = dragRef.current;
      dragRef.current = null;
      setSnapFrame(null);
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
      if (!d) return;

      if (d.kind === "move") {
        if (d.deltaFrames === 0 && d.targetTrack === d.startTrack) return; // no-op
        // Resolve every dragged clip's current location.
        const locs = d.companions
          .map((id) => {
            const loc = findClipLoc(timeline, id);
            return loc
              ? { id, ti: loc[0], clip: timeline.tracks[loc[0]].clips[loc[1]] }
              : null;
          })
          .filter((x): x is NonNullable<typeof x> => x !== null);
        if (locs.length === 0) return;

        // One group-floor FRAME delta so the earliest clip lands at >=0 and the
        // whole selection keeps its relative spacing (not per-clip max(0,...)).
        const minStart = Math.min(...locs.map((l) => l.clip.startFrame));
        const frameDelta = Math.max(d.deltaFrames, -minStart);

        // One group TRACK delta: step toward 0 until every clip stays in-bounds
        // and lands on a type-compatible track (rigid group, not per-clip clamp).
        const rawTrackDelta = d.targetTrack - d.startTrack;
        let trackDelta = rawTrackDelta;
        const step = rawTrackDelta > 0 ? -1 : 1;
        while (trackDelta !== 0) {
          const ok = locs.every((l) => {
            const to = l.ti + trackDelta;
            return to >= 0 && to < timeline.tracks.length && compatibleTracks(timeline, l.ti, to);
          });
          if (ok) break;
          trackDelta += step;
        }

        if (frameDelta === 0 && trackDelta === 0) return; // nothing actually moves
        const moves = locs.map((l) => ({
          clipId: l.id,
          toTrack: l.ti + trackDelta,
          toFrame: l.clip.startFrame + frameDelta,
        }));
        void edit.moveClips(moves);
        return;
      }

      if (d.kind === "trimLeft" || d.kind === "trimRight") {
        if (d.deltaFrames === 0) return;
        const edge = d.kind === "trimLeft" ? "left" : "right";
        // Linked partners trim together (upstream commitTrim): apply the SAME
        // timeline-frame edge delta to every clip in the link group, each
        // converted to its own SOURCE-frame trim via round(delta*speed).
        const groupIds = expandLinkGroup(timeline, new Set([d.hit.clip.id]));
        const edits = [...groupIds]
          .map((id) => {
            const loc = findClipLoc(timeline, id);
            if (!loc) return null;
            const clip = timeline.tracks[loc[0]].clips[loc[1]];
            const v = trimSourceValues(clip, edge, d.deltaFrames);
            return { clipId: id, trimStartFrame: v.trimStartFrame, trimEndFrame: v.trimEndFrame };
          })
          .filter((e): e is NonNullable<typeof e> => e !== null);
        void edit.trimClips(edits);
      }
    },
    [timeline],
  );

  // Ghost preview offsets for the active drag (read from dragRef during render).
  const drag = dragRef.current;

  // Right-click on a clip -> context menu.
  const onContextMenu = useCallback(
    (e: React.MouseEvent) => {
      const { docX, docY } = toDoc(e);
      const hit = hitTestClip(timeline, docX, docY, zoomScale, trackHeights);
      if (!hit) return; // empty space: keep the default (suppressed) menu
      e.preventDefault();
      // If the clip isn't already selected, select just it so menu actions
      // target the right clip.
      if (!selectedClipIds.has(hit.clip.id)) {
        selectClips(new Set([hit.clip.id]));
      }
      setMenu({ clipId: hit.clip.id, x: e.clientX, y: e.clientY });
    },
    [toDoc, timeline, zoomScale, trackHeights, selectedClipIds, selectClips],
  );

  return (
    <div
      ref={viewportRef}
      style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden" }}
    >
      {/* Content canvas (clips + backgrounds), positioned right of header column. */}
      <canvas
        ref={contentCanvasRef}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={endDrag}
        onLostPointerCapture={endDrag}
        onContextMenu={onContextMenu}
        style={{
          position: "absolute",
          left: LAYOUT.trackHeaderWidth,
          top: 0,
          touchAction: "none",
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

      {/* Clip right-click context menu. */}
      {menu && (
        <ClipContextMenu
          clipId={menu.clipId}
          left={menu.x}
          top={menu.y}
          onClose={() => setMenu(null)}
        />
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
