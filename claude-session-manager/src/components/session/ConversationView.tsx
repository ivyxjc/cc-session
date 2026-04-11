import { useEffect, useState } from "react";
import { getMessages, getSubagents, listSessions } from "../../lib/tauri";
import type { ParsedMessage, SessionSummary, SubagentSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { SessionHeader } from "./SessionHeader";
import { MessageBubble } from "../message/MessageBubble";
import { SubagentView } from "../message/SubagentView";

export function ConversationView() {
  const { selectedSessionId } = useAppStore();
  const [session, setSession] = useState<SessionSummary | null>(null);
  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [subagents, setSubagents] = useState<SubagentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(true);

  useEffect(() => {
    if (!selectedSessionId) return;
    setLoading(true);
    setOffset(0);
    setMessages([]);

    Promise.all([
      listSessions({ projectId: undefined }).then((sessions) =>
        sessions.find((s) => s.id === selectedSessionId) || null
      ),
      getMessages(selectedSessionId, 0, 50),
      getSubagents(selectedSessionId),
    ]).then(([sess, msgs, subs]) => {
      setSession(sess);
      setMessages(msgs);
      setSubagents(subs);
      setHasMore(msgs.length === 50);
      setOffset(50);
      setLoading(false);
    });
  }, [selectedSessionId]);

  const loadMore = async () => {
    if (!selectedSessionId || !hasMore) return;
    const more = await getMessages(selectedSessionId, offset, 50);
    setMessages((prev) => [...prev, ...more]);
    setHasMore(more.length === 50);
    setOffset((prev) => prev + 50);
  };

  if (loading || !session) {
    return <div className="p-6 text-zinc-500">Loading conversation...</div>;
  }

  return (
    <div className="h-full flex flex-col">
      <SessionHeader session={session} onRefresh={() => {
        // Re-fetch session metadata to reflect tag/backup changes
        listSessions({ projectId: undefined }).then((sessions) => {
          const updated = sessions.find((s) => s.id === selectedSessionId);
          if (updated) setSession(updated);
        });
      }} />
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.map((msg, i) => (
          <MessageBubble key={i} message={msg} />
        ))}
        {hasMore && (
          <button
            onClick={loadMore}
            className="w-full py-2 text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
          >
            Load more messages...
          </button>
        )}
        {subagents.length > 0 && (
          <div className="mt-4 pt-4 border-t border-zinc-200 dark:border-zinc-800">
            <h3 className="text-sm font-medium text-zinc-500 mb-2">Subagents ({subagents.length})</h3>
            {subagents.map((sa) => (
              <SubagentView key={sa.id} subagent={sa} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
