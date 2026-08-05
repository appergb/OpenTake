import { useEffect, useRef, useState, type CSSProperties } from "react";
import {
  Plus,
  FolderOpen,
  Settings as SettingsIcon,
  Film,
  Trash2,
  Library,
  MoreHorizontal,
  Sparkles,
  Cpu,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT, type TFunction } from "../../i18n";
import { assetUrl } from "../../lib/asset";
import * as api from "../../lib/api";
import type { GenerationLog } from "../../lib/types";
import { useEditorUiStore } from "../../store/uiStore";
import { useRecentStore, type RecentProject } from "../../store/recentStore";
import {
  newProjectAndEnter,
  openSampleProject,
  openProjectViaDialog,
  openProjectPath,
} from "../../store/projectActions";

export function formatProjectRelativeTime(
  t: TFunction,
  timestamp: number,
  now = Date.now(),
): string {
  const then = new Date(timestamp);
  const current = new Date(now);
  const thenDay = new Date(then.getFullYear(), then.getMonth(), then.getDate()).getTime();
  const currentDay = new Date(
    current.getFullYear(),
    current.getMonth(),
    current.getDate(),
  ).getTime();
  const days = Math.max(0, Math.round((currentDay - thenDay) / 86_400_000));
  if (days === 0) return t("home.relative.today");
  if (days === 1) return t("home.relative.yesterday");
  if (days < 7) return t("home.relative.daysAgo", { count: days });
  if (days < 35) return t("home.relative.weeksAgo", { count: Math.floor(days / 7) });
  return t("home.relative.monthsAgo", { count: Math.max(1, Math.floor(days / 30)) });
}

/** `createdAt` rows are Apple-reference-date seconds (upstream Swift `Date`
 *  encoding) — the 2001-01-01 epoch. Convert to Unix ms for display. */
export function appleReferenceSecondsToMs(seconds: number): number {
  return (seconds + 978_307_200) * 1000;
}

type GenerationActivityState =
  | { status: "loading" }
  | { status: "error" }
  | { status: "ready"; log: GenerationLog };

/** Read-only AI generation audit for the current session: newest rows first
 *  (timestamp · model · credits) plus total spend. Mirrors `generation_log`;
 *  the UI never mutates the log — the core's generation lifecycle is the only
 *  writer. Loads whenever Home becomes active; outside Tauri it resolves to the
 *  honest empty log. */
