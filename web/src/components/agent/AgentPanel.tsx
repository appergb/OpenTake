import {
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  ChevronDown,
  ChevronRight,
  Clapperboard,
  MessageSquare,
  Plus,
  Send,
  Settings as SettingsIcon,
  Square,
  Wrench,
  X,
} from "lucide-react";
import { useT } from "../../i18n";
import {
  chatCancel,
  chatSend,
  chatSessionSetOpen,
  chatSessions,
  isTauri,
  onChatDelta,
  onChatDone,
  onChatToolCall,
} from "../../lib/api";
import type { ChatMessage, ChatSession, ChatToolCall } from "../../lib/types";
import { useSettingsStore } from "../../store/settingsStore";
import { mintSessionId, useChatStore } from "../../store/chatStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { MotionPanel } from "./MotionPanel";

const NO_KEY_HINT = /Settings|设置|API key/i;

export function AgentPanel() {
  const t = useT();
  const provider = useSettingsStore((state) => state.byokProvider);
  const setSettingsOpen = useEditorUiStore((state) => state.setSettingsOpen);
  const projectEpoch = useProjectStore((state) => state.projectEpoch);
  const projectPath = useProjectStore((state) => state.projectPath);

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
  const composerDraft = useChatStore((state) => state.composerDraft);
  const setComposerDraft = useChatStore((state) => state.setComposerDraft);

  const [input, setInput] = useState("");
  const [panelMode, setPanelMode] = useState<"chat" | "motion">("chat");
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const sessionsRef = useRef<ChatSession[]>([]);
  const tabMutationRef = useRef<Promise<void>>(Promise.resolve());
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (composerDraft === null) return;
    setInput(composerDraft);
    setComposerDraft(null);
  }, [composerDraft, setComposerDraft]);

  function commitSessions(next: ChatSession[]) {
    sessionsRef.current = next;
    setSessions(next);
  }

  function updateSessions(update: (current: ChatSession[]) => ChatSession[]) {
    commitSessions(update(sessionsRef.current));
  }

  function enqueueTabMutation(operation: () => Promise<void>) {
    const pending = tabMutationRef.current.then(operation, operation);
    tabMutationRef.current = pending.then(
      () => undefined,
      () => undefined,
    );
  }

  useEffect(() => {
    let unDelta: () => void = () => {};
    let unTool: () => void = () => {};
    let unDone: () => void = () => {};
    void (async () => {
      unDelta = await onChatDelta((event) => {
        if (
          event.projectEpoch === useProjectStore.getState().projectEpoch &&
          event.projectPath === useProjectStore.getState().projectPath &&
          event.sessionId === useChatStore.getState().sessionId
        ) {
          appendDelta(event.delta);
        }
      });
      unTool = await onChatToolCall((event) => {
        if (
          event.projectEpoch === useProjectStore.getState().projectEpoch &&
          event.projectPath === useProjectStore.getState().projectPath &&
          event.sessionId === useChatStore.getState().sessionId
        ) {
          upsertToolCall(event.toolCall);
        }
      });
      unDone = await onChatDone((event) => {
        if (
          event.projectEpoch === useProjectStore.getState().projectEpoch &&
          event.projectPath === useProjectStore.getState().projectPath &&
          event.sessionId === useChatStore.getState().sessionId
        ) {
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
    const freshSessionId = mintSessionId();
    reset(freshSessionId);
    commitSessions([]);
    if (!isTauri || !projectPath) return;
    let disposed = false;
    const loadingEpoch = projectEpoch;
    const loadingPath = projectPath;
    void chatSessions(loadingEpoch, loadingPath)
      .then((projectSessions) => {
        if (disposed) return;
        const project = useProjectStore.getState();
        const chat = useChatStore.getState();
        if (
          project.projectEpoch !== loadingEpoch ||
          project.projectPath !== loadingPath ||
          chat.sessionId !== freshSessionId ||
          chat.messages.length !== 0 ||
          chat.streaming
        ) {
          return;
        }
        const openSessions = projectSessions.filter((session) => session.isOpen !== false);
        commitSessions(openSessions);
        const latest = openSessions[0];
        if (latest) {
          reset(latest.id);
          setMessages(latest.messages);
        } else {
          const optimistic: ChatSession = {
            id: freshSessionId,
            messages: [],
            createdAt: Date.now(),
            isOpen: true,
          };
          commitSessions([optimistic]);
          enqueueTabMutation(async () => {
            if (disposed) return;
            try {
              const created = await chatSessionSetOpen(
                freshSessionId,
                true,
                loadingEpoch,
                loadingPath,
              );
              if (disposed) return;
              const currentProject = useProjectStore.getState();
              if (
                currentProject.projectEpoch === loadingEpoch &&
                currentProject.projectPath === loadingPath &&
                useChatStore.getState().sessionId === freshSessionId
              ) {
                updateSessions((current) =>
                  current.map((session) =>
                    session.id === freshSessionId ? created : session,
                  ),
                );
              }
            } catch {
              // Keep the local empty tab; sending a message will surface a
              // project persistence error through the normal chat path.
            }
          });
        }
      })
      .catch(() => {});
    return () => {
      disposed = true;
    };
  }, [projectEpoch, projectPath, reset, setMessages]);

  useEffect(() => {
    updateSessions((current) =>
      current.map((session) =>
        session.id === sessionId ? { ...session, messages } : session,
      ),
    );
  }, [messages, sessionId]);

  useEffect(() => {
    const element = scrollRef.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [messages]);

  async function send() {
    const text = input.trim();
    if (!text || streaming || !projectPath) return;
    const sendingEpoch = projectEpoch;
    const sendingPath = projectPath;
    const sendingSessionId = sessionId;
    setInput("");
    pushUser(text);
    const placeholderId = `assistant-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    beginStream(placeholderId);
    try {
      await chatSend(sendingSessionId, text, provider, sendingEpoch, sendingPath);
    } catch (error) {
      const project = useProjectStore.getState();
      const chat = useChatStore.getState();
      if (
        project.projectEpoch !== sendingEpoch ||
        project.projectPath !== sendingPath ||
        chat.sessionId !== sendingSessionId
      ) {
        return;
      }
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
    if (!projectPath) return;
    void chatCancel(sessionId, projectEpoch, projectPath).catch(() => {});
  }

  function openSession(session: ChatSession) {
    if (streaming || session.id === sessionId || !projectPath) return;
    reset(session.id);
    setMessages(session.messages);
  }

  function newChat() {
    if (streaming || !projectPath) return;
    const openingEpoch = projectEpoch;
    const openingPath = projectPath;
    enqueueTabMutation(() => createNewChatNow(openingEpoch, openingPath));
  }

  async function createNewChatNow(openingEpoch: number, openingPath: string) {
    const project = useProjectStore.getState();
    if (project.projectEpoch !== openingEpoch || project.projectPath !== openingPath) return;
    const createdId = mintSessionId();
    const optimistic: ChatSession = {
      id: createdId,
      messages: [],
      createdAt: Date.now(),
      isOpen: true,
    };
    reset(createdId);
    updateSessions((current) => [optimistic, ...current]);
    try {
      const persisted = await chatSessionSetOpen(
        createdId,
        true,
        openingEpoch,
        openingPath,
      );
      const currentProject = useProjectStore.getState();
      if (
        currentProject.projectEpoch === openingEpoch &&
        currentProject.projectPath === openingPath
      ) {
        updateSessions((current) =>
          current.map((session) => (session.id === createdId ? persisted : session)),
        );
      }
    } catch {
      // Keep the reversible local tab available; the next message surfaces any
      // project persistence failure through the existing chat error path.
    }
  }

  function closeChat(session: ChatSession) {
    if (streaming || !projectPath) return;
    const closingEpoch = projectEpoch;
    const closingPath = projectPath;
    enqueueTabMutation(() => closeChatNow(session.id, closingEpoch, closingPath));
  }

  async function closeChatNow(
    closingSessionId: string,
    closingEpoch: number,
    closingPath: string,
  ) {
    const before = useProjectStore.getState();
    if (before.projectEpoch !== closingEpoch || before.projectPath !== closingPath) return;
    try {
      await chatSessionSetOpen(closingSessionId, false, closingEpoch, closingPath);
    } catch {
      return;
    }
    const project = useProjectStore.getState();
    if (project.projectEpoch !== closingEpoch || project.projectPath !== closingPath) return;
    const remaining = sessionsRef.current.filter(
      (candidate) => candidate.id !== closingSessionId,
    );
    commitSessions(remaining);
    if (closingSessionId !== useChatStore.getState().sessionId) return;
    const next = remaining[0];
    if (next) {
      reset(next.id);
      setMessages(next.messages);
    } else {
      await createNewChatNow(closingEpoch, closingPath);
    }
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
          {panelMode === "chat" ? t("agent.title") : t("motion.heading")}
        </span>
        {panelMode === "chat" && <button
          type="button"
          onClick={() => void newChat()}
          disabled={streaming || !projectPath}
          title={t("agent.newTab")}
          aria-label={t("agent.newTab")}
          className="hover-area"
          style={{
            width: 26,
            height: 26,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: "var(--radius-sm)",
            color: "var(--text-secondary)",
            opacity: streaming || !projectPath ? 0.4 : 1,
          }}
        >
          <Plus size={14} />
        </button>}
      </div>

      <div
        role="group"
        aria-label={t("agent.modes")}
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 3,
          padding: "var(--space-xs) var(--space-sm)",
          borderBottom: "var(--bw-hairline) solid var(--border-subtle)",
          flexShrink: 0,
        }}
      >
        <PanelModeButton
          active={panelMode === "chat"}
          label={t("agent.chatMode")}
          icon={<MessageSquare size={13} />}
          onClick={() => setPanelMode("chat")}
        />
        <PanelModeButton
          active={panelMode === "motion"}
          label={t("agent.motionMode")}
          icon={<Clapperboard size={13} />}
          onClick={() => setPanelMode("motion")}
        />
      </div>

      {panelMode === "chat" ? <>
      <div
        role="tablist"
        aria-label={t("agent.tabs")}
        style={{
          display: "flex",
          gap: 2,
          overflowX: "auto",
          padding: "var(--space-xs) var(--space-sm)",
          borderBottom: "var(--bw-hairline) solid var(--border-subtle)",
          flexShrink: 0,
        }}
      >
        {sessions.map((session, index) => {
          const title = sessionTitle(session, `${t("agent.newChat")} ${index + 1}`);
          const active = session.id === sessionId;
          return (
            <div
              key={session.id}
              style={{
                display: "inline-flex",
                alignItems: "center",
                minWidth: 0,
                borderRadius: "var(--radius-sm)",
                background: active ? "var(--bg-elevated)" : "transparent",
              }}
            >
              <button
                type="button"
                role="tab"
                aria-selected={active}
                aria-label={title}
                disabled={streaming}
                onClick={() => openSession(session)}
                style={{
                  maxWidth: 120,
                  height: 24,
                  padding: "0 4px 0 var(--space-sm)",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  color: active ? "var(--text-primary)" : "var(--text-muted)",
                  fontSize: "var(--fs-xs)",
                }}
              >
                {title}
              </button>
              <button
                type="button"
                aria-label={`${t("agent.closeTab")} ${title}`}
                disabled={streaming}
                onClick={() => void closeChat(session)}
                className="hover-area"
                style={{
                  width: 24,
                  height: 24,
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  borderRadius: "var(--radius-xs)",
                  color: "var(--text-muted)",
                  opacity: streaming ? 0.4 : 1,
                }}
              >
                <X size={11} />
              </button>
            </div>
          );
        })}
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
      </> : <MotionPanel />}
    </div>
  );
}

function PanelModeButton({
  active,
  label,
  icon,
  onClick,
}: {
  active: boolean;
  label: string;
  icon: ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      style={{
        height: 27,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        gap: 6,
        borderRadius: "var(--radius-sm)",
        background: active ? "var(--bg-elevated)" : "transparent",
        color: active ? "var(--text-primary)" : "var(--text-muted)",
        fontSize: "var(--fs-xs)",
      }}
    >
      {icon}
      {label}
    </button>
  );
}

function sessionTitle(session: ChatSession, fallback: string): string {
  const firstUserMessage = session.messages.find(
    (message) => message.role === "user" && message.content.trim().length > 0,
  );
  if (!firstUserMessage) return fallback;
  const compact = firstUserMessage.content.trim().replace(/\s+/g, " ");
  return compact.length > 20 ? `${compact.slice(0, 20)}…` : compact;
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
