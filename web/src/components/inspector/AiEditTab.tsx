import { useEffect, useMemo, useRef, useState } from "react";
import { RotateCcw, Sparkles, Square, X } from "lucide-react";
import { useT } from "../../i18n";
import type { Clip, ClipPropertiesReq } from "../../lib/types";
import * as edit from "../../store/editActions";
import { Icon } from "../ui/Icon";

export interface AiEditProposal {
  id: string;
  /** i18n 键：标题与解释文案（数值经 explanationParams 插值）。 */
  titleKey: string;
  explanationKey: string;
  explanationParams: Record<string, string | number>;
  properties: ClipPropertiesReq;
}

export type AiEditSuggester = (
  clip: Clip,
  intent: string,
  fps: number,
  signal: AbortSignal,
) => Promise<AiEditProposal[]>;

/**
 * Build a deterministic, offline-safe edit proposal. The AI Edit surface is a
 * proposal/review boundary, not a second editing engine: accepted values still
 * travel through the shared SetClipProperties command and therefore create one
 * normal undo entry. A future remote suggester can be injected without changing
 * that command contract.
 */
export const suggestAiEdits: AiEditSuggester = async (clip, intent, fps, signal) => {
  await Promise.resolve();
  if (signal.aborted) throw new DOMException("Cancelled", "AbortError");

  const normalized = intent.trim().toLowerCase();
  const fadeFrames = Math.max(1, Math.min(Math.round(fps * 0.4), Math.floor(clip.durationFrames / 3)));
  if (/快|紧凑|speed|faster|punch/.test(normalized)) {
    const speed = Math.min(4, Math.max(0.25, Number((clip.speed * 1.25).toFixed(2))));
    return [
      {
        id: "pace-up",
        titleKey: "inspector.aiEdit.proposal.paceUp.title",
        explanationKey: "inspector.aiEdit.proposal.paceUp.explanation",
        explanationParams: { from: clip.speed.toFixed(2), to: speed.toFixed(2) },
        properties: { speed },
      },
    ];
  }
  if (/淡|柔和|smooth|fade|gentle/.test(normalized)) {
    return [
      {
        id: "gentle-fade",
        titleKey: "inspector.aiEdit.proposal.gentleFade.title",
        explanationKey: "inspector.aiEdit.proposal.gentleFade.explanation",
        explanationParams: { frames: fadeFrames },
        properties: { fadeInFrames: fadeFrames, fadeOutFrames: fadeFrames },
      },
    ];
  }
  return [
    {
      id: "balanced-polish",
      titleKey: "inspector.aiEdit.proposal.balancedPolish.title",
      explanationKey: "inspector.aiEdit.proposal.balancedPolish.explanation",
      explanationParams: { frames: fadeFrames },
      properties: { fadeInFrames: fadeFrames, fadeOutFrames: fadeFrames, opacity: 1 },
    },
  ];
};

type Status = "idle" | "generating" | "review" | "applying" | "applied" | "error";

