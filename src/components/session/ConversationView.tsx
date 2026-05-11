import { useCallback, useEffect, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { getLatestMessages, getMessages, getSubagents, listSessions } from "../../lib/tauri";
import type { ViewMessage, SessionSummary, SubagentSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { SessionHeader } from "./SessionHeader";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";
import {
  useIncrementalToolResults,
  getMessageKey,
  findSubagentMessageIndex,
} from "../../lib/conversation";

const INITIAL_LOAD = 100;
const OLDER_BATCH = 50;

export function ConversationView() {
  const { selectedSessionId } = useAppStore();
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [messages, setMessages] = useState<ViewMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [subagentsExpanded, setSubagentsExpanded] = useState(false);

  const [firstItemIndex, setFirstItemIndex] = useState(0);
  const loadingOlderRef = useRef(false);
  const earliestOffsetRef = useRef(0);
  const hasOlderRef = useRef(false);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const toolResults = useIncrementalToolResults(messages);

  useEffect(() => {
    if (!selectedSessionId) return;
    setLoading(true);
    setSubagentsExpanded(false);

    Promise.all([
      listSessions({ projectId: undefined, showHidden: true }).then((sessions) =>
        sessions.find((s) => s.id === selectedSessionId) || null,
      ),
      getLatestMessages(selectedSessionId, INITIAL_LOAD),
      getSubagents(selectedSessionId),
    ])
      .then(([sess, result, subs]) => {
        const startOffset = result.totalCount - result.messages.length;
        setSession(sess);
        setMessages(result.messages);
        setSubagents(subs);
        setFirstItemIndex(startOffset);
        earliestOffsetRef.current = startOffset;
        hasOlderRef.current = startOffset > 0;
        loadingOlderRef.current = false;
        setLoading(false);
      })
      .catch((e) => {
        console.error("Failed to load conversation:", e);
        setSession(null);
        setMessages([]);
        setSubagents([]);
        setLoading(false);
      });
  }, [selectedSessionId]);

  const locateSubagent = useCallback(async (description: string) => {
    // First check in currently loaded messages
    let idx = findSubagentMessageIndex(messages, description);
    if (idx >= 0) {
      virtuosoRef.current?.scrollToIndex({ index: idx, align: "center", behavior: "smooth" });
      return;
    }
    // Not in loaded messages — load all history then search
    if (selectedSessionId && earliestOffsetRef.current > 0) {
      const older = await getMessages(selectedSessionId, 0, earliestOffsetRef.current);
      setMessages((prev) => [...older, ...prev]);
      setFirstItemIndex(0);
      earliestOffsetRef.current = 0;
      hasOlderRef.current = false;

      // Search in the full message list (older + current)
      idx = findSubagentMessageIndex(older, description);
      if (idx >= 0) {
        // Use setTimeout to let Virtuoso process the new items
        setTimeout(() => {
          virtuosoRef.current?.scrollToIndex({ index: idx, align: "center", behavior: "smooth" });
        }, 100);
      }
    }
  }, [messages, selectedSessionId]);

  const handleStartReached = useCallback(() => {
    if (!selectedSessionId || !hasOlderRef.current || loadingOlderRef.current) return;
    loadingOlderRef.current = true;

    const loadCount = Math.min(OLDER_BATCH, earliestOffsetRef.current);
    const newOffset = earliestOffsetRef.current - loadCount;

    getMessages(selectedSessionId, newOffset, loadCount).then((older) => {
      setMessages((prev) => [...older, ...prev]);
      setFirstItemIndex(newOffset);
      earliestOffsetRef.current = newOffset;
      hasOlderRef.current = newOffset > 0;
      loadingOlderRef.current = false;
    });
  }, [selectedSessionId]);

  if (loading) {
    return <div className="p-6 text-zinc-500">Loading conversation...</div>;
  }
  if (!session) {
    return <div className="p-6 text-zinc-500">Session not found.</div>;
  }

  return (
    <div className="h-full flex flex-col">
      <SessionHeader
        session={session}
        onRefresh={() => {
          listSessions({ projectId: undefined }).then((sessions) => {
            const updated = sessions.find((s) => s.id === selectedSessionId);
            if (updated) setSession(updated);
          });
        }}
      />

      {/* Virtualized message list */}
      <div className="flex-1">
        <Virtuoso
          ref={virtuosoRef}
          data={messages}
          firstItemIndex={firstItemIndex}
          initialTopMostItemIndex={messages.length - 1}
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
                <div className="text-center text-xs text-zinc-400 py-2">
                  Loading older messages...
                </div>
              ) : null,
          }}
        />
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
