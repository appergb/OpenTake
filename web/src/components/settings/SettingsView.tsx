/**
 * Settings view. Reachable from both the Home sidebar and the editor title bar.
 * Panes (single scrollable page in this phase): General (language), Appearance
 * (dark window layout), Import, AI (BYOK), MCP, optional Account, and About.
 * Preferences persist via `settingsStore` / `i18nStore`;
 * the BYOK key is stored in the OS keychain via the `secret_*` Tauri commands
 * (see `lib/api.ts`) — the plaintext key never reaches this component's
 * persisted state.
 */

import { useEffect, useRef, useState, type CSSProperties } from "react";
import {
  Bot,
  Copy,
  Download,
  ExternalLink,
  FolderOpen,
  HardDrive,
  Info,
  Palette,
  Plug,
  Settings as SettingsIcon,
  Trash2,
  User,
  X,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { Dropdown } from "../ui/Dropdown";
import { useT, useI18nStore, LOCALES } from "../../i18n";
import {
  useSettingsStore,
  type ByokProvider,
  type WindowSizeOpt,
} from "../../store/settingsStore";
import { useEditorUiStore, type SettingsPaneId } from "../../store/uiStore";
import { openDialog } from "../../lib/dialog";
import {
  codexAuthStatus,
  codexLoginCancel,
  codexLoginStart,
  codexLogout,
  secretSave,
  secretLoad,
  secretDelete,
  type CodexAuthStatus,
} from "../../lib/api";
import type { SecretStatus } from "../../lib/types";
import { AccountPane } from "./AccountPane";
import { StoragePane } from "./StoragePane";
import { UpdateSettingsControl } from "./UpdateDialog";

const settingsPanelStyle: CSSProperties = {
  width: "100%",
  height: "100%",
  maxWidth: 960,
  maxHeight: 620,
  background: "var(--bg-base)",
  borderRadius: "var(--radius-lg)",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  position: "relative",
};

const settingsSectionStyle: CSSProperties = {
  padding: "0 var(--space-sm)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-lg)",
};

const settingsControlStyle: CSSProperties = {
  background: "var(--home-hover)",
  border: "none",
};

const SETTINGS_PANES: Array<{ id: SettingsPaneId; icon: typeof SettingsIcon; labelKey: string }> = [
  { id: "general", icon: SettingsIcon, labelKey: "settings.section.general" },
  { id: "appearance", icon: Palette, labelKey: "settings.section.appearance" },
  { id: "import", icon: Download, labelKey: "settings.section.import" },
  { id: "ai", icon: Bot, labelKey: "settings.section.ai" },
  { id: "mcp", icon: Plug, labelKey: "settings.section.mcp" },
  { id: "shortcuts", icon: Copy, labelKey: "settings.section.shortcuts" },
  { id: "account", icon: User, labelKey: "settings.section.account" },
  { id: "storage", icon: HardDrive, labelKey: "settings.section.storage" },
  { id: "about", icon: Info, labelKey: "settings.section.about" },
];

const settingsSidebarStyle: CSSProperties = {
  width: 150,
  flex: "0 0 auto",
  minHeight: 0,
  padding: "var(--space-xs)",
  background: "rgba(0, 0, 0, 0.22)",
  overflowY: "auto",
};

const SETTINGS_FOCUSABLE = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[href]",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

export function SettingsView() {
  const t = useT();
  const setSettingsOpen = useEditorUiStore((s) => s.setSettingsOpen);
  const activePane = useEditorUiStore((s) => s.settingsPane);
  const setActivePane = useEditorUiStore((s) => s.setSettingsPane);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;

    dialog.focus({ preventScroll: true });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === "Escape") {
        event.preventDefault();
        setSettingsOpen(false);
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = [...dialog.querySelectorAll<HTMLElement>(SETTINGS_FOCUSABLE)].filter(
        (element) => element.tabIndex >= 0 && element.getAttribute("aria-hidden") !== "true",
      );
      if (focusable.length === 0) {
        event.preventDefault();
        dialog.focus({ preventScroll: true });
        return;
      }

      const first = focusable[0]!;
      const last = focusable[focusable.length - 1]!;
      const active = document.activeElement;
      if (event.shiftKey && (active === dialog || active === first || !dialog.contains(active))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (active === last || !dialog.contains(active))) {
        event.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      if (previousFocus?.isConnected) previousFocus.focus({ preventScroll: true });
    };
  }, [setSettingsOpen]);

  return (
    <div
      role="presentation"
      className="app-dialog-backdrop"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: "rgba(0, 0, 0, 0.65)",
        backdropFilter: "blur(12px)",
        WebkitBackdropFilter: "blur(12px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: "var(--space-xl)",
        zIndex: 1000,
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-dialog-title"
        tabIndex={-1}
        className="app-dialog-surface"
        style={settingsPanelStyle}
      >
        <header
          data-tauri-drag-region
          style={{
            height: 48,
            flex: "0 0 auto",
            display: "flex",
            alignItems: "center",
            padding: "0 var(--space-lg)",
            background: "var(--bg-base)",
          }}
        >
          <span
            id="settings-dialog-title"
            data-tauri-drag-region
            style={{ fontSize: "var(--fs-md-lg)", fontWeight: "var(--fw-semibold)", flex: 1 }}
          >
            {t("settings.title")}
          </span>
          <div style={{ display: "flex", alignItems: "center", gap: "var(--space-md)" }}>
            <button
              type="button"
              onClick={() => setSettingsOpen(false)}
              className="hover-area"
              style={{
                height: 28,
                padding: "0 var(--space-lg)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text-primary)",
                background: "var(--home-hover)",
                fontSize: "var(--fs-sm-md)",
                fontWeight: "var(--fw-medium)",
              }}
            >
              {t("settings.done")}
            </button>
            <button
              type="button"
              title="Close"
              aria-label="Close"
              onClick={() => setSettingsOpen(false)}
              className="hover-area"
              style={{
                width: 28,
                height: 28,
                borderRadius: "var(--radius-sm)",
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--text-secondary)",
              }}
            >
              <Icon icon={X} size={15} />
            </button>
          </div>
        </header>

        <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
          <SettingsSidebar activePane={activePane} onSelect={setActivePane} />
          <div
            style={{
              flex: 1,
              minWidth: 0,
              overflowY: "auto",
              padding: "var(--space-lg) var(--space-xl) var(--space-xl)",
            }}
          >
            {renderActivePane(activePane)}
          </div>
        </div>
      </div>
    </div>
  );
}

