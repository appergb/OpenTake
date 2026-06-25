/**
 * Home view (CapCut-style launcher, modeled on upstream `Project/HomeView.swift`).
 * Shown on launch before the editor. Left sidebar: New Project / Open Project /
 * Settings. Right content: a welcome header + the recent-projects grid (recents
 * persisted in localStorage). Selecting an action or a recent card enters the
 * editor. Built entirely from design tokens so it sits consistently with the
 * editor's dark surface.
 */

import { useState } from "react";
import { Plus, FolderOpen, Settings as SettingsIcon, Film, Trash2, Library, LogIn, LogOut, FileQuestion, Sparkles } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT, type TFunction } from "../../i18n";
import { useEditorUiStore } from "../../store/uiStore";
import { useRecentStore, type RecentProject } from "../../store/recentStore";
import {
  newProjectAndEnter,
  openProjectViaDialog,
  openProjectPath,
} from "../../store/projectActions";

export function HomeView() {
  const t = useT();
  const [signedIn, setSignedIn] = useState(false);
  const [seenWelcome, setSeenWelcome] = useState(
    () => typeof localStorage !== "undefined" && localStorage.getItem("seenWelcome") === "true",
  );
  const [seenBadge, setSeenBadge] = useState(
    () => typeof localStorage !== "undefined" && localStorage.getItem("seenVersionBadge") === __APP_VERSION__,
  );

  const dismissWelcome = () => {
    localStorage.setItem("seenWelcome", "true");
    setSeenWelcome(true);
  };
  const dismissBadge = () => {
    localStorage.setItem("seenVersionBadge", __APP_VERSION__);
    setSeenBadge(true);
  };

  return (
    <div
      style={{
        display: "flex",
        height: "100%",
        width: "100%",
        background: "var(--bg-base)",
        color: "var(--text-primary)",
        position: "relative",
      }}
    >
      <Sidebar signedIn={signedIn} onToggleSignIn={() => setSignedIn((v) => !v)} />
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
            position: "relative",
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
          {!seenBadge && (
            <button
              type="button"
              onClick={dismissBadge}
              style={{
                position: "absolute",
                top: "var(--space-xl)",
                right: "var(--space-xl-xxl)",
                display: "inline-flex",
                alignItems: "center",
                gap: 4,
                height: 26,
                padding: "0 var(--space-md)",
                borderRadius: "var(--radius-lg)",
                background: "var(--accent-primary)",
                color: "#fff",
                fontSize: "var(--fs-xs)",
                fontWeight: "var(--fw-semibold)",
                border: "none",
                cursor: "pointer",
              }}
            >
              <Icon icon={Sparkles} size={12} />
              {t("home.newInVersion", { version: __APP_VERSION__ })}
            </button>
          )}
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
          {t("home.samples")}
        </h2>
        <SampleProjectsStrip t={t} />

        <h2
          style={{
            margin: 0,
            padding: "var(--space-lg) var(--space-xl-xxl) var(--space-sm)",
            fontSize: "var(--fs-md)",
            fontWeight: "var(--fw-semibold)",
            color: "var(--text-secondary)",
          }}
        >
          {t("home.myProjects")}
        </h2>
        <ProjectGrid />
      </main>

      {/* Welcome overlay — first launch only */}
      {!seenWelcome && (
        <div
          onClick={dismissWelcome}
          style={{
            position: "absolute",
            inset: 0,
            zIndex: 100,
            background: "rgba(0,0,0,0.65)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--bg-raised)",
              border: "var(--bw-thin) solid var(--border-primary)",
              borderRadius: "var(--radius-lg)",
              padding: "var(--space-xl-xxl)",
              maxWidth: 400,
              textAlign: "center",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: "var(--space-lg)",
            }}
          >
            <Icon icon={Film} size={40} strokeWidth={1.2} />
            <h2 style={{ margin: 0, fontSize: "var(--fs-title2)", fontWeight: "var(--fw-light)" }}>
              {t("home.welcomeOverlayTitle")}
            </h2>
            <p style={{ margin: 0, fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
              {t("home.welcomeOverlayBody")}
            </p>
            <button
              type="button"
              onClick={dismissWelcome}
              style={{
                height: 34,
                padding: "0 var(--space-xl)",
                borderRadius: "var(--radius-sm)",
                background: "var(--accent-primary)",
                color: "#fff",
                fontSize: "var(--fs-md)",
                fontWeight: "var(--fw-semibold)",
                border: "none",
                cursor: "pointer",
              }}
            >
              {t("home.welcomeOverlayStart")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function Sidebar({
  signedIn,
  onToggleSignIn,
}: {
  signedIn: boolean;
  onToggleSignIn: () => void;
}) {
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
      <SidebarRow
        icon={signedIn ? LogOut : LogIn}
        label={signedIn ? t("home.signOut") : t("home.signIn")}
        onClick={onToggleSignIn}
      />
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

/** Strip of sample projects (Issue #40 review — "SampleProjectsStrip 示例区"). */
function SampleProjectsStrip({ t }: { t: TFunction }) {
  const samples = [
    { key: "demo", label: t("home.sampleDemo") },
    { key: "tutorial", label: t("home.sampleTutorial") },
    { key: "template", label: t("home.sampleTemplate") },
    { key: "more", label: "…" },
  ];

  return (
    <div
      style={{
        padding: "0 var(--space-xl-xxl) var(--space-md)",
        display: "flex",
        gap: "var(--space-md)",
        overflowX: "auto",
      }}
    >
      {samples.map((s) => (
        <button
          key={s.key}
          type="button"
          onClick={() => alert(t("home.sampleComingSoon"))}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            height: 80,
            minWidth: 140,
            borderRadius: "var(--radius-md)",
            background: "var(--bg-raised)",
            border: "var(--bw-thin) solid var(--border-primary)",
            color: "var(--text-secondary)",
            fontSize: "var(--fs-sm-md)",
            fontWeight: "var(--fw-medium)",
            cursor: "pointer",
            whiteSpace: "nowrap",
          }}
        >
          <Icon icon={Film} size={18} strokeWidth={1.4} />
          <span style={{ marginLeft: "var(--space-sm)" }}>{s.label}</span>
        </button>
      ))}
    </div>
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
        transform: hovered ? "scale(1.03)" : "scale(1)",
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

/** Format `openedAt` (epoch ms) as a relative time string — today / yesterday /
 *  N days ago / N weeks ago / N months ago. Mirrors upstream's
 *  `RelativeDateTimeFormatter` output. */
function relativeTime(openedAt: number, t: TFunction): string {
  const now = Date.now();
  const diffMs = now - openedAt;
  const dayMs = 86_400_000;
  const days = Math.floor(diffMs / dayMs);
  if (days <= 0) return t("home.relative.today");
  if (days === 1) return t("home.relative.yesterday");
  if (days < 7) return t("home.relative.daysAgo", { count: days });
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return t("home.relative.weeksAgo", { count: weeks });
  const months = Math.floor(days / 30);
  return t("home.relative.monthsAgo", { count: months });
}

function ProjectCard({ entry }: { entry: RecentProject }) {
  const t = useT();
  const remove = useRecentStore((s) => s.remove);
  const [hovered, setHovered] = useState(false);
  const [missing, setMissing] = useState(false);

  const handleOpen = async () => {
    try {
      await openProjectPath(entry.path);
    } catch {
      setMissing(true);
    }
  };

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{
        position: "relative",
        zIndex: hovered ? 2 : 1,
        transform: hovered ? "scale(1.03)" : "scale(1)",
        transition: "transform var(--anim-transition) var(--ease-out)",
      }}
    >
      <button
        type="button"
        onClick={() => void handleOpen()}
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
          {/* Bottom gradient + name overlay (mirrors upstream ProjectCard's
              60pt black gradient + white title). Keeps the title inside the
              thumbnail so the card footprint matches upstream. */}
          <div
            style={{
              position: "absolute",
              left: 0,
              right: 0,
              bottom: 0,
              height: 60,
              background: "linear-gradient(to top, rgba(0,0,0,0.7), rgba(0,0,0,0))",
              display: "flex",
              alignItems: "flex-end",
              padding: "var(--space-sm)",
              pointerEvents: "none",
            }}
          >
            <span
              style={{
                color: "#fff",
                fontSize: "var(--fs-sm)",
                fontWeight: "var(--fw-medium)",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                maxWidth: "100%",
              }}
            >
              {entry.name}
            </span>
          </div>
          {/* File missing overlay (Issue #40 review) */}
          {missing && (
            <div
              style={{
                position: "absolute",
                inset: 0,
                background: "rgba(0,0,0,0.55)",
                display: "flex",
                flexDirection: "column",
                alignItems: "center",
                justifyContent: "center",
                gap: "var(--space-xs)",
                color: "var(--text-muted)",
                pointerEvents: "none",
              }}
            >
              <Icon icon={FileQuestion} size={22} strokeWidth={1.4} />
              <span style={{ fontSize: "var(--fs-xs)" }}>{t("home.fileMissing")}</span>
            </div>
          )}
        </div>
        <div
          className="tabular"
          style={{
            marginTop: "var(--space-sm)",
            fontSize: "var(--fs-xs)",
            color: "var(--text-muted)",
          }}
        >
          {relativeTime(entry.openedAt, t)}
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
            borderRadius: "50%",
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
