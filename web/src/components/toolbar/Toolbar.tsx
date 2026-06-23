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
}: {
  glyph: string;
  title: string;
  serif?: boolean;
  fontSize?: number;
  onClick?: () => void;
}) {
  return (
    <HoverButton title={title} onClick={onClick}>
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
  const canUndo = useProjectStore((s) => s.canUndo);
  const canRedo = useProjectStore((s) => s.canRedo);

  // Logarithmic slider mapping (ToolbarView.swift:50-53): travel uniform per
  // zoom factor; get=log(zoom), set=exp(value).
  const logMin = Math.log(minZoomScale);
  const logMax = Math.log(ZOOM.max);
  const sliderValue = (Math.log(zoomScale) - logMin) / (logMax - logMin || 1);

  const onSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
    const t = Number(e.target.value);
    setZoomScale(Math.exp(logMin + t * (logMax - logMin)));
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
        <HoverButton title={t("toolbar.undo")} disabled={!canUndo} onClick={() => edit.undo()}>
          <Icon icon={RotateCcw} size={13} />
        </HoverButton>
        <HoverButton title={t("toolbar.redo")} disabled={!canRedo} onClick={() => edit.redo()}>
          <Icon icon={RotateCw} size={13} />
        </HoverButton>
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
        <HoverButton title={t("toolbar.split")} onClick={() => edit.splitAtPlayhead()}>
          <Icon icon={SplitSquareHorizontal} size={13} />
        </HoverButton>
        <GlyphButton
          glyph="["
          title={t("toolbar.trimStart")}
          onClick={() => edit.trimStartToPlayhead()}
        />
        <GlyphButton glyph="]" title={t("toolbar.trimEnd")} onClick={() => edit.trimEndToPlayhead()} />
      </div>

      <Divider />

      {/* Add text */}
      <GlyphButton glyph="T" title={t("toolbar.addText")} serif fontSize={17} />

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