function GenerationActivity({ active }: { active: boolean }) {
  const t = useT();
  const [state, setState] = useState<GenerationActivityState>({ status: "loading" });

  useEffect(() => {
    if (!active) return;
    let disposed = false;
    setState({ status: "loading" });
    api
      .generationLog()
      .then((log) => {
        if (!disposed) setState({ status: "ready", log });
      })
      .catch(() => {
        if (!disposed) setState({ status: "error" });
      });
    return () => {
      disposed = true;
    };
  }, [active]);

  const totalCredits =
    state.status === "ready"
      ? state.log.entries.reduce((sum, entry) => sum + (entry.costCredits ?? 0), 0)
      : 0;

  return (
    <section
      aria-labelledby="home-generation-heading"
      style={{
        padding: "var(--space-md) var(--space-xl-xxl) var(--space-xl-xxl)",
        borderTop: "1px solid var(--home-border)",
        color: "var(--home-muted-foreground)",
        fontSize: "var(--fs-xs)",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "var(--space-md)",
          marginBottom: "var(--space-sm)",
        }}
      >
        <h2
          id="home-generation-heading"
          style={{
            margin: 0,
            display: "inline-flex",
            alignItems: "center",
            gap: "var(--space-xs)",
            fontSize: "var(--fs-xs)",
            fontWeight: 500,
          }}
        >
          <Icon icon={Cpu} size={13} />
          {t("home.generationActivity")}
        </h2>
        {state.status === "ready" && state.log.entries.length > 0 && (
          <span className="tabular">
            {t("home.generationActivityTotal", {
              count: state.log.entries.length,
              credits: totalCredits,
            })}
          </span>
        )}
      </div>
      {state.status === "loading" && <div role="status">{t("home.generationActivityLoading")}</div>}
      {state.status === "error" && (
        <div role="alert">{t("home.generationActivityFailed")}</div>
      )}
      {state.status === "ready" && state.log.entries.length === 0 && (
        <div>{t("home.generationActivityEmpty")}</div>
      )}
      {state.status === "ready" && state.log.entries.length > 0 && (
        <ul
          style={{
            listStyle: "none",
            margin: 0,
            padding: 0,
            maxHeight: 132,
            overflowY: "auto",
            display: "grid",
            gap: "var(--space-xxs)",
          }}
        >
          {[...state.log.entries].reverse().map((entry) => {
            const when = entry.createdAt
              ? formatProjectRelativeTime(t, appleReferenceSecondsToMs(entry.createdAt))
              : undefined;
            return (
              <li
                key={entry.id}
                title={
                  entry.createdAt
                    ? new Date(appleReferenceSecondsToMs(entry.createdAt)).toLocaleString()
                    : undefined
                }
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "var(--space-sm)",
                  minWidth: 0,
                }}
              >
                <span style={{ flex: "0 0 auto" }}>{when ?? "—"}</span>
                <span style={{ flex: "0 0 auto", color: "var(--home-foreground)" }}>
                  {entry.model}
                </span>
                <span style={{ flex: 1, minWidth: 0 }} />
                <span className="tabular" style={{ flex: "0 0 auto" }}>
                  {entry.costCredits !== undefined
                    ? `${entry.costCredits} ${t("home.generationActivityCreditsUnit")}`
                    : t("home.generationActivityCostUnknown")}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

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

const projectMenuButtonStyle: CSSProperties = {
  minHeight: 28,
  padding: "0 var(--space-sm)",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "flex-start",
  gap: "var(--space-xs)",
  borderRadius: "var(--radius-sm)",
  color: "var(--home-muted-foreground)",
};

type ProjectAction = "new" | "open" | "sample" | null;

export const HOME_NOTICE_STORAGE_KEY = "opentake.home.lastSeenVersion";
export const HOME_NOTICE_VERSION = __APP_VERSION__;

type HomeNotice = "welcome" | "whatsNew" | null;

export function resolveHomeNotice(
  lastSeenVersion: string | null,
  hasRecentProjects: boolean,
): Exclude<HomeNotice, null> | null {
  if (lastSeenVersion === HOME_NOTICE_VERSION) return null;
  return lastSeenVersion === null && !hasRecentProjects ? "welcome" : "whatsNew";
}

function loadHomeNotice(hasRecentProjects: boolean): HomeNotice {
  try {
    return resolveHomeNotice(localStorage.getItem(HOME_NOTICE_STORAGE_KEY), hasRecentProjects);
  } catch {
    return hasRecentProjects ? "whatsNew" : "welcome";
  }
}

function persistHomeNoticeSeen() {
  try {
    localStorage.setItem(HOME_NOTICE_STORAGE_KEY, HOME_NOTICE_VERSION);
  } catch {
    // Storage can be unavailable in privacy-restricted browser previews. The
    // notice still dismisses for this session; the editor remains usable.
  }
}

export function HomeView() {
  const recents = useRecentStore((s) => s.recents);
  const active = useEditorUiStore((s) => s.view === "home");
  const [projectAction, setProjectAction] = useState<ProjectAction>(null);
  const [homeNotice, setHomeNotice] = useState<HomeNotice>(() =>
    loadHomeNotice(recents.length > 0),
  );
  const projectActionRef = useRef<ProjectAction>(null);

  const dismissHomeNotice = () => {
    persistHomeNoticeSeen();
    setHomeNotice(null);
  };

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

  // Home is kept alive while the editor is open. Revalidate when it becomes
  // active or the window regains focus so a File Provider download completed
  // in Finder becomes usable without requiring an application restart.
  useEffect(() => {
    if (!active) return;
    const validate = () => {
      void useRecentStore.getState().validateRecents();
    };
    validate();
    window.addEventListener("focus", validate);
    return () => window.removeEventListener("focus", validate);
  }, [active]);

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
              active={active}
              recents={recents}
              projectAction={projectAction}
              onNew={() => void runProjectAction("new", newProjectAndEnter)}
              onOpen={() => void runProjectAction("open", openProjectViaDialog)}
              onOpenPath={(path) =>
                void runProjectAction("open", () => openProjectPath(path))
              }
            />
          )}
          <GenerationActivity active={active} />
        </section>
      </main>
      {homeNotice && (
        <HomeNoticeDialog active={active} kind={homeNotice} onDismiss={dismissHomeNotice} />
      )}
    </div>
  );
}

function HomeNoticeDialog({
  active,
  kind,
  onDismiss,
}: {
  active: boolean;
  kind: Exclude<HomeNotice, null>;
  onDismiss: () => void;
}) {
  const t = useT();
  const buttonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!active) return;
    buttonRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      onDismiss();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active, onDismiss]);

  const isWelcome = kind === "welcome";
  return (
    <div
      data-testid="home-notice-backdrop"
      className="app-dialog-backdrop"
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 40,
        display: "grid",
        placeItems: "center",
        padding: "var(--space-xl)",
        background: "rgba(0,0,0,0.62)",
        backdropFilter: "blur(8px)",
      }}
    >
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="home-notice-title"
        className="app-dialog-surface"
        style={{
          width: "min(520px, 100%)",
          padding: "var(--space-xl-xxl)",
          border: "1px solid var(--home-border)",
          borderRadius: "var(--radius-xl)",
          background: "var(--home-popover)",
          boxShadow: "var(--home-panel-shadow)",
          color: "var(--home-foreground)",
        }}
      >
        <Icon icon={Sparkles} size={24} />
        <h1 id="home-notice-title" style={{ margin: "var(--space-md) 0 var(--space-sm)", fontSize: "var(--fs-title2)" }}>
          {isWelcome
            ? t("home.welcomeOverlayTitle")
            : t("home.newInVersion", { version: HOME_NOTICE_VERSION })}
        </h1>
        <p style={{ margin: 0, color: "var(--home-muted-foreground)", lineHeight: 1.6 }}>
          {isWelcome ? t("home.welcomeOverlayBody") : t("home.updateOverlayBody")}
        </p>
        <button
          ref={buttonRef}
          type="button"
          onClick={onDismiss}
          style={{
            marginTop: "var(--space-xl)",
            minHeight: 34,
            padding: "0 var(--space-lg-xl)",
            borderRadius: "var(--radius-md)",
            background: "var(--home-primary)",
            color: "var(--home-primary-foreground)",
            fontWeight: "var(--fw-semibold)",
          }}
        >
          {isWelcome ? t("home.welcomeOverlayStart") : t("home.updateOverlayDismiss")}
        </button>
      </section>
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
  active,
  recents,
  projectAction,
  onNew,
  onOpen,
  onOpenPath,
}: {
  active: boolean;
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
    if (!active) return;
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
          const selected = recents.find((entry) => entry.path === selectedPath);
          if (!selected?.missing && !selected?.offline) onOpenPath(selectedPath);
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [active, busy, onNew, onOpenPath, recents, selectedPath]);

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
        <span className="tabular">{t("home.recentCount", { count: recents.length })}</span>
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
            onDoubleClick={() => {
              if (!entry.missing && !entry.offline) onOpenPath(entry.path);
            }}
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
  const reveal = useRecentStore((s) => s.reveal);
  const trash = useRecentStore((s) => s.trash);
  const thumbnailPathsValidated = useRecentStore((s) => s.thumbnailPathsValidated);
  const [hovered, setHovered] = useState(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const [confirmTrash, setConfirmTrash] = useState(false);
  const [pending, setPending] = useState<"reveal" | "remove" | "trash" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [thumbnailFailed, setThumbnailFailed] = useState(false);

  const actionsVisible = hovered || actionsOpen || confirmTrash || actionError !== null;
  const unavailable = Boolean(entry.missing || entry.offline);
  const unavailableLabel = entry.missing ? t("home.fileMissing") : t("home.fileOffline");
  const coverUrl = unavailable || thumbnailFailed || !thumbnailPathsValidated
    ? null
    : assetUrl(entry.thumbnailPath);
  const modifiedLabel = formatProjectRelativeTime(t, entry.modifiedAt ?? entry.openedAt);

  useEffect(
    () => setThumbnailFailed(false),
    [entry.missing, entry.offline, entry.modifiedAt, entry.thumbnailPath, thumbnailPathsValidated],
  );

  const runReveal = async () => {
    setPending("reveal");
    setActionError(null);
    try {
      await reveal(entry.path);
      setActionsOpen(false);
    } catch (error) {
      setActionError(t("home.revealFailed", {
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setPending(null);
    }
  };

  const runTrash = async () => {
    setPending("trash");
    setActionError(null);
    try {
      await trash(entry.path);
      setConfirmTrash(false);
      setActionsOpen(false);
    } catch (error) {
      setActionError(t("home.trashFailed", {
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setPending(null);
    }
  };

  const runRemove = async () => {
    setPending("remove");
    setActionError(null);
    try {
      await remove(entry.path);
      setActionsOpen(false);
    } catch (error) {
      setActionError(t("home.removeFailed", {
        error: error instanceof Error ? error.message : String(error),
      }));
    } finally {
      setPending(null);
    }
  };

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
        setActionsOpen(true);
      }}
      style={{
        position: "relative",
        minWidth: 0,
      }}
    >
      <button
        type="button"
        disabled={disabled}
        aria-label={unavailable ? `${entry.name} · ${unavailableLabel}` : entry.name}
        aria-pressed={selected}
        aria-describedby={unavailable ? `unavailable-${entry.path}` : undefined}
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
              width: "100%",
              height: 48,
              flex: "0 0 auto",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              borderRadius: "var(--radius-md)",
              background: "var(--home-muted)",
              color: "var(--home-muted-foreground)",
              overflow: "hidden",
            }}
          >
            {coverUrl ? (
              <img
                src={coverUrl}
                alt=""
                onError={() => setThumbnailFailed(true)}
                style={{ width: "100%", height: "100%", objectFit: "cover" }}
              />
            ) : <Icon icon={Film} size={16} />}
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
              {unavailable ? (
                <span id={`unavailable-${entry.path}`} style={{ color: "var(--status-error)" }}>
                  {unavailableLabel}
                </span>
              ) : modifiedLabel}
            </div>
            {unavailable && (
              <div
                className="tabular"
                style={{
                  marginTop: 2,
                  fontSize: "var(--fs-xs)",
                  color: "var(--home-muted-foreground)",
                }}
              >
                {modifiedLabel}
              </div>
            )}
          </div>
        </div>
      </button>

      {!disabled && (
        <button
          type="button"
          title={t("home.projectActions")}
          aria-label={t("home.projectActions")}
          aria-expanded={actionsOpen || confirmTrash}
          onClick={(e) => {
            e.stopPropagation();
            setActionsOpen((open) => !open);
            setConfirmTrash(false);
            setActionError(null);
          }}
          onFocus={() => setHovered(true)}
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
            color: "var(--home-muted-foreground)",
            opacity: actionsVisible ? 1 : 0.62,
            zIndex: 4,
            border: "1px solid var(--home-border)",
          }}
        >
          <Icon icon={MoreHorizontal} size={14} />
        </button>
      )}

      {hovered && !disabled && !actionsOpen && !confirmTrash && (
        <button
          type="button"
          title={t("home.remove")}
          aria-label={t("home.remove")}
          onClick={(event) => {
            event.stopPropagation();
            void runRemove();
          }}
          style={{
            position: "absolute",
            top: "var(--space-sm)",
            right: 38,
            width: 26,
            height: 26,
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--home-border)",
            background: "var(--home-popover)",
            color: "var(--status-error)",
            zIndex: 4,
          }}
        >
          <Icon icon={Trash2} size={13} />
        </button>
      )}

      {(actionsOpen || confirmTrash || actionError) && !disabled && (
        <div
          role="dialog"
          aria-label={confirmTrash ? t("home.confirmTrashTitle") : t("home.projectActions")}
          className="app-dialog-surface"
          onClick={(event) => event.stopPropagation()}
          style={{
            position: "absolute",
            top: 38,
            right: "var(--space-sm)",
            width: 220,
            padding: "var(--space-sm)",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--home-border)",
            background: "var(--home-popover)",
            boxShadow: "var(--home-panel-shadow)",
            zIndex: 8,
          }}
        >
          {confirmTrash ? (
            <>
              <div style={{ fontSize: "var(--fs-xs)", lineHeight: 1.45 }}>
                {t("home.confirmTrashBody", { name: entry.name })}
              </div>
              <div style={{ display: "flex", gap: "var(--space-xs)", marginTop: "var(--space-sm)" }}>
                <button
                  type="button"
                  disabled={pending !== null}
                  onClick={() => void runTrash()}
                  style={{ ...projectMenuButtonStyle, color: "var(--status-error)" }}
                >
                  <Icon icon={Trash2} size={13} /> {pending === "trash" ? t("home.trashing") : t("home.moveToTrash")}
                </button>
                <button
                  type="button"
                  disabled={pending !== null}
                  style={projectMenuButtonStyle}
                  onClick={() => {
                    setConfirmTrash(false);
                    setActionError(null);
                  }}
                >
                  {t("common.cancel")}
                </button>
              </div>
            </>
          ) : (
            <div style={{ display: "grid", gap: "var(--space-xs)" }}>
              <button
                type="button"
                disabled={pending !== null}
                style={{ ...projectMenuButtonStyle, width: "100%" }}
                onClick={() => void runReveal()}
              >
                <Icon icon={FolderOpen} size={13} /> {pending === "reveal" ? t("home.revealing") : t("home.revealInFinder")}
              </button>
              <button
                type="button"
                disabled={pending !== null}
                style={{ ...projectMenuButtonStyle, width: "100%" }}
                onClick={() => void runRemove()}
              >
                {pending === "remove" ? t("home.removing") : t("home.removeFromRecents")}
              </button>
              <button
                type="button"
                disabled={pending !== null}
                onClick={() => setConfirmTrash(true)}
                style={{
                  ...projectMenuButtonStyle,
                  width: "100%",
                  color: "var(--status-error)",
                }}
              >
                <Icon icon={Trash2} size={13} /> {t("home.moveToTrash")}
              </button>
            </div>
          )}
          {actionError && (
            <div role="alert" style={{ marginTop: "var(--space-sm)", color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
              {actionError}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