function SettingsSidebar({
  activePane,
  onSelect,
}: {
  activePane: SettingsPaneId;
  onSelect: (pane: SettingsPaneId) => void;
}) {
  const t = useT();

  return (
    <nav style={settingsSidebarStyle} aria-label={t("settings.title")}>
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xxs)" }}>
        {SETTINGS_PANES.map((pane) => {
          const selected = pane.id === activePane;
          return (
            <button
              key={pane.id}
              type="button"
              onClick={() => onSelect(pane.id)}
              className="hover-area"
              style={{
                width: "100%",
                height: 28,
                display: "flex",
                alignItems: "center",
                gap: "var(--space-sm)",
                padding: "0 var(--space-sm)",
                borderRadius: "var(--radius-sm)",
                background: selected ? "var(--home-selected)" : "transparent",
                color: selected ? "var(--text-primary)" : "var(--text-secondary)",
                fontSize: "var(--fs-sm)",
                fontWeight: selected ? "var(--fw-semibold)" : "var(--fw-medium)",
                textAlign: "left",
              }}
            >
              <Icon icon={pane.icon} size={13} />
              <span>{t(pane.labelKey)}</span>
            </button>
          );
        })}
      </div>
    </nav>
  );
}

function renderActivePane(activePane: SettingsPaneId) {
  switch (activePane) {
    case "general":
      return <GeneralPane />;
    case "appearance":
      return <AppearancePane />;
    case "import":
      return <ImportPane />;
    case "ai":
      return <AiPane />;
    case "mcp":
      return <McpPane />;
    case "shortcuts":
      return <ShortcutsPane />;
    case "account":
      return <AccountPane />;
    case "storage":
      return <StoragePane />;
    case "about":
      return <AboutPane />;
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h2
        style={{
          margin: "0 0 var(--space-md)",
          fontSize: "var(--fs-xxs)",
          fontWeight: "var(--fw-semibold)",
          letterSpacing: "var(--tracking-wide)",
          textTransform: "uppercase",
          color: "var(--text-muted)",
        }}
      >
        {title}
      </h2>
      <div
        style={settingsSectionStyle}
      >
        {children}
      </div>
    </section>
  );
}

