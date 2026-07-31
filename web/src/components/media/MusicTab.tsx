import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Music, Plus, Sparkles, Upload } from "lucide-react";
import { useT } from "../../i18n";
import type { MediaItem } from "../../lib/types";
import { sourceName, useLibraryStore } from "../../store/libraryStore";
import { useMediaStore } from "../../store/mediaStore";
import { importFilesViaDialog } from "../../store/mediaActions";
import { addMediaToTimeline } from "../../store/editActions";
import { useEditorUiStore } from "../../store/uiStore";
import { useChatStore } from "../../store/chatStore";
import { Icon } from "../ui/Icon";

export function MusicTab({
  onPlace = addMediaToTimeline,
}: {
  onPlace?: (item: MediaItem) => Promise<unknown>;
}) {
  const t = useT();
  const projectItems = useMediaStore((state) => state.items);
  const libraryEntries = useLibraryStore((state) => state.entries);
  const libraryLoading = useLibraryStore((state) => state.loading);
  const libraryError = useLibraryStore((state) => state.error);
  const refreshLibrary = useLibraryStore((state) => state.refresh);
  const importToProject = useLibraryStore((state) => state.importToProject);
  const [query, setQuery] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{
    message: string;
    kind: "success" | "error";
  } | null>(null);

  useEffect(() => {
    void refreshLibrary();
  }, [refreshLibrary]);

  const normalizedQuery = query.trim().toLowerCase();
  const projectMusic = useMemo(
    () =>
      projectItems.filter(
        (item) =>
          item.type === "audio" &&
          (normalizedQuery === "" || item.name.toLowerCase().includes(normalizedQuery)),
      ),
    [normalizedQuery, projectItems],
  );
  const savedMusic = useMemo(
    () =>
      libraryEntries.filter((entry) => {
        if (entry.type !== "audio" || entry.category === "sound") return false;
        const name = sourceName(entry.source ?? entry.storedPath);
        return normalizedQuery === "" || name.toLowerCase().includes(normalizedQuery);
      }),
    [libraryEntries, normalizedQuery],
  );

  async function place(item: MediaItem) {
    if (busyId !== null) return;
    setBusyId(item.id);
    setFeedback(null);
    try {
      await onPlace(item);
      setFeedback({ message: t("media.music.placed", { name: item.name }), kind: "success" });
    } catch (reason) {
      setFeedback({
        message: t("media.music.placeFailed", {
          error: reason instanceof Error ? reason.message : String(reason),
        }),
        kind: "error",
      });
    } finally {
      setBusyId(null);
    }
  }

  async function importAndPlace(libraryId: string) {
    if (busyId !== null) return;
    setBusyId(libraryId);
    setFeedback(null);
    try {
      const imported = await importToProject(libraryId);
      if (!imported) throw new Error(t("media.music.importFailed"));
      const item = useMediaStore.getState().items.find((candidate) => candidate.id === imported.id);
      if (!item) throw new Error(t("media.music.importRefreshFailed"));
      await onPlace(item);
      setFeedback({
        message: t("media.music.placed", { name: imported.name }),
        kind: "success",
      });
    } catch (reason) {
      setFeedback({
        message: t("media.music.placeFailed", {
          error: reason instanceof Error ? reason.message : String(reason),
        }),
        kind: "error",
      });
    } finally {
      setBusyId(null);
    }
  }

  function prepareGeneration() {
    useChatStore.getState().setComposerDraft(t("media.music.generateDraft"));
    const ui = useEditorUiStore.getState();
    if (!ui.agentPanelVisible) ui.toggleAgentPanel();
  }

  return (
    <div
      data-testid="music-tab"
      style={{ display: "flex", flexDirection: "column", minHeight: 0, height: "100%" }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          padding: "var(--space-sm)",
          borderBottom: "var(--bw-thin) solid var(--border-primary)",
        }}
      >
        <button type="button" onClick={() => void importFilesViaDialog()} style={secondaryButtonStyle}>
          <Icon icon={Upload} size={12} />
          {t("media.music.import")}
        </button>
        <button
          type="button"
          data-action="generate-music"
          onClick={prepareGeneration}
          style={generateButtonStyle}
        >
          <Icon icon={Sparkles} size={12} />
          {t("media.music.generate")}
        </button>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label={t("media.search")}
          placeholder={t("media.search")}
          style={{
            flex: 1,
            minWidth: 80,
            height: 24,
            padding: "0 var(--space-sm)",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--border-primary)",
            background: "var(--bg-raised)",
            color: "var(--text-primary)",
            fontSize: "var(--fs-xs)",
          }}
        />
      </div>

      {feedback && (
        <div
          role={feedback.kind === "error" ? "alert" : "status"}
          style={{
            padding: "var(--space-xs) var(--space-sm)",
            color:
              feedback.kind === "error" ? "var(--status-error)" : "var(--text-secondary)",
            fontSize: "var(--fs-xs)",
          }}
        >
          {feedback.message}
        </div>
      )}
      {libraryError && (
        <div role="alert" style={{ padding: "var(--space-xs) var(--space-sm)", color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
          {libraryError}
        </div>
      )}

      <div style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "var(--space-sm)" }}>
        <MusicSection title={t("media.music.projectSection")} empty={projectMusic.length === 0}>
          {projectMusic.map((item) => (
            <MusicRow
              key={item.id}
              name={item.name}
              disabled={busyId !== null || !!item.missing}
              buttonLabel={t("media.music.place")}
              dataAttribute={{ "data-place-media": item.id }}
              onClick={() => void place(item)}
            />
          ))}
        </MusicSection>

        <MusicSection
          title={t("media.music.librarySection")}
          empty={!libraryLoading && savedMusic.length === 0}
          loading={libraryLoading}
        >
          {savedMusic.map((entry) => (
            <MusicRow
              key={entry.id}
              name={sourceName(entry.source ?? entry.storedPath) || entry.id}
              disabled={busyId !== null}
              buttonLabel={t("media.music.importAndPlace")}
              dataAttribute={{ "data-import-library": entry.id }}
              onClick={() => void importAndPlace(entry.id)}
            />
          ))}
        </MusicSection>
      </div>
    </div>
  );
}

