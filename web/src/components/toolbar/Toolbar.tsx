/**
 * Toolbar (SPEC §4). Height 38, lives inside the timeline panel above the
 * timeline. Left group: Undo/Redo | Pointer/Razor | Split/Trim[/] | Text(T);
 * right: logarithmic zoom slider with -/+ magnifier icons.
 */

import {
  RotateCcw,
  RotateCw,
  MousePointer2,
  Scissors,
  SplitSquareHorizontal,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useState } from "react";
import { HoverButton } from "../ui/HoverButton";
import { Icon } from "../ui/Icon";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { ZOOM } from "../../lib/theme";
import * as edit from "../../store/editActions";
import { useT } from "../../i18n";

function Divider() {
  return (
    <div
      style={{
        width: "var(--bw-thin)",
        height: "var(--space-xl)",
        background: "var(--border-primary)",
        flex: "0 0 auto",
        margin: "0 var(--space-xxs)",
      }}
    />
  );
}

/** Bracket / glyph button (Trim Start "[", Trim End "]", Text "T"). */
function GlyphButton({
  glyph,
  title,
  serif = false,
  fontSize = 16,
  onClick,
  disabled = false,
}: {
  glyph: string;
  title: string;
  serif?: boolean;
  fontSize?: number;
  onClick?: () => void;
  disabled?: boolean;
}) {
  return (
    <HoverButton title={title} onClick={onClick} disabled={disabled}>
      <span
        style={{
          fontFamily: serif ? "var(--font-serif)" : "var(--font-mono)",
          fontSize,
          fontWeight: serif ? "var(--fw-bold)" : "var(--fw-semibold)",
          lineHeight: 1,
        }}
      >
        {glyph}
      </span>
    </HoverButton>
  );
}

