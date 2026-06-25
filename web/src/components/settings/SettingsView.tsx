/**
 * Settings view. Reachable from both the Home sidebar and the editor title bar.
 * Thin shell: sidebar + detail layout, each pane in its own file (Issue #40
 * review — "SettingsView.tsx > 800 行规约").
 *
 * 12 panes: General, Appearance, Import, Models, AI, MCP, Storage,
 * Notifications, Privacy, Shortcuts, Account, About.
 */

import { useState } from "react";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import type { PaneId } from "./shared";
import { SettingsSidebar } from "./SettingsSidebar";
import { GeneralPane } from "./panes/GeneralPane";
import { AppearancePane } from "./panes/AppearancePane";
import { ImportPane } from "./panes/ImportPane";
import { ModelsPane } from "./panes/ModelsPane";
import { AiPane } from "./panes/AiPane";
import { McpInstructionsPane } from "./panes/McpInstructionsPane";
import { StoragePane } from "./panes/StoragePane";
import { NotificationsPane } from "./panes/NotificationsPane";
import { PrivacyPane } from "./panes/PrivacyPane";
import { ShortcutsPane } from "./panes/ShortcutsPane";
import { AccountPane } from "./panes/AccountPane";
import { AboutPane } from "./panes/AboutPane";

export function SettingsView() {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);
  const [active, setActive] = useState<PaneId>("general");

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        width: "100%",
        background: "var(--bg-surface)",
        color: "var(--text-primary)",
      }}
    >
      <header
        data-tauri-drag-region
        style={{
          height: 38,
          flex: "0 0 auto",
          display: "flex",
          alignItems: "center",
          gap: "var(--space-sm)",
          padding: "0 var(--space-md) 0 var(--titlebar-safe-left)",
          background: "var(--bg-base)",
          borderBottom: "var(--bw-thin) solid var(--border-primary)",
        }}
      >
        <span style={{ fontSize: "var(--fs-md)", fontWeight: "var(--fw-semibold)" }}>
          {t("settings.title")}
        </span>
        <div style={{ flex: 1 }} />
        <button
          type="button"
          onClick={() => setView("home")}
          className="hover-area"
          style={{
            height: 26,
            padding: "0 var(--space-md)",
            borderRadius: "var(--radius-sm)",
            color: "var(--text-secondary)",
            fontSize: "var(--fs-sm)",
            fontWeight: "var(--fw-medium)",
          }}
        >
          {t("settings.done")}
        </button>
      </header>

      <div style={{ flex: 1, display: "flex", minWidth: 0 }}>
        <SettingsSidebar active={active} onSelect={setActive} />
        <div style={{ flex: 1, overflowY: "auto", minWidth: 0 }}>
          <div
            style={{
              maxWidth: 640,
              margin: "0 auto",
              padding: "var(--space-xl) var(--space-xl-xxl) var(--space-xxl)",
              display: "flex",
              flexDirection: "column",
              gap: "var(--space-xl-xxl)",
            }}
          >
            {active === "general" && <GeneralPane />}
            {active === "appearance" && <AppearancePane />}
            {active === "import" && <ImportPane />}
            {active === "models" && <ModelsPane />}
            {active === "ai" && <AiPane />}
            {active === "mcp" && <McpInstructionsPane />}
            {active === "storage" && <StoragePane />}
            {active === "notifications" && <NotificationsPane />}
            {active === "privacy" && <PrivacyPane />}
            {active === "shortcuts" && <ShortcutsPane />}
            {active === "account" && <AccountPane />}
            {active === "about" && <AboutPane />}
          </div>
        </div>
      </div>
    </div>
  );
}