/**
 * TextTab (SPEC §6.3). Inspector tab for text clips. MVP: edits `textContent`
 * only — `textStyle` (fontSize/color/align) requires a backend extension to
 * `ClipPropertiesReq` and is left for a follow-up. Commits on blur via
 * SetClipProperties.
 */

import { useEffect, useState } from "react";
import * as edit from "../../store/editActions";
import type { TFunction } from "../../i18n";
import type { Clip } from "../../lib/types";

export function TextTab({ clip, t }: { clip: Clip; t: TFunction }) {
  const [value, setValue] = useState(clip.textContent ?? "");

  // Reset local state when the selected clip changes.
  useEffect(() => {
    setValue(clip.textContent ?? "");
  }, [clip.id, clip.textContent]);

  const commit = () => {
    if (value === (clip.textContent ?? "")) return;
    void edit.setClipProperties([clip.id], { textContent: value });
  };

  return (
    <section>
      <div style={{ marginBottom: "var(--space-sm)", fontSize: "var(--fs-xxs)", fontWeight: "var(--fw-semibold)", letterSpacing: "var(--tracking-wide)", color: "var(--text-muted)", textTransform: "uppercase" }}>
        {t("inspector.section.text")}
      </div>
      <textarea
        value={value}
        placeholder={t("inspector.textPlaceholder")}
        onChange={(e) => setValue(e.target.value)}
        onBlur={commit}
        rows={4}
        style={{
          width: "100%",
          resize: "vertical",
          minHeight: 80,
          padding: "var(--space-sm)",
          fontSize: "var(--fs-sm)",
          color: "var(--text-primary)",
          background: "var(--bg-elevated)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: 4,
          fontFamily: "var(--font-sans)",
          outline: "none",
        }}
      />
    </section>
  );
}
