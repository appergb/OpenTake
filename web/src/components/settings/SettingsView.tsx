/**
 * Settings view. Reachable from both the Home sidebar and the editor title bar.
 * Panes (single scrollable page in this phase): General (language), Appearance
 * (theme), Import (default folder), AI (BYOK key), and About (version / license).
 * Preferences persist via `settingsStore` / `i18nStore`;
 * the BYOK key is stored in the OS keychain via the `secret_*` Tauri commands
 * (see `lib/api.ts`) — the plaintext key never reaches this component's
 * persisted state.
 */

import { useEffect, useState } from "react";
import { Check, FolderOpen, Trash2, X } from "lucide-react";
import { Icon } from "../ui/Icon";
import { Dropdown } from "../ui/Dropdown";
import { useT, useI18nStore, LOCALES } from "../../i18n";
import {
  useSettingsStore,
  type Theme,
  type ByokProvider,
  type WindowSizeOpt,
} from "../../store/settingsStore";
import { useEditorUiStore } from "../../store/uiStore";
import { openDialog } from "../../lib/dialog";
import { secretSave, secretLoad, secretDelete } from "../../lib/api";
import type { SecretStatus } from "../../lib/types";

export function SettingsView() {
  const t = useT();
  const setSettingsOpen = useEditorUiStore((s) => s.setSettingsOpen);

  return (
    <div
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
        zIndex: 1000,
      }}
    >
      <style>{`
        @keyframes scaleIn {
          from { opacity: 0; transform: scale(0.96); }
          to { opacity: 1; transform: scale(1); }
        }
      `}</style>
      <div
        style={{
          width: 580,
          height: 520,
          background: "var(--bg-raised)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-lg)",
          boxShadow: "var(--shadow-lg)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          position: "relative",
          animation: "scaleIn 0.2s var(--ease-out)",
        }}
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
            borderBottom: "var(--bw-thin) solid var(--border-primary)",
          }}
        >
          <span
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
                background: "var(--bg-prominent)",
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

        <div style={{ flex: 1, overflowY: "auto" }}>
          <div
            style={{
              padding: "var(--space-lg) var(--space-lg) var(--space-xl)",
              display: "flex",
              flexDirection: "column",
              gap: "var(--space-xl)",
            }}
          >
            <GeneralPane />
            <AppearancePane />
            <ImportPane />
            <AiPane />
            <AboutPane />
          </div>
        </div>
      </div>
    </div>
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
  const windowSize = useSettingsStore((s) => s.windowSize);
  const setWindowSize = useSettingsStore((s) => s.setWindowSize);

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
      <Field
        label={t("settings.windowSize")}
        description={t("settings.windowSizeDesc")}
        control={
          <Segmented<WindowSizeOpt>
            value={windowSize}
            options={[
              { id: "standard", label: t("settings.windowSize.standard") },
              { id: "compact", label: t("settings.windowSize.compact") },
            ]}
            onChange={setWindowSize}
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
