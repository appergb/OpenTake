/**
 * Agent panel (HANDOFF §3.3). The in-app chat surface: a message list (user
 * right / assistant left / tool-call cards inline), a streaming input, and a
 * cancel button. Streams over `chat_delta` / `chat_tool_call` / `chat_done`;
 * tool calls reuse the Rust `Dispatcher` (the same 44-tool pipeline the MCP
 * server exposes), so a "tighten the silences" message lands on the exact same
 * `tighten_silences` → `ripple_delete_ranges` path an external agent would use.
 *
 * No-key path: the backend returns a guided assistant message; the panel
 * detects it and renders an "Open Settings" affordance.
 */

import { useEffect, useRef, useState } from "react";
import { Send, Square, Trash2, Settings as SettingsIcon, ChevronDown, ChevronRight, Wrench } from "lucide-react";
import { useT } from "../../i18n";
import { useChatStore, mintSessionId } from "../../store/chatStore";
import { useEditorUiStore } from "../../store/uiStore";
import {
  isTauri,
  chatSend,
  chatCancel,
  chatHistory,
  onChatDelta,
  onChatToolCall,
  onChatDone,
} from "../../lib/api";
import type { ChatMessage, ChatToolCall } from "../../lib/types";

const NO_KEY_HINT = /no API key|Settings/i;

