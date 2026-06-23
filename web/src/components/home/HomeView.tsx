/**
 * Home view (CapCut-style launcher, modeled on upstream `Project/HomeView.swift`).
 * Shown on launch before the editor. Left sidebar: New Project / Open Project /
 * Settings. Right content: a welcome header + the recent-projects grid (recents
 * persisted in localStorage). Selecting an action or a recent card enters the
 * editor. Built entirely from design tokens so it sits consistently with the
 * editor's dark surface.
 */

import { useState } from "react";
import { Plus, FolderOpen, Settings as SettingsIcon, Film, Trash2, Library } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useRecentStore, type RecentProject } from "../../store/recentStore";
import {
  newProjectAndEnter,
  openProjectViaDialog,
  openProjectPath,
} from "../../store/projectActions";

export function HomeView() {
  const t = useT();
  return (
    <div
      style={{
        display: "flex",
        height: "100%",
        width: "100%",
        background: "var(--bg-base)",
        color: "var(--text-primary)",
      }}
    >
      <Sidebar />
      <main
        style={{
          flex: 1,
          minWidth: 0,
          display: "flex",
          flexDirection: "column",
          background:
            "radial-gradient(120% 80% at 100% 0%, rgba(245,239,228,0.05), transparent 60%), var(--bg-surface)",
        }}
      >
        <header
          data-tauri-drag-region
          style={{
            padding: "var(--space-xxl) var(--space-xl-xxl) var(--space-xl)",
          }}
        >
          <h1
            style={{
              margin: 0,
              fontSize: "var(--fs-title2)",
              fontWeight: "var(--fw-light)",
              letterSpacing: "var(--tracking-tight)",
              color: "var(--text-primary)",
            }}
          >
            {t("home.welcome")}
          </h1>
          <p
            style={{
              margin: "var(--space-sm) 0 0",
              fontSize: "var(--fs-sm-md)",
              color: "var(--text-tertiary)",
              maxWidth: 520,
            }}
          >
            {t("app.tagline")}
          </p>
        </header>

        <h2
          style={{
            margin: 0,
            padding: "0 var(--space-xl-xxl) var(--space-sm)",
            fontSize: "var(--fs-md)",
            fontWeight: "var(--fw-semibold)",
            color: "var(--text-secondary)",
          }}
        >
          {t("home.myProjects")}
        </h2>
        <ProjectGrid />
      </main>
    </div>
  );
}

function Sidebar() {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);
  const [opening, setOpening] = useState(false);

  const handleOpen = async () => {
    setOpening(true);
    try {
      await openProjectViaDialog();
    } finally {
      setOpening(false);
    }
  };

  return (
    <aside
      style={{
        width: 220,
        flex: "0 0 auto",
        display: "flex",
        flexDirection: "column",
        padding: "var(--titlebar-safe-top) var(--space-md) var(--space-xl)",
        background: "var(--bg-raised)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
      }}
    >
      <div
        style={{
          padding: "0 var(--space-sm) var(--space-xl)",
          fontSize: "var(--fs-md-lg)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--tracking-tight)",
          color: "var(--text-primary)",
        }}
      >
        {t("app.name")}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xxs)" }}>
        <SidebarRow icon={Plus} label={t("home.newProject")} onClick={() => void newProjectAndEnter()} />
        <SidebarRow
          icon={FolderOpen}
          label={opening ? t("home.opening") : t("home.openProject")}
          onClick={() => void handleOpen()}
        />
      </div>

      <SidebarRow icon={Library} label={t("library.entry")} onClick={() => setView("library")} />

      <div style={{ flex: 1 }} />

      <SidebarRow icon={SettingsIcon} label={t("home.settings")} onClick={() => setView("settings")} />
    </aside>
  );
}

function SidebarRow({
  icon,
  label,
  onClick,
}: {
  icon: typeof Plus;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="hover-area"
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        width: "100%",
        height: 34,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        color: "var(--text-secondary)",
        fontSize: "var(--fs-md)",
        fontWeight: "var(--fw-medium)",
        textAlign: "left",
      }}
    >
      <Icon icon={icon} size={15} />
      <span>{label}</span>
    </button>
  );
}

