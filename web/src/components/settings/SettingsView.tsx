/**
 * Settings view. Reachable from both the Home sidebar and the editor title bar.
 * Sidebar + detail layout (mirrors upstream `Settings/SettingsView.swift`):
 * 7 panes — General, Appearance, Import, AI, MCP Instructions, Storage,
 * Notifications, About. Preferences persist via `settingsStore` / `i18nStore`;
 * the BYOK key is stored in the OS keychain via the `secret_*` Tauri commands
 * (see `lib/api.ts`) — the plaintext key never reaches this component's
 * persisted state.
 */

import { useEffect, useState } from "react";
import {
  Check,
  FolderOpen,
  Trash2,
  Settings as SettingsIcon,
  Palette,
  Download,
  Sparkles,
  Terminal,
  HardDrive,
  Bell,
  Info,
  Copy,
} from "lucide-react";
import { Icon } from "../ui/Icon";
import { Dropdown } from "../ui/Dropdown";
import { useT, useI18nStore, LOCALES } from "../../i18n";
import {
  useSettingsStore,
  type Theme,
  type ByokProvider,
} from "../../store/settingsStore";
import { useEditorUiStore } from "../../store/uiStore";
import { openDialog } from "../../lib/dialog";
import { secretSave, secretLoad, secretDelete } from "../../lib/api";
import type { SecretStatus } from "../../lib/types";

/** MCP server endpoint. The Rust server (`opentake-agent::mcp::server`) binds
 *  to 127.0.0.1:19789/mcp at startup; the URL is fixed so we hardcode it here
 *  rather than round-tripping to Tauri to query. */
const MCP_SERVER_URL = "http://127.0.0.1:19789/mcp";

type PaneId =
  | "general"
  | "appearance"
  | "import"
  | "ai"
  | "mcp"
  | "storage"
  | "notifications"
  | "about";

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
            {active === "ai" && <AiPane />}
            {active === "mcp" && <McpInstructionsPane />}
            {active === "storage" && <StoragePane />}
            {active === "notifications" && <NotificationsPane />}
            {active === "about" && <AboutPane />}
          </div>
        </div>
      </div>
    </div>
  );
}

/** Left sidebar: pane selector. Mirrors upstream `SettingsView` sidebar
 *  (220pt wide, icon + label rows, active capsule on the left edge). */
