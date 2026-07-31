/**
 * PanelShell — the per-leaf-panel wrapper (SPEC §2.5/§2.6): a surface-colored
 * rounded card (radius 6) floating on the base "groove", with a 2.5px gap inset,
 * plus the focus ring (accent.primary 1.5px @ 0.6 opacity) and click-to-focus.
 */

import type { ReactNode } from "react";
import { useEditorUiStore, type Panel } from "../../store/uiStore";
import { useT } from "../../i18n";

interface PanelShellProps {
  panel: Panel;
  children: ReactNode;
}

export function PanelShell({ panel, children }: PanelShellProps) {
  const t = useT();
  const focusedPanel = useEditorUiStore((s) => s.focusedPanel);
  const focusPanel = useEditorUiStore((s) => s.focusPanel);
  const focused = focusedPanel === panel;

  return (
    // Outer = base groove color; the inner card is the surface (SPEC §2.5).
    <div
      data-editor-panel={panel}
      data-focused={focused ? "true" : "false"}
      role="region"
      aria-label={t(`layout.panel.${panel}`)}
      className="editor-panel-shell"
      style={{ background: "var(--bg-base)" }}
      onMouseDown={() => focusPanel(panel)}
    >
      <div
        className="editor-panel-card"
        style={{ background: "var(--bg-surface)" }}
      >
        {children}
      </div>
      {/* Focus ring overlay — never intercepts the mouse. */}
      <div
        aria-hidden
        data-panel-focus-ring
        className="editor-panel-focus-ring"
        style={{
          opacity: focused ? 0.6 : 0,
        }}
      />
    </div>
  );
}

/** Standard 28px panel header bar (SPEC §6.1 panelHeaderBar). */
export function PanelHeaderBar({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        height: "var(--panel-header-height)",
        display: "flex",
        alignItems: "center",
        gap: "var(--space-xs)",
        padding: "0 var(--space-lg)",
        background: "var(--bg-raised)",
        borderBottom: "var(--bw-thin) solid var(--border-primary)",
        flex: "0 0 auto",
        fontSize: "var(--fs-sm-md)",
        color: "var(--text-secondary)",
      }}
    >
      {children}
    </div>
  );
}
