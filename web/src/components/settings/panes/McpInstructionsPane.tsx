/**
 * MCP Instructions pane. Surfaces the built-in MCP server URL and one-line
 * install commands for Cursor / Claude Code / Codex / Claude Desktop.
 *
 * Extracted from SettingsView.tsx (Issue #40 review). Fixes:
 * - Cursor config: added `type: "http"` field (review #4)
 * - Claude Desktop: changed to `npx mcp-remote` format (review #5)
 */

import { useState } from "react";
import { Copy } from "lucide-react";
import { Icon } from "../../ui/Icon";
import { useT } from "../../../i18n";
import { Section, Field, CodeBlock, Subsection } from "../shared";

/** MCP server endpoint. The Rust server (`opentake-agent::mcp::server`) binds
 *  to 127.0.0.1:19789/mcp at startup; the URL is fixed so we hardcode it here
 *  rather than round-tripping to Tauri to query. */
const MCP_SERVER_URL = "http://127.0.0.1:19789/mcp";

export function McpInstructionsPane() {
  const t = useT();
  const [copied, setCopied] = useState(false);

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard may be unavailable (non-Tauri / permissions); silently no-op.
    }
  };

  const claudeCodeCmd = `claude mcp add --transport http opentake ${MCP_SERVER_URL}`;
  const codexCmd = `codex mcp add opentake --url ${MCP_SERVER_URL}`;
  // Fix #4: Cursor config with `type: "http"` field (reviewer 反馈).
  const cursorConfig = JSON.stringify(
    {
      mcpServers: {
        opentake: { type: "http", url: MCP_SERVER_URL },
      },
    },
    null,
    2,
  );
  // Fix #5: Claude Desktop uses `npx mcp-remote` (reviewer 反馈), not the
  // same url-based format as Cursor.
  const claudeDesktopConfig = JSON.stringify(
    {
      mcpServers: {
        opentake: {
          command: "npx",
          args: ["mcp-remote", MCP_SERVER_URL],
        },
      },
    },
    null,
    2,
  );

  return (
    <Section title={t("mcp.title")}>
      <div style={{ fontSize: "var(--fs-sm-md)", color: "var(--text-tertiary)", lineHeight: 1.5 }}>
        {t("mcp.overview")}
      </div>

      <Field
        label={t("mcp.serverUrl")}
        description={t("mcp.serverUrlDesc")}
        control={
          <button
            type="button"
            onClick={() => void copy(MCP_SERVER_URL)}
            className="hover-area"
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 4,
              height: 26,
              padding: "0 var(--space-md)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: "var(--text-secondary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            <Icon icon={Copy} size={12} />
            {copied ? t("mcp.copied") : t("mcp.copy")}
          </button>
        }
      />
      <CodeBlock text={MCP_SERVER_URL} />

      <Subsection label={t("mcp.claudeCode")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.claudeCodeCmd")}
        </div>
        <CodeBlock text={claudeCodeCmd} onCopy={() => void copy(claudeCodeCmd)} />
      </Subsection>

      <Subsection label={t("mcp.codex")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.codexCmd")}
        </div>
        <CodeBlock text={codexCmd} onCopy={() => void copy(codexCmd)} />
      </Subsection>

      <Subsection label={t("mcp.cursor")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.cursorManual")}
        </div>
        <CodeBlock text={cursorConfig} onCopy={() => void copy(cursorConfig)} />
      </Subsection>

      <Subsection label={t("mcp.claudeDesktop")}>
        <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-tertiary)" }}>
          {t("mcp.claudeDesktopManual")}
        </div>
        <CodeBlock text={claudeDesktopConfig} onCopy={() => void copy(claudeDesktopConfig)} />
      </Subsection>

      <div style={{ fontSize: "var(--fs-xs)", color: "var(--text-muted)" }}>
        {t("mcp.note")}
      </div>
    </Section>
  );
}