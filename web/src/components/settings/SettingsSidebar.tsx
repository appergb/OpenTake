/**
 * Settings sidebar — pane selector with 12 entries (Issue #40).
 * Extracted from SettingsView.tsx and expanded with 4 new panes:
 * Models, Privacy, Shortcuts, Account.
 *
 * Mirrors upstream `SettingsView` sidebar (220pt wide, icon + label rows,
 * active capsule on the left edge).
 */

import {
  Settings as SettingsIcon,
  Palette,
  Download,
  Sparkles,
  Terminal,
  HardDrive,
  Bell,
  Info,
  Cpu,
  Shield,
  Keyboard,
  User,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import type { PaneId } from "./shared";

export function SettingsSidebar({
  active,
  onSelect,
}: {
  active: PaneId;
  onSelect: (id: PaneId) => void;
}) {
  const t = useT();
  const items: Array<{ id: PaneId; icon: typeof SettingsIcon; label: string }> = [
    { id: "general", icon: SettingsIcon, label: t("settings.section.general") },
    { id: "appearance", icon: Palette, label: t("settings.section.appearance") },
    { id: "import", icon: Download, label: t("settings.section.import") },
    { id: "models", icon: Cpu, label: t("settings.section.models") },
    { id: "ai", icon: Sparkles, label: t("settings.section.ai") },
    { id: "mcp", icon: Terminal, label: t("settings.section.mcp") },
    { id: "storage", icon: HardDrive, label: t("settings.section.storage") },
    { id: "notifications", icon: Bell, label: t("settings.section.notifications") },
    { id: "privacy", icon: Shield, label: t("settings.section.privacy") },
    { id: "shortcuts", icon: Keyboard, label: t("settings.section.shortcuts") },
    { id: "account", icon: User, label: t("settings.section.account") },
    { id: "about", icon: Info, label: t("settings.section.about") },
  ];
  return (
    <aside
      style={{
        width: 180,
        flex: "0 0 auto",
        padding: "var(--space-md) var(--space-xs)",
        background: "var(--bg-base)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
        display: "flex",
        flexDirection: "column",
        gap: 2,
      }}
    >
      {items.map((it) => {
        const selected = it.id === active;
        return (
          <button
            key={it.id}
            type="button"
            onClick={() => onSelect(it.id)}
            className="hover-area"
            style={{
              position: "relative",
              display: "flex",
              alignItems: "center",
              gap: "var(--space-sm)",
              width: "100%",
              height: 30,
              padding: "0 var(--space-sm)",
              borderRadius: "var(--radius-sm)",
              background: selected ? "var(--bg-raised)" : "transparent",
              color: selected ? "var(--text-primary)" : "var(--text-secondary)",
              fontSize: "var(--fs-sm-md)",
              fontWeight: selected ? "var(--fw-medium)" : "var(--fw-regular)",
              textAlign: "left",
            }}
          >
            {selected && (
              <span
                style={{
                  position: "absolute",
                  left: -4,
                  top: "50%",
                  transform: "translateY(-50%)",
                  width: "var(--bw-thick)",
                  height: 16,
                  background: "var(--accent-primary)",
                  borderRadius: 1,
                }}
              />
            )}
            <Icon icon={it.icon} size={14} />
            <span>{it.label}</span>
          </button>
        );
      })}
    </aside>
  );
}