function Field({
  label,
  description,
  control,
}: {
  label: string;
  description?: string;
  control: React.ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-lg)",
        justifyContent: "space-between",
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>{label}</div>
        {description && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", marginTop: 2 }}>
            {description}
          </div>
        )}
      </div>
      <div style={{ flex: "0 0 auto" }}>{control}</div>
    </div>
  );
}

function GeneralPane() {
  const t = useT();
  const locale = useI18nStore((s) => s.locale);
  const setLocale = useI18nStore((s) => s.setLocale);
  const proxyPlaybackEnabled = useSettingsStore((s) => s.proxyPlaybackEnabled);
  const setProxyPlaybackEnabled = useSettingsStore((s) => s.setProxyPlaybackEnabled);
  return (
    <Section title={t("settings.section.general")}>
      <Field
        label={t("settings.language")}
        description={t("settings.languageDesc")}
        control={
          <Dropdown
            value={locale}
            options={LOCALES}
            onChange={setLocale}
            ariaLabel={t("settings.language")}
          />
        }
      />
      <Field
        label={t("settings.proxyPlayback")}
        description={t("settings.proxyPlaybackDesc")}
        control={
          <label
            style={{
              width: 24,
              height: 24,
              display: "inline-grid",
              placeItems: "center",
              cursor: "pointer",
            }}
          >
            <input
              type="checkbox"
              role="switch"
              aria-label={t("settings.proxyPlayback")}
              checked={proxyPlaybackEnabled}
              onChange={(event) => setProxyPlaybackEnabled(event.currentTarget.checked)}
              style={{ width: 16, height: 16, margin: 0 }}
            />
          </label>
        }
      />
    </Section>
  );
}

function AppearancePane() {
  const t = useT();
  const windowSize = useSettingsStore((s) => s.windowSize);
  const setWindowSize = useSettingsStore((s) => s.setWindowSize);
  const options: Array<{ id: WindowSizeOpt; label: string }> = [
    { id: "standard", label: t("settings.darkLayout.standard") },
    { id: "compact", label: t("settings.darkLayout.compact") },
  ];

  const select = (size: WindowSizeOpt) => {
    void setWindowSize(size);
  };

  return (
    <Section title={t("settings.section.appearance")}>
      <Field
        label={t("settings.darkLayout")}
        description={t("settings.windowSizeDesc")}
        control={
          <div
            role="radiogroup"
            aria-label={t("settings.windowSize")}
            style={{
              display: "flex",
              width: 244,
              padding: 2,
              gap: 2,
              background: "var(--home-hover)",
              borderRadius: "var(--radius-sm)",
            }}
          >
            {options.map((option, index) => {
              const active = option.id === windowSize;
              return (
                <button
                  key={option.id}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  tabIndex={active ? 0 : -1}
                  onClick={() => select(option.id)}
                  onKeyDown={(event) => {
                    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight" && event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
                    event.preventDefault();
                    const direction = event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1;
                    const nextIndex = (index + direction + options.length) % options.length;
                    const next = options[nextIndex]!;
                    select(next.id);
                    (event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="radio"]')[nextIndex])?.focus();
                  }}
                  style={{
                    display: "flex",
                    flex: "1 1 0",
                    alignItems: "center",
                    justifyContent: "center",
                    height: 28,
                    padding: "0 var(--space-sm)",
                    border: active ? "var(--bw-thin) solid var(--border-primary)" : "var(--bw-thin) solid transparent",
                    borderRadius: "var(--radius-xs-sm)",
                    background: active ? "var(--home-selected)" : "transparent",
                    color: active ? "var(--text-primary)" : "var(--text-tertiary)",
                    fontSize: "var(--fs-sm)",
                    fontWeight: "var(--fw-medium)",
                  }}
                >
                  <span style={{ display: "flex", width: "100%", justifyContent: "center" }}>{option.label}</span>
                </button>
              );
            })}
          </div>
        }
      />
    </Section>
  );
}

