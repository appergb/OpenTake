/**
 * TextTab (SPEC §6.3). Inspector tab for text clips. Edits `textContent` (text
 * box) plus the full `textStyle` (font / size / color / alignment / background /
 * border / shadow). Text commits on blur; style controls commit immediately via
 * SetClipProperties (the backend writes `clip.text_style`, the render layer
 * re-rasterizes the text box on the next `timeline_changed`).
 */

import { useEffect, useState } from "react";
import { AlignCenter, AlignLeft, AlignRight, type LucideIcon } from "lucide-react";
import * as edit from "../../store/editActions";
import { Icon } from "../ui/Icon";
import { ScrubbableNumberField } from "./ScrubbableNumberField";
import { RADIUS, SPACE } from "../../lib/theme";
import type { TFunction } from "../../i18n";
import type { Clip, Rgba, TextAlignment, TextStyle } from "../../lib/types";

const COLOR_SWATCH_SIZE = SPACE.lgXl;

/** Same default as `DEFAULT_TEXT_STYLE` in editActions / domain `TextStyle`. */
function completeTextStyle(style: TextStyle | undefined): TextStyle {
  return {
    fontName: style?.fontName ?? "Helvetica-Bold",
    fontSize: style?.fontSize ?? 96,
    fontScale: style?.fontScale ?? 1,
    color: { r: 1, g: 1, b: 1, a: 1, ...style?.color },
    alignment: style?.alignment ?? "center",
    shadow: {
      enabled: style?.shadow?.enabled ?? true,
      color: { r: 0, g: 0, b: 0, a: 0.6, ...style?.shadow?.color },
      offsetX: style?.shadow?.offsetX ?? 0,
      offsetY: style?.shadow?.offsetY ?? -2,
      blur: style?.shadow?.blur ?? 6,
    },
    background: {
      enabled: style?.background?.enabled ?? false,
      color: { r: 0, g: 0, b: 0, a: 0.6, ...style?.background?.color },
    },
    border: {
      enabled: style?.border?.enabled ?? false,
      color: { r: 0, g: 0, b: 0, a: 1, ...style?.border?.color },
    },
  };
}

/** A short, opinionated list of common font families. Free-text is also allowed
 *  so any installed system font name works (the rasterizer resolves it). */
const FONT_OPTIONS = [
  "Helvetica-Bold",
  "Helvetica",
  "Arial-BoldMT",
  "ArialMT",
  "TimesNewRomanPS-BoldMT",
  "Georgia",
  "Courier-Bold",
  "Verdana",
];

const ALIGN_ICON: Record<TextAlignment, LucideIcon> = {
  left: AlignLeft,
  center: AlignCenter,
  right: AlignRight,
};

