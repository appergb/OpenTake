import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import {
  ChevronDown,
  ChevronRight,
  Send,
  Settings as SettingsIcon,
  Square,
  Trash2,
  Wrench,
} from "lucide-react";
import { useT } from "../../i18n";
import {
  chatCancel,
  chatHistory,
  chatSend,
  isTauri,
  onChatDelta,
  onChatDone,
  onChatToolCall,
} from "../../lib/api";
import type { ChatMessage, ChatToolCall } from "../../lib/types";
import { useSettingsStore } from "../../store/settingsStore";
import { mintSessionId, useChatStore } from "../../store/chatStore";
import { useEditorUiStore } from "../../store/uiStore";

const NO_KEY_HINT = /Settings|设置|API key/i;

export function AgentPanel() {
  const t = useT();
  const provider = useSettingsStore((state) => state.byokProvider);
  const setSettingsOpen = useEditorUiStore((state) => state.setSettingsOpen);

  const sessionId = useChatStore((state) => state.sessionId);
  const messages = useChatStore((state) => state.messages);
  const streaming = useChatStore((state) => state.streaming);
  const pushUser = useChatStore((state) => state.pushUser);
  const beginStream = useChatStore((state) => state.beginStream);
  const appendDelta = useChatStore((state) => state.appendDelta);
  const upsertToolCall = useChatStore((state) => state.upsertToolCall);
  const finalize = useChatStore((state) => state.finalize);
  const setMessages = useChatStore((state) => state.setMessages);
  const reset = useChatStore((state) => state.reset);

  const [input, setInput] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unDelta: () => void = () => {};
    let unTool: () => void = () => {};
    let unDone: () => void = () => {};
    void (async () => {
      unDelta = await onChatDelta((event) => {
        if (event.sessionId === useChatStore.getState().sessionId) {
          appendDelta(event.delta);
        }
      });
      unTool = await onChatToolCall((event) => {
        if (event.sessionId === useChatStore.getState().sessionId) {
          upsertToolCall(event.toolCall);
        }
      });
      unDone = await onChatDone((event) => {
        if (event.sessionId === useChatStore.getState().sessionId) {
          finalize(event.message);
        }
      });
    })();
    return () => {
      unDelta();
      unTool();
      unDone();
    };
  }, [appendDelta, finalize, upsertToolCall]);

  useEffect(() => {
    if (!isTauri) return;
    void chatHistory(sessionId).then(setMessages).catch(() => {});
  }, [sessionId, setMessages]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [messages]);

  async function send() {
    const text = input.trim();
    if (!text || streaming) return;
    setInput("");
    pushUser(text);
    const placeholderId = `assistant-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    beginStream(placeholderId);
    try {
      await chatSend(sessionId, text, provider);
    } catch (error) {
      finalize({
        id: placeholderId,
        role: "assistant",
        content: `⚠️ ${error instanceof Error ? error.message : String(error)}`,
        toolCalls: [],
        createdAt: Date.now(),
      });
    }
  }

  function cancel() {
    void chatCancel(sessionId).catch(() => {});
  }

  function clearChat() {
    if (streaming) return;
    reset(mintSessionId());
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      void send();
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
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "var(--space-sm) var(--space-md)",
          borderBottom: "var(--bw-hairline) solid var(--border-subtle)",
          flexShrink: 0,
        }}
      >
        <span style={{ fontSize: "var(--fs-sm)", fontWeight: 600, color: "var(--text-primary)" }}>
          {t("agent.title")}
        </span>
        <button
          type="button"
          onClick={clearChat}
          disabled={streaming}
          title={t("agent.clear")}
          className="hover-area"
          style={{
            width: 26,
            height: 26,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: "var(--radius-sm)",
            color: "var(--text-secondary)",
            opacity: streaming ? 0.4 : 1,
          }}
        >
          <Trash2 size={14} />
        </button>
      </div>

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
            {isTauri ? t("agent.empty") : t("agent.desktopOnly")}
          </div>
        )}
        {messages.map((message) => (
          <MessageRow
            key={message.id}
            message={message}
            onOpenSettings={() => setSettingsOpen(true)}
          />
        ))}
      </div>

      <div
        style={{
          borderTop: "var(--bw-hairline) solid var(--border-subtle)",
          padding: "var(--space-sm) var(--space-md)",
          display: "flex",
          gap: "var(--space-sm)",
          alignItems: "flex-end",
          flexShrink: 0,
        }}
      >
        <textarea
          value={input}
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder={t("agent.inputPlaceholder")}
          disabled={!isTauri || streaming}
          rows={1}
          style={{
            flex: 1,
            resize: "none",
            border: "var(--bw-thin) solid var(--border-subtle)",
            borderRadius: "var(--radius-sm)",
            padding: "var(--space-sm) var(--space-md)",
            fontFamily: "inherit",
            fontSize: "var(--fs-sm)",
            color: "var(--text-primary)",
            background: "var(--bg-elevated)",
            minHeight: 34,
            maxHeight: 120,
            outline: "none",
            opacity: !isTauri ? 0.6 : 1,
          }}
        />
        {streaming ? (
          <button
            type="button"
            onClick={cancel}
            title={t("agent.cancel")}
            style={iconButtonStyle("var(--accent-spotlight)", "#fff")}
          >
            <Square size={14} />
          </button>
        ) : (
          <button
            type="button"
            onClick={() => void send()}
            disabled={!isTauri || !input.trim()}
            title={t("agent.send")}
            style={{
              ...iconButtonStyle("var(--accent-primary)", "#111"),
              opacity: isTauri && input.trim() ? 1 : 0.4,
              cursor: isTauri && input.trim() ? "pointer" : "not-allowed",
            }}
          >
            <Send size={14} />
          </button>
        )}
      </div>
    </div>
  );
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
    return (
      <div
        style={{
          alignSelf: "center",
          maxWidth: "80%",
          fontSize: "var(--fs-xs)",
          color: "var(--text-muted)",
          background: "var(--bg-elevated)",
          borderRadius: "var(--radius-sm)",
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
        maxWidth: "88%",
        display: "flex",
        flexDirection: "column",
        gap: "var(--space-xs)",
      }}
    >
      <div
        style={{
          background: isUser ? "var(--accent-primary)" : "var(--bg-elevated)",
          color: isUser ? "#111" : "var(--text-primary)",
          borderRadius: "var(--radius-sm)",
          padding: "var(--space-sm) var(--space-md)",
          fontSize: "var(--fs-sm)",
          lineHeight: 1.45,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {message.content || (isAssistant && message.toolCalls.length > 0 ? "" : "…")}
      </div>
      {isAssistant &&
        message.toolCalls.map((toolCall) => <ToolCallCard key={toolCall.id} toolCall={toolCall} />)}
      {guided && (
        <button
          type="button"
          onClick={onOpenSettings}
          className="hover-area"
          style={{
            alignSelf: "flex-start",
            display: "inline-flex",
            alignItems: "center",
            gap: 4,
            height: 26,
            padding: "0 var(--space-sm)",
            borderRadius: "var(--radius-sm)",
            border: "var(--bw-thin) solid var(--border-subtle)",
            color: "var(--text-secondary)",
            fontSize: "var(--fs-xs)",
          }}
        >
          <SettingsIcon size={12} />
          {t("agent.openSettings")}
        </button>
      )}
    </div>
  );
}

function ToolCallCard({ toolCall }: { toolCall: ChatToolCall }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const isError = toolCall.isError === true;
  const argsJson = JSON.stringify(toolCall.args, null, 2);
  const resultJson =
    toolCall.result == null ? "" : JSON.stringify(toolCall.result, null, 2);

  return (
    <div
      style={{
        background: "var(--bg-elevated)",
        border: `var(--bw-thin) solid ${
          isError ? "var(--accent-danger, #ff6b6b)" : "var(--border-subtle)"
        }`,
        borderRadius: "var(--radius-sm)",
        padding: "var(--space-xs) var(--space-sm)",
        fontSize: "var(--fs-xs)",
        alignSelf: "flex-start",
        maxWidth: "100%",
      }}
    >
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        style={{
          background: "transparent",
          border: "none",
          cursor: "pointer",
          color: isError ? "var(--accent-danger, #ff6b6b)" : "var(--text-primary)",
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
        <span style={{ fontWeight: 600 }}>{toolCall.name}</span>
        {toolCall.result == null && (
          <span style={{ color: "var(--text-muted)", marginLeft: 4 }}>…</span>
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
            <div style={{ color: "var(--text-muted)", marginBottom: 2 }}>{t("agent.toolArgs")}</div>
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

function iconButtonStyle(background: string, color: string): CSSProperties {
  return {
    width: 34,
    height: 34,
    border: "none",
    borderRadius: "var(--radius-sm)",
    background,
    color,
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    cursor: "pointer",
    flexShrink: 0,
  };
}

const preStyle: CSSProperties = {
  margin: 0,
  padding: "var(--space-xs)",
  background: "rgba(0, 0, 0, 0.18)",
  borderRadius: "var(--radius-sm)",
  fontSize: "var(--fs-xs)",
  fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
  color: "var(--text-secondary)",
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 200,
  overflowY: "auto",
};
