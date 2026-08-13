/**
 * Settings Storage pane: real on-disk usage for the derived caches (thumbnails,
 * waveforms, search index, downloaded models, other) with per-category clear
 * buttons. Semantic port of upstream `Settings/StoragePane.swift` — adapted to
 * OpenTake's cache layout, where the two visual families share one dir and the
 * search index/models are separate surfaces.
 *
 * The Rust side is authoritative (`storage_usage` / `storage_clear` in
 * src-tauri/src/storage.rs): it computes real byte counts and deletes ONLY the
 * requested derived caches, recreating cache roots so the engine stays
 * functional. The models category is a re-download, not a lazily-rebuilt cache,
 * so it requires an explicit inline confirm step here AND the Rust command
 * rejects it unless `modelsConfirmed` is set. Outside Tauri the API resolves to
 * an honest empty report and this pane renders its unsupported state.
 */

import { useEffect, useRef, useState, type CSSProperties } from "react";
import { HardDrive } from "lucide-react";
import { useT } from "../../i18n";
import { storageClear, storageUsage } from "../../lib/api";
import type { StorageCategoryId, StorageUsage } from "../../lib/types";
import { formatBytes } from "../../lib/storageFormat";
import { Icon } from "../ui/Icon";
import { Reveal } from "../ui/Reveal";

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
      <div style={sectionStyle}>{children}</div>
    </section>
  );
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

/** All category rows in the authoritative server order. */
const CATEGORY_ORDER: StorageCategoryId[] = [
  "thumbnails",
  "waveforms",
  "searchIndex",
  "models",
  "other",
];

