/**
 * View menu overlay (SPEC §2.9). Hosts the Layout-preset switch and panel
 * visibility toggles that were previously inlined into the title bar, so the
 * §2.8 title bar can stay a 1:1 copy of the upstream (Agent toggle + Export
 * only). A click-out / Escape dismisses it. Every action reuses the existing
 * uiStore mutators and matches the §9.6 keyboard shortcuts (⌘1–3, ⌘0, ⌘⌥0).
 */

import { useEffect, useRef, useState } from "react";
import { Menu, PanelLeft, PanelRight, Columns3, Check } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useEditorUiStore, type LayoutPreset } from "../../store/uiStore";
import { useT } from "../../i18n";

const PRESETS: Array<{ id: LayoutPreset; icon: typeof PanelLeft; labelKey: string; key: string }> = [
  { id: "default", icon: Columns3, labelKey: "view.layoutDefault", key: "⌘1" },
  { id: "media", icon: PanelLeft, labelKey: "view.layoutMedia", key: "⌘2" },
  { id: "vertical", icon: PanelRight, labelKey: "view.layoutVertical", key: "⌘3" },
];

export function ViewMenu() {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const t = useT();

  const layoutPreset = useEditorUiStore((s) => s.layoutPreset);
  const setLayoutPreset = useEditorUiStore((s) => s.setLayoutPreset);
  const mediaVisible = useEditorUiStore((s) => s.mediaPanelVisible);
  const toggleMedia = useEditorUiStore((s) => s.toggleMediaPanel);
  const inspectorVisible = useEditorUiStore((s) => s.inspectorPanelVisible);
  const toggleInspector = useEditorUiStore((s) => s.toggleInspectorPanel);

  // Dismiss on outside click or Escape.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div ref={rootRef} style={{ position: "relative", display: "inline-flex" }}>
      <button
        title={t("view.menu")}
        aria-label={t("view.menu")}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="hover-area"
        style={{
          width: 26,
          height: 26,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-secondary)",
          opacity: open ? 1 : 0.7,
        }}
      >
        <Icon icon={Menu} size={13} />
      </button>

      {open && (
        <div
          role="menu"
          style={{
            position: "absolute",
            top: "calc(100% + 6px)",
            left: 0,
            minWidth: 210,
            padding: "var(--space-xs)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            borderRadius: "var(--radius-md)",
            boxShadow: "var(--shadow-lg)",
            zIndex: 200,
          }}
        >
          <MenuSectionLabel>{t("view.layout")}</MenuSectionLabel>
          {PRESETS.map((p) => (
            <MenuItem
              key={p.id}
              icon={p.icon}
              label={t(p.labelKey)}
              shortcut={p.key}
              checked={layoutPreset === p.id}
              onClick={() => {
                setLayoutPreset(p.id);
                setOpen(false);
              }}
            />
          ))}

          <MenuDivider />

          <MenuSectionLabel>{t("view.panels")}</MenuSectionLabel>
          <MenuItem
            icon={PanelLeft}
            label={t("view.mediaPanel")}
            shortcut="⌘0"
            checked={mediaVisible}
            onClick={toggleMedia}
          />
          <MenuItem
            icon={PanelRight}
            label={t("view.inspector")}
            shortcut="⌘⌥0"
            checked={inspectorVisible}
            onClick={toggleInspector}
          />
        </div>
      )}
    </div>
  );
}

function MenuSectionLabel({ children }: { children: string }) {
  return (
    <div
      style={{
        padding: "var(--space-xxs) var(--space-sm)",
        fontSize: "var(--fs-xs)",
        fontWeight: "var(--fw-semibold)",
        color: "var(--text-tertiary)",
        textTransform: "uppercase",
        letterSpacing: "0.04em",
      }}
    >
      {children}
    </div>
  );
}

function MenuDivider() {
  return (
    <div
      style={{
        height: "var(--bw-thin)",
        background: "var(--border-primary)",
        margin: "var(--space-xs) var(--space-xs)",
      }}
    />
  );
}

function MenuItem({
  icon,
  label,
  shortcut,
  checked,
  onClick,
}: {
  icon: typeof PanelLeft;
  label: string;
  shortcut: string;
  checked: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitemcheckbox"
      aria-checked={checked}
      onClick={onClick}
      className="hover-area"
      style={{
        width: "100%",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        height: 28,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        color: checked ? "var(--text-primary)" : "var(--text-secondary)",
        fontSize: "var(--fs-sm)",
        fontWeight: "var(--fw-medium)",
        textAlign: "left",
      }}
    >
      <span style={{ display: "inline-flex", width: 14, justifyContent: "center" }}>
        <Icon icon={icon} size={13} />
      </span>
      <span style={{ flex: 1 }}>{label}</span>
      <span
        style={{
          display: "inline-flex",
          width: 14,
          justifyContent: "center",
          color: "var(--text-primary)",
          opacity: checked ? 1 : 0,
        }}
      >
        <Icon icon={Check} size={13} />
      </span>
      <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", minWidth: 28, textAlign: "right" }}>
        {shortcut}
      </span>
    </button>
  );
}