export function Toolbar() {
  const t = useT();
  const toolMode = useEditorUiStore((s) => s.toolMode);
  const setToolMode = useEditorUiStore((s) => s.setToolMode);
  const zoomScale = useEditorUiStore((s) => s.zoomScale);
  const minZoomScale = useEditorUiStore((s) => s.minZoomScale);
  const setZoomScale = useEditorUiStore((s) => s.setZoomScale);
  const pushToast = useEditorUiStore((s) => s.pushToast);
  const canUndo = useProjectStore((s) => s.canUndo);
  const canRedo = useProjectStore((s) => s.canRedo);
  const [toolbarPending, setToolbarPending] = useState<
    "undo" | "redo" | "split" | "trimStart" | "trimEnd" | null
  >(null);

  // Logarithmic slider mapping (ToolbarView.swift:50-53): travel uniform per
  // zoom factor; get=log(zoom), set=exp(value).
  const logMin = Math.log(minZoomScale);
  const logMax = Math.log(ZOOM.max);
  const sliderValue = (Math.log(zoomScale) - logMin) / (logMax - logMin || 1);

  const onSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
    const t = Number(e.target.value);
    setZoomScale(Math.exp(logMin + t * (logMax - logMin)));
  };

  const onUndo = async () => {
    if (!canUndo || toolbarPending) return;
    setToolbarPending("undo");
    try {
      await edit.undo();
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      pushToast(`${t("toolbar.undo")}: ${message}`);
    } finally {
      setToolbarPending(null);
    }
  };

  const onRedo = async () => {
    if (!canRedo || toolbarPending) return;
    setToolbarPending("redo");
    try {
      await edit.redo();
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      pushToast(`${t("toolbar.redo")}: ${message}`);
    } finally {
      setToolbarPending(null);
    }
  };

  const onSplit = async () => {
    if (toolbarPending) return;
    setToolbarPending("split");
    try {
      await edit.splitAtPlayhead();
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      pushToast(`${t("toolbar.split")}: ${message}`);
    } finally {
      setToolbarPending(null);
    }
  };

  const onTrimStart = async () => {
    if (toolbarPending) return;
    setToolbarPending("trimStart");
    try {
      await edit.trimStartToPlayhead();
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      pushToast(`${t("toolbar.trimStart")}: ${message}`);
    } finally {
      setToolbarPending(null);
    }
  };

  const onTrimEnd = async () => {
    if (toolbarPending) return;
    setToolbarPending("trimEnd");
    try {
      await edit.trimEndToPlayhead();
    } catch (reason: unknown) {
      const message = reason instanceof Error ? reason.message : String(reason);
      pushToast(`${t("toolbar.trimEnd")}: ${message}`);
    } finally {
      setToolbarPending(null);
    }
  };

  return (
    <div
      style={{
        height: "var(--toolbar-height)",
        flex: "0 0 auto",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-md)",
        padding: "0 var(--space-md)",
        background: "var(--bg-surface)",
        borderBottom: "var(--bw-thin) solid var(--border-primary)",
      }}
    >
      {/* Undo / Redo */}
      <div style={{ display: "flex", alignItems: "center" }}>
        <span aria-busy={toolbarPending === "undo" || undefined} style={{ display: "inline-flex" }}>
          <HoverButton
            title={t("toolbar.undo")}
            disabled={!canUndo || toolbarPending !== null}
            onClick={() => void onUndo()}
          >
            <Icon icon={RotateCcw} size={13} />
          </HoverButton>
        </span>
        <span aria-busy={toolbarPending === "redo" || undefined} style={{ display: "inline-flex" }}>
          <HoverButton
            title={t("toolbar.redo")}
            disabled={!canRedo || toolbarPending !== null}
            onClick={() => void onRedo()}
          >
            <Icon icon={RotateCw} size={13} />
          </HoverButton>
        </span>
      </div>

      <Divider />

      {/* Tool mode */}
      <div style={{ display: "flex", alignItems: "center" }}>
        <HoverButton
          title={t("toolbar.pointer")}
          active={toolMode === "pointer"}
          onClick={() => setToolMode("pointer")}
        >
          <Icon icon={MousePointer2} size={13} />
        </HoverButton>
        <HoverButton
          title={t("toolbar.razor")}
          active={toolMode === "razor"}
          onClick={() => setToolMode("razor")}
        >
          <Icon icon={Scissors} size={13} />
        </HoverButton>
      </div>

      <Divider />

      {/* Split / Trim */}
      <div style={{ display: "flex", alignItems: "center" }}>
        <span aria-busy={toolbarPending === "split" || undefined} style={{ display: "inline-flex" }}>
          <HoverButton
            title={t("toolbar.split")}
            disabled={toolbarPending !== null}
            onClick={() => void onSplit()}
          >
            <Icon icon={SplitSquareHorizontal} size={13} />
          </HoverButton>
        </span>
        <span
          aria-busy={toolbarPending === "trimStart" || undefined}
          style={{ display: "inline-flex" }}
        >
          <GlyphButton
            glyph="["
            title={t("toolbar.trimStart")}
            disabled={toolbarPending !== null}
            onClick={() => void onTrimStart()}
          />
        </span>
        <span
          aria-busy={toolbarPending === "trimEnd" || undefined}
          style={{ display: "inline-flex" }}
        >
          <GlyphButton
            glyph="]"
            title={t("toolbar.trimEnd")}
            disabled={toolbarPending !== null}
            onClick={() => void onTrimEnd()}
          />
        </span>
      </div>

      <Divider />

      {/* Add text */}
      <GlyphButton glyph="T" title={t("toolbar.addText")} serif fontSize={17} onClick={() => edit.addTextClip()} />

      <div style={{ flex: 1 }} />

      {/* Zoom slider (logarithmic) */}
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-xs)" }}>
        <span style={{ color: "var(--text-tertiary)", display: "inline-flex" }}>
          <Icon icon={ZoomOut} size={11} />
        </span>
        <input
          type="range"
          min={0}
          max={1}
          step={0.001}
          value={sliderValue}
          onChange={onSlider}
          className="zoom-slider"
          style={{ width: 100 }}
          aria-label={t("toolbar.zoom")}
        />
        <span style={{ color: "var(--text-tertiary)", display: "inline-flex" }}>
          <Icon icon={ZoomIn} size={11} />
        </span>
      </div>
    </div>
  );
}
