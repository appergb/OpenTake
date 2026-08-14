import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
import {
  ChevronDown,
  ChevronRight,
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
  chatHistory,
  chatSend,
  chatSessionSetOpen,
  chatSessions,
  isTauri,
  onChatDelta,
  onChatDone,
  onChatToolCall,
  type ChatStreamDecodeFailure,
  type ChatStreamIdentity,
} from "../../lib/api";
import type { AgentContentBlock, ChatMessage, ChatSession } from "../../lib/types";
import { useSettingsStore } from "../../store/settingsStore";
import { mintSessionId, useChatStore } from "../../store/chatStore";
import { useEditorUiStore } from "../../store/uiStore";
import { useProjectStore } from "../../store/projectStore";
import { Reveal } from "../ui/Reveal";

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
  const beginMessage = useChatStore((state) => state.beginMessage);
  const appendBlockDelta = useChatStore((state) => state.appendBlockDelta);
  const upsertBlock = useChatStore((state) => state.upsertBlock);
  const finalize = useChatStore((state) => state.finalize);
  const requestHistoryResync = useChatStore((state) => state.requestHistoryResync);
  const takeHistoryResyncRequest = useChatStore((state) => state.takeHistoryResyncRequest);
  const historyResyncRequests = useChatStore((state) => state.historyResyncRequests);
  const setMessagesForSession = useChatStore((state) => state.setMessagesForSession);
  const deleteSession = useChatStore((state) => state.deleteSession);
  const reset = useChatStore((state) => state.reset);
  const composerDraft = useChatStore((state) => state.composerDraft);
  const setComposerDraft = useChatStore((state) => state.setComposerDraft);

  const [input, setInput] = useState("");
  const [pendingSessionId, setPendingSessionId] = useState<string | null>(null);
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const sessionsRef = useRef<ChatSession[]>([]);
  const inputRef = useRef("");
  const resyncProjectRef = useRef<Record<string, { projectEpoch: number; projectPath: string }>>({});
  const tabMutationRef = useRef<Promise<void>>(Promise.resolve());
  const scrollRef = useRef<HTMLDivElement>(null);
  const mountedRef = useRef(true);
  const interactionLocked = streaming || pendingSessionId === sessionId;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    if (composerDraft === null) return;
    inputRef.current = composerDraft;
    setInput(composerDraft);
    setComposerDraft(null);
  }, [composerDraft, setComposerDraft]);

  useEffect(() => () => {
    setComposerDraft(inputRef.current || null);
  }, [setComposerDraft]);

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
    let disposed = false;
    const unsubscribers: Array<() => void> = [];
    const install = (subscription: Promise<() => void>) => {
      void subscription
        .then((unsubscribe) => {
          if (disposed) unsubscribe();
          else unsubscribers.push(unsubscribe);
        })
        .catch(() => {});
    };
    const matchesProject = (event: ChatStreamIdentity) => {
      const project = useProjectStore.getState();
      return event.projectEpoch === project.projectEpoch && event.projectPath === project.projectPath;
    };
    const begin = (event: ChatStreamIdentity) => {
      resyncProjectRef.current[event.sessionId] = {
        projectEpoch: event.projectEpoch,
        projectPath: event.projectPath,
      };
      beginMessage(event.sessionId, event.messageId);
      setPendingSessionId((current) => current === event.sessionId ? null : current);
    };
    const malformed = (failure: ChatStreamDecodeFailure) => {
      if (!failure.sessionId) return;
      const chat = useChatStore.getState();
      const isKnownSession = failure.sessionId === chat.sessionId ||
        sessionsRef.current.some((session) => session.id === failure.sessionId);
      if (!isKnownSession) return;
      const project = useProjectStore.getState();
      if (project.projectPath) {
        resyncProjectRef.current[failure.sessionId] = {
          projectEpoch: project.projectEpoch,
          projectPath: project.projectPath,
        };
      }
      setPendingSessionId((current) => current === failure.sessionId ? null : current);
      requestHistoryResync(failure.sessionId, failure.messageId, failure.reason);
    };

    install(onChatDelta((event) => {
      if (!matchesProject(event)) return;
      begin(event);
      appendBlockDelta(
        event.sessionId,
        event.messageId,
        event.sequence,
        event.blockIndex,
        event.delta,
      );
    }, malformed));
    install(onChatToolCall((event) => {
      if (!matchesProject(event)) return;
      begin(event);
      upsertBlock(
        event.sessionId,
        event.messageId,
        event.sequence,
        event.blockIndex,
        event.block,
      );
    }, malformed));
    install(onChatDone((event) => {
      if (!matchesProject(event)) return;
      begin(event);
      finalize(event.sessionId, event.messageId, event.sequence, event.message);
    }, malformed));

    return () => {
      disposed = true;
      unsubscribers.splice(0).forEach((unsubscribe) => unsubscribe());
    };
  }, [appendBlockDelta, beginMessage, finalize, requestHistoryResync, upsertBlock]);

  useEffect(() => {
    if (!projectPath || Object.keys(historyResyncRequests).length === 0) return;
    const request = takeHistoryResyncRequest();
    if (!request) return;
    const requestProject = resyncProjectRef.current[request.sessionId];
    delete resyncProjectRef.current[request.sessionId];
    if (
      requestProject &&
      (requestProject.projectEpoch !== projectEpoch || requestProject.projectPath !== projectPath)
    ) {
      return;
    }
    const loadingEpoch = requestProject?.projectEpoch ?? projectEpoch;
    const loadingPath = requestProject?.projectPath ?? projectPath;
    void chatHistory(request.sessionId, loadingEpoch, loadingPath)
      .then((history) => {
        const project = useProjectStore.getState();
        if (
          !mountedRef.current ||
          project.projectEpoch !== loadingEpoch ||
          project.projectPath !== loadingPath
        ) {
          return;
        }
        setMessagesForSession(request.sessionId, history);
        updateSessions((current) => current.map((session) =>
          session.id === request.sessionId ? { ...session, messages: history } : session,
        ));
      })
      .catch(() => {});
  }, [
    historyResyncRequests,
    projectEpoch,
    projectPath,
    setMessagesForSession,
    takeHistoryResyncRequest,
  ]);

  useEffect(() => {
    const previousSessionId = useChatStore.getState().sessionId;
    const freshSessionId = mintSessionId();
    resyncProjectRef.current = {};
    setPendingSessionId(null);
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
        openSessions.forEach((session) => setMessagesForSession(session.id, session.messages));
        const latest = openSessions.find((session) => session.id === previousSessionId) ??
          openSessions[0];
        if (latest) {
          reset(latest.id);
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
  }, [projectEpoch, projectPath, reset, setMessagesForSession]);

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
    if (!text || interactionLocked || !projectPath) return;
    const sendingEpoch = projectEpoch;
    const sendingPath = projectPath;
    const sendingSessionId = sessionId;
    inputRef.current = "";
    setInput("");
    pushUser(text);
    setPendingSessionId(sendingSessionId);
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
      const errorText = `⚠️ ${error instanceof Error ? error.message : String(error)}`;
      const errorMessage: ChatMessage = {
        id: `assistant-local-error-${Date.now()}`,
        role: "assistant",
        content: errorText,
        toolCalls: [],
        blocks: [{ type: "text", text: errorText }],
        createdAt: Date.now(),
      };
      setMessagesForSession(
        sendingSessionId,
        [...(chat.sessionMessages[sendingSessionId] ?? []), errorMessage],
      );
      setPendingSessionId((current) => current === sendingSessionId ? null : current);
    }
  }

  function cancel() {
    if (!projectPath) return;
    void chatCancel(sessionId, projectEpoch, projectPath).catch(() => {});
  }

  function openSession(session: ChatSession) {
    if (interactionLocked || session.id === sessionId || !projectPath) return;
    const storedMessages = useChatStore.getState().sessionMessages[session.id];
    if (!storedMessages) setMessagesForSession(session.id, session.messages);
    reset(session.id);
  }

  function newChat() {
    if (interactionLocked || !projectPath) return;
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
    if (interactionLocked || !projectPath) return;
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
    deleteSession(closingSessionId);
    if (closingSessionId !== useChatStore.getState().sessionId) return;
    const next = remaining[0];
    if (next) {
      const storedMessages = useChatStore.getState().sessionMessages[next.id];
      if (!storedMessages) setMessagesForSession(next.id, next.messages);
      reset(next.id);
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
          {t("agent.title")}
        </span>
        <button
          type="button"
          onClick={() => void newChat()}
          disabled={interactionLocked || !projectPath}
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
            opacity: interactionLocked || !projectPath ? 0.4 : 1,
          }}
        >
          <Plus size={14} />
        </button>
      </div>

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
                disabled={interactionLocked}
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
                disabled={interactionLocked}
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
                  opacity: interactionLocked ? 0.4 : 1,
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
        {groupConversationMessages(messages).map((turnMessages) => (
          <ConversationMessage
            key={turnMessages[0].id}
            messages={turnMessages}
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
          className="agent-composer__input"
          value={input}
          onChange={(event) => {
            inputRef.current = event.target.value;
            setInput(event.target.value);
          }}
          onKeyDown={onKeyDown}
          placeholder={t("agent.inputPlaceholder")}
          aria-label={t("agent.inputPlaceholder")}
          disabled={!isTauri || interactionLocked}
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
            opacity: !isTauri ? 0.6 : 1,
          }}
        />
        {interactionLocked ? (
          <button
            type="button"
            onClick={cancel}
            title={t("agent.cancel")}
            aria-label={t("agent.cancel")}
            className="agent-composer__action"
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
            aria-label={t("agent.send")}
            className="agent-composer__action"
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

function sessionTitle(session: ChatSession, fallback: string): string {
  const firstUserMessage = session.messages.find(
    (message) => message.role === "user" && authoritativeMessageText(message).trim().length > 0,
  );
  if (!firstUserMessage) return fallback;
  const compact = authoritativeMessageText(firstUserMessage).trim().replace(/\s+/g, " ");
  return compact.length > 20 ? `${compact.slice(0, 20)}…` : compact;
}

type ConversationMessageProps = (
  | { message: ChatMessage; messages?: never }
  | { message?: never; messages: ChatMessage[] }
) & {
  onOpenSettings: () => void;
};

export function ConversationMessage({
  message,
  messages,
  onOpenSettings,
}: ConversationMessageProps) {
  const t = useT();
  const turnMessages = messages ?? (message ? [message] : []);
  const firstMessage = turnMessages[0];
  if (!firstMessage) return null;
  const isUser = turnMessages.length === 1 && firstMessage.role === "user";
  const guided = turnMessages.some(
    (candidate) => candidate.role === "assistant" &&
      NO_KEY_HINT.test(authoritativeMessageText(candidate)),
  );

  return (
    <div
      className={`agent-message ${isUser ? "agent-message--user" : "agent-message--assistant"}`}
    >
      {isUser
        ? <div className="agent-message__user-surface">
            {authoritativeMessageText(firstMessage)}
          </div>
        : <AssistantTurn messages={turnMessages} />}
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

function authoritativeMessageText(message: ChatMessage): string {
  if (message.blocks === undefined) return message.content;
  return message.blocks
    .flatMap((block) => block.type === "text" ? [block.text] : [])
    .join("");
}

function groupConversationMessages(messages: ChatMessage[]): ChatMessage[][] {
  const groups: ChatMessage[][] = [];
  messages.forEach((message) => {
    const previous = groups[groups.length - 1];
    if (message.role !== "user" && previous && previous[0].role !== "user") {
      previous.push(message);
    } else {
      groups.push([message]);
    }
  });
  return groups;
}

type ToolActivityBlock = Exclude<AgentContentBlock, { type: "text" }>;

type AssistantTurnProps =
  | { message: ChatMessage; messages?: never }
  | { message?: never; messages: ChatMessage[] };

export function AssistantTurn({ message, messages }: AssistantTurnProps) {
  const turnMessages = messages ?? (message ? [message] : []);
  const toolNames = new Map<string, string>();
  turnMessages.forEach((candidate) => {
    candidate.blocks?.forEach((block) => {
      if (block.type === "toolUse") toolNames.set(block.id, block.name);
    });
  });
  const entries = turnMessages.flatMap((candidate) => {
    if (candidate.blocks !== undefined) {
      return candidate.blocks.map((block, messageBlockIndex) => ({
        block,
        key: `${candidate.id}-${messageBlockIndex}`,
      }));
    }
    const legacyBlocks: AgentContentBlock[] = [];
    if (candidate.content) legacyBlocks.push({ type: "text", text: candidate.content });
    legacyBlocks.push(...candidate.toolCalls.map((toolCall) => ({
      type: "toolUse" as const,
      id: toolCall.id,
      name: toolCall.name,
      input: toolCall.args,
      result: toolCall.result,
      isError: toolCall.isError,
    })));
    return legacyBlocks.map((block, messageBlockIndex) => ({
      block,
      key: `${candidate.id}-legacy-${messageBlockIndex}`,
    }));
  });

  return (
    <div className="agent-assistant-turn" data-assistant-turn>
      {entries.map(({ block, key }, index) => {
        if (block.type === "text") {
          return (
            <div
              className="agent-assistant-turn__text"
              data-agent-block-index={index}
              data-agent-block-type="text"
              key={key}
            >
              {block.text}
            </div>
          );
        }
        return (
          <InlineToolActivity
            block={block}
            dataBlockIndex={index}
            key={key}
            toolName={block.type === "toolResult" ? toolNames.get(block.toolUseId) : undefined}
          />
        );
      })}
    </div>
  );
}

export function InlineToolActivity({
  block,
  dataBlockIndex,
  toolName,
}: {
  block: ToolActivityBlock;
  dataBlockIndex?: number;
  toolName?: string;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const reactId = useId();
  const disclosureId = `agent-tool-${reactId.replace(/:/g, "")}`;
  const isError = block.isError === true;
  const pending = block.type === "toolUse" && block.result === undefined && !isError;
  const status = isError ? "error" : pending ? "running" : "complete";
  const statusLabel = t(
    status === "error"
      ? "agent.toolFailed"
      : status === "running"
        ? "agent.toolRunning"
        : "agent.toolComplete",
  );
  const label = block.type === "toolUse"
    ? block.name
    : toolName ?? t("agent.toolResult");

  return (
    <div
      className="agent-tool-activity"
      data-agent-block-index={dataBlockIndex}
      data-agent-block-type={block.type}
      data-status={status}
      data-tool-activity
    >
      <button
        type="button"
        aria-controls={disclosureId}
        aria-expanded={open}
        aria-label={`${label}: ${statusLabel}`}
        data-tool-activity-trigger
        onClick={() => setOpen((value) => !value)}
        className="agent-tool-activity__trigger"
      >
        {open
          ? <ChevronDown aria-hidden="true" size={12} />
          : <ChevronRight aria-hidden="true" size={12} />}
        <Wrench aria-hidden="true" size={12} />
        <span className="agent-tool-activity__name">{label}</span>
        <span
          aria-atomic="true"
          aria-live="polite"
          className="agent-tool-activity__status"
          role="status"
        >
          {statusLabel}
        </span>
      </button>
      <Reveal id={disclosureId} open={open} role="group">
        <div className="agent-tool-activity__details">
          {block.type === "toolUse"
            ? <>
                <ToolDetail label={t("agent.toolArgs")} value={prettyJson(block.input)} />
                {block.result !== undefined && (
                  <ToolDetail label={t("agent.toolResult")} value={prettyJson(block.result)} />
                )}
              </>
            : block.content.map((content, index) => {
                if (content.kind === "text") {
                  return <ToolDetail key={index} label={t("agent.toolResult")} value={content.text} />;
                }
                const source = safeRasterDataUri(content.mediaType, content.base64);
                return source
                  ? <img
                      alt={t("agent.toolImageAlt", { tool: label })}
                      className="agent-tool-activity__image"
                      key={index}
                      src={source}
                    />
                  : <span className="agent-tool-activity__image-error" key={index}>
                      {t("agent.toolImageUnavailable")}
                    </span>;
              })}
        </div>
      </Reveal>
    </div>
  );
}

function ToolDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="agent-tool-activity__detail">
      <div className="agent-tool-activity__detail-label">{label}</div>
      <pre>{value}</pre>
    </div>
  );
}

function prettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2) ?? String(value);
  } catch {
    return String(value);
  }
}

const SAFE_RASTER_MEDIA_TYPES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);
const BASE64_PATTERN = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

function safeRasterDataUri(mediaType: string, base64: string): string | null {
  const normalizedMediaType = mediaType.trim().toLowerCase();
  if (
    !SAFE_RASTER_MEDIA_TYPES.has(normalizedMediaType) ||
    base64.length === 0 ||
    !BASE64_PATTERN.test(base64)
  ) {
    return null;
  }
  return `data:${normalizedMediaType};base64,${base64}`;
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
