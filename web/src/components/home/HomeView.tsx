import { useEffect, useState, type CSSProperties } from "react";
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

const homeShellStyle: CSSProperties = {
  display: "flex",
  height: "100%",
  width: "100%",
  background:
    "radial-gradient(1200px 760px at 88% -12%, rgba(255,255,255,0.045), transparent 54%), linear-gradient(180deg, rgba(255,255,255,0.015), transparent 30%), var(--home-bg)",
  color: "var(--home-foreground)",
};

const homeSidebarStyle: CSSProperties = {
  width: 204,
  flex: "0 0 auto",
  display: "flex",
  flexDirection: "column",
  padding: "var(--titlebar-safe-top) var(--space-md) var(--space-xl)",
  background: "transparent",
};

const homeMainStyle: CSSProperties = {
  flex: 1,
  minWidth: 0,
  padding: "var(--home-stage-inset)",
};

const homeWorkspaceStyle: CSSProperties = {
  height: "100%",
  display: "flex",
  flexDirection: "column",
  minWidth: 0,
  overflow: "hidden",
  background: "#111",
  border: "1px solid var(--home-border)",
  borderRadius: "18px",
  boxShadow: "var(--home-panel-shadow)",
};

const subtleTransition = "background-color var(--anim-hover) var(--ease-out), border-color var(--anim-hover) var(--ease-out), color var(--anim-hover) var(--ease-out)";

export function HomeView() {
  const recents = useRecentStore((s) => s.recents);

  // Validate recent projects on mount to filter out folders deleted on disk
  useEffect(() => {
    void useRecentStore.getState().validateRecents();
  }, []);

  return (
    <div style={homeShellStyle}>
      <Sidebar />
      <main style={homeMainStyle}>
        <section style={homeWorkspaceStyle}>
          {recents.length === 0 ? <EmptyLauncher /> : <ProjectLauncher recents={recents} />}
        </section>
      </main>
    </div>
  );
}

function Sidebar() {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);
  const setSettingsOpen = useEditorUiStore((s) => s.setSettingsOpen);
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
    <aside data-tauri-drag-region style={homeSidebarStyle}>
      <div
        data-tauri-drag-region
        style={{
          padding: "0 var(--space-sm) var(--space-xl-xxl)",
          fontSize: "var(--fs-xl)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--tracking-tight)",
          color: "var(--home-foreground)",
        }}
      >
        {t("app.name")}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <SidebarRow primary icon={Plus} label={t("home.newProject")} onClick={() => void newProjectAndEnter()} />
        <SidebarRow
          icon={FolderOpen}
          label={opening ? t("home.opening") : t("home.openProject")}
          onClick={() => void handleOpen()}
        />
      </div>

      <div style={{ height: "var(--space-md)" }} />
      <SidebarRow icon={Library} label={t("library.entry")} onClick={() => setView("library")} />

      <div style={{ flex: 1 }} />

      <SidebarRow icon={SettingsIcon} label={t("home.settings")} onClick={() => setSettingsOpen(true)} />
    </aside>
  );
}

function SidebarRow({
  icon,
  label,
  onClick,
  primary = false,
}: {
  icon: typeof Plus;
  label: string;
  onClick: () => void;
  primary?: boolean;
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
        height: 32,
        padding: "0 var(--space-md)",
        borderRadius: "var(--radius-md)",
        background: primary ? "var(--home-primary)" : "transparent",
        color: primary ? "var(--home-primary-foreground)" : "var(--home-muted-foreground)",
        fontSize: "var(--fs-sm-md)",
        fontWeight: "var(--fw-medium)",
        textAlign: "left",
        transition: subtleTransition,
      }}
    >
      <Icon icon={icon} size={15} />
      <span>{label}</span>
    </button>
  );
}

function EmptyLauncher() {
  const t = useT();
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
    <section
      data-tauri-drag-region
      style={{
        flex: 1,
        minHeight: 0,
        display: "flex",
        flexDirection: "column",
        alignItems: "flex-start",
        justifyContent: "center",
        padding: "var(--space-xl-xxl)",
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          width: "min(560px, 100%)",
          margin: "0 auto",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          textAlign: "center",
        }}
      >
        <div
          data-tauri-drag-region
          style={{
            width: 34,
            height: 34,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            marginBottom: "var(--space-md-lg)",
            color: "var(--home-foreground)",
          }}
        >
          <Icon icon={Film} size={26} strokeWidth={1.6} />
        </div>
        <h1
          data-tauri-drag-region
          style={{
            margin: 0,
            fontSize: "var(--fs-title2)",
            fontWeight: "var(--fw-semibold)",
            letterSpacing: "var(--tracking-tight)",
            color: "var(--home-foreground)",
          }}
        >
          {t("home.welcome")}
        </h1>
        <p
          data-tauri-drag-region
          style={{
            margin: "var(--space-sm) 0 0",
            fontSize: "var(--fs-sm-md)",
            color: "var(--home-muted-foreground)",
            maxWidth: 460,
            lineHeight: 1.55,
          }}
        >
          {t("app.tagline")}
        </p>
        <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-xl)" }}>
          <LauncherButton primary label={t("home.newProject")} onClick={() => void newProjectAndEnter()} />
          <LauncherButton label={opening ? t("home.opening") : t("home.openProject")} onClick={() => void handleOpen()} />
        </div>
        <div
          style={{
            marginTop: "var(--space-xl)",
            color: "var(--home-muted-foreground)",
            fontSize: "var(--fs-xs)",
            lineHeight: 1.5,
          }}
        >
          {t("home.recentEmpty")}
        </div>
      </div>
    </section>
  );
}

