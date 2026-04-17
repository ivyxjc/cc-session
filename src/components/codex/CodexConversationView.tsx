import { useCallback, useEffect, useRef, useState } from "react";
import { Virtuoso, type VirtuosoHandle } from "react-virtuoso";
import { codexGetSession, codexGetLatestMessages, codexGetMessages, codexGetSubagents, codexGetSubagentMessages, exportCodexSession } from "../../lib/tauri";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "../../stores/toastStore";
import type { ViewMessage, CodexSession, CodexSubagent } from "../../lib/types";
import type { ToolResult } from "../../lib/toolResults";
import { formatTokens, formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";
import { CopyText } from "../common/CopyText";
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

  const [session, setSession] = useState<CodexSession | null>(null);
  const [messages, setMessages] = useState<ViewMessage[]>([]);
  const [subagents, setSubagents] = useState<CodexSubagent[]>([]);
  const [loading, setLoading] = useState(true);
  const [subagentsExpanded, setSubagentsExpanded] = useState(false);
  const [exporting, setExporting] = useState(false);

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
      codexGetSession(selectedCodexThreadId),
      codexGetLatestMessages(selectedCodexThreadId, INITIAL_LOAD),
      codexGetSubagents(selectedCodexThreadId),
    ]).then(([sess, result, subs]) => {
      setSession(sess);
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
          <div className="flex-1" />
          <button
            onClick={async () => {
              if (!selectedCodexThreadId) return;
              const slug = session?.title?.slice(0, 30).replace(/[^a-zA-Z0-9]/g, "_") || selectedCodexThreadId.slice(0, 8);
              const defaultDir = session?.cwd || "";
              const filePath = await saveDialog({
                defaultPath: defaultDir ? `${defaultDir}/${slug}.zip` : `${slug}.zip`,
                filters: [{ name: "ZIP", extensions: ["zip"] }],
              });
              if (filePath) {
                setExporting(true);
                try {
                  await exportCodexSession(selectedCodexThreadId, filePath as string);
                  toast.success("Export successful!");
                } catch (e) {
                  toast.error(`Export failed: ${e}`);
                }
                setExporting(false);
              }
            }}
            disabled={exporting}
            className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
          >
            {exporting ? "Exporting..." : "Export zip"}
          </button>
          {session?.source && (
            <span className={`text-xs px-1.5 py-0.5 rounded font-medium ${
              session.source === "cli" ? "text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-950" :
              session.source === "vscode" ? "text-purple-600 dark:text-purple-400 bg-purple-50 dark:bg-purple-950" :
              "text-orange-600 dark:text-orange-400 bg-orange-50 dark:bg-orange-950"
            }`}>
              {session.source}
            </span>
          )}
        </div>
        <h1 className="text-lg font-semibold mt-2 break-words">
          {session?.title || session?.firstUserMessage?.slice(0, 80) || "Untitled"}
        </h1>
        <CopyText text={selectedCodexThreadId} className="text-sm text-zinc-400 font-mono" />
        <div className="text-sm text-zinc-500 mt-0.5">
          {session?.cwd?.split("/").pop() || "\u2014"} &middot; {session?.gitBranch || "\u2014"} &middot; {session?.model || "\u2014"} &middot; {session?.approvalMode || "default"}
        </div>
        <div className="text-xs text-zinc-400 mt-1">
          {messages.length} msgs &middot; {formatTokens(session?.tokensUsed || 0)} tokens &middot; v{session?.cliVersion || "?"} &middot; {formatRelativeTime(session ? session.updatedAt * 1000 : null)}
        </div>
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
