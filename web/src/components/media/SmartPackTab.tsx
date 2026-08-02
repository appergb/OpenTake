import { useState } from "react";
import { useT } from "../../i18n";
import { RADIUS, SPACE } from "../../lib/theme";
import { AvatarGenerationTab, VoiceCloneTab } from "./IdentityGenerationTabs";
import { ScriptToVideoTab } from "./ScriptToVideoTab";

type Mode = "script" | "avatar" | "voice";

export function SmartPackTab() {
  const t = useT();
  const [mode, setMode] = useState<Mode>("script");
  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", minHeight: 0 }}>
      <div role="tablist" aria-label={t("smartPack.workflows")} style={{ display: "flex", gap: SPACE.xs, padding: `${SPACE.sm}px ${SPACE.mdLg}px`, borderBottom: "var(--bw-thin) solid var(--border-subtle)" }}>
        {(["script", "avatar", "voice"] as const).map((value) => (
          <button key={value} type="button" role="tab" aria-selected={mode === value} onClick={() => setMode(value)} style={{ padding: `5px ${SPACE.md}px`, borderRadius: RADIUS.sm, background: mode === value ? "var(--bg-selected)" : "transparent", color: mode === value ? "var(--text-primary)" : "var(--text-tertiary)", fontSize: "var(--fs-xs)" }}>
            {t(`smartPack.${value}`)}
          </button>
        ))}
      </div>
      <div style={{ flex: 1, minHeight: 0 }}>
        {mode === "script" ? <ScriptToVideoTab /> : mode === "avatar" ? <AvatarGenerationTab /> : <VoiceCloneTab />}
      </div>
    </div>
  );
}