export function AgentPanel() {
  const t = useT();
  const setView = useEditorUiStore((s) => s.setView);

  const sessionId = useChatStore((s) => s.sessionId);
  const messages = useChatStore((s) => s.messages);
  const streaming = useChatStore((s) => s.streaming);
  const pushUser = useChatStore((s) => s.pushUser);
  const beginStream = useChatStore((s) => s.beginStream);
  const appendDelta = useChatStore((s) => s.appendDelta);
  const upsertToolCall = useChatStore((s) => s.upsertToolCall);
  const finalize = useChatStore((s) => s.finalize);
  const setMessages = useChatStore((s) => s.setMessages);
  const reset = useChatStore((s) => s.reset);

  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  // Subscribe to the three streaming events for the lifetime of the panel.
  useEffect(() => {
    let unDelta: () => void = () => {};
    let unTool: () => void = () => {};
    let unDone: () => void = () => {};
    (async () => {
      unDelta = await onChatDelta((e) => {
        if (e.sessionId === useChatStore.getState().sessionId) appendDelta(e.delta);
      });
      unTool = await onChatToolCall((e) => {
        if (e.sessionId === useChatStore.getState().sessionId) upsertToolCall(e.toolCall);
      });
      unDone = await onChatDone((e) => {
        if (e.sessionId === useChatStore.getState().sessionId) finalize(e.message);
      });
    })();
    return () => {
      unDelta();
      unTool();
      unDone();
    };
  }, [appendDelta, upsertToolCall, finalize]);

  // Load persisted history on mount (a re-mount after navigating away + back).
  useEffect(() => {
    if (!isTauri) return;
    chatHistory(sessionId).then(setMessages).catch(() => {});
  }, [sessionId, setMessages]);

  // Auto-scroll to the bottom on new content.
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages]);

  async function send() {
    const text = input.trim();
    if (!text || streaming) return;
    setInput("");
    pushUser(text);
    // Mint a placeholder assistant id so deltas have a target before the
    // backend emits its own id via `chat_done`.
    const placeholder = `a-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    beginStream(placeholder);
    try {
      await chatSend(sessionId, text);
    } catch (e) {
      finalize({
        id: placeholder,
        role: "assistant",
        content: `⚠️ ${e instanceof Error ? e.message : String(e)}`,
        toolCalls: [],
        createdAt: Date.now(),
      });
    }
  }

  function cancel() {
    chatCancel(sessionId).catch(() => {});
  }

  function clearChat() {
    const newId = mintSessionId();
    reset(newId);
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  return (
    <div
      style={{
        height: "100%",
        width: "100%",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        background: "var(--bg-panel)",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "var(--space-sm) var(--space-md)",
          borderBottom: "1px solid var(--border-subtle)",
          flexShrink: 0,
        }}
      >
        <span style={{ fontSize: "var(--fs-sm)", fontWeight: 600, color: "var(--text)" }}>
          {t("agent.title")}
        </span>
        <button
          onClick={clearChat}
          disabled={streaming}
          title={t("agent.clear")}
          style={{
            background: "transparent",
            border: "none",
            cursor: streaming ? "not-allowed" : "pointer",
            opacity: streaming ? 0.4 : 1,
            color: "var(--text-muted)",
            display: "flex",
            alignItems: "center",
            padding: 4,
            borderRadius: 6,
          }}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {/* Message list */}
      <div
        ref={scrollRef}
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          padding: "var(--space-md)",
          display: "flex",
          flexDirection: "column",
          gap: "var(--space-sm)",
        }}
      >
        {messages.length === 0 && !streaming && (
          <div
            style={{
              color: "var(--text-muted)",
              fontSize: "var(--fs-sm)",
              textAlign: "center",
              marginTop: "var(--space-lg)",
              padding: "0 var(--space-md)",
            }}
          >
            {isTauri
              ? t("agent.inputPlaceholder")
              : "Agent requires the desktop app (LLM + tool dispatch)."}
          </div>
        )}
        {messages.map((m) => (
          <MessageRow key={m.id} message={m} onOpenSettings={() => setView("settings")} />
        ))}
      </div>

      {/* Input */}
      <div
        style={{
          borderTop: "1px solid var(--border-subtle)",
          padding: "var(--space-sm) var(--space-md)",
          display: "flex",
          gap: "var(--space-sm)",
          alignItems: "flex-end",
          flexShrink: 0,
        }}
      >
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={t("agent.inputPlaceholder")}
          rows={1}
          style={{
            flex: 1,
            resize: "none",
            border: "1px solid var(--border-subtle)",
            borderRadius: 8,
            padding: "var(--space-sm) var(--space-md)",
            fontFamily: "inherit",
            fontSize: "var(--fs-sm)",
            color: "var(--text)",
            background: "var(--bg-input)",
            maxHeight: 120,
            outline: "none",
          }}
        />
        {streaming ? (
          <button
            onClick={cancel}
            title={t("agent.cancel")}
            style={btnStyle("var(--danger)", "var(--danger)")}
          >
            <Square size={14} />
          </button>
        ) : (
          <button
            onClick={send}
            disabled={!input.trim()}
            title={t("agent.send")}
            style={{
              ...btnStyle("var(--accent)", "var(--accent)"),
              opacity: input.trim() ? 1 : 0.4,
              cursor: input.trim() ? "pointer" : "not-allowed",
            }}
          >
            <Send size={14} />
          </button>
        )}
      </div>
    </div>
  );
}

function btnStyle(_fg: string, bg: string): React.CSSProperties {
  return {
    background: bg,
    color: "#fff",
    border: "none",
    borderRadius: 8,
    padding: "var(--space-sm)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    cursor: "pointer",
    flexShrink: 0,
  };
}

function MessageRow({
  message,
  onOpenSettings,
}: {
  message: ChatMessage;
  onOpenSettings: () => void;
}) {
  const t = useT();
  const isUser = message.role === "user";
  const isAssistant = message.role === "assistant";
  const isTool = message.role === "tool";
  const guided = isAssistant && NO_KEY_HINT.test(message.content);

  if (isTool) {
    // Tool-result messages are rendered inline on the assistant turn that
    // requested them (via toolCalls cards); a bare tool message is rare and
    // rendered as a muted system note.
    return (
      <div
        style={{
          alignSelf: "center",
          maxWidth: "80%",
          fontSize: "var(--fs-xs)",
          color: "var(--text-muted)",
          background: "var(--bg-elevated)",
          borderRadius: 6,
          padding: "2px var(--space-sm)",
        }}
      >
        {message.content.slice(0, 200)}
      </div>
    );
  }

  return (
    <div
      style={{
        alignSelf: isUser ? "flex-end" : "flex-start",
        maxWidth: "85%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xs)",
      }}
    >
      <div
        style={{
          background: isUser ? "var(--accent)" : "var(--bg-elevated)",
          color: isUser ? "#fff" : "var(--text)",
          borderRadius: 10,
          padding: "var(--space-sm) var(--space-md)",
          fontSize: "var(--fs-sm)",
          lineHeight: 1.4,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {message.content || (isAssistant && message.toolCalls.length ? "" : "…")}
      </div>
      {isAssistant &&
        message.toolCalls.map((tc) => (
          <ToolCallCard key={tc.id} tc={tc} />
        ))}
      {guided && (
        <button
          onClick={onOpenSettings}
          style={{
            alignSelf: "flex-start",
            background: "transparent",
            border: "1px solid var(--accent)",
            color: "var(--accent)",
            borderRadius: 8,
            padding: "var(--space-xs) var(--space-sm)",
            fontSize: "var(--fs-xs)",
            cursor: "pointer",
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
          }}
        >
          <SettingsIcon size={12} />
          {t("agent.openSettings")}
        </button>
      )}
    </div>
  );
}

function ToolCallCard({ tc }: { tc: ChatToolCall }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const isError = tc.isError === true;
  const argsJson = JSON.stringify(tc.args, null, 2);
  const resultJson =
    tc.result == null ? "" : JSON.stringify(tc.result, null, 2);

  return (
    <div
      style={{
        background: "var(--bg-input)",
        border: `1px solid ${isError ? "var(--danger)" : "var(--border-subtle)"}`,
        borderRadius: 8,
        padding: "var(--space-xs) var(--space-sm)",
        fontSize: "var(--fs-xs)",
        alignSelf: "flex-start",
        maxWidth: "100%",
      }}
    >
      <button
        onClick={() => setOpen((o) => !o)}
        style={{
          background: "transparent",
          border: "none",
          cursor: "pointer",
          color: isError ? "var(--danger)" : "var(--text)",
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: 0,
          fontFamily: "inherit",
          fontSize: "inherit",
        }}
      >
        {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        <Wrench size={12} />
        <span style={{ fontWeight: 600 }}>{tc.name}</span>
        {tc.result == null && (
          <span style={{ color: "var(--text-muted)", marginLeft: 4 }}>…</span>
        )}
        {isError && (
          <span style={{ color: "var(--danger)", marginLeft: 4 }}>error</span>
        )}
      </button>
      {open && (
        <div
          style={{
            marginTop: "var(--space-xs)",
            display: "flex",
            flexDirection: "column",
            gap: "var(--space-xs)",
          }}
        >
          <div>
            <div style={{ color: "var(--text-muted)", marginBottom: 2 }}>
              {t("agent.toolArgs")}
            </div>
            <pre style={preStyle}>{argsJson}</pre>
          </div>
          {resultJson && (
            <div>
              <div style={{ color: "var(--text-muted)", marginBottom: 2 }}>
                {t("agent.toolResult")}
              </div>
              <pre style={preStyle}>{resultJson}</pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

const preStyle: React.CSSProperties = {
  margin: 0,
  padding: "var(--space-xs)",
  background: "var(--bg-panel)",
  borderRadius: 4,
  fontSize: "var(--fs-xs)",
  fontFamily: "var(--font-mono, monospace)",
  color: "var(--text)",
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 200,
  overflowY: "auto",
};
