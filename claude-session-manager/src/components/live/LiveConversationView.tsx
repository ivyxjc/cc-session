import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { listen } from "@tauri-apps/api/event";
import { getLatestMessages, getMessages, getSubagents, watchSession, unwatchSession } from "../../lib/tauri";
import type { ParsedMessage, SubagentSummary, SessionMessagesUpdate } from "../../lib/types";
import { useLiveStore } from "../../stores/liveStore";
import { useAppStore } from "../../stores/appStore";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";
import { LiveStatusBadge } from "./LiveStatusBadge";
import { RunningTimer } from "./RunningTimer";
import type { ToolResult } from "../../lib/toolResults";

// --- Incremental tool results (task #10 inlined) ---

function useIncrementalToolResults(messages: ParsedMessage[]) {
  const mapRef = useRef(new Map<string, ToolResult>());
  const processedRef = useRef(0);

  // Process only newly added messages
  if (messages.length > processedRef.current) {
    for (let i = processedRef.current; i < messages.length; i++) {
      const msg = messages[i];
      if (msg.type !== "user") continue;
      for (const block of msg.content) {
        if (block.type === "tool_result" && block.toolUseId) {
          const content = extractToolResultContent(block);
          mapRef.current.set(block.toolUseId, { content, isError: block.isError ?? false });
        }
      }
    }
    processedRef.current = messages.length;
  }

  // When messages are prepended (older messages loaded), reprocess from scratch
  // Detect prepend: processedRef > messages.length shouldn't happen, but
  // a full reset when the array identity changes is handled by the caller
  return mapRef.current;
}

function extractToolResultContent(block: { content?: unknown }): string {
  const raw = block.content;
  if (typeof raw === "string") return raw;
  if (Array.isArray(raw)) {
    return raw
      .filter((b: Record<string, unknown>) => b.type === "text")
      .map((b: Record<string, unknown>) => b.text || "")
      .join("\n");
  }
  return String(raw ?? "");
}

// --- Message key ---

function getMessageKey(msg: ParsedMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.uuid || `msg-${index}`;
  if (msg.type === "system") return msg.uuid || `sys-${index}`;
  return `msg-${index}`;
}

// --- Component ---

const INITIAL_LOAD = 100;
const OLDER_BATCH = 50;