function ImportPane() {
  const t = useT();
  const folder = useSettingsStore((s) => s.defaultImportFolder);
  const setFolder = useSettingsStore((s) => s.setDefaultImportFolder);

  const choose = async () => {
    const open = await openDialog();
    if (!open) return;
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") setFolder(selected);
  };

  return (
    <Section title={t("settings.section.import")}>
      <Field
        label={t("settings.defaultImportFolder")}
        description={folder ?? t("settings.notSet")}
        control={
          <div style={{ display: "inline-flex", gap: "var(--space-xs)" }}>
            <button
              type="button"
              onClick={() => void choose()}
              className="hover-area"
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 4,
                height: 26,
                padding: "0 var(--space-md)",
                borderRadius: "var(--radius-sm)",
                ...settingsControlStyle,
                color: "var(--text-secondary)",
                fontSize: "var(--fs-sm)",
                fontWeight: "var(--fw-medium)",
              }}
            >
              <Icon icon={FolderOpen} size={13} />
              {t("settings.chooseFolder")}
            </button>
            {folder && (
              <button
                type="button"
                onClick={() => setFolder(null)}
                className="hover-area"
                style={{
                  height: 26,
                  padding: "0 var(--space-md)",
                  borderRadius: "var(--radius-sm)",
                  color: "var(--text-tertiary)",
                  fontSize: "var(--fs-sm)",
                }}
              >
                {t("settings.clear")}
              </button>
            )}
          </div>
        }
      />
    </Section>
  );
}

const PROVIDERS: Array<{ id: ByokProvider; label: string }> = [
  { id: "codex", label: "Codex / ChatGPT" },
  { id: "anthropic", label: "Anthropic" },
  { id: "fal", label: "fal.ai" },
  { id: "replicate", label: "Replicate" },
  { id: "openai", label: "OpenAI" },
  { id: "elevenlabs", label: "ElevenLabs" },
  { id: "google", label: "Google" },
];

/** Narrow a rejected-promise reason (a `String` from the Tauri boundary, or an
 *  `Error`) to a displayable message. */
