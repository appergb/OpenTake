/**
 * Shared UI primitives for Settings panes. Extracted from SettingsView.tsx
 * to keep each pane file focused and the main view ≤ 300 lines (Issue #40
 * review — "SettingsView.tsx > 800 行规约").
 *
 * Exports: Section, Field, Segmented, CodeBlock, Subsection, Value, PaneId.
 */

import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { Icon } from "../ui/Icon";
import { useT } from "../../i18n";

export type PaneId =
  | "general"
  | "appearance"
  | "import"
  | "ai"
  | "mcp"
  | "storage"
  | "notifications"
  | "about"
  | "models"
  | "privacy"
  | "shortcuts"
  | "account";

export function Section({ title, children }: { title: string; children: React.ReactNode }) {
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
      <div
        style={{
          background: "var(--bg-raised)",
          border: "var(--bw-thin) solid var(--border-primary)",
          borderRadius: "var(--radius-md)",
          padding: "var(--space-md) var(--space-lg)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-lg)",
        }}
      >
        {children}
      </div>
    </section>
  );
}

export function Field({
  label,
  description,
  control,
}: {
  label: string;
  description?: string;
  control: React.ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "var(--space-lg)",
        justifyContent: "space-between",
      }}
    >
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)" }}>{label}</div>
        {description && (
          <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)", marginTop: 2 }}>
            {description}
          </div>
        )}
      </div>
      <div style={{ flex: "0 0 auto" }}>{control}</div>
    </div>
  );
}

/** Segmented control used for enum settings (language/theme). */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: Array<{ id: T; label: string }>;
  onChange: (id: T) => void;
}) {
  return (
    <div
      style={{
        display: "inline-flex",
        padding: 2,
        gap: 2,
        background: "var(--bg-base)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-sm)",
      }}
    >
      {options.map((opt) => {
        const active = opt.id === value;
        return (
          <button
            key={opt.id}
            type="button"
            onClick={() => onChange(opt.id)}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 24,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-xs-sm)",
              background: active ? "var(--bg-prominent)" : "transparent",
              color: active ? "var(--text-primary)" : "var(--text-tertiary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {active && <Icon icon={Check} size={11} />}
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}

export function Subsection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-xs)" }}>
      <div style={{ fontSize: "var(--fs-md)", color: "var(--text-primary)", fontWeight: "var(--fw-medium)" }}>
        {label}
      </div>
      {children}
    </div>
  );
}

/** Read-only code block with optional copy button. Used for MCP commands and
 *  JSON configs. */
export function CodeBlock({ text, onCopy }: { text: string; onCopy?: () => void }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const handleCopy = () => {
    if (onCopy) {
      onCopy();
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  };
  return (
    <div
      style={{
        position: "relative",
        background: "var(--bg-base)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: "var(--radius-sm)",
        padding: "var(--space-sm) var(--space-md)",
        paddingRight: onCopy ? 64 : undefined,
      }}
    >
      <pre
        className="tabular"
        style={{
          margin: 0,
          fontSize: "var(--fs-xs)",
          color: "var(--text-secondary)",
          whiteSpace: "pre-wrap",
          wordBreak: "break-all",
          fontFamily: "var(--font-mono, ui-monospace, monospace)",
        }}
      >
        {text}
      </pre>
      {onCopy && (
        <button
          type="button"
          onClick={handleCopy}
          className="hover-area"
          title={t("mcp.copy")}
          aria-label={t("mcp.copy")}
          style={{
            position: "absolute",
            top: 4,
            right: 4,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            width: 22,
            height: 22,
            borderRadius: "var(--radius-xs)",
            color: "var(--text-tertiary)",
          }}
        >
          <Icon icon={copied ? Check : Copy} size={11} />
        </button>
      )}
    </div>
  );
}

export function Value({ children }: { children: React.ReactNode }) {
  return (
    <span className="tabular" style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-secondary)" }}>
      {children}
    </span>
  );
}