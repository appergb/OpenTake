/**
 * AI / BYOK settings pane — provider selection + API key management.
 * Extracted from SettingsView.tsx (Issue #40 review).
 */

import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { Icon } from "../../ui/Icon";
import { useT } from "../../../i18n";
import { useSettingsStore, type ByokProvider } from "../../../store/settingsStore";
import { secretSave, secretLoad, secretDelete } from "../../../lib/api";
import type { SecretStatus } from "../../../lib/types";
import { Section, Field, Segmented } from "../shared";

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

export function AiPane() {
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