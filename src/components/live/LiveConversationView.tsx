import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { listen } from "@tauri-apps/api/event";
import { getLatestMessages, getMessages, getSubagents, watchSession, unwatchSession, listSessions } from "../../lib/tauri";
import type { ViewMessage, SubagentSummary, SessionMessagesUpdate, SessionSummary, AiSummaryProgress } from "../../lib/types";
import { useLiveStore } from "../../stores/liveStore";
import { useAppStore } from "../../stores/appStore";
import { formatTokens, formatFileSize } from "../../lib/format";
import { backupSession } from "../../lib/tauri";
import { toast } from "../../stores/toastStore";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { MultiplexerButton } from "../common/MultiplexerButton";
import { TagManager } from "../common/TagManager";
import { TagBadge } from "../common/TagBadge";
import { LiveStatusBadge } from "./LiveStatusBadge";
import { RunningTimer } from "./RunningTimer";
import { TerminalPane } from "../terminal/TerminalPane";
import {
  useIncrementalToolResults,
  getMessageKey,
  findSubagentMessageIndex,
} from "../../lib/conversation";

const INITIAL_LOAD = 100;
const OLDER_BATCH = 50;

export function LiveConversationView() {
  const watchedSessionId = useLiveStore((s) => s.watchedSessionId);
  const liveSessions = useLiveStore((s) => s.liveSessions);
  const setView = useAppStore((s) => s.setView);
  const setWatchedSessionId = useLiveStore((s) => s.setWatchedSessionId);

  const [messages, setMessages] = useState<ViewMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [showTagManager, setShowTagManager] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [tags, setTags] = useState<{ id: number; name: string; color: string }[]>([]);
  const [dbSession, setDbSession] = useState<SessionSummary | null>(null);
  const [showTerminal, setShowTerminal] = useState(false);

  const [subagentsExpanded, setSubagentsExpanded] = useState(false);

  // For prepending: firstItemIndex tells Virtuoso the "virtual" index of the first item
  const [firstItemIndex, setFirstItemIndex] = useState(0);
  const loadingOlderRef = useRef(false);
  const earliestOffsetRef = useRef(0);
  const hasOlderRef = useRef(false);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const atBottomRef = useRef(true);

  // Batched incoming messages (task #11 inlined)
  const pendingRef = useRef<ViewMessage[]>([]);
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
      listSessions({ projectId: undefined, showHidden: true }).then((sessions) =>
        sessions.find((s) => s.id === dbSessionId) || null,
      ),
    ]).then(([result, subs, session]) => {
      const startOffset = result.totalCount - result.messages.length;
      setMessages(result.messages);
      setSubagents(subs);
      setTags(session?.tags || []);
      setDbSession(session);
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

  // Refresh dbSession when AI summary batch (or per-session generation) completes
  // for this session — keeps the title / aiTags up to date without reload.
  useEffect(() => {
    if (!dbSessionId) return;
    const unlisten = listen<AiSummaryProgress>("ai-summary-progress", (event) => {
      if (event.payload.sessionDbId !== dbSessionId || event.payload.status !== "ok") return;
      listSessions({ projectId: undefined, showHidden: true }).then((sessions) => {
        const s = sessions.find((x) => x.id === dbSessionId) || null;
        if (s) setDbSession(s);
      });
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [dbSessionId]);

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

  const locateSubagent = useCallback(async (description: string) => {
    let idx = findSubagentMessageIndex(messages, description);
    if (idx >= 0) {
      virtuosoRef.current?.scrollToIndex({ index: idx, align: "center", behavior: "smooth" });
      return;
    }
    if (dbSessionId && earliestOffsetRef.current > 0) {
      const older = await getMessages(dbSessionId, 0, earliestOffsetRef.current);
      setMessages((prev) => [...older, ...prev]);
      setFirstItemIndex(0);
      earliestOffsetRef.current = 0;
      hasOlderRef.current = false;

      idx = findSubagentMessageIndex(older, description);
      if (idx >= 0) {
        setTimeout(() => {
          virtuosoRef.current?.scrollToIndex({ index: idx, align: "center", behavior: "smooth" });
        }, 100);
      }
    }
  }, [messages, dbSessionId]);

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
      {/* Header — matches SessionHeader layout */}
      <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
        <div className="flex items-center gap-2">
          <button
            onClick={handleBack}
            className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
          >
            &larr; Live
          </button>
          <div className="flex-1" />
          {liveSession && <LiveStatusBadge isAlive={liveSession.isAlive} />}
          {liveSession?.isAlive && (
            <span className="text-xs text-zinc-400"><RunningTimer startedAt={liveSession.startedAt} /></span>
          )}
          {liveSession?.dbSessionId && (
            <button
              onClick={async () => {
                setBackingUp(true);
                try {
                  await backupSession(liveSession.dbSessionId!);
                  toast.success("Backup successful!");
                } catch (e) {
                  toast.error(`Backup failed: ${e}`);
                } finally { setBackingUp(false); }
              }}
              disabled={backingUp}
              className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
            >
              {backingUp ? "Backing up..." : "Backup"}
            </button>
          )}
          {liveSession?.dbSessionId && (
            <div className="relative">
              <button
                onClick={() => setShowTagManager(!showTagManager)}
                className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                Tags
              </button>
              {showTagManager && (
                <div className="absolute right-0 top-8 z-10 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-lg">
                  <TagManager
                    sessionId={liveSession.dbSessionId}
                    currentTags={tags}
                    onUpdate={() => {
                      setShowTagManager(false);
                      // Refresh tags
                      listSessions({ projectId: undefined }).then((sessions) => {
                        const s = sessions.find((s) => s.id === liveSession.dbSessionId);
                        if (s) setTags(s.tags);
                      });
                    }}
                  />
                </div>
              )}
            </div>
          )}
          <button
            onClick={() => setShowTerminal((v) => !v)}
            className={`text-sm px-2 py-0.5 border rounded ${
              showTerminal
                ? "border-emerald-400 bg-emerald-50 dark:bg-emerald-950 text-emerald-700 dark:text-emerald-400"
                : "border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
            }`}
            title="Embed the matching multiplexer session in a terminal pane"
          >
            {showTerminal ? "Hide terminal" : "Terminal"}
          </button>
          <OpenTerminalButton path={liveSession?.cwd || ""} sessionId={liveSession?.sessionId} />
          <MultiplexerButton path={liveSession?.cwd || ""} />
          {liveSession?.dbSessionId && (
            <FavoriteButton sessionId={liveSession.dbSessionId} initialFavorited={false} />
          )}
        </div>
        <h1
          className={`text-lg font-semibold mt-2 break-words ${
            dbSession?.summary && dbSession.summarySource === "heuristic"
              ? "italic text-zinc-600 dark:text-zinc-400"
              : ""
          }`}
        >
          {dbSession?.summary || liveSession?.projectName || liveSession?.cwd.split("/").pop() || "\u2014"}
        </h1>
        <CopyText text={watchedSessionId} className="text-sm text-zinc-400 font-mono" />
        <div className="text-sm text-zinc-500 mt-0.5">
          {liveSession?.projectName || "\u2014"} &middot; {liveSession?.gitBranch || "\u2014"} &middot; {liveSession?.version || "\u2014"} &middot; PID {liveSession?.pid}
        </div>
        <div className="text-xs text-zinc-400 mt-1">
          {messages.length} msgs &middot; total {formatTokens((liveSession?.totalInputTokens || 0) + (liveSession?.totalOutputTokens || 0) + (liveSession?.totalCacheCreationTokens || 0) + (liveSession?.totalCacheReadTokens || 0))}
          {" "}&middot; in {formatTokens(liveSession?.totalInputTokens || 0)}
          {" "}&middot; out {formatTokens(liveSession?.totalOutputTokens || 0)}
          {" "}&middot; cache R {formatTokens(liveSession?.totalCacheReadTokens || 0)}
          {" "}&middot; cache W {formatTokens(liveSession?.totalCacheCreationTokens || 0)}
          {liveSession?.fileSize != null && <> &middot; {formatFileSize(liveSession.fileSize)}</>}
        </div>
        {(tags.length > 0 || (dbSession?.aiTags && dbSession.aiTags.length > 0)) && (
          <div className="flex gap-1 mt-2 flex-wrap">
            {tags.map((tag) => <TagBadge key={tag.id} tag={tag} />)}
            {dbSession?.aiTags?.map((tag) => (
              <span
                key={`ai-${tag}`}
                className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs rounded border border-emerald-300 dark:border-emerald-800 text-emerald-700 dark:text-emerald-400"
                title="AI-generated tag"
              >
                <span className="text-[10px] opacity-70">AI</span>{tag}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Main area: either the message list OR the terminal (terminal takes full area to
          avoid being the smallest zellij client and shrinking the user's other attached terminals). */}
      <div className="flex-1 min-h-0">
        {showTerminal ? (
          <TerminalPane
            cwd={liveSession?.cwd || ""}
            livePid={liveSession?.pid}
            onClose={() => setShowTerminal(false)}
          />
        ) : (
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
        )}
      </div>

      {/* Subagents — collapsed by default, expands to 50% height */}
      {subagents.length > 0 && (
        <div className={`border-t border-zinc-200 dark:border-zinc-800 flex flex-col ${subagentsExpanded ? "max-h-[50vh]" : ""}`}>
          <button
            onClick={() => setSubagentsExpanded((v) => !v)}
            className="w-full px-4 py-2 text-sm text-left text-zinc-500 hover:bg-zinc-50 dark:hover:bg-zinc-900 flex items-center gap-2 shrink-0"
          >
            <span className="font-mono text-xs">{subagentsExpanded ? "\u25BC" : "\u25B6"}</span>
            <span className="font-medium">Subagents ({subagents.length})</span>
          </button>
          {subagentsExpanded && (
            <div className="px-4 pb-3 overflow-y-auto space-y-2 flex-1 min-h-0">
              {subagents.map((sa) => (
                <SubagentView
                  key={sa.id}
                  subagent={sa}
                  onLocate={() => locateSubagent(sa.description)}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