export function TextTab({ clip, t }: { clip: Clip; t: TFunction }) {
  const [value, setValue] = useState(clip.textContent ?? "");
  const [style, setStyle] = useState<TextStyle>(() => completeTextStyle(clip.textStyle));

  // Reset local state when the selected clip (or its persisted style) changes.
  useEffect(() => {
    setValue(clip.textContent ?? "");
  }, [clip.id, clip.textContent]);
  useEffect(() => {
    setStyle(completeTextStyle(clip.textStyle));
  }, [clip.id, clip.textStyle]);

  const commitText = () => {
    if (value === (clip.textContent ?? "")) return;
    void edit.setClipProperties([clip.id], { textContent: value });
  };

  // Commit a whole new style (style edits are immediate, like the grade panel).
  const commitStyle = (next: TextStyle) => {
    setStyle(next);
    void edit.setClipProperties([clip.id], { textStyle: next });
  };

  return (
    <section style={{ display: "flex", flexDirection: "column", gap: "var(--space-lg)" }}>
      <div>
        <SectionLabel label={t("inspector.section.text")} />
        <textarea
          value={value}
          placeholder={t("inspector.textPlaceholder")}
          onChange={(e) => setValue(e.target.value)}
          onBlur={commitText}
          rows={4}
          style={{
            width: "100%",
            resize: "vertical",
            minHeight: 80,
            padding: "var(--space-sm)",
            fontSize: "var(--fs-sm)",
            color: "var(--text-primary)",
            background: "var(--bg-elevated)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: 4,
            fontFamily: "var(--font-sans)",
            outline: "none",
          }}
        />
      </div>

      <div>
        <SectionLabel label={t("inspector.section.textStyle")} />

        <Row label={t("inspector.field.fontFamily")}>
          <select
            aria-label={t("inspector.field.fontFamily")}
            value={style.fontName}
            onChange={(e) => commitStyle({ ...style, fontName: e.target.value })}
            style={{
              maxWidth: 120,
              fontSize: "var(--fs-sm)",
              color: "var(--accent-primary)",
              background: "var(--bg-raised)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-xs)",
              padding: "1px 4px",
            }}
          >
            {(FONT_OPTIONS.includes(style.fontName)
              ? FONT_OPTIONS
              : [style.fontName, ...FONT_OPTIONS]
            ).map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </Row>

        <Row label={t("inspector.field.fontSize")}>
          <ScrubbableNumberField
            value={style.fontSize}
            min={4}
            max={512}
            sensitivity={0.5}
            format={(v) => v.toFixed(0)}
            width={56}
            onCommit={(v) => commitStyle({ ...style, fontSize: v })}
          />
        </Row>

        <Row label={t("inspector.field.textColor")}>
          <ColorSwatch
            label={t("inspector.field.textColor")}
            color={style.color}
            onCommit={(color) => commitStyle({ ...style, color })}
          />
        </Row>

        <Row label={t("inspector.field.alignment")}>
          <div style={{ display: "inline-flex", gap: 2 }}>
            {(["left", "center", "right"] as TextAlignment[]).map((a) => (
              <AlignButton
                key={a}
                align={a}
                active={style.alignment === a}
                title={t(`inspector.align.${a}`)}
                onClick={() => commitStyle({ ...style, alignment: a })}
              />
            ))}
          </div>
        </Row>

        <ToggleColorRow
          label={t("inspector.field.background")}
          enabled={style.background.enabled}
          color={style.background.color}
          onToggle={(enabled) =>
            commitStyle({ ...style, background: { ...style.background, enabled } })
          }
          onColor={(color) =>
            commitStyle({ ...style, background: { ...style.background, color } })
          }
        />

        <ToggleColorRow
          label={t("inspector.field.border")}
          enabled={style.border.enabled}
          color={style.border.color}
          onToggle={(enabled) =>
            commitStyle({ ...style, border: { ...style.border, enabled } })
          }
          onColor={(color) => commitStyle({ ...style, border: { ...style.border, color } })}
        />

        <ToggleColorRow
          label={t("inspector.field.shadow")}
          enabled={style.shadow.enabled}
          color={style.shadow.color}
          onToggle={(enabled) =>
            commitStyle({ ...style, shadow: { ...style.shadow, enabled } })
          }
          onColor={(color) => commitStyle({ ...style, shadow: { ...style.shadow, color } })}
        />
      </div>
    </section>
  );
}

function SectionLabel({ label }: { label: string }) {
  return (
    <div
      style={{
        marginBottom: "var(--space-sm)",
        fontSize: "var(--fs-xxs)",
        fontWeight: "var(--fw-semibold)",
        letterSpacing: "var(--tracking-wide)",
        color: "var(--text-muted)",
        textTransform: "uppercase",
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
        minHeight: 22,
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
        style={{ flexShrink: 0, display: "inline-flex", alignItems: "center", gap: "var(--space-xs)" }}
      >
        {children}
      </span>
    </div>
  );
}

/** A toggle checkbox + (when enabled) a color swatch, on one row. Used for the
 *  text background, border, and shadow fills. */
function ToggleColorRow({
  label,
  enabled,
  color,
  onToggle,
  onColor,
}: {
  label: string;
  enabled: boolean;
  color: Rgba;
  onToggle: (enabled: boolean) => void;
  onColor: (color: Rgba) => void;
}) {
  return (
    <Row label={label}>
      <input
        type="checkbox"
        aria-label={label}
        checked={enabled}
        style={{ accentColor: "var(--accent-primary)", cursor: "pointer" }}
        onChange={(e) => onToggle(e.target.checked)}
      />
      {enabled && <ColorSwatch label={label} color={color} onCommit={onColor} />}
    </Row>
  );
}

function AlignButton({
  align,
  active,
  title,
  onClick,
}: {
  align: TextAlignment;
  active: boolean;
  title: string;
  onClick: () => void;
}) {
  return (
    <button
      title={title}
      aria-label={title}
      aria-pressed={active}
      onClick={onClick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: 24,
        height: 22,
        color: active ? "var(--text-primary)" : "var(--text-tertiary)",
        background: active ? "var(--bg-raised)" : "transparent",
        border: `var(--bw-thin) solid ${active ? "var(--accent-primary)" : "var(--border-primary)"}`,
        borderRadius: "var(--radius-xs)",
        cursor: "pointer",
      }}
    >
      <Icon icon={ALIGN_ICON[align]} size={13} />
    </button>
  );
}

/** A native color picker bound to an `Rgba`. The picker edits RGB; alpha is
 *  preserved verbatim (text colors are usually opaque, fills keep their alpha). */
function ColorSwatch({
  label,
  color,
  onCommit,
}: {
  label: string;
  color: Rgba;
  onCommit: (color: Rgba) => void;
}) {
  return (
    <input
      aria-label={label}
      type="color"
      value={rgbaToHex(color)}
      onChange={(e) => onCommit({ ...hexToRgb(e.target.value), a: color.a })}
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
  );
}

function channelHex(value: number): string {
  const clamped = Math.max(0, Math.min(255, Math.round(value * 255)));
  return clamped.toString(16).padStart(2, "0");
}

function rgbaToHex(color: Rgba): string {
  return `#${channelHex(color.r)}${channelHex(color.g)}${channelHex(color.b)}`;
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
  const parsed = Number.parseInt(expanded, 16);
  if (!Number.isFinite(parsed)) return { r: 1, g: 1, b: 1 };
  return {
    r: ((parsed >> 16) & 0xff) / 255,
    g: ((parsed >> 8) & 0xff) / 255,
    b: (parsed & 0xff) / 255,
  };
}