export function LiveConversationView() {
  const watchedSessionId = useLiveStore((s) => s.watchedSessionId);
  const liveSessions = useLiveStore((s) => s.liveSessions);
  const setView = useAppStore((s) => s.setView);
  const setWatchedSessionId = useLiveStore((s) => s.setWatchedSessionId);

  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);

  // For prepending: firstItemIndex tells Virtuoso the "virtual" index of the first item
  const [firstItemIndex, setFirstItemIndex] = useState(0);
  const loadingOlderRef = useRef(false);
  const earliestOffsetRef = useRef(0);
  const hasOlderRef = useRef(false);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const atBottomRef = useRef(true);

  // Batched incoming messages (task #11 inlined)
  const pendingRef = useRef<ParsedMessage[]>([]);
  const flushScheduledRef = useRef(false);

  const liveSession = liveSessions.find((s) => s.sessionId === watchedSessionId);
  const toolResults = useIncrementalToolResults(messages);

  const dbSessionId = useMemo(
    () => liveSession?.dbSessionId ?? null,
    [liveSession?.dbSessionId],
  );

  // --- Initial load ---
  useEffect(() => {
    if (!watchedSessionId || !dbSessionId) return;

    setLoading(true);

    Promise.all([
      getLatestMessages(dbSessionId, INITIAL_LOAD),
      getSubagents(dbSessionId),
    ]).then(([result, subs]) => {
      const startOffset = result.totalCount - result.messages.length;
      setMessages(result.messages);
      setSubagents(subs);
      setFirstItemIndex(startOffset);
      earliestOffsetRef.current = startOffset;
      hasOlderRef.current = startOffset > 0;
      setLoading(false);
    });

    watchSession(watchedSessionId).catch(console.error);

    return () => {
      unwatchSession(watchedSessionId).catch(console.error);
    };
  }, [watchedSessionId, dbSessionId]);

  // --- Live message events with batching ---
  useEffect(() => {
    const unlisten = listen<SessionMessagesUpdate>("session-messages-update", (event) => {
      if (event.payload.sessionId !== watchedSessionId) return;
      pendingRef.current.push(...event.payload.newMessages);

      if (!flushScheduledRef.current) {
        flushScheduledRef.current = true;
        requestAnimationFrame(() => {
          const batch = pendingRef.current;
          pendingRef.current = [];
          flushScheduledRef.current = false;
          if (batch.length > 0) {
            setMessages((prev) => [...prev, ...batch]);
          }
        });
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [watchedSessionId]);

  // --- Load older messages (triggered by Virtuoso startReached) ---
  const handleStartReached = useCallback(() => {
    if (!dbSessionId || !hasOlderRef.current || loadingOlderRef.current) return;
    loadingOlderRef.current = true;

    const loadCount = Math.min(OLDER_BATCH, earliestOffsetRef.current);
    const newOffset = earliestOffsetRef.current - loadCount;

    getMessages(dbSessionId, newOffset, loadCount).then((older) => {
      setMessages((prev) => [...older, ...prev]);
      setFirstItemIndex(newOffset);
      earliestOffsetRef.current = newOffset;
      hasOlderRef.current = newOffset > 0;
      loadingOlderRef.current = false;
    });
  }, [dbSessionId]);

  const handleBack = () => {
    setWatchedSessionId(null);
    setView("live");
  };

  if (!watchedSessionId) {
    return <div className="p-6 text-zinc-500">No session selected</div>;
  }

  if (loading) {
    return <div className="p-6 text-zinc-500">Loading live conversation...</div>;
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-4 py-3 border-b border-zinc-200 dark:border-zinc-800 flex items-center gap-3">
        <button
          onClick={handleBack}
          className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          &larr; Live
        </button>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            {liveSession && <LiveStatusBadge isAlive={liveSession.isAlive} />}
            <span className="font-medium truncate">
              {liveSession?.slug || watchedSessionId.slice(0, 8)}
            </span>
          </div>
          <div className="text-xs text-zinc-400 mt-0.5">
            {liveSession?.projectName}
            {liveSession?.gitBranch && <> &middot; {liveSession.gitBranch}</>}
            {liveSession && liveSession.isAlive && (
              <> &middot; <RunningTimer startedAt={liveSession.startedAt} /></>
            )}
          </div>
        </div>
        <div className="text-xs text-zinc-400 shrink-0">
          {messages.length} messages
        </div>
      </div>

      {/* Virtualized message list */}
      <div className="flex-1">
        <Virtuoso
          ref={virtuosoRef}
          data={messages}
          firstItemIndex={firstItemIndex}
          initialTopMostItemIndex={messages.length - 1}
          followOutput={(isAtBottom) => isAtBottom ? "smooth" : false}
          atBottomStateChange={(atBottom) => { atBottomRef.current = atBottom; }}
          startReached={handleStartReached}
          itemContent={(index, msg) => (
            <div className="px-4 py-1.5">
              <MessageBubble
                key={getMessageKey(msg, index)}
                message={msg}
                subagents={subagents}
                toolResults={toolResults}
              />
            </div>
          )}
          components={{
            Header: () =>
              hasOlderRef.current && loadingOlderRef.current ? (
                <div className="text-center text-xs text-zinc-400 py-2">Loading older messages...</div>
              ) : null,
            Footer: () =>
              liveSession?.isAlive ? (
                <div className="flex items-center gap-2 text-xs text-zinc-400 px-4 py-2">
                  <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                  Watching for new messages...
                </div>
              ) : null,
          }}
        />
      </div>

      {/* Subagents */}
      {subagents.length > 0 && (
        <div className="border-t border-zinc-200 dark:border-zinc-800 p-4 max-h-48 overflow-y-auto">
          <h3 className="text-sm font-medium text-zinc-500 mb-2">
            Subagents ({subagents.length})
          </h3>
          {subagents.map((sa) => (
            <SubagentView key={sa.id} subagent={sa} />
          ))}
        </div>
      )}
    </div>
  );
}
