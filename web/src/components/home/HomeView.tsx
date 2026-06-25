/**
 * Home view (CapCut-style launcher, modeled on upstream `Project/HomeView.swift`).
 * Shown on launch before the editor. Left sidebar: New Project / Open Project /
 * Settings. Right content: a welcome header + the recent-projects grid (recents
 * persisted in localStorage). Selecting an action or a recent card enters the
 * editor. Built entirely from design tokens so it sits consistently with the
 * editor's dark surface.
 */

import { useState, useEffect } from "react";
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

  // Validate recent projects on mount to filter out folders deleted on disk
  useEffect(() => {
    void useRecentStore.getState().validateRecents();
  }, []);

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
            padding: "calc(var(--titlebar-safe-top) + var(--space-md)) var(--space-xl-xxl) var(--space-lg)",
          }}
        >
          <h1
            data-tauri-drag-region
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
            data-tauri-drag-region
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
      data-tauri-drag-region
      style={{
        width: 200,
        flex: "0 0 auto",
        display: "flex",
        flexDirection: "column",
        padding: "var(--titlebar-safe-top) var(--space-md) var(--space-xl)",
        background: "var(--bg-raised)",
        borderRight: "var(--bw-thin) solid var(--border-primary)",
      }}
    >
      <div
        data-tauri-drag-region
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
  const [selectedPath, setSelectedPath] = useState<string | null>(null);

  // Listen to KeyDown to open project when selected + Enter is pressed
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter" && selectedPath) {
        if (selectedPath === "new") {
          void newProjectAndEnter();
        } else {
          void openProjectPath(selectedPath);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [selectedPath]);

  return (
    <div
      onClick={() => setSelectedPath(null)}
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
        <NewProjectCard
          selected={selectedPath === "new"}
          onClick={(e) => {
            e.stopPropagation();
            setSelectedPath("new");
          }}
          onDoubleClick={() => void newProjectAndEnter()}
        />
        {recents.map((entry) => (
          <ProjectCard
            key={entry.path}
            entry={entry}
            selected={selectedPath === entry.path}
            onClick={(e) => {
              e.stopPropagation();
              setSelectedPath(entry.path);
            }}
            onDoubleClick={() => void openProjectPath(entry.path)}
          />
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

function NewProjectCard({
  selected,
  onClick,
  onDoubleClick,
}: {
  selected: boolean;
  onClick: (e: React.MouseEvent) => void;
  onDoubleClick: () => void;
}) {
  const t = useT();
  const [hovered, setHovered] = useState(false);
  const [tilt, setTilt] = useState({ x: 0, y: 0 });

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width - 0.5;
    const y = (e.clientY - rect.top) / rect.height - 0.5;
    setTilt({ x, y });
  };

  const handleMouseLeave = () => {
    setHovered(false);
    setTilt({ x: 0, y: 0 });
  };

  const rotateX = -tilt.y * 14;
  const rotateY = tilt.x * 14;
  const shiftX = tilt.x * 6;
  const shiftY = tilt.y * 6;

  const transform = hovered
    ? `perspective(1000px) rotateX(${rotateX}deg) rotateY(${rotateY}deg) translate3d(${shiftX}px, ${shiftY}px, 0) scale3d(1.04, 1.04, 1.04)`
    : selected
    ? `perspective(1000px) rotateX(0deg) rotateY(0deg) translate3d(0, 0, 0) scale3d(1.02, 1.02, 1.02)`
    : `perspective(1000px) rotateX(0deg) rotateY(0deg) translate3d(0, 0, 0) scale3d(1, 1, 1)`;

  const transition = hovered
    ? "transform 0.08s ease-out, box-shadow 0.08s ease-out"
    : "transform 0.4s cubic-bezier(0.25, 1, 0.5, 1), box-shadow 0.4s cubic-bezier(0.25, 1, 0.5, 1)";

  const shadowX = -tilt.x * 12;
  const shadowY = -tilt.y * 12;
  const boxShadow = hovered
    ? `${shadowX}px ${shadowY}px 20px rgba(0, 0, 0, 0.45)`
    : selected
    ? "0 0 12px rgba(242, 153, 51, 0.35), var(--shadow-lg)"
    : "var(--shadow-md)";

  return (
    <div
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={handleMouseLeave}
      onMouseMove={handleMouseMove}
      style={{
        display: "block",
        width: "100%",
        textAlign: "left",
        position: "relative",
        zIndex: hovered ? 2 : 1,
        transform,
        transition,
        cursor: "default",
      }}
    >
      <div
        style={{
          position: "relative",
          aspectRatio: "5 / 4",
          borderRadius: "var(--radius-md-lg)",
          background: "var(--bg-placeholder)",
          border: selected
            ? "2px solid var(--accent-timecode)"
            : hovered
            ? "1px solid var(--border-divider)"
            : "1px solid var(--border-primary)",
          boxShadow,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
          overflow: "hidden",
          transition: "border-color 0.2s ease",
        }}
      >
        <Icon icon={Plus} size={30} strokeWidth={1.4} />
        {hovered && (
          <div
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              bottom: 0,
              background: `radial-gradient(circle 100px at ${(tilt.x + 0.5) * 100}% ${(tilt.y + 0.5) * 100}%, rgba(255, 255, 255, 0.12), transparent)`,
              pointerEvents: "none",
              zIndex: 3,
            }}
          />
        )}
      </div>
      <div
        style={{
          marginTop: "var(--space-sm)",
          fontSize: "var(--fs-sm-md)",
          color: selected ? "var(--accent-timecode)" : "var(--text-secondary)",
          fontWeight: selected ? "var(--fw-semibold)" : "normal",
          transition: "color 0.2s ease",
        }}
      >
        {t("home.untitled")}
      </div>
    </div>
  );
}

function ProjectCard({
  entry,
  selected,
  onClick,
  onDoubleClick,
}: {
  entry: RecentProject;
  selected: boolean;
  onClick: (e: React.MouseEvent) => void;
  onDoubleClick: () => void;
}) {
  const t = useT();
  const remove = useRecentStore((s) => s.remove);
  const [hovered, setHovered] = useState(false);
  const [tilt, setTilt] = useState({ x: 0, y: 0 });

  const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width - 0.5;
    const y = (e.clientY - rect.top) / rect.height - 0.5;
    setTilt({ x, y });
  };

  const handleMouseLeave = () => {
    setHovered(false);
    setTilt({ x: 0, y: 0 });
  };

  const rotateX = -tilt.y * 14;
  const rotateY = tilt.x * 14;
  const shiftX = tilt.x * 6;
  const shiftY = tilt.y * 6;

  const transform = hovered
    ? `perspective(1000px) rotateX(${rotateX}deg) rotateY(${rotateY}deg) translate3d(${shiftX}px, ${shiftY}px, 0) scale3d(1.04, 1.04, 1.04)`
    : selected
    ? `perspective(1000px) rotateX(0deg) rotateY(0deg) translate3d(0, 0, 0) scale3d(1.02, 1.02, 1.02)`
    : `perspective(1000px) rotateX(0deg) rotateY(0deg) translate3d(0, 0, 0) scale3d(1, 1, 1)`;

  const transition = hovered
    ? "transform 0.08s ease-out, box-shadow 0.08s ease-out"
    : "transform 0.4s cubic-bezier(0.25, 1, 0.5, 1), box-shadow 0.4s cubic-bezier(0.25, 1, 0.5, 1)";

  const shadowX = -tilt.x * 12;
  const shadowY = -tilt.y * 12;
  const boxShadow = hovered
    ? `${shadowX}px ${shadowY}px 20px rgba(0, 0, 0, 0.45)`
    : selected
    ? "0 0 12px rgba(242, 153, 51, 0.35), var(--shadow-lg)"
    : "var(--shadow-md)";

  // Make display path compact by replacing user home dir with ~
  const cleanDisplayPath = entry.path.replace(/^\/Users\/[^\/]+/, "~");

  return (
    <div
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={handleMouseLeave}
      onMouseMove={handleMouseMove}
      style={{
        position: "relative",
        zIndex: hovered ? 2 : 1,
        transform,
        transition,
        cursor: "default",
      }}
    >
      <div style={{ display: "block", width: "100%", textAlign: "left" }}>
        <div
          style={{
            position: "relative",
            aspectRatio: "5 / 4",
            borderRadius: "var(--radius-md-lg)",
            background: "var(--bg-placeholder)",
            border: selected
              ? "2px solid var(--accent-timecode)"
              : hovered
              ? "1px solid var(--border-divider)"
              : "1px solid var(--border-primary)",
            boxShadow,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: "var(--text-muted)",
            overflow: "hidden",
            transition: "border-color 0.2s ease",
          }}
        >
          <Icon icon={Film} size={28} strokeWidth={1.4} />
          {hovered && (
            <div
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                right: 0,
                bottom: 0,
                background: `radial-gradient(circle 100px at ${(tilt.x + 0.5) * 100}% ${(tilt.y + 0.5) * 100}%, rgba(255, 255, 255, 0.12), transparent)`,
                pointerEvents: "none",
                zIndex: 3,
              }}
            />
          )}
        </div>
        <div
          style={{
            marginTop: "var(--space-sm)",
            fontSize: "var(--fs-sm-md)",
            color: selected ? "var(--accent-timecode)" : "var(--text-primary)",
            fontWeight: selected ? "var(--fw-semibold)" : "normal",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            transition: "color 0.2s ease",
          }}
        >
          {entry.name}
        </div>
        <div
          className="tabular"
          title={entry.path}
          style={{
            fontSize: "var(--fs-xs)",
            color: "var(--text-muted)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {cleanDisplayPath}
        </div>
      </div>

      {hovered && (
        <button
          type="button"
          title={t("home.remove")}
          aria-label={t("home.remove")}
          onClick={(e) => {
            e.stopPropagation();
            remove(entry.path);
          }}
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
            zIndex: 4,
          }}
        >
          <Icon icon={Trash2} size={14} />
        </button>
      )}
    </div>
  );
}