function ProjectGrid() {
  const t = useT();
  const recents = useRecentStore((s) => s.recents);

  return (
    <div
      style={{
        flex: 1,
        overflowY: "auto",
        // Top padding gives the hover scale-up room so first-row cards aren't
        // clipped by the scroll viewport's top edge.
        padding: "var(--space-md) var(--space-xl-xxl) var(--space-xl-xxl)",
      }}
    >
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fill, minmax(170px, 1fr))",
          gap: "var(--space-xl)",
          alignContent: "start",
        }}
      >
        <NewProjectCard onClick={() => void newProjectAndEnter()} />
        {recents.map((entry) => (
          <ProjectCard key={entry.path} entry={entry} />
        ))}
      </div>
      {recents.length === 0 && (
        <p
          style={{
            marginTop: "var(--space-xl)",
            color: "var(--text-muted)",
            fontSize: "var(--fs-sm-md)",
          }}
        >
          {t("home.recentEmpty")}
        </p>
      )}
    </div>
  );
}

function NewProjectCard({ onClick }: { onClick: () => void }) {
  const t = useT();
  const [hovered, setHovered] = useState(false);
  return (
    <button
      type="button"
      onClick={onClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        display: "block",
        width: "100%",
        textAlign: "left",
        position: "relative",
        zIndex: hovered ? 2 : 1,
        transform: hovered ? "scale(1.02)" : "scale(1)",
        transition: "transform var(--anim-transition) var(--ease-out)",
      }}
    >
      <div
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          borderRadius: "var(--radius-md-lg)",
          background: "var(--bg-placeholder)",
          border: `var(--bw-thin) solid ${hovered ? "var(--border-divider)" : "var(--border-primary)"}`,
          boxShadow: hovered ? "var(--shadow-lg)" : "var(--shadow-md)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          overflow: "hidden",
        }}
      >
        <Icon icon={Plus} size={30} strokeWidth={1.4} />
      </div>
      <div
        style={{
          marginTop: "var(--space-sm)",
          fontSize: "var(--fs-sm-md)",
          color: "var(--text-secondary)",
        }}
      >
        {t("home.untitled")}
      </div>
    </button>
  );
}

function ProjectCard({ entry }: { entry: RecentProject }) {
  const t = useT();
  const remove = useRecentStore((s) => s.remove);
  const [hovered, setHovered] = useState(false);

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        position: "relative",
        zIndex: hovered ? 2 : 1,
        transform: hovered ? "scale(1.02)" : "scale(1)",
        transition: "transform var(--anim-transition) var(--ease-out)",
      }}
    >
      <button
        type="button"
        onClick={() => void openProjectPath(entry.path)}
        style={{ display: "block", width: "100%", textAlign: "left" }}
      >
        <div
          style={{
            position: "relative",
            aspectRatio: "5 / 4",
            borderRadius: "var(--radius-md-lg)",
            background: "var(--bg-placeholder)",
            border: `var(--bw-thin) solid ${hovered ? "var(--border-divider)" : "var(--border-primary)"}`,
            boxShadow: hovered ? "var(--shadow-lg)" : "var(--shadow-md)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-muted)",
            overflow: "hidden",
          }}
        >
          <Icon icon={Film} size={28} strokeWidth={1.4} />
        </div>
        <div
          style={{
            marginTop: "var(--space-sm)",
            fontSize: "var(--fs-sm-md)",
            color: "var(--text-primary)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {entry.name}
        </div>
        <div
          className="tabular"
          style={{
            fontSize: "var(--fs-xs)",
            color: "var(--text-muted)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {entry.path}
        </div>
      </button>

      {hovered && (
        <button
          type="button"
          title={t("home.remove")}
          aria-label={t("home.remove")}
          onClick={() => remove(entry.path)}
          className="hover-area"
          style={{
            position: "absolute",
            top: "var(--space-sm)",
            right: "var(--space-sm)",
            width: "var(--icon-lg)",
            height: "var(--icon-lg)",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: "var(--radius-sm)",
            background: "rgba(0,0,0,0.55)",
            color: "var(--status-error)",
          }}
        >
          <Icon icon={Trash2} size={14} />
        </button>
      )}
    </div>
  );
}
