/**
 * Account pane (HANDOFF §3.8). OpenTake ships **no official backend**; this
 * pane lets a user point the app at a self-hosted backend, verify a token
 * against it, and store that token in the OS keychain. Nothing here gates
 * local editing — the disclaimer is shown prominently so the user knows not
 * signing in does not affect any editing / export / Agent feature.
 *
 * Style mirrors the other panes in `SettingsView.tsx` (same theme vars +
 * `settingsControlStyle`-equivalent surfaces) so it reads as part of the
 * settings shell. The backend URL + token never live in React state beyond
 * the brief draft the user is editing; the authoritative copy is the keychain.
 */

import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { LogOut, User } from "lucide-react";
import { useT } from "../../i18n";
import {
  accountGetBackendUrl,
  accountGetStatus,
  accountLogin,
  accountLogout,
  accountSetBackendUrl,
} from "../../lib/api";
import type { AccountStatus } from "../../lib/types";
import { Icon } from "../ui/Icon";

const controlStyle: CSSProperties = {
  background: "var(--home-hover)",
  border: "none",
};

const sectionStyle: CSSProperties = {
  padding: "0 var(--space-sm)",
  display: "flex",
  flexDirection: "column",
  gap: "var(--space-lg)",
};

function Section({ title, children }: { title: string; children: ReactNode }) {
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
      <div style={sectionStyle}>{children}</div>
    </section>
  );
}

/** Narrow a rejected-promise reason (a `String` from the Tauri boundary, or an
 *  `Error`) to a displayable message — same helper shape as `AiPane`. */
