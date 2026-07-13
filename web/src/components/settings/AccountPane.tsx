/**
 * Optional account login for a user-configured backend. OpenTake provides no
 * official account service, and this pane never gates local product features.
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

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function AccountPane() {
  const t = useT();
  const [backendUrl, setBackendUrl] = useState<string | null>(null);
  const [urlDraft, setUrlDraft] = useState("");
  const [tokenDraft, setTokenDraft] = useState("");
  const [status, setStatus] = useState<AccountStatus>({ type: "offline" });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    void accountGetBackendUrl().then(
      (url) => {
        if (!alive) return;
        setBackendUrl(url);
        setUrlDraft(url ?? "");
      },
      (reason) => {
        if (alive) setError(errorMessage(reason));
      },
    );
    void accountGetStatus().then(
      (nextStatus) => {
        if (alive) setStatus(nextStatus);
      },
      (reason) => {
        if (alive) setError(errorMessage(reason));
      },
    );
    return () => {
      alive = false;
    };
  }, []);

  const refreshStatus = async () => {
    setStatus(await accountGetStatus());
  };

  const saveUrl = async () => {
    if (busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const trimmed = urlDraft.trim();
      const nextUrl = trimmed.length === 0 ? null : trimmed;
      await accountSetBackendUrl(nextUrl);
      setBackendUrl(await accountGetBackendUrl());
      await refreshStatus();
      setMessage(t("account.backendUrlSaved"));
    } catch (reason) {
      setError(t("account.backendUrlSaveFailed", { error: errorMessage(reason) }));
    } finally {
      setBusy(false);
    }
  };

  const clearUrl = async () => {
    if (busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      await accountSetBackendUrl(null);
      setBackendUrl(null);
      setUrlDraft("");
      setTokenDraft("");
      await refreshStatus();
    } catch (reason) {
      setError(t("account.backendUrlSaveFailed", { error: errorMessage(reason) }));
    } finally {
      setBusy(false);
    }
  };

  const login = async () => {
    const trimmed = tokenDraft.trim();
    if (!backendUrl || trimmed.length === 0 || busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    setStatus({ type: "connecting" });
    try {
      await accountLogin(trimmed);
      setTokenDraft("");
      await refreshStatus();
    } catch (reason) {
      setError(t("account.loginFailed", { error: errorMessage(reason) }));
      await refreshStatus();
    } finally {
      setBusy(false);
    }
  };

  const logout = async () => {
    if (busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      await accountLogout();
      setTokenDraft("");
      await refreshStatus();
    } catch (reason) {
      setError(errorMessage(reason));
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
        return t("account.status.online", { userId: status.info.userId });
      case "error":
        return t("account.status.error", { message: status.message });
    }
  })();

  return (
    <Section title={t("settings.section.account")}>
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

      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <label
          htmlFor="account-backend-url"
          style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}
        >
          {t("account.backendUrl")}
        </label>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("account.backendUrlDesc")}
        </div>
        <div style={{ display: "flex", gap: "var(--space-xs)" }}>
          <input
            id="account-backend-url"
            type="url"
            autoComplete="url"
            value={urlDraft}
            onChange={(event) => {
              setUrlDraft(event.target.value);
              setMessage(null);
              setError(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") void saveUrl();
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
        {message && (
          <div role="status" style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
            {message}
          </div>
        )}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <label
          htmlFor="account-token"
          style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}
        >
          {t("account.token")}
        </label>
        <div style={{ display: "flex", gap: "var(--space-xs)" }}>
          <input
            id="account-token"
            type="password"
            autoComplete="off"
            value={tokenDraft}
            onChange={(event) => {
              setTokenDraft(event.target.value);
              setError(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter") void login();
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
            disabled={busy || !backendUrl || tokenDraft.trim().length === 0}
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
              opacity: busy || !backendUrl ? 0.4 : 1,
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
          <div role="alert" style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>
            {error}
          </div>
        )}
      </div>

      <div
        aria-live="polite"
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