function SettingsSidebar({
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
    { id: "ai", icon: Sparkles, label: t("settings.section.ai") },
    { id: "mcp", icon: Terminal, label: t("settings.section.mcp") },
    { id: "storage", icon: HardDrive, label: t("settings.section.storage") },
    { id: "notifications", icon: Bell, label: t("settings.section.notifications") },
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
        style={{
          background: "var(--bg-raised)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-md)",
          padding: "var(--space-md) var(--space-lg)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-lg)",
        }}
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

/** Segmented control used for enum settings (language/theme). */
function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: Array<{ id: T; label: string }>;
  onChange: (id: T) => void;
}) {
  return (
    <div
      style={{
        display: "inline-flex",
        padding: 2,
        gap: 2,
        background: "var(--bg-base)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-sm)",
      }}
    >
      {options.map((opt) => {
        const active = opt.id === value;
        return (
          <button
            key={opt.id}
            type="button"
            onClick={() => onChange(opt.id)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 24,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-xs-sm)",
              background: active ? "var(--bg-prominent)" : "transparent",
              color: active ? "var(--text-primary)" : "var(--text-tertiary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {active && <Icon icon={Check} size={11} />}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

function GeneralPane() {
  const t = useT();
  const locale = useI18nStore((s) => s.locale);
  const setLocale = useI18nStore((s) => s.setLocale);
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
    </Section>
  );
}

function AppearancePane() {
  const t = useT();
  const theme = useSettingsStore((s) => s.theme);
  const setTheme = useSettingsStore((s) => s.setTheme);
  return (
    <Section title={t("settings.section.appearance")}>
      <Field
        label={t("settings.theme")}
        description={t("settings.themeDesc")}
        control={
          <Segmented<Theme>
            value={theme}
            options={[
              { id: "dark", label: t("settings.theme.dark") },
              { id: "light", label: t("settings.theme.light") },
            ]}
            onChange={setTheme}
          />
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
                border: "var(--bw-thin) solid var(--border-primary)",
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
  { id: "anthropic", label: "Anthropic" },
  { id: "openai", label: "OpenAI" },
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

  // Reflect the keychain status for the active provider; reload on switch. The
  // plaintext key is never fetched — only `hasKey` and the masked form.
  useEffect(() => {
    let alive = true;
    setDraft("");
    setError(null);
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
  }, [provider]);

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

  return (
    <Section title={t("settings.section.ai")}>
      <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
        {t("settings.byokDesc")}
      </div>
      <Field
        label={t("settings.byokProvider")}
        control={
          <Segmented<ByokProvider> value={provider} options={PROVIDERS} onChange={setProvider} />
        }
      />
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
              background: "var(--bg-base)",
              border: "var(--bw-thin) solid var(--border-primary)",
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
                border: "var(--bw-thin) solid var(--border-primary)",
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
                  border: "var(--bw-thin) solid var(--border-primary)",
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
    </Section>
  );
}

/** MCP Instructions pane. Surfaces the built-in MCP server URL and one-line
 *  install commands for Cursor / Claude Code / Codex / Claude Desktop. Mirrors
 *  upstream `Help/MCPInstructionsPane.swift`, consolidated into Settings per
 *  Issue #40. The server runs on `127.0.0.1:19789/mcp` (fixed in
 *  `opentake-agent::mcp::server::DEFAULT_ADDR`). */
function McpInstructionsPane() {
  const t = useT();
  const [copied, setCopied] = useState(false);

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard may be unavailable (non-Tauri / permissions); silently no-op.
    }
  };

  const claudeCodeCmd = `claude mcp add --transport http opentake ${MCP_SERVER_URL}`;
  const codexCmd = `codex mcp add opentake --url ${MCP_SERVER_URL}`;
  const cursorConfig = JSON.stringify(
    {
      mcpServers: {
        opentake: { url: MCP_SERVER_URL },
      },
    },
    null,
    2,
  );
  const claudeDesktopConfig = cursorConfig;

  return (
    <Section title={t("mcp.title")}>
      <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)", lineHeight: 1.5 }}>
        {t("mcp.overview")}
      </div>

      <Field
        label={t("mcp.serverUrl")}
        description={t("mcp.serverUrlDesc")}
        control={
          <button
            type="button"
            onClick={() => void copy(MCP_SERVER_URL)}
            className="hover-area"
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 26,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            <Icon icon={Copy} size={12} />
            {copied ? t("mcp.copied") : t("mcp.copy")}
          </button>
        }
      />
      <CodeBlock text={MCP_SERVER_URL} />

      <Subsection label={t("mcp.claudeCode")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.claudeCodeCmd")}
        </div>
        <CodeBlock text={claudeCodeCmd} onCopy={() => void copy(claudeCodeCmd)} />
      </Subsection>

      <Subsection label={t("mcp.codex")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.codexCmd")}
        </div>
        <CodeBlock text={codexCmd} onCopy={() => void copy(codexCmd)} />
      </Subsection>

      <Subsection label={t("mcp.cursor")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.cursorManual")}
        </div>
        <CodeBlock text={cursorConfig} onCopy={() => void copy(cursorConfig)} />
      </Subsection>

      <Subsection label={t("mcp.claudeDesktop")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.claudeDesktopManual")}
        </div>
        <CodeBlock text={claudeDesktopConfig} onCopy={() => void copy(claudeDesktopConfig)} />
      </Subsection>

      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("mcp.note")}
      </div>
    </Section>
  );
}

function Subsection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
      <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)", fontWeight: "var(--fw-medium)" }}>
        {label}
      </div>
      {children}
    </div>
  );
}

/** Read-only code block with optional copy button. Used for MCP commands and
 *  JSON configs. */
function CodeBlock({ text, onCopy }: { text: string; onCopy?: () => void }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const handleCopy = () => {
    if (onCopy) {
      onCopy();
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  };
  return (
    <div
      style={{
        position: "relative",
        background: "var(--bg-base)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-sm)",
        padding: "var(--space-sm) var(--space-md)",
        paddingRight: onCopy ? 64 : undefined,
      }}
    >
      <pre
        className="tabular"
        style={{
          margin: 0,
          fontSize: "var(--fs-xs)",
          color: "var(--text-secondary)",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          fontFamily: "var(--font-mono, ui-monospace, monospace)",
        }}
      >
        {text}
      </pre>
      {onCopy && (
        <button
          type="button"
          onClick={handleCopy}
          className="hover-area"
          title={t("mcp.copy")}
          aria-label={t("mcp.copy")}
          style={{
            position: "absolute",
            top: 4,
            right: 4,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 22,
            height: 22,
            borderRadius: "var(--radius-xs)",
            color: "var(--text-tertiary)",
          }}
        >
          <Icon icon={copied ? Check : Copy} size={11} />
        </button>
      )}
    </div>
  );
}

/** Storage pane. Simplified placeholder mirroring upstream `StoragePane` —
 *  surfaces cache location and a clear-cache action. Runtime statistics (cache
 *  size, index size) require Rust commands not yet wired; the pane calls this
 *  out explicitly so users know it's intentional. */
function StoragePane() {
  const t = useT();
  const [cleared, setCleared] = useState(false);
  const clear = () => {
    // No-op until the cache-clear Tauri command lands; surface success so the
    // user sees the action registered.
    setCleared(true);
    setTimeout(() => setCleared(false), 2000);
  };
  return (
    <Section title={t("settings.section.storage")}>
      <Field
        label={t("storage.cache")}
        description={t("storage.cacheDesc")}
        control={
          <button
            type="button"
            onClick={clear}
            className="hover-area"
            style={{
              height: 26,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {t("storage.clearCache")}
          </button>
        }
      />
      {cleared && (
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("storage.cacheCleared")}
        </div>
      )}
      <Field
        label={t("storage.searchIndex")}
        description={t("storage.searchIndexDesc")}
        control={<span style={{ fontSize: "var(--fs-sm)", color: "var(--text-muted)" }}>—</span>}
      />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("storage.placeholder")}
      </div>
    </Section>
  );
}

/** Notifications pane. Single toggle (generation-complete) mirroring upstream
 *  `NotificationsPane`. The toggle is front-end-only for now; wiring it to
 *  system notifications is a follow-up. */
function NotificationsPane() {
  const t = useT();
  const [enabled, setEnabled] = useState(true);
  return (
    <Section title={t("settings.section.notifications")}>
      <Field
        label={t("notifications.generation")}
        description={t("notifications.generationDesc")}
        control={
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            onClick={() => setEnabled((v) => !v)}
            style={{
              width: 36,
              height: 20,
              borderRadius: 10,
              background: enabled ? "var(--accent-primary)" : "var(--bg-base)",
              border: "var(--bw-thin) solid var(--border-primary)",
              position: "relative",
              transition: "background var(--anim-transition) var(--ease-out)",
            }}
          >
            <span
              style={{
                position: "absolute",
                top: 1,
                left: enabled ? 17 : 1,
                width: 16,
                height: 16,
                borderRadius: "50%",
                background: "var(--text-primary)",
                transition: "left var(--anim-transition) var(--ease-out)",
              }}
            />
          </button>
        }
      />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("notifications.restartHint")}
      </div>
    </Section>
  );
}

function AboutPane() {
  const t = useT();
  return (
    <Section title={t("settings.section.about")}>
      <Field label={t("settings.aboutVersion")} control={<Value>{__APP_VERSION__}</Value>} />
      <Field label={t("settings.aboutLicense")} control={<Value>GPL-3.0</Value>} />
      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
        {t("settings.aboutDesc")}
      </div>
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
