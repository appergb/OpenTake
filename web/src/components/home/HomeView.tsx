import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Plus, FolderOpen, Settings as SettingsIcon, Film, Trash2, Library } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useRecentStore, type RecentProject } from "../../store/recentStore";
import {
  newProjectAndEnter,
  openSampleProject,
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

type ProjectAction = "new" | "open" | "sample" | null;

export function HomeView() {
  const recents = useRecentStore((s) => s.recents);
  const [projectAction, setProjectAction] = useState<ProjectAction>(null);
  const projectActionRef = useRef<ProjectAction>(null);

  const runProjectAction = async (
    action: Exclude<ProjectAction, null>,
    operation: () => Promise<void>,
  ) => {
    if (projectActionRef.current) return;
    projectActionRef.current = action;
    setProjectAction(action);
    try {
      await operation();
    } catch {
      // Project actions own their localized error toast;
      // keep the UI gesture handled so a native rejection is not unhandled.
    } finally {
      projectActionRef.current = null;
      setProjectAction(null);
    }
  };

  // Validate recent projects on mount to filter out folders deleted on disk
  useEffect(() => {
    void useRecentStore.getState().validateRecents();
  }, []);

  return (
    <div style={homeShellStyle}>
      <Sidebar
        projectAction={projectAction}
        onNew={() => void runProjectAction("new", newProjectAndEnter)}
        onOpen={() => void runProjectAction("open", openProjectViaDialog)}
      />
      <main style={homeMainStyle}>
        <section style={homeWorkspaceStyle}>
          <SampleProjectsStrip
            busy={projectAction !== null}
            onOpen={(slug, tutorial) =>
              void runProjectAction("sample", () => openSampleProject(slug, tutorial))
            }
          />
          {recents.length === 0 ? (
            <EmptyLauncher
              projectAction={projectAction}
              onNew={() => void runProjectAction("new", newProjectAndEnter)}
              onOpen={() => void runProjectAction("open", openProjectViaDialog)}
            />
          ) : (
            <ProjectLauncher
              recents={recents}
              projectAction={projectAction}
              onNew={() => void runProjectAction("new", newProjectAndEnter)}
              onOpen={() => void runProjectAction("open", openProjectViaDialog)}
              onOpenPath={(path) =>
                void runProjectAction("open", () => openProjectPath(path))
              }
            />
          )}
        </section>
      </main>
    </div>
  );
}

const SAMPLE_PROJECTS = [
  { slug: "product-demo", label: "home.sampleDemo", tutorial: false },
  { slug: "quick-tutorial", label: "home.sampleTutorial", tutorial: true },
  { slug: "template-project", label: "home.sampleTemplate", tutorial: false },
] as const;

function SampleProjectsStrip({
  busy,
  onOpen,
}: {
  busy: boolean;
  onOpen: (slug: string, tutorial: boolean) => void;
}) {
  const t = useT();
  return (
    <section
      aria-labelledby="home-samples-heading"
      style={{ padding: "var(--titlebar-safe-top) var(--space-xl-xxl) 0" }}
    >
      <h2
        id="home-samples-heading"
        style={{ margin: "0 0 var(--space-sm)", fontSize: "var(--fs-xs)", fontWeight: 500 }}
      >
        {t("home.samples")}
      </h2>
      <div style={{ display: "flex", gap: "var(--space-sm)" }}>
        {SAMPLE_PROJECTS.map((sample) => (
          <button
            key={sample.slug}
            type="button"
            disabled={busy}
            aria-busy={busy || undefined}
            onClick={() => onOpen(sample.slug, sample.tutorial)}
            className="hover-area"
            style={{
              minHeight: 34,
              padding: "0 var(--space-md)",
              border: "1px solid var(--home-border)",
              borderRadius: "var(--radius-md)",
              background: "rgba(255,255,255,0.018)",
              color: "var(--home-muted-foreground)",
              opacity: busy ? 0.55 : 1,
            }}
          >
            {t(sample.label)}
          </button>
        ))}
      </div>
    </section>
  );
}

function Sidebar({
  projectAction,
  onNew,
  onOpen,
}: {
  projectAction: ProjectAction;
  onNew: () => void;
  onOpen: () => void;
}) {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);
  const setSettingsOpen = useEditorUiStore((s) => s.setSettingsOpen);
  const busy = projectAction !== null;

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
        <SidebarRow
          primary
          icon={Plus}
          label={projectAction === "new" ? t("home.creating") : t("home.newProject")}
          onClick={onNew}
          disabled={busy}
        />
        <SidebarRow
          icon={FolderOpen}
          label={projectAction === "open" ? t("home.opening") : t("home.openProject")}
          onClick={onOpen}
          disabled={busy}
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
  disabled = false,
}: {
  icon: typeof Plus;
  label: string;
  onClick: () => void;
  primary?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-busy={disabled || undefined}
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
        opacity: disabled ? 0.55 : 1,
      }}
    >
      <Icon icon={icon} size={15} />
      <span>{label}</span>
    </button>
  );
}

