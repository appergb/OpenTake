import { useEffect, useMemo, useRef, useState } from "react";
import { useT } from "../../i18n";
import * as api from "../../lib/api";
import { assetUrl } from "../../lib/asset";
import { RADIUS, SPACE } from "../../lib/theme";
import type { AvatarGenerationResult, VoiceCloneResult, VoiceModelRecord } from "../../lib/types";
import * as edit from "../../store/editActions";
import { useMediaStore } from "../../store/mediaStore";
import { useProjectStore } from "../../store/projectStore";

type Phase = "idle" | "running" | "ready" | "applied";

const EMPTY_VOICE_MODELS: readonly VoiceModelRecord[] = [];

function consentId(scope: string): string {
  const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `consent-${scope}-${id}`;
}

export interface AvatarDependencies {
  run: typeof api.generateAvatar;
  cancel: typeof api.cancelAdvancedWorkflow;
  undo: typeof edit.undo;
}

const avatarDefaults: AvatarDependencies = { run: api.generateAvatar, cancel: api.cancelAdvancedWorkflow, undo: edit.undo };

export function AvatarGenerationTab({ dependencies = avatarDefaults }: { dependencies?: AvatarDependencies }) {
  const t = useT();
  const items = useMediaStore((state) => state.items);
  const portraits = useMemo(() => items.filter((item) => item.type === "image"), [items]);
  const audio = useMemo(() => items.filter((item) => item.type === "audio" && item.hasAudio), [items]);
  const [portraitMediaRef, setPortrait] = useState("");
  const [audioMediaRef, setAudio] = useState("");
  const [startFrame, setStartFrame] = useState(0);
  const [consent, setConsent] = useState("");
  const [costAuthorized, setCostAuthorized] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [result, setResult] = useState<AvatarGenerationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);

  useEffect(() => { if (!portraits.some((item) => item.id === portraitMediaRef)) setPortrait(portraits[0]?.id ?? ""); }, [portraits, portraitMediaRef]);
  useEffect(() => { if (!audio.some((item) => item.id === audioMediaRef)) setAudio(audio[0]?.id ?? ""); }, [audio, audioMediaRef]);

  const run = async () => {
    const id = ++operation.current;
    setPhase("running"); setError(null);
    try {
      const next = await dependencies.run({ portraitMediaRef, audioMediaRef, consentId: consent, costAuthorized, startFrame });
      if (operation.current !== id) return;
      setResult(next); setPhase("applied");
    } catch (reason) {
      if (operation.current !== id) return;
      setError(reason instanceof Error ? reason.message : String(reason)); setPhase("idle");
    }
  };
  const cancel = async () => { operation.current += 1; await dependencies.cancel(); setPhase("idle"); };
  const undo = async () => { setPhase("running"); try { await dependencies.undo(); setResult(null); setPhase("idle"); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); setPhase("applied"); } };
  const busy = phase === "running";
  const preview = assetUrl(result?.result.previewPath);
  return (
    <div data-testid="avatar-generation-tab" style={rootStyle}>
      <p style={descriptionStyle}>{t("avatar.description")}</p>
      <label style={labelStyle}>{t("avatar.portrait")}<select aria-label={t("avatar.portrait")} value={portraitMediaRef} disabled={busy || phase === "applied"} onChange={(event) => setPortrait(event.currentTarget.value)} style={inputStyle}>{portraits.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label>
      <label style={labelStyle}>{t("avatar.audio")}<select aria-label={t("avatar.audio")} value={audioMediaRef} disabled={busy || phase === "applied"} onChange={(event) => setAudio(event.currentTarget.value)} style={inputStyle}>{audio.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label>
      <label style={labelStyle}>{t("avatar.startFrame")}<input aria-label={t("avatar.startFrame")} type="number" min={0} value={startFrame} disabled={busy || phase === "applied"} onChange={(event) => setStartFrame(Math.max(0, Number(event.currentTarget.value) || 0))} style={inputStyle} /></label>
      <label style={checkStyle}><input type="checkbox" checked={Boolean(consent)} disabled={busy || phase === "applied"} onChange={(event) => setConsent(event.currentTarget.checked ? consentId("avatar") : "")} />{t("avatar.consent")}</label>
      <label style={checkStyle}><input type="checkbox" checked={costAuthorized} disabled={busy || phase === "applied"} onChange={(event) => setCostAuthorized(event.currentTarget.checked)} />{t("avatar.cost")}</label>
      {preview && <video controls src={preview} aria-label={t("avatar.preview")} style={{ width: "100%", maxHeight: 180, borderRadius: RADIUS.md, background: "black" }} />}
      <div style={buttonRowStyle}>{phase === "applied" ? <button type="button" onClick={() => void undo()} style={primaryButtonStyle}>{t("avatar.undo")}</button> : <button type="button" disabled={busy || !portraitMediaRef || !audioMediaRef || !consent || !costAuthorized} onClick={() => void run()} style={primaryButtonStyle}>{busy ? t("avatar.progress") : error ? t("common.retry") : t("avatar.generate")}</button>}{busy && <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>{t("common.cancel")}</button>}</div>
      {result && <div role="status" style={successStyle}>{t("avatar.ready", { frames: result.result.durationFrames })}</div>}
      {error && <div role="alert" style={errorStyle}>{error}</div>}
    </div>
  );
}

export interface VoiceDependencies {
  run: typeof api.cloneVoice;
  cancel: typeof api.cancelAdvancedWorkflow;
  undo: typeof edit.undo;
}

const voiceDefaults: VoiceDependencies = { run: api.cloneVoice, cancel: api.cancelAdvancedWorkflow, undo: edit.undo };

export function VoiceCloneTab({ dependencies = voiceDefaults }: { dependencies?: VoiceDependencies }) {
  const t = useT();
  const items = useMediaStore((state) => state.items);
  const storedVoiceModels = useProjectStore((state) => state.timeline.voiceModels);
  const records = storedVoiceModels ?? EMPTY_VOICE_MODELS;
  const references = useMemo(() => items.filter((item) => item.type === "audio" && item.hasAudio), [items]);
  const [referenceAudioMediaRef, setReference] = useState("");
  const [voiceName, setVoiceName] = useState("");
  const [voiceId, setVoiceId] = useState("");
  const [prompt, setPrompt] = useState("");
  const [consent, setConsent] = useState("");
  const [costAuthorized, setCostAuthorized] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [result, setResult] = useState<VoiceCloneResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [locallyRevoked, setLocallyRevoked] = useState<string[]>([]);
  const operation = useRef(0);
  useEffect(() => { if (!references.some((item) => item.id === referenceAudioMediaRef)) setReference(references[0]?.id ?? ""); }, [references, referenceAudioMediaRef]);
  useEffect(() => { const available = records.find((record) => !record.revoked && !locallyRevoked.includes(record.id)); if (!voiceId && available) { setVoiceId(available.id); setConsent(available.consentId); } }, [records, locallyRevoked, voiceId]);
  const execute = async (request: Parameters<typeof api.cloneVoice>[0]) => {
    const id = ++operation.current; setPhase("running"); setError(null);
    try { const next = await dependencies.run(request); if (operation.current !== id) return; setResult(next); if (next.result.action === "enroll") setVoiceId(next.result.voiceId); if (next.result.action === "revoke") { setLocallyRevoked((current) => [...current, next.result.voiceId]); setVoiceId(""); } setPhase(next.result.action === "generate" ? "applied" : "ready"); }
    catch (reason) { if (operation.current !== id) return; setError(reason instanceof Error ? reason.message : String(reason)); setPhase("idle"); }
  };
  const cancel = async () => { operation.current += 1; await dependencies.cancel(); setPhase("idle"); };
  const undo = async () => { setPhase("running"); try { await dependencies.undo(); setResult(null); setPhase("ready"); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); setPhase("applied"); } };
  const busy = phase === "running";
  const activeRecord = records.find((record) => record.id === voiceId);
  const activeConsent = activeRecord?.consentId ?? consent;
  const preview = assetUrl(result?.result.previewPath);
  return (
    <div data-testid="voice-clone-tab" style={rootStyle}>
      <p style={descriptionStyle}>{t("voice.description")}</p>
      <label style={labelStyle}>{t("voice.reference")}<select aria-label={t("voice.reference")} value={referenceAudioMediaRef} disabled={busy} onChange={(event) => setReference(event.currentTarget.value)} style={inputStyle}>{references.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select></label>
      <label style={labelStyle}>{t("voice.name")}<input aria-label={t("voice.name")} value={voiceName} disabled={busy} onChange={(event) => setVoiceName(event.currentTarget.value)} style={inputStyle} /></label>
      <label style={checkStyle}><input type="checkbox" checked={Boolean(consent)} disabled={busy || Boolean(activeRecord)} onChange={(event) => setConsent(event.currentTarget.checked ? consentId("voice") : "")} />{t("voice.consent")}</label>
      <label style={checkStyle}><input type="checkbox" checked={costAuthorized} disabled={busy} onChange={(event) => setCostAuthorized(event.currentTarget.checked)} />{t("voice.cost")}</label>
      <button type="button" disabled={busy || !referenceAudioMediaRef || !voiceName.trim() || !consent || !costAuthorized} onClick={() => void execute({ action: "enroll", referenceAudioMediaRef, voiceName: voiceName.trim(), consentId: consent, costAuthorized })} style={secondaryButtonStyle}>{t("voice.enroll")}</button>
      <label style={labelStyle}>{t("voice.model")}<select aria-label={t("voice.model")} value={voiceId} disabled={busy} onChange={(event) => { const record = records.find((item) => item.id === event.currentTarget.value); setVoiceId(event.currentTarget.value); if (record) setConsent(record.consentId); }} style={inputStyle}><option value="">{t("voice.chooseModel")}</option>{records.filter((record) => !record.revoked && !locallyRevoked.includes(record.id)).map((record) => <option key={record.id} value={record.id}>{record.voiceName}</option>)}{result?.result.action === "enroll" && !records.some((record) => record.id === result.result.voiceId) && <option value={result.result.voiceId}>{result.result.voiceName}</option>}</select></label>
      <label style={labelStyle}>{t("voice.prompt")}<textarea aria-label={t("voice.prompt")} value={prompt} disabled={busy || phase === "applied"} onChange={(event) => setPrompt(event.currentTarget.value)} style={{ ...inputStyle, minHeight: 60 }} /></label>
      {preview && <audio controls src={preview} aria-label={t("voice.audition")} style={{ width: "100%" }} />}
      <div style={buttonRowStyle}>{phase === "applied" ? <button type="button" onClick={() => void undo()} style={primaryButtonStyle}>{t("voice.undo")}</button> : <button type="button" disabled={busy || !voiceId || !activeConsent || !prompt.trim() || !costAuthorized} onClick={() => void execute({ action: "generate", voiceId, prompt: prompt.trim(), consentId: activeConsent, costAuthorized })} style={primaryButtonStyle}>{busy ? t("voice.progress") : error ? t("common.retry") : t("voice.generate")}</button>}<button type="button" disabled={busy || !voiceId || !activeConsent} onClick={() => void execute({ action: "revoke", voiceId, consentId: activeConsent })} style={dangerButtonStyle}>{t("voice.revoke")}</button>{busy && <button type="button" onClick={() => void cancel()} style={secondaryButtonStyle}>{t("common.cancel")}</button>}</div>
      {result && <div role="status" style={successStyle}>{t(`voice.status.${result.result.action}`)}</div>}
      {error && <div role="alert" style={errorStyle}>{error}</div>}
    </div>
  );
}

const rootStyle = { height: "100%", overflowY: "auto", padding: SPACE.mdLg, display: "flex", flexDirection: "column", gap: SPACE.md } as const;
const descriptionStyle = { color: "var(--text-secondary)", fontSize: "var(--fs-sm)", margin: 0 } as const;
const labelStyle = { display: "flex", flexDirection: "column", gap: 3, color: "var(--text-tertiary)", fontSize: "var(--fs-xs)" } as const;
const checkStyle = { display: "flex", alignItems: "flex-start", gap: SPACE.sm, color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
const inputStyle = { width: "100%", boxSizing: "border-box", padding: "5px var(--space-sm)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "var(--fs-xs)" } as const;
const buttonRowStyle = { display: "flex", flexWrap: "wrap", gap: SPACE.sm } as const;
const primaryButtonStyle = { minHeight: 28, padding: "4px var(--space-md)", borderRadius: "var(--radius-sm)", background: "var(--ai-gradient)", color: "#111", fontSize: "var(--fs-xs)", fontWeight: "var(--fw-semibold)" } as const;
const secondaryButtonStyle = { minHeight: 28, padding: "4px var(--space-md)", borderRadius: "var(--radius-sm)", border: "var(--bw-thin) solid var(--border-primary)", background: "var(--bg-raised)", color: "var(--text-secondary)", fontSize: "var(--fs-xs)" } as const;
const dangerButtonStyle = { ...secondaryButtonStyle, color: "var(--status-error)" } as const;
const successStyle = { color: "var(--status-success)", fontSize: "var(--fs-xs)" } as const;
const errorStyle = { color: "var(--status-error)", fontSize: "var(--fs-xs)" } as const;