function MusicSection({
  title,
  empty,
  loading = false,
  children,
}: {
  title: string;
  empty: boolean;
  loading?: boolean;
  children: ReactNode;
}) {
  const t = useT();
  return (
    <section style={{ marginBottom: "var(--space-lg)" }}>
      <h3
        style={{
          margin: "0 0 var(--space-xs)",
          color: "var(--text-muted)",
          fontSize: "var(--fs-xxs)",
          letterSpacing: "var(--tracking-wide)",
          textTransform: "uppercase",
        }}
      >
        {title}
      </h3>
      {loading ? (
        <div style={emptyStyle}>{t("media.importing")}</div>
      ) : empty ? (
        <div style={emptyStyle}>{t("media.music.empty")}</div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xxs)" }}>
          {children}
        </div>
      )}
    </section>
  );
}

function MusicRow({
  name,
  disabled,
  buttonLabel,
  dataAttribute,
  onClick,
}: {
  name: string;
  disabled: boolean;
  buttonLabel: string;
  dataAttribute: Record<string, string>;
  onClick: () => void;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-sm)",
        minHeight: 34,
        padding: "0 var(--space-sm)",
        borderRadius: "var(--radius-sm)",
        background: "var(--bg-raised)",
      }}
    >
      <Icon icon={Music} size={13} />
      <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: "var(--fs-sm)" }}>
        {name}
      </span>
      <button
        type="button"
        {...dataAttribute}
        disabled={disabled}
        onClick={onClick}
        style={secondaryButtonStyle}
      >
        <Icon icon={Plus} size={11} />
        {buttonLabel}
      </button>
    </div>
  );
}

const emptyStyle = {
  padding: "var(--space-md)",
  color: "var(--text-tertiary)",
  fontSize: "var(--fs-xs)",
  textAlign: "center",
} as const;

const secondaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  minHeight: 23,
  padding: "1px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-thin) solid var(--border-primary)",
  background: "var(--bg-prominent)",
  color: "var(--text-primary)",
  fontSize: "var(--fs-xs)",
} as const;

const generateButtonStyle = {
  ...secondaryButtonStyle,
  border: "none",
  background: "var(--ai-gradient)",
  color: "#111",
  fontWeight: "var(--fw-semibold)",
} as const;