function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function AiPane() {
  const t = useT();
  const provider = useSettingsStore((s) => s.byokProvider);
  const setProvider = useSettingsStore((s) => s.setByokProvider);
  const [draft, setDraft] = useState("");
  const [status, setStatus] = useState<SecretStatus>({ hasKey: false, masked: "" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [codexStatus, setCodexStatus] = useState<CodexAuthStatus | null>(null);
  const codexRequestGeneration = useRef(0);
  const codexActionInFlight = useRef(false);
  const isCodex = provider === "codex";

  // Reflect the keychain status for the active provider; reload on switch. The
  // plaintext key is never fetched — only `hasKey` and the masked form.
  useEffect(() => {
    let alive = true;
    const requestGeneration = ++codexRequestGeneration.current;
    codexActionInFlight.current = false;
    setBusy(false);
    setDraft("");
    setError(null);
    if (provider === "codex") {
      setCodexStatus(null);
      void codexAuthStatus().then(
        (next) => {
          if (alive && requestGeneration === codexRequestGeneration.current) {
            setCodexStatus(next);
          }
        },
        (reason) => {
          if (alive && requestGeneration === codexRequestGeneration.current) {
            setError(t("settings.codexActionFailed", { error: errorMessage(reason) }));
          }
        },
      );
      return () => {
        alive = false;
      };
    }
    setCodexStatus(null);
    void secretLoad(provider).then(
      (s) => {
        if (alive) setStatus(s);
      },
      () => {
        if (alive) setStatus({ hasKey: false, masked: "" });
      },
    );
    return () => {
      alive = false;
    };
  }, [provider, t]);

  useEffect(() => {
    if (!isCodex || !codexStatus?.loginInProgress) return;
    let alive = true;
    const timer = window.setInterval(() => {
      if (codexActionInFlight.current) return;
      const requestGeneration = ++codexRequestGeneration.current;
      void codexAuthStatus().then(
        (next) => {
          if (alive && requestGeneration === codexRequestGeneration.current) {
            setCodexStatus(next);
          }
        },
        () => undefined,
      );
    }, 1200);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [codexStatus?.loginInProgress, isCodex]);

  const trimmed = draft.trim();

  const save = async () => {
    if (trimmed.length === 0 || busy) return;
    setBusy(true);
    setError(null);
    try {
      setStatus(await secretSave(provider, trimmed));
      setDraft("");
    } catch (e) {
      setError(t("settings.byokSaveFailed", { error: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      setStatus(await secretDelete(provider));
      setDraft("");
    } catch (e) {
      setError(t("settings.byokSaveFailed", { error: errorMessage(e) }));
    } finally {
      setBusy(false);
    }
  };

  const runCodexAction = async (
    action: () => Promise<CodexAuthStatus>,
  ) => {
    if (busy) return;
    const requestGeneration = ++codexRequestGeneration.current;
    codexActionInFlight.current = true;
    setBusy(true);
    setError(null);
    try {
      const next = await action();
      if (requestGeneration === codexRequestGeneration.current) {
        setCodexStatus(next);
      }
    } catch (reason) {
      if (requestGeneration === codexRequestGeneration.current) {
        setError(t("settings.codexActionFailed", { error: errorMessage(reason) }));
      }
    } finally {
      if (requestGeneration === codexRequestGeneration.current) {
        codexActionInFlight.current = false;
        setBusy(false);
      }
    }
  };

  return (
    <Section title={t("settings.section.ai")}>
      <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
        {isCodex ? t("settings.codexDesc") : t("settings.byokDesc")}
      </div>
      <Field
        label={t("settings.byokProvider")}
        control={
          <Dropdown<ByokProvider>
            value={provider}
            options={PROVIDERS}
            onChange={setProvider}
            ariaLabel={t("settings.byokProvider")}
            minWidth={190}
          />
        }
      />
      {isCodex ? (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-sm)" }}>
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              minHeight: 34,
              padding: "0 var(--space-sm)",
              borderRadius: "var(--radius-sm)",
              ...settingsControlStyle,
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-sm)" }}>
              <span
                aria-hidden="true"
                style={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  background: codexStatus?.authenticated
                    ? "var(--status-success)"
                    : codexStatus?.loginInProgress
                      ? "var(--accent-primary)"
                      : "var(--text-tertiary)",
                }}
              />
              <span style={{ fontSize: "var(--fs-sm)", color: "var(--text-primary)" }}>
                {!codexStatus
                  ? t("settings.codexChecking")
                  : !codexStatus.available
                    ? t("settings.codexUnavailable")
                    : codexStatus.loginInProgress
                      ? t("settings.codexWaiting")
                      : codexStatus.authenticated
                        ? t("settings.codexSignedIn", {
                            method: codexStatus.authMethod ?? "ChatGPT",
                          })
                        : t("settings.codexSignedOut")}
              </span>
            </div>
            {codexStatus?.version && (
              <span className="tabular" style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
                {codexStatus.version}
              </span>
            )}
          </div>
          <div style={{ display: "flex", gap: "var(--space-xs)" }}>
            {codexStatus?.loginInProgress ? (
              <button
                type="button"
                disabled={busy}
                onClick={() => void runCodexAction(codexLoginCancel)}
                className="hover-area"
                style={{ height: 28, padding: "0 var(--space-lg)", borderRadius: "var(--radius-sm)", ...settingsControlStyle, color: "var(--text-primary)", fontSize: "var(--fs-sm)" }}
              >
                {t("settings.codexCancel")}
              </button>
            ) : codexStatus?.authenticated ? (
              <button
                type="button"
                disabled={busy}
                onClick={() => void runCodexAction(codexLogout)}
                className="hover-area"
                style={{ height: 28, padding: "0 var(--space-lg)", borderRadius: "var(--radius-sm)", ...settingsControlStyle, color: "var(--text-primary)", fontSize: "var(--fs-sm)" }}
              >
                {t("settings.codexLogout")}
              </button>
            ) : (
              <button
                type="button"
                disabled={busy || codexStatus === null || codexStatus.available === false}
                onClick={() => void runCodexAction(codexLoginStart)}
                className="hover-area"
                style={{ display: "inline-flex", alignItems: "center", gap: "var(--space-xs)", height: 28, padding: "0 var(--space-lg)", borderRadius: "var(--radius-sm)", ...settingsControlStyle, color: "var(--text-primary)", fontSize: "var(--fs-sm)", opacity: busy || codexStatus === null || codexStatus.available === false ? 0.4 : 1 }}
              >
                <Icon icon={ExternalLink} size={13} />
                {t("settings.codexLogin")}
              </button>
            )}
          </div>
          {error && (
            <div style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>{error}</div>
          )}
        </div>
      ) : (
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <label style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>
          {t("settings.byokKey")}
        </label>
        <div style={{ display: "flex", gap: "var(--space-xs)" }}>
          <input
            type="password"
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value);
              setError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
            }}
            placeholder={status.hasKey ? status.masked : t("settings.byokKeyPlaceholder")}
            className="tabular"
            style={{
              flex: 1,
              height: 28,
              ...settingsControlStyle,
              borderRadius: "var(--radius-sm)",
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              padding: "0 var(--space-sm)",
            }}
          />
          {trimmed.length > 0 ? (
            <button
              type="button"
              disabled={busy}
              onClick={() => void save()}
              className="hover-area"
              style={{
                height: 28,
                padding: "0 var(--space-lg)",
                borderRadius: "var(--radius-sm)",
                ...settingsControlStyle,
                color: "var(--text-primary)",
                fontSize: "var(--fs-sm)",
                fontWeight: "var(--fw-medium)",
                opacity: busy ? 0.4 : 1,
              }}
            >
              {t("settings.byokSave")}
            </button>
          ) : (
            status.hasKey && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void remove()}
                className="hover-area"
                title={t("settings.byokDelete")}
                aria-label={t("settings.byokDelete")}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  width: 28,
                  height: 28,
                  borderRadius: "var(--radius-sm)",
                  ...settingsControlStyle,
                  color: "var(--text-secondary)",
                  opacity: busy ? 0.4 : 1,
                }}
              >
                <Icon icon={Trash2} size={14} />
              </button>
            )
          )}
        </div>
        {error ? (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>{error}</div>
        ) : (
          status.hasKey &&
          trimmed.length === 0 && (
            <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
              {t("settings.byokSaved")}
            </div>
          )
        )}
      </div>
      )}
    </Section>
  );
}