export function AiEditTab({
  clip,
  fps,
  unavailableReason,
  suggest = suggestAiEdits,
  onApply = (clipId, properties) => edit.setClipProperties([clipId], properties),
  onUndo = edit.undo,
}: {
  clip: Clip;
  fps: number;
  unavailableReason?: string | null;
  suggest?: AiEditSuggester;
  onApply?: (clipId: string, properties: ClipPropertiesReq) => Promise<unknown>;
  onUndo?: () => Promise<unknown>;
}) {
  const t = useT();
  const [intent, setIntent] = useState(() => t("inspector.aiEdit.defaultIntent"));
  const [status, setStatus] = useState<Status>("idle");
  const [proposals, setProposals] = useState<AiEditProposal[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef<AbortController | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      requestRef.current?.abort();
    };
  }, []);

  useEffect(() => {
    requestRef.current?.abort();
    setStatus("idle");
    setProposals([]);
    setSelectedId(null);
    setError(null);
  }, [clip.id]);

  const selected = useMemo(
    () => proposals.find((proposal) => proposal.id === selectedId) ?? proposals[0] ?? null,
    [proposals, selectedId],
  );

  async function generate() {
    if (status === "generating" || status === "applying" || unavailableReason) return;
    requestRef.current?.abort();
    const controller = new AbortController();
    requestRef.current = controller;
    setStatus("generating");
    setError(null);
    setProposals([]);
    setSelectedId(null);
    try {
      const next = await suggest(clip, intent.trim(), fps, controller.signal);
      if (!mountedRef.current || controller.signal.aborted) return;
      if (next.length === 0) throw new Error(t("inspector.aiEdit.noSuggestion"));
      setProposals(next);
      setSelectedId(next[0].id);
      setStatus("review");
    } catch (reason) {
      if (!mountedRef.current || controller.signal.aborted) return;
      setError(reason instanceof Error ? reason.message : String(reason));
      setStatus("error");
    } finally {
      if (requestRef.current === controller) requestRef.current = null;
    }
  }

  function cancel() {
    requestRef.current?.abort();
    requestRef.current = null;
    setStatus("idle");
    setError(null);
  }

  function reject() {
    setProposals([]);
    setSelectedId(null);
    setError(null);
    setStatus("idle");
  }

  async function apply() {
    if (!selected || status !== "review") return;
    setStatus("applying");
    setError(null);
    try {
      await onApply(clip.id, selected.properties);
      if (mountedRef.current) setStatus("applied");
    } catch (reason) {
      if (!mountedRef.current) return;
      setError(reason instanceof Error ? reason.message : String(reason));
      setStatus("error");
    }
  }

  async function undo() {
    if (status !== "applied") return;
    setStatus("applying");
    setError(null);
    try {
      await onUndo();
      if (mountedRef.current) {
        setProposals([]);
        setSelectedId(null);
        setStatus("idle");
      }
    } catch (reason) {
      if (!mountedRef.current) return;
      setError(reason instanceof Error ? reason.message : String(reason));
      setStatus("error");
    }
  }

  return (
    <section
      data-testid="ai-edit-tab"
      aria-label={t("inspector.tab.aiEdit")}
      style={{ display: "flex", flexDirection: "column", gap: "var(--space-md)" }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "var(--space-xs)",
          fontSize: "var(--fs-sm-md)",
          fontWeight: "var(--fw-semibold)",
          background: "var(--ai-gradient)",
          WebkitBackgroundClip: "text",
          color: "transparent",
        }}
      >
        <Icon icon={Sparkles} size={13} />
        {t("inspector.aiEdit.heading")}
      </div>

      {/* 诚实披露：这里是本地确定性启发式（suggestAiEdits），不调用任何 AI 模型。 */}
      <div style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
        {t("inspector.aiEdit.disclosure")}
      </div>

      {unavailableReason ? (
        <div role="status" style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-sm)" }}>
          {unavailableReason}
        </div>
      ) : (
        <>
          <label style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
            <span style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
              {t("inspector.aiEdit.intent")}
            </span>
            <textarea
              value={intent}
              disabled={status === "generating" || status === "applying"}
              onChange={(event) => setIntent(event.target.value)}
              rows={3}
              placeholder={t("inspector.aiEdit.placeholder")}
              style={{
                resize: "vertical",
                minHeight: 58,
                padding: "var(--space-sm)",
                borderRadius: "var(--radius-sm)",
                border: "var(--bw-thin) solid var(--border-primary)",
                background: "var(--bg-raised)",
                color: "var(--text-primary)",
                font: "inherit",
              }}
            />
          </label>
          <div style={{ display: "flex", gap: "var(--space-xs)" }}>
            <button
              type="button"
              data-action="generate"
              disabled={!intent.trim() || status === "generating" || status === "applying"}
              onClick={() => void generate()}
              style={primaryButtonStyle}
            >
              <Icon icon={Sparkles} size={12} />
              {status === "generating"
                ? t("inspector.aiEdit.generating")
                : t("inspector.aiEdit.generate")}
            </button>
            {status === "generating" && (
              <button type="button" data-action="cancel" onClick={cancel} style={secondaryButtonStyle}>
                <Icon icon={Square} size={11} />
                {t("inspector.aiEdit.cancel")}
              </button>
            )}
          </div>
        </>
      )}

      {error && (
        <div role="alert" style={{ color: "var(--status-error)", fontSize: "var(--fs-xs)" }}>
          {error}
        </div>
      )}

      {selected && (status === "review" || status === "applying" || status === "applied") && (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-sm)",
            padding: "var(--space-sm-md)",
            borderRadius: "var(--radius-md)",
            border: "var(--bw-thin) solid var(--border-primary)",
            background: "var(--bg-raised)",
          }}
        >
          {proposals.map((proposal) => (
            <button
              key={proposal.id}
              type="button"
              role="radio"
              aria-checked={selected.id === proposal.id}
              disabled={status !== "review"}
              onClick={() => setSelectedId(proposal.id)}
              style={{
                textAlign: "left",
                padding: "var(--space-xs)",
                borderRadius: "var(--radius-sm)",
                border:
                  selected.id === proposal.id
                    ? "var(--bw-thin) solid var(--accent-primary)"
                    : "var(--bw-thin) solid transparent",
                color: "var(--text-primary)",
              }}
            >
              <strong style={{ display: "block", fontSize: "var(--fs-sm)" }}>
                {t(proposal.titleKey)}
              </strong>
              <span style={{ color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
                {t(proposal.explanationKey, proposal.explanationParams)}
              </span>
            </button>
          ))}
          <div style={{ display: "flex", gap: "var(--space-xs)", flexWrap: "wrap" }}>
            {status === "applied" ? (
              <button type="button" data-action="undo" onClick={() => void undo()} style={secondaryButtonStyle}>
                <Icon icon={RotateCcw} size={12} />
                {t("inspector.aiEdit.undo")}
              </button>
            ) : (
              <>
                <button
                  type="button"
                  data-action="apply"
                  disabled={status !== "review"}
                  onClick={() => void apply()}
                  style={primaryButtonStyle}
                >
                  <Icon icon={Sparkles} size={12} />
                  {status === "applying"
                    ? t("inspector.aiEdit.applying")
                    : t("inspector.aiEdit.apply")}
                </button>
                <button
                  type="button"
                  data-action="reject"
                  disabled={status !== "review"}
                  onClick={reject}
                  style={secondaryButtonStyle}
                >
                  <Icon icon={X} size={12} />
                  {t("inspector.aiEdit.reject")}
                </button>
              </>
            )}
          </div>
          {status === "applied" && (
            <div role="status" style={{ color: "var(--status-success)", fontSize: "var(--fs-xs)" }}>
              {t("inspector.aiEdit.applied")}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

const primaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  minHeight: 24,
  padding: "2px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  background: "var(--ai-gradient)",
  color: "#111",
  fontSize: "var(--fs-xs)",
  fontWeight: "var(--fw-semibold)",
} as const;

const secondaryButtonStyle = {
  display: "inline-flex",
  alignItems: "center",
  gap: 4,
  minHeight: 24,
  padding: "2px var(--space-sm)",
  borderRadius: "var(--radius-sm)",
  border: "var(--bw-thin) solid var(--border-primary)",
  background: "var(--bg-prominent)",
  color: "var(--text-primary)",
  fontSize: "var(--fs-xs)",
} as const;
