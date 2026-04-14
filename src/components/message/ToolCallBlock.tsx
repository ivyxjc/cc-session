import { useState } from "react";
import { CodeBlock } from "./CodeBlock";
import { getSubagentMessages } from "../../lib/tauri";
import type { ViewContentBlock, ViewMessage, SubagentSummary } from "../../lib/types";
import type { ToolResult } from "../../lib/toolResults";
import { MessageBubble } from "./MessageBubble";

interface Props {
  block: ViewContentBlock;
  subagents?: SubagentSummary[];
  toolResult?: ToolResult;
}

export function ToolCallBlock({ block, subagents, toolResult }: Props) {
  const [expanded, setExpanded] = useState(false);
  const [agentMessages, setAgentMessages] = useState<ViewMessage[]>([]);
  const [agentLoaded, setAgentLoaded] = useState(false);

  const toolName = block.name || "Unknown";
  const input = block.input || {};

  // For Agent tool calls, find the matching subagent
  const matchedSubagent =
    toolName === "Agent" && subagents
      ? subagents.find((sa) => {
          const desc = (input as { description?: string }).description || "";
          return sa.description === desc;
        })
      : undefined;

  // Extract display info based on tool type
  let summary = "";
  let codeContent = "";
  let language = "text";

  switch (toolName) {
    case "Bash": {
      summary = (input as { command?: string }).command?.split("\n")[0] || "";
      codeContent = (input as { command?: string }).command || "";
      language = "bash";
      break;
    }
    case "Read": {
      summary = (input as { file_path?: string }).file_path || "";
      break;
    }
    case "Edit": {
      summary = (input as { file_path?: string }).file_path || "";
      break;
    }
    case "Write": {
      summary = (input as { file_path?: string }).file_path || "";
      codeContent = (input as { content?: string }).content || "";
      break;
    }
    case "Grep": {
      summary = `"${(input as { pattern?: string }).pattern || ""}"`;
      break;
    }
    case "Glob": {
      summary = (input as { pattern?: string }).pattern || "";
      break;
    }
    case "Agent": {
      summary = (input as { description?: string }).description || "";
      break;
    }
    default: {
      summary = JSON.stringify(input).slice(0, 100);
    }
  }

  const handleExpand = async () => {
    if (matchedSubagent && !agentLoaded) {
      const msgs = await getSubagentMessages(matchedSubagent.id, 0, 200);
      setAgentMessages(msgs);
      setAgentLoaded(true);
    }
    setExpanded(!expanded);
  };

  const isAgent = toolName === "Agent";
  const headerColor = isAgent
    ? "text-purple-600 dark:text-purple-400"
    : "text-blue-600 dark:text-blue-400";

  const agentType = matchedSubagent
    ? matchedSubagent.agentType
    : (input as { subagent_type?: string }).subagent_type;

  // Has output to show?
  const hasResult = toolResult && toolResult.content;

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden my-1">
      <button
        onClick={handleExpand}
        className="w-full text-left px-3 py-2 text-sm bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "\u25BC" : "\u25B6"}</span>
        <span className={`font-medium ${headerColor}`}>{toolName}</span>
        {isAgent && agentType && (
          <span className="text-zinc-400 text-xs">[{agentType}]</span>
        )}
        <span className="text-zinc-500 truncate">{summary}</span>
        {toolResult?.isError && (
          <span className="text-red-500 text-xs ml-auto shrink-0">error</span>
        )}
      </button>
      {expanded && (
        <div className="p-3 space-y-2">
          {/* Tool input */}
          {isAgent && matchedSubagent && agentMessages.length > 0 ? (
            <div className="space-y-3 max-h-[600px] overflow-y-auto bg-zinc-50/50 dark:bg-zinc-900/50 rounded p-2">
              {agentMessages.map((msg, i) => (
                <MessageBubble key={i} message={msg} />
              ))}
            </div>
          ) : isAgent && matchedSubagent && agentLoaded ? (
            <div className="text-xs text-zinc-400 italic">No messages found for this subagent</div>
          ) : (
            <>
              {codeContent && <CodeBlock code={codeContent} language={language} />}
              {!codeContent && (
                <pre className="text-xs text-zinc-500 overflow-x-auto whitespace-pre-wrap break-all">
                  {JSON.stringify(input, null, 2)}
                </pre>
              )}
            </>
          )}

          {/* Tool output */}
          {hasResult && (
            <div className={`mt-2 border-t border-zinc-200 dark:border-zinc-700 pt-2`}>
              <div className="text-xs font-medium text-zinc-400 mb-1">Output</div>
              <pre
                className={`text-xs overflow-x-auto whitespace-pre-wrap break-all max-h-80 overflow-y-auto p-2 rounded ${
                  toolResult.isError
                    ? "text-red-500 bg-red-50 dark:bg-red-950/30"
                    : "text-zinc-600 dark:text-zinc-300 bg-zinc-100 dark:bg-zinc-800"
                }`}
              >
                {toolResult.content}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
