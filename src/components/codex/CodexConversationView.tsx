import { useCallback, useEffect, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { codexGetLatestMessages, codexGetMessages, codexGetSubagents, codexGetSubagentMessages } from "../../lib/tauri";
import type { ViewMessage, CodexSubagent } from "../../lib/types";
import type { ToolResult } from "../../lib/toolResults";
import { useAppStore } from "../../stores/appStore";
import { MessageBubble } from "../message/MessageBubble";

function useIncrementalToolResults(messages: ViewMessage[]) {
  const mapRef = useRef(new Map<string, ToolResult>());
  const processedRef = useRef(0);

  if (messages.length > processedRef.current) {
    for (let i = processedRef.current; i < messages.length; i++) {
      const msg = messages[i];
      if (msg.type !== "user") continue;
      for (const block of msg.content) {
        if (block.type === "toolResult" && block.toolCallId) {
          const content = typeof block.content === "string" ? block.content : String(block.content ?? "");
          mapRef.current.set(block.toolCallId, { content, isError: block.isError ?? false });
        }
      }
    }
    processedRef.current = messages.length;
  }

  return mapRef.current;
}

function getMessageKey(msg: ViewMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.id || `msg-${index}`;
  if (msg.type === "system") return msg.id || `sys-${index}`;
  return `msg-${index}`;
}

const INITIAL_LOAD = 100;
const OLDER_BATCH = 50;

export function CodexConversationView() {
  const selectedCodexThreadId = useAppStore((s) => s.selectedCodexThreadId);
  const setView = useAppStore((s) => s.setView);

  const [messages, setMessages] = useState<ViewMessage[]>([]);
  const [subagents, setSubagents] = useState<CodexSubagent[]>([]);
  const [loading, setLoading] = useState(true);
  const [subagentsExpanded, setSubagentsExpanded] = useState(false);

  const [firstItemIndex, setFirstItemIndex] = useState(0);
  const loadingOlderRef = useRef(false);
  const earliestOffsetRef = useRef(0);
  const hasOlderRef = useRef(false);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const toolResults = useIncrementalToolResults(messages);

  useEffect(() => {
    if (!selectedCodexThreadId) return;
    setLoading(true);
    setSubagentsExpanded(false);

    Promise.all([
      codexGetLatestMessages(selectedCodexThreadId, INITIAL_LOAD),
      codexGetSubagents(selectedCodexThreadId),
    ]).then(([result, subs]) => {
      const startOffset = result.totalCount - result.messages.length;
      setMessages(result.messages);
      setSubagents(subs);
      setFirstItemIndex(startOffset);
      earliestOffsetRef.current = startOffset;
      hasOlderRef.current = startOffset > 0;
      loadingOlderRef.current = false;
      setLoading(false);
    });
  }, [selectedCodexThreadId]);

  const handleStartReached = useCallback(() => {
    if (!selectedCodexThreadId || !hasOlderRef.current || loadingOlderRef.current) return;
    loadingOlderRef.current = true;

    const loadCount = Math.min(OLDER_BATCH, earliestOffsetRef.current);
    const newOffset = earliestOffsetRef.current - loadCount;

    codexGetMessages(selectedCodexThreadId, newOffset, loadCount).then((older) => {
      setMessages((prev) => [...older, ...prev]);
      setFirstItemIndex(newOffset);
      earliestOffsetRef.current = newOffset;
      hasOlderRef.current = newOffset > 0;
      loadingOlderRef.current = false;
    });
  }, [selectedCodexThreadId]);

  if (!selectedCodexThreadId) {
    return <div className="p-6 text-zinc-500">No session selected</div>;
  }

  if (loading) {
    return <div className="p-6 text-zinc-500">Loading conversation...</div>;
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
        <div className="flex items-center gap-2">
          <button
            onClick={() => setView("codexSessions")}
            className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
          >
            &larr; Sessions
          </button>
          <span className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950 px-2 py-0.5 rounded-full font-medium">
            Codex
          </span>
        </div>
        <div className="text-xs text-zinc-400 font-mono mt-1">{selectedCodexThreadId}</div>
      </div>

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
                toolResults={toolResults}
              />
            </div>
          )}
          components={{
            Header: () =>
              hasOlderRef.current && loadingOlderRef.current ? (
                <div className="text-center text-xs text-zinc-400 py-2">Loading older messages...</div>
              ) : null,
          }}
        />
      </div>

      {/* Subagents */}
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
                <CodexSubagentView key={sa.id} subagent={sa} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function CodexSubagentView({ subagent }: { subagent: CodexSubagent }) {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<ViewMessage[]>([]);
  const [loaded, setLoaded] = useState(false);

  const handleExpand = async () => {
    if (!loaded) {
      const msgs = await codexGetSubagentMessages(subagent.id, 0, 200);
      setMessages(msgs);
      setLoaded(true);
    }
    setExpanded(!expanded);
  };

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden">
      <button
        onClick={handleExpand}
        className="w-full text-left px-3 py-2 text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "\u25BC" : "\u25B6"}</span>
        <span className="font-medium text-emerald-600 dark:text-emerald-400">
          {subagent.nickname || "Agent"}
        </span>
        {subagent.role && <span className="text-zinc-400 text-xs">[{subagent.role}]</span>}
        <span className="text-zinc-500 truncate">{subagent.title}</span>
      </button>
      {expanded && (
        <div className="p-3 space-y-3 max-h-96 overflow-y-auto bg-zinc-50/50 dark:bg-zinc-900/50">
          {messages.length === 0 && loaded && (
            <div className="text-xs text-zinc-400 italic">No messages found</div>
          )}
          {messages.map((msg, i) => (
            <MessageBubble key={i} message={msg} />
          ))}
        </div>
      )}
    </div>
  );
}