function AboutPane() {
  const t = useT();
  return (
    <Section title={t("settings.section.about")}>
      <Field label={t("settings.aboutVersion")} control={<Value>{__APP_VERSION__}</Value>} />
      <Field label={t("settings.aboutLicense")} control={<Value>GPL-3.0</Value>} />
      <UpdateSettingsControl />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
        {t("settings.aboutDesc")}
      </div>
    </Section>
  );
}

function ShortcutsPane() {
  const t = useT();
  const rows = [
    [t("settings.shortcutsPlay"), "Space"],
    [t("settings.shortcutsUndo"), "⌘Z"],
    [t("settings.shortcutsRedo"), "⇧⌘Z"],
    [t("settings.shortcutsDelete"), "⌫"],
    [t("settings.shortcutsSave"), "⌘S"],
    [t("settings.shortcutsNew"), "⌘N"],
    [t("view.mediaPanel"), "⌘0"],
    [t("view.inspector"), "⌘⌥0"],
    [t("view.agentPanel"), "⌘⌥A"],
    [t("view.maximizeFocused"), "`"],
    [t("view.layoutDefault"), "⌘1"],
    [t("view.layoutMedia"), "⌘2"],
    [t("view.layoutVertical"), "⌘3"],
    [t("view.enterFullScreen"), "⌘F"],
  ];
  return (
    <Section title={t("settings.section.shortcuts")}>
      {rows.map(([label, shortcut]) => (
        <Field key={label} label={label} control={<Value>{shortcut}</Value>} />
      ))}
    </Section>
  );
}

function Value({ children }: { children: React.ReactNode }) {
  return (
    <span className="tabular" style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-secondary)" }}>
      {children}
    </span>
  );
}

/**
 * External MCP is fail-closed in this Beta until the app has an explicit,
 * authenticated pairing flow. Official Codex sign-in uses a separate
 * per-turn authenticated endpoint and remains available in the AI pane.
 */
function McpPane() {
  const t = useT();

  return (
    <Section title={t("settings.section.mcp")}>
      <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)", fontWeight: "var(--fw-medium)" }}>
        {t("mcp.title")}
      </div>
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", marginTop: -12 }}>
        {t("mcp.overview")}
      </div>
      <div
        role="status"
        aria-live="polite"
        data-external-mcp-status="paused"
        style={{
          display: "flex",
          alignItems: "flex-start",
          gap: "var(--space-md)",
          ...settingsControlStyle,
          borderRadius: "var(--radius-sm)",
          padding: "var(--space-md)",
        }}
      >
        <Icon icon={Plug} size={16} />
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
          <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-primary)", fontWeight: "var(--fw-medium)" }}>
            {t("mcp.pausedTitle")}
          </div>
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
            {t("mcp.pausedDesc")}
          </div>
        </div>
      </div>
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{t("mcp.note")}</div>
    </Section>
  );
}
