import { useState } from "react";
import { CodeBlock } from "./CodeBlock";
import type { ContentBlock } from "../../lib/types";

export function ToolCallBlock({ block }: { block: ContentBlock }) {
  const [expanded, setExpanded] = useState(false);

  const toolName = block.name || "Unknown";
  const input = block.input || {};

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

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden my-1">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-3 py-2 text-sm bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "\u25BC" : "\u25B6"}</span>
        <span className="font-medium text-blue-600 dark:text-blue-400">{toolName}</span>
        <span className="text-zinc-500 truncate">{summary}</span>
      </button>
      {expanded && (
        <div className="p-3 space-y-2">
          {codeContent && <CodeBlock code={codeContent} language={language} />}
          {!codeContent && (
            <pre className="text-xs text-zinc-500 overflow-x-auto">
              {JSON.stringify(input, null, 2)}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