function LauncherButton({
  label,
  onClick,
  primary = false,
}: {
  label: string;
  onClick: () => void;
  primary?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        height: 34,
        padding: "0 var(--space-lg-xl)",
        borderRadius: "var(--radius-md)",
        background: primary ? "var(--home-primary)" : "transparent",
        color: primary ? "var(--home-primary-foreground)" : "var(--home-muted-foreground)",
        border: primary ? "none" : "1px solid var(--home-border)",
        fontSize: "var(--fs-sm-md)",
        fontWeight: primary ? "var(--fw-semibold)" : "var(--fw-medium)",
        transition: subtleTransition,
      }}
    >
      {label}
    </button>
  );
}

function ProjectLauncher({ recents }: { recents: RecentProject[] }) {
  const t = useT();
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);

  const handleOpen = async () => {
    setOpening(true);
    try {
      await openProjectViaDialog();
    } finally {
      setOpening(false);
    }
  };

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
        minHeight: 0,
        padding: "var(--titlebar-safe-top) var(--space-xl-xxl) var(--space-xl-xxl)",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <ProjectHero opening={opening} onOpen={() => void handleOpen()} />
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          width: "100%",
          margin: "0 0 var(--space-sm)",
          color: "var(--home-muted-foreground)",
          fontSize: "var(--fs-xs)",
        }}
      >
        <span>{t("home.myProjects")}</span>
        <span className="tabular">{recents.length} recent</span>
      </div>
      <div
        style={{
          width: "100%",
          display: "grid",
          gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
          gap: "var(--space-sm)",
          minHeight: 0,
          overflowY: "auto",
        }}
      >
        {recents.map((entry) => (
          <ProjectGridCard
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
    </div>
  );
}

function ProjectHero({ opening, onOpen }: { opening: boolean; onOpen: () => void }) {
  const t = useT();

  return (
    <section
      data-tauri-drag-region
      style={{
        width: "min(760px, 100%)",
        margin: "0 0 var(--space-xl)",
        display: "flex",
        flexDirection: "column",
        alignItems: "flex-start",
        textAlign: "left",
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          width: 34,
          height: 34,
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          marginBottom: "var(--space-md-lg)",
          color: "var(--home-foreground)",
        }}
      >
        <Icon icon={Film} size={26} strokeWidth={1.6} />
      </div>
      <h1
        data-tauri-drag-region
        style={{
          margin: 0,
          fontSize: "var(--fs-title2)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--tracking-tight)",
          color: "var(--home-foreground)",
        }}
      >
        {t("home.welcome")}
      </h1>
      <p
        data-tauri-drag-region
        style={{
          margin: "var(--space-sm) 0 0",
          fontSize: "var(--fs-sm-md)",
          color: "var(--home-muted-foreground)",
          maxWidth: 460,
          lineHeight: 1.55,
        }}
      >
        {t("app.tagline")}
      </p>
      <div style={{ display: "flex", gap: "var(--space-sm)", marginTop: "var(--space-xl)" }}>
        <LauncherButton primary label={t("home.newProject")} onClick={() => void newProjectAndEnter()} />
        <LauncherButton label={opening ? t("home.opening") : t("home.openProject")} onClick={onOpen} />
      </div>
    </section>
  );
}

function ProjectGridCard({
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

  const cleanDisplayPath = entry.path.replace(/^\/Users\/[^\/]+/, "~");

  return (
    <div
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="home-project-card"
      style={{
        position: "relative",
        minHeight: 96,
        padding: "var(--space-md)",
        borderRadius: "var(--radius-md)",
        background: selected ? "var(--home-selected)" : "rgba(255,255,255,0.018)",
        border: selected ? "1px solid rgba(255,255,255,0.32)" : "1px solid var(--home-border)",
        color: "var(--home-foreground)",
        transition: subtleTransition,
        cursor: "default",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-start", gap: "var(--space-md)" }}>
        <div
          style={{
            width: 24,
            height: 24,
            flex: "0 0 auto",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: "var(--radius-md)",
            background: "var(--home-muted)",
            color: "var(--home-muted-foreground)",
          }}
        >
          <Icon icon={Film} size={13} />
        </div>
        <div style={{ minWidth: 0, flex: 1 }}>
          <div
            style={{
              fontSize: "var(--fs-sm-md)",
              color: "var(--home-foreground)",
              fontWeight: selected ? "var(--fw-semibold)" : "var(--fw-medium)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {entry.name}
          </div>
          <div
            className="tabular"
            title={entry.path}
            style={{
              marginTop: "var(--space-xs)",
              fontSize: "var(--fs-xs)",
              color: "var(--home-muted-foreground)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {cleanDisplayPath}
          </div>
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
            width: 26,
            height: 26,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: "var(--radius-md)",
            background: "var(--home-popover)",
            color: "var(--status-error)",
            zIndex: 4,
            border: "1px solid var(--home-border)",
          }}
        >
          <Icon icon={Trash2} size={14} />
        </button>
      )}
    </div>
  );
}
