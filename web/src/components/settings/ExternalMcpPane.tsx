import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Check, Copy, Plug, RefreshCw, ShieldAlert, Trash2 } from "lucide-react";
import { useT } from "../../i18n";
import {
  externalMcpPair,
  externalMcpRegenerate,
  externalMcpRevoke,
  externalMcpSetEnabled,
  externalMcpStatus,
  onExternalMcpStatusChanged,
} from "../../lib/api";
import type {
  ExternalMcpClientSummary,
  ExternalMcpPairingReceipt,
  ExternalMcpStatus,
} from "../../lib/types";
import { Icon } from "../ui/Icon";
import { Reveal } from "../ui/Reveal";

type Confirmation = {
  action: "regenerate" | "revoke";
  client: ExternalMcpClientSummary;
};

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

function clientConfig(receipt: ExternalMcpPairingReceipt): string {
  return JSON.stringify({
    mcpServers: {
      [receipt.client.name]: {
        type: "http",
        url: receipt.endpoint,
        headers: {
          Authorization: `Bearer ${receipt.bearerToken}`,
        },
      },
    },
  }, null, 2);
}

function listenerStatusKey(status: ExternalMcpStatus | null): string {
  return status?.state ?? "loading";
}

export function ExternalMcpPane({
  onReceiptOperationPendingChange,
}: {
  onReceiptOperationPendingChange?: (pending: boolean) => void;
}) {
  const t = useT();
  const [status, setStatus] = useState<ExternalMcpStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [clientName, setClientName] = useState("");
  const [receipt, setReceipt] = useState<ExternalMcpPairingReceipt | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyPending, setCopyPending] = useState(false);
  const [pending, setPending] = useState<"enable" | "pair" | "regenerate" | "revoke" | null>(null);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [receiptOperationSettlement, setReceiptOperationSettlement] = useState(0);
  const [focusTarget, setFocusTarget] = useState<
    { kind: "regenerate"; clientId: string } | { kind: "client"; clientId: string | null } | null
  >(null);
  const paneRef = useRef<HTMLElement>(null);
  const latestRevisionRef = useRef(-1);
  const mountedRef = useRef(false);
  const operationEpochRef = useRef(0);
  const receiptEpochRef = useRef(0);
  const receiptOperationLockedRef = useRef(false);
  const actionTriggerRef = useRef<HTMLButtonElement | null>(null);
  const receiptDismissRef = useRef<HTMLButtonElement | null>(null);

  const applyStatus = useCallback((next: ExternalMcpStatus) => {
    if (next.revision < latestRevisionRef.current) return;
    latestRevisionRef.current = next.revision;
    setStatus(next);
  }, []);

  useEffect(() => {
    let alive = true;
    let unlisten = () => {};
    mountedRef.current = true;

    const subscribe = async () => {
      try {
        // The API listener establishes its event subscription before its own
        // refresh. This explicit snapshot additionally reconciles mutations
        // that complete while the component is mounting.
        const dispose = await onExternalMcpStatusChanged((next) => {
          if (alive) applyStatus(next);
        });
        if (!alive) {
          dispose();
          return;
        }
        unlisten = dispose;
        const current = await externalMcpStatus();
        if (alive) applyStatus(current);
      } catch (reason) {
        if (alive) setError(t("mcp.error.status", { error: errorMessage(reason) }));
      }
    };

    void subscribe();
    return () => {
      alive = false;
      mountedRef.current = false;
      operationEpochRef.current += 1;
      receiptEpochRef.current += 1;
      unlisten();
    };
  }, [applyStatus, t]);

  useEffect(() => {
    if (!receiptOperationLockedRef.current) return;
    receiptOperationLockedRef.current = false;
    onReceiptOperationPendingChange?.(false);
  }, [onReceiptOperationPendingChange, receiptOperationSettlement]);

  useEffect(() => () => {
    onReceiptOperationPendingChange?.(false);
  }, [onReceiptOperationPendingChange]);

  useLayoutEffect(() => {
    if (!focusTarget) return;
    const clientId = focusTarget.clientId;
    const clientRow = [...(paneRef.current?.querySelectorAll<HTMLElement>(
      "[data-external-mcp-client]",
    ) ?? [])].find((row) => row.dataset.externalMcpClient === clientId);
    const target = clientRow?.querySelector<HTMLButtonElement>(
      '[data-external-mcp-action="regenerate"]',
    ) ?? paneRef.current;
    target?.focus({ preventScroll: true });
    setFocusTarget(null);
  }, [focusTarget]);

  const focusConfirmation = useCallback((button: HTMLButtonElement | null) => {
    button?.focus({ preventScroll: true });
  }, []);

  const isCurrentOperation = (epoch: number) =>
    mountedRef.current && operationEpochRef.current === epoch;

  const refresh = async (epoch: number) => {
    const current = await externalMcpStatus();
    if (isCurrentOperation(epoch)) applyStatus(current);
  };

  const refreshAfterMutation = (epoch: number) => {
    void refresh(epoch).catch((reason) => {
      if (isCurrentOperation(epoch)) setError(t("mcp.error.status", { error: errorMessage(reason) }));
    });
  };

  const clearReceipt = () => {
    receiptEpochRef.current += 1;
    setReceipt(null);
    setCopied(false);
  };

  const showReceipt = (next: ExternalMcpPairingReceipt) => {
    receiptEpochRef.current += 1;
    setReceipt(next);
    setCopied(false);
  };

  const beginReceiptOperation = () => {
    receiptOperationLockedRef.current = true;
    onReceiptOperationPendingChange?.(true);
  };

  const runEnabledChange = async () => {
    if (!status || pending || copyPending) return;
    const epoch = ++operationEpochRef.current;
    setPending("enable");
    setError(null);
    try {
      const next = await externalMcpSetEnabled(!isListening);
      if (isCurrentOperation(epoch)) applyStatus(next);
    } catch (reason) {
      if (isCurrentOperation(epoch)) {
        setError(t("mcp.error.command", { error: errorMessage(reason) }));
        try {
          await refresh(epoch);
        } catch {
          // The command error is more actionable than a second failed refresh.
        }
      }
    } finally {
      if (isCurrentOperation(epoch)) setPending(null);
    }
  };

  const runPair = async () => {
    if (pending || copyPending) return;
    const name = clientName.trim();
    if (!name) {
      setError(t("mcp.error.clientName"));
      return;
    }
    const epoch = ++operationEpochRef.current;
    beginReceiptOperation();
    setPending("pair");
    setError(null);
    clearReceipt();
    try {
      const next = await externalMcpPair(name);
      if (!isCurrentOperation(epoch)) return;
      showReceipt(next);
      setClientName("");
      refreshAfterMutation(epoch);
    } catch (reason) {
      if (isCurrentOperation(epoch)) setError(t("mcp.error.command", { error: errorMessage(reason) }));
    } finally {
      if (isCurrentOperation(epoch)) {
        setPending(null);
        setReceiptOperationSettlement((settlement) => settlement + 1);
      }
    }
  };

  const runConfirmation = async () => {
    if (!confirmation || pending || copyPending) return;
    const currentConfirmation = confirmation;
    const activeClients = status?.clients.filter((client) => client.revokedAt === null) ?? [];
    const revokedIndex = activeClients.findIndex((client) => client.id === currentConfirmation.client.id);
    const epoch = ++operationEpochRef.current;
    if (currentConfirmation.action === "regenerate") {
      beginReceiptOperation();
    }
    setPending(currentConfirmation.action);
    setError(null);
    clearReceipt();
    try {
      if (currentConfirmation.action === "regenerate") {
        const next = await externalMcpRegenerate(currentConfirmation.client.id);
        if (!isCurrentOperation(epoch)) return;
        showReceipt(next);
        refreshAfterMutation(epoch);
        setFocusTarget({ kind: "regenerate", clientId: currentConfirmation.client.id });
      } else {
        const next = await externalMcpRevoke(currentConfirmation.client.id);
        if (isCurrentOperation(epoch)) {
          applyStatus(next);
          const remainingClients = next.clients.filter((client) => client.revokedAt === null);
          const focusIndex = Math.min(Math.max(0, revokedIndex), remainingClients.length - 1);
          const nextClientId = remainingClients[focusIndex]?.id ?? null;
          setFocusTarget({ kind: "client", clientId: nextClientId });
        }
      }
      if (isCurrentOperation(epoch)) setConfirmation(null);
    } catch (reason) {
      if (isCurrentOperation(epoch)) setError(t("mcp.error.command", { error: errorMessage(reason) }));
    } finally {
      if (isCurrentOperation(epoch)) {
        setPending(null);
        if (currentConfirmation.action === "regenerate") {
          setReceiptOperationSettlement((settlement) => settlement + 1);
        }
      }
    }
  };

  const cancelConfirmation = () => {
    setConfirmation(null);
    setError(null);
    actionTriggerRef.current?.focus({ preventScroll: true });
  };

  const dismissReceipt = () => {
    if (copyPending) return;
    // Receipt tokens are intentionally component-local. Clearing the state
    // before closing the disclosure removes the bearer from the DOM at once.
    clearReceipt();
    receiptDismissRef.current?.blur();
  };

  const copyConfig = async () => {
    if (!receipt || copyPending) return;
    const receiptEpoch = receiptEpochRef.current;
    setCopyPending(true);
    setError(null);
    try {
      if (!navigator.clipboard?.writeText) throw new Error(t("mcp.error.clipboardUnavailable"));
      await navigator.clipboard.writeText(clientConfig(receipt));
      if (mountedRef.current && receiptEpochRef.current === receiptEpoch) setCopied(true);
    } catch (reason) {
      if (mountedRef.current && receiptEpochRef.current === receiptEpoch) {
        setError(t("mcp.error.clipboard", { error: errorMessage(reason) }));
      }
    } finally {
      if (mountedRef.current && receiptEpochRef.current === receiptEpoch) setCopyPending(false);
    }
  };

  const isListening = status?.state === "listening";
  const controlsDisabled = status === null || pending !== null || copyPending;
  const state = listenerStatusKey(status);
  const statusTitle = status ? t(`mcp.status.${status.state}`) : t("mcp.status.loading");

  return (
    <section className="external-mcp-pane" ref={paneRef} tabIndex={-1}>
      <header className="external-mcp-pane__header">
        <div>
          <h2>{t("settings.section.mcp")}</h2>
          <p>{t("mcp.overview")}</p>
        </div>
        <label className="external-mcp-toggle">
          <input
            type="checkbox"
            role="switch"
            checked={isListening}
            disabled={controlsDisabled}
            aria-label={t("mcp.enabled")}
            aria-describedby="external-mcp-listener-status"
            onChange={() => void runEnabledChange()}
          />
          <span aria-hidden="true" className="external-mcp-toggle__track" />
        </label>
      </header>

      <div
        id="external-mcp-listener-status"
        role="status"
        aria-live="polite"
        data-external-mcp-status={state}
        className={`external-mcp-status external-mcp-status--${state}`}
      >
        <Icon icon={status?.state === "authFailure" || status?.state === "portConflict" ? ShieldAlert : Plug} size={16} />
        <div>
          <strong>{statusTitle}</strong>
          <span>{status?.error ?? t(`mcp.statusDesc.${state}`)}</span>
        </div>
      </div>

      <div className="external-mcp-endpoint">
        <span>{t("mcp.endpoint")}</span>
        <code>{status?.endpoint ?? "http://127.0.0.1:19789/mcp"}</code>
      </div>

      <div className="external-mcp-pair-form">
        <label htmlFor="external-mcp-client-name">{t("mcp.clientName")}</label>
        <div className="external-mcp-pair-form__controls">
          <input
            id="external-mcp-client-name"
            name="external-mcp-client-name"
            type="text"
            value={clientName}
            maxLength={128}
            disabled={controlsDisabled}
            placeholder={t("mcp.clientNamePlaceholder")}
            onChange={(event) => setClientName(event.target.value)}
          />
          <button
            type="button"
            className="hover-area"
            disabled={controlsDisabled}
            onClick={() => void runPair()}
          >
            {pending === "pair" ? t("mcp.pairing") : t("mcp.pair")}
          </button>
        </div>
      </div>

      <Reveal open={receipt !== null}>
        {receipt && (
          <div className="external-mcp-receipt" role="region" aria-label={t("mcp.tokenTitle")}>
            <div>
              <strong>{t("mcp.tokenTitle")}</strong>
              <p>{t("mcp.tokenDesc")}</p>
            </div>
            <code className="external-mcp-receipt__token">{receipt.bearerToken}</code>
            <div className="external-mcp-receipt__actions">
              <button
                type="button"
                className="hover-area"
                disabled={copyPending}
                onClick={() => void copyConfig()}
              >
                <Icon icon={copied ? Check : Copy} size={13} />
                {copied ? t("mcp.configCopied") : t("mcp.copyConfig")}
              </button>
              <button
                ref={receiptDismissRef}
                type="button"
                className="hover-area"
                disabled={copyPending}
                onClick={dismissReceipt}
              >
                {t("mcp.tokenDismiss")}
              </button>
            </div>
          </div>
        )}
      </Reveal>

      <div className="external-mcp-clients">
        <div className="external-mcp-clients__heading">
          <h3>{t("mcp.clients")}</h3>
          <span>{status?.clients.length ?? 0}</span>
        </div>
        {status?.clients.length ? (
          <ul>
            {status.clients.map((client) => {
              const isRevoked = client.revokedAt !== null;
              const isConfirming = confirmation?.client.id === client.id;
              return (
                <li key={client.id} className="external-mcp-client" data-external-mcp-client={client.id}>
                  <div className="external-mcp-client__summary">
                    <strong>{client.name}</strong>
                    <span>{isRevoked ? t("mcp.revoked") : t("mcp.clientDigest", { digest: client.tokenDigest })}</span>
                  </div>
                  {!isRevoked && (
                    <div className="external-mcp-client__actions">
                      <button
                        type="button"
                        className="hover-area"
                        aria-expanded={isConfirming && confirmation.action === "regenerate"}
                        aria-controls={`external-mcp-confirm-${client.id}`}
                        data-external-mcp-action="regenerate"
                        disabled={pending !== null || copyPending}
                        onClick={(event) => {
                          actionTriggerRef.current = event.currentTarget;
                          setError(null);
                          setConfirmation({ action: "regenerate", client });
                        }}
                      >
                        <Icon icon={RefreshCw} size={13} />
                        {t("mcp.regenerate")}
                      </button>
                      <button
                        type="button"
                        className="hover-area external-mcp-client__revoke"
                        aria-expanded={isConfirming && confirmation.action === "revoke"}
                        aria-controls={`external-mcp-confirm-${client.id}`}
                        disabled={pending !== null || copyPending}
                        onClick={(event) => {
                          actionTriggerRef.current = event.currentTarget;
                          setError(null);
                          setConfirmation({ action: "revoke", client });
                        }}
                      >
                        <Icon icon={Trash2} size={13} />
                        {t("mcp.revoke")}
                      </button>
                    </div>
                  )}
                  <Reveal open={isConfirming} id={`external-mcp-confirm-${client.id}`}>
                    {isConfirming && (
                      <div
                        role="group"
                        aria-label={t(confirmation.action === "regenerate" ? "mcp.regenerateConfirmTitle" : "mcp.revokeConfirmTitle")}
                        className="external-mcp-confirmation"
                        onKeyDown={(event) => {
                          if (event.key !== "Escape") return;
                          event.preventDefault();
                          cancelConfirmation();
                        }}
                      >
                        <strong>{t(confirmation.action === "regenerate" ? "mcp.regenerateConfirmTitle" : "mcp.revokeConfirmTitle")}</strong>
                        <p>{t(confirmation.action === "regenerate" ? "mcp.regenerateConfirmDesc" : "mcp.revokeConfirmDesc")}</p>
                        <div>
                          <button
                            ref={focusConfirmation}
                            type="button"
                            className={confirmation.action === "revoke"
                              ? "hover-area external-mcp-confirmation__destructive"
                              : "hover-area"}
                            disabled={pending !== null || copyPending}
                            onClick={() => void runConfirmation()}
                          >
                            {t(confirmation.action === "regenerate" ? "mcp.confirmRegenerate" : "mcp.confirmRevoke")}
                          </button>
                          <button
                            type="button"
                            className="hover-area"
                            disabled={pending !== null || copyPending}
                            onClick={cancelConfirmation}
                          >
                            {t("mcp.cancel")}
                          </button>
                        </div>
                      </div>
                    )}
                  </Reveal>
                </li>
              );
            })}
          </ul>
        ) : (
          <p className="external-mcp-clients__empty">{t("mcp.clientsEmpty")}</p>
        )}
      </div>

      {error && <div role="alert" className="external-mcp-error">{error}</div>}
      <p className="external-mcp-note">{t("mcp.note")}</p>
    </section>
  );
}