export function StoragePane() {
  const t = useT();
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [clearing, setClearing] = useState<StorageCategoryId | null>(null);
  const [confirming, setConfirming] = useState<StorageCategoryId | null>(null);
  const [focusTarget, setFocusTarget] = useState<"model" | "next" | null>(null);
  const clearButtonRefs = useRef<Partial<Record<StorageCategoryId, HTMLButtonElement | null>>>({});

  useEffect(() => {
    let alive = true;
    setError(null);
    storageUsage().then(
      (next) => {
        if (alive) setUsage(next);
      },
      (reason) => {
        if (alive) setError(errorMessage(reason));
      },
    );
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (!focusTarget) return;

    if (focusTarget === "model") {
      clearButtonRefs.current.models?.focus();
    } else {
      const modelIndex = CATEGORY_ORDER.indexOf("models");
      const next = CATEGORY_ORDER.slice(modelIndex + 1)
        .map((id) => clearButtonRefs.current[id])
        .find((button): button is HTMLButtonElement => Boolean(button && !button.disabled));
      next?.focus();
    }

    setFocusTarget(null);
  }, [focusTarget, usage, clearing, confirming]);

  const runClear = async (categories: StorageCategoryId[], modelsConfirmed: boolean) => {
    if (clearing) return;
    const inFlight = categories[0]!;
    let succeeded = false;
    setClearing(inFlight);
    setError(null);
    try {
      const next = await storageClear(categories, modelsConfirmed);
      setUsage(next);
      succeeded = true;
      if (inFlight === "models") setFocusTarget("next");
    } catch (reason) {
      setError(t("storage.error", { error: errorMessage(reason) }));
    } finally {
      setClearing(null);
      if (succeeded || inFlight !== "models") setConfirming(null);
    }
  };

  const startClear = (category: StorageCategoryId) => {
    if (clearing) return;
    if (category === "models" && confirming !== "models") {
      // Models are re-downloads: the confirm step is mandatory (and the Rust
      // command independently rejects an unconfirmed models clear).
      setError(null);
      setConfirming("models");
      return;
    }
    void runClear([category], category === "models");
  };

  const cancelConfirm = () => {
    setError(null);
    setConfirming(null);
    setFocusTarget("model");
  };

  if (usage === null && error === null) {
    return (
      <Section title={t("settings.section.storage")}>
        <div role="status" style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
          {t("storage.loading")}
        </div>
      </Section>
    );
  }

  if (usage === null) {
    return (
      <Section title={t("settings.section.storage")}>
        <div role="alert" style={{ fontSize: "var(--fs-sm-md)", color: "var(--status-error)" }}>
          {t("storage.error", { error: error ?? "" })}
        </div>
      </Section>
    );
  }

  // Outside Tauri there is no backend file system: the API resolves to the
  // honest empty report with no cache root — render the unsupported state
  // rather than fake statistics.
  if (usage.cacheRoot.length === 0) {
    return (
      <Section title={t("settings.section.storage")}>
        <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
          {t("storage.unsupported")}
        </div>
      </Section>
    );
  }

  const totalZero = usage.totalBytes === 0;

  return (
    <Section title={t("settings.section.storage")}>
      <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
        <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)" }}>
          {t("storage.desc")}
        </div>
        <div style={{ display: "flex", gap: "var(--space-md)", alignItems: "baseline" }}>
          <span
            className="tabular"
            style={{ fontSize: "var(--fs-md-lg)", color: "var(--text-primary)", fontWeight: "var(--fw-semibold)" }}
          >
            {formatBytes(usage.totalBytes)}
          </span>
          <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
            {t("storage.total")}
          </span>
          <span
            className="tabular"
            title={usage.cacheRoot}
            style={{
              flex: 1,
              minWidth: 0,
              overflow: "hidden",
              whiteSpace: "nowrap",
              textOverflow: "ellipsis",
              direction: "rtl",
              textAlign: "left",
              fontSize: "var(--fs-xxs)",
              color: "var(--text-tertiary)",
            }}
          >
            {usage.cacheRoot}
          </span>
        </div>
      </div>

      {totalZero && (
        <div role="status" style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("storage.empty")}
        </div>
      )}

      {CATEGORY_ORDER.map((id) => {
        const category = usage.categories.find((candidate) => candidate.id === id);
        if (!category) return null;
        const busy = clearing !== null;
        const isClearingThis = clearing === id;
        const showConfirm = confirming === id;
        return (
          <div key={id} className="storage-category-row" data-storage-row={id}>
            <div
              className="storage-category-row__main"
              style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-lg)",
                justifyContent: "space-between",
              }}
            >
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>
                  {t(`storage.category.${id}`)}
                  <span className="tabular" style={{ marginLeft: "var(--space-sm)", fontSize: "var(--fs-xs)", color: "var(--text-secondary)" }}>
                    {formatBytes(category.bytes)}
                  </span>
                </div>
                <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", marginTop: 2 }}>
                  {t(`storage.categoryDesc.${id}`)}
                </div>
              </div>
              <div style={{ flex: "0 0 auto" }}>
                <button
                  ref={(button) => {
                    clearButtonRefs.current[id] = button;
                  }}
                  type="button"
                  data-category={id}
                  data-action="clear"
                  disabled={busy || showConfirm || category.bytes === 0}
                  onClick={() => startClear(id)}
                  className="hover-area"
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 4,
                    height: 28,
                    padding: "0 var(--space-md)",
                    borderRadius: "var(--radius-sm)",
                    ...controlStyle,
                    color: "var(--text-secondary)",
                    fontSize: "var(--fs-sm)",
                    fontWeight: "var(--fw-medium)",
                    opacity: busy || showConfirm || category.bytes === 0 ? 0.4 : 1,
                  }}
                >
                  {isClearingThis && <Icon icon={HardDrive} size={13} />}
                  {isClearingThis ? t("storage.clearing") : t("storage.clear")}
                </button>
              </div>
            </div>
            {id === "models" && (
              <Reveal open={showConfirm} role="group">
                <div className="storage-model-confirmation">
                  <div className="storage-model-confirmation__actions">
                    <span style={{ fontSize: "var(--fs-xs)", color: "var(--text-secondary)" }}>
                      {t("storage.clearConfirmTitle")}
                    </span>
                    <button
                      type="button"
                      data-category="models"
                      data-action="confirm-remove"
                      disabled={busy}
                      onClick={() => void runClear(["models"], true)}
                      className="hover-area"
                      style={{
                        height: 28,
                        padding: "0 var(--space-md)",
                        borderRadius: "var(--radius-sm)",
                        ...controlStyle,
                        color: "var(--status-error)",
                        fontSize: "var(--fs-sm)",
                        fontWeight: "var(--fw-medium)",
                      }}
                    >
                      {t("storage.confirmRemove")}
                    </button>
                    <button
                      type="button"
                      data-category="models"
                      data-action="confirm-cancel"
                      disabled={busy}
                      onClick={cancelConfirm}
                      className="hover-area"
                      style={{
                        height: 28,
                        padding: "0 var(--space-md)",
                        borderRadius: "var(--radius-sm)",
                        color: "var(--text-tertiary)",
                        fontSize: "var(--fs-sm)",
                      }}
                    >
                      {t("storage.confirmCancel")}
                    </button>
                  </div>
                  <div style={{ fontSize: "var(--fs-xxs)", color: "var(--text-muted)" }}>
                    {t("storage.clearConfirmBody")}
                  </div>
                  {error && (
                    <div role="alert" style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>
                      {error}
                    </div>
                  )}
                </div>
              </Reveal>
            )}
          </div>
        );
      })}

      {error && confirming !== "models" && (
        <div role="alert" style={{ fontSize: "var(--fs-xs)", color: "var(--status-error)" }}>
          {error}
        </div>
      )}
    </Section>
  );
}