function EmptyLauncher({
  projectAction,
  onNew,
  onOpen,
}: {
  projectAction: ProjectAction;
  onNew: () => void;
  onOpen: () => void;
}) {
  const t = useT();
  const busy = projectAction !== null;

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
          <LauncherButton
            primary
            label={projectAction === "new" ? t("home.creating") : t("home.newProject")}
            onClick={onNew}
            disabled={busy}
          />
          <LauncherButton
            label={projectAction === "open" ? t("home.opening") : t("home.openProject")}
            onClick={onOpen}
            disabled={busy}
          />
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
  disabled = false,
}: {
  label: string;
  onClick: () => void;
  primary?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-busy={disabled || undefined}
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
        opacity: disabled ? 0.55 : 1,
      }}
    >
      {label}
    </button>
  );
}

function ProjectLauncher({
  recents,
  projectAction,
  onNew,
  onOpen,
  onOpenPath,
}: {
  recents: RecentProject[];
  projectAction: ProjectAction;
  onNew: () => void;
  onOpen: () => void;
  onOpenPath: (path: string) => void;
}) {
  const t = useT();
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const busy = projectAction !== null;

  // Listen to KeyDown to open project when selected + Enter is pressed
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Enter" && selectedPath && !busy) {
        const target = e.target instanceof Element ? e.target : null;
        const interactiveTarget = target?.closest(
          "button, a, input, textarea, select, [role='button'], [contenteditable='true']",
        );
        if (interactiveTarget && !target?.closest(".home-project-card")) return;
        e.preventDefault();
        if (selectedPath === "new") {
          onNew();
        } else {
          onOpenPath(selectedPath);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onNew, onOpenPath, selectedPath]);

  return (
    <div
      onClick={() => setSelectedPath(null)}
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        padding: "var(--titlebar-safe-top) var(--space-xl-xxl) var(--space-xl-xxl)",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <ProjectHero projectAction={projectAction} onNew={onNew} onOpen={onOpen} />
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
          minWidth: 0,
          overflowX: "hidden",
          overflowY: "auto",
        }}
      >
        {recents.map((entry) => (
          <ProjectGridCard
            key={entry.path}
            entry={entry}
            selected={selectedPath === entry.path}
            disabled={busy}
            onSelect={() => setSelectedPath(entry.path)}
            onDoubleClick={() => onOpenPath(entry.path)}
          />
        ))}
      </div>
    </div>
  );
}

function ProjectHero({
  projectAction,
  onNew,
  onOpen,
}: {
  projectAction: ProjectAction;
  onNew: () => void;
  onOpen: () => void;
}) {
  const t = useT();
  const busy = projectAction !== null;

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
        <LauncherButton
          primary
          label={projectAction === "new" ? t("home.creating") : t("home.newProject")}
          onClick={onNew}
          disabled={busy}
        />
        <LauncherButton
          label={projectAction === "open" ? t("home.opening") : t("home.openProject")}
          onClick={onOpen}
          disabled={busy}
        />
      </div>
    </section>
  );
}

function ProjectGridCard({
  entry,
  selected,
  disabled,
  onSelect,
  onDoubleClick,
}: {
  entry: RecentProject;
  selected: boolean;
  disabled: boolean;
  onSelect: () => void;
  onDoubleClick: () => void;
}) {
  const t = useT();
  const remove = useRecentStore((s) => s.remove);
  const [hovered, setHovered] = useState(false);

  const cleanDisplayPath = entry.path.replace(/^\/Users\/[^\/]+/, "~");

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        position: "relative",
        minWidth: 0,
      }}
    >
      <button
        type="button"
        disabled={disabled}
        aria-label={entry.name}
        aria-pressed={selected}
        onClick={(event) => {
          event.stopPropagation();
          onSelect();
        }}
        onFocus={onSelect}
        onDoubleClick={onDoubleClick}
        className="home-project-card"
        style={{
          width: "100%",
          minHeight: 96,
          padding: "var(--space-md)",
          borderRadius: "var(--radius-md)",
          background: selected ? "var(--home-selected)" : "rgba(255,255,255,0.018)",
          border: selected ? "1px solid rgba(255,255,255,0.32)" : "1px solid var(--home-border)",
          color: "var(--home-foreground)",
          transition: subtleTransition,
          cursor: "default",
          textAlign: "left",
          opacity: disabled ? 0.55 : 1,
        }}
      >
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-start",
            gap: "var(--space-md)",
            minWidth: 0,
            width: "100%",
          }}
        >
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
          <div style={{ minWidth: 0, width: "100%" }}>
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
      </button>

      {hovered && !disabled && (
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