function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function AccountPane() {
  const t = useT();
  const [backendUrl, setBackendUrl] = useState<string | null>(null);
  const [urlDraft, setUrlDraft] = useState("");
  const [status, setStatus] = useState<AccountStatus>({ type: "offline" });
  const [tokenDraft, setTokenDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [urlMsg, setUrlMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Load the configured backend URL + live status on mount. The plaintext
  // token is never fetched — only `hasKey`-equivalent status.
  useEffect(() => {
    let alive = true;
    void accountGetBackendUrl().then((u) => {
      if (alive) {
        setBackendUrl(u);
        setUrlDraft(u ?? "");
      }
    });
    void accountGetStatus().then(
      (s) => {
        if (alive) setStatus(s);
      },
      () => {
        /* outside Tauri: stay offline */
      },
    );
    return () => {
      alive = false;
    };
  }, []);

  // Re-fetch status after a login/logout so the badge updates without a pane
  // re-mount. Backend URL changes don't change status (offline stays offline).
  const refreshStatus = async () => {
    try {
      setStatus(await accountGetStatus());
    } catch {
      /* outside Tauri */
    }
  };

  const saveUrl = async () => {
    if (busy) return;
    setBusy(true);
    setUrlMsg(null);
    setError(null);
    try {
      const trimmed = urlDraft.trim();
      await accountSetBackendUrl(trimmed.length === 0 ? null : trimmed);
      setBackendUrl(trimmed.length === 0 ? null : trimmed);
      setUrlMsg(t("account.backendUrlSaved"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const clearUrl = async () => {
    if (busy) return;
    setBusy(true);
    setUrlMsg(null);
    setError(null);
    try {
      await accountSetBackendUrl(null);
      setBackendUrl(null);
      setUrlDraft("");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const login = async () => {
    const trimmed = tokenDraft.trim();
    if (trimmed.length === 0 || busy) return;
    setBusy(true);
    setError(null);
    // Optimistically flip the badge so the user sees the request is in flight
    // before the round-trip resolves.
    setStatus({ type: "connecting" });
    try {
      await accountLogin(trimmed);
      setTokenDraft("");
      await refreshStatus();
    } catch (e) {
      setError(t("account.loginFailed", { error: errorMessage(e) }));
      await refreshStatus();
    } finally {
      setBusy(false);
    }
  };

  const logout = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await accountLogout();
      await refreshStatus();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const statusLabel = (() => {
    switch (status.type) {
      case "offline":
        return t("account.status.offline");
      case "connecting":
        return t("account.status.connecting");
      case "online":
        return t("account.status.online", {
          userId: status.info?.userId ?? "",
        });
      case "error":
        return t("account.status.error", { message: status.message });
    }
  })();

  return (
    <Section title={t("settings.section.account")}>
      {/* Prominent disclaimer: no official backend, local features unaffected. */}
      <div
        style={{
          padding: "var(--space-sm) var(--space-md)",
          borderRadius: "var(--radius-sm)",
          background: "var(--home-hover)",
          borderLeft: "3px solid var(--status-warning, var(--text-muted))",
          fontSize: "var(--fs-sm-md)",
          color: "var(--text-secondary)",
        }}
      >
        {t("account.disclaimer")}
      </div>

      {/* Backend URL field */}
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>
          {t("account.backendUrl")}
        </div>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("account.backendUrlDesc")}
        </div>
        <div style={{ display: "flex", gap: "var(--space-xs)" }}>
          <input
            type="url"
            value={urlDraft}
            onChange={(e) => {
              setUrlDraft(e.target.value);
              setUrlMsg(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void saveUrl();
            }}
            placeholder={t("account.backendUrlPlaceholder")}
            spellCheck={false}
            className="tabular"
            style={{
              flex: 1,
              height: 28,
              ...controlStyle,
              borderRadius: "var(--radius-sm)",
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              padding: "0 var(--space-sm)",
            }}
          />
          <button
            type="button"
            disabled={busy}
            onClick={() => void saveUrl()}
            className="hover-area"
            style={{
              height: 28,
              padding: "0 var(--space-lg)",
              borderRadius: "var(--radius-sm)",
              ...controlStyle,
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
              opacity: busy ? 0.4 : 1,
            }}
          >
            {t("account.saveBackendUrl")}
          </button>
          {backendUrl && (
            <button
              type="button"
              disabled={busy}
              onClick={() => void clearUrl()}
              className="hover-area"
              style={{
                height: 28,
                padding: "0 var(--space-md)",
                borderRadius: "var(--radius-sm)",
                ...controlStyle,
                color: "var(--text-tertiary)",
                fontSize: "var(--fs-sm)",
                opacity: busy ? 0.4 : 1,
              }}
            >
              {t("account.clearBackendUrl")}
            </button>
          )}
        </div>
        {urlMsg && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>{urlMsg}</div>
        )}
      </div>

      {/* Login form */}
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <label style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>
          {t("account.token")}
        </label>
        <div style={{ display: "flex", gap: "var(--space-xs)" }}>
          <input
            type="password"
            value={tokenDraft}
            onChange={(e) => {
              setTokenDraft(e.target.value);
              setError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void login();
            }}
            placeholder={t("account.tokenPlaceholder")}
            className="tabular"
            style={{
              flex: 1,
              height: 28,
              ...controlStyle,
              borderRadius: "var(--radius-sm)",
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              padding: "0 var(--space-sm)",
            }}
          />
          <button
            type="button"
            disabled={busy || tokenDraft.trim().length === 0}
            onClick={() => void login()}
            className="hover-area"
            style={{
              height: 28,
              padding: "0 var(--space-lg)",
              borderRadius: "var(--radius-sm)",
              ...controlStyle,
              color: "var(--text-primary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
              opacity: busy ? 0.4 : 1,
            }}
          >
            {t("account.login")}
          </button>
          {status.type === "online" && (
            <button
              type="button"
              disabled={busy}
              onClick={() => void logout()}
              className="hover-area"
              title={t("account.logout")}
              aria-label={t("account.logout")}
              style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                width: 28,
                height: 28,
                borderRadius: "var(--radius-sm)",
                ...controlStyle,
                color: "var(--text-secondary)",
                opacity: busy ? 0.4 : 1,
              }}
            >
              <Icon icon={LogOut} size={14} />
            </button>
          )}
        </div>
        {error && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>{error}</div>
        )}
      </div>

      {/* Status badge */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-sm)",
          fontSize: "var(--fs-sm-md)",
          color:
            status.type === "online"
              ? "var(--text-primary)"
              : status.type === "error"
                ? "var(--status-error)"
                : "var(--text-tertiary)",
        }}
      >
        <Icon icon={User} size={14} />
        <span>{statusLabel}</span>
      </div>
    </Section>
  );
}
