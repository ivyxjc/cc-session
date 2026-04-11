import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getMessages, getSubagents, listSessions, watchSession, unwatchSession } from "../../lib/tauri";
import type { ParsedMessage, SessionSummary, SubagentSummary, SessionMessagesUpdate } from "../../lib/types";
import { useLiveStore } from "../../stores/liveStore";
import { useAppStore } from "../../stores/appStore";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";
import { LiveStatusBadge } from "./LiveStatusBadge";
import { RunningTimer } from "./RunningTimer";
import { buildToolResultsMap } from "../../lib/toolResults";

function getMessageKey(msg: ParsedMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.uuid || `msg-${index}`;
  if (msg.type === "system") return msg.uuid || `sys-${index}`;
  return `msg-${index}`;
}

export function LiveConversationView() {
  const watchedSessionId = useLiveStore((s) => s.watchedSessionId);
  const liveSessions = useLiveStore((s) => s.liveSessions);
  const setView = useAppStore((s) => s.setView);
  const setWatchedSessionId = useLiveStore((s) => s.setWatchedSessionId);

  const [session, setSession] = useState<SessionSummary | null>(null);
  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const scrollRef = useRef<HTMLDivElement>(null);
  const autoScrollRef = useRef(true);

  const liveSession = liveSessions.find((s) => s.sessionId === watchedSessionId);
  const toolResults = useMemo(() => buildToolResultsMap(messages), [messages]);

  // Stable primitive for the effect dependency
  const dbSessionId = useMemo(
    () => liveSession?.dbSessionId ?? null,
    [liveSession?.dbSessionId],
  );

  // Load initial messages and start watching
  useEffect(() => {
    if (!watchedSessionId || !dbSessionId) return;

    setLoading(true);

    Promise.all([
      listSessions({ projectId: undefined }).then(
        (sessions) => sessions.find((s) => s.id === dbSessionId) || null,
      ),
      getMessages(dbSessionId, 0, 500),
      getSubagents(dbSessionId),
    ]).then(([sess, msgs, subs]) => {
      setSession(sess);
      setMessages(msgs);
      setSubagents(subs);
      setLoading(false);
    });

    // Start fs-notify watch
    watchSession(watchedSessionId).catch(console.error);

    return () => {
      unwatchSession(watchedSessionId).catch(console.error);
    };
  }, [watchedSessionId, dbSessionId]);

  // Listen for new messages via Tauri event
  useEffect(() => {
    const unlisten = listen<SessionMessagesUpdate>("session-messages-update", (event) => {
      if (event.payload.sessionId === watchedSessionId) {
        setMessages((prev) => [...prev, ...event.payload.newMessages]);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [watchedSessionId]);

  // Auto-scroll to bottom when new messages arrive
  useEffect(() => {
    if (autoScrollRef.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages.length]);

  // Detect if user has scrolled away from bottom
  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    autoScrollRef.current = scrollHeight - scrollTop - clientHeight < 100;
  };

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
              {session?.slug || liveSession?.slug || watchedSessionId.slice(0, 8)}
            </span>
          </div>
          <div className="text-xs text-zinc-400 mt-0.5">
            {session?.projectName || liveSession?.projectName}
            {(session?.gitBranch || liveSession?.gitBranch) && (
              <> &middot; {session?.gitBranch || liveSession?.gitBranch}</>
            )}
            {liveSession && liveSession.isAlive && (
              <> &middot; <RunningTimer startedAt={liveSession.startedAt} /></>
            )}
          </div>
        </div>
        <div className="text-xs text-zinc-400 shrink-0">
          {messages.length} messages
        </div>
      </div>

      {/* Messages */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-4 space-y-3"
      >
        {messages.map((msg, i) => (
          <MessageBubble key={getMessageKey(msg, i)} message={msg} subagents={subagents} toolResults={toolResults} />
        ))}

        {liveSession?.isAlive && (
          <div className="flex items-center gap-2 text-xs text-zinc-400 py-2">
            <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
            Watching for new messages...
          </div>
        )}
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
