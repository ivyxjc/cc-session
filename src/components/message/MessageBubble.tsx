import { memo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ViewMessage, ViewContentBlock, SubagentSummary } from "../../lib/types";
import type { ToolResult } from "../../lib/toolResults";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallBlock } from "./ToolCallBlock";
import { DiffView } from "./DiffView";
import { CodeBlock } from "./CodeBlock";
import { extractImagePath, ImageFromPath } from "./ImageFromPath";
import { CopyButton } from "../common/CopyButton";
import { formatDateTime } from "../../lib/format";

function MessageTime({ timestamp }: { timestamp: string | null }) {
  if (!timestamp) return null;
  const ms = Date.parse(timestamp);
  if (Number.isNaN(ms)) return null;
  return (
    <span className="text-xs text-zinc-400 shrink-0 tabular-nums">
      {formatDateTime(ms)}
    </span>
  );
}

function blockToText(block: ViewContentBlock): string {
  switch (block.type) {
    case "text":
      return block.text || "";
    case "thinking":
      return block.thinking ? `[thinking]\n${block.thinking}` : "";
    case "toolCall": {
      const name = block.name || "tool";
      const input = block.input ? JSON.stringify(block.input, null, 2) : "";
      return input ? `[tool: ${name}]\n${input}` : `[tool: ${name}]`;
    }
    case "toolResult": {
      const raw = block.content;
      let content: string;
      if (typeof raw === "string") {
        content = raw;
      } else if (Array.isArray(raw)) {
        content = raw
          .filter((b: Record<string, unknown>) => b.type === "text")
          .map((b: Record<string, unknown>) => b.text || "")
          .join("\n");
      } else {
        content = String(raw ?? "");
      }
      const prefix = block.isError ? "[tool result · error]" : "[tool result]";
      return content ? `${prefix}\n${content}` : prefix;
    }
    case "image":
      return "[image]";
    default:
      return "";
  }
}

function messageToText(message: ViewMessage): string {
  if (message.type === "system") {
    return message.content || "";
  }
  return message.content
    .map(blockToText)
    .filter((s) => s.length > 0)
    .join("\n\n");
}

function renderContentBlock(
  block: ViewContentBlock,
  index: number,
  subagents?: SubagentSummary[],
  toolResults?: Map<string, ToolResult>,
) {
  switch (block.type) {
    case "text": {
      const text = block.text || "";
      // Detect [Image: source: /path] or [Image source: /path] patterns
      const imagePath = extractImagePath(text);
      if (imagePath) {
        return (
          <div key={index} className="my-1">
            <ImageFromPath path={imagePath} />
          </div>
        );
      }
      return (
        <div key={index} className="prose dark:prose-invert prose-sm max-w-none">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children }) {
                const match = /language-(\w+)/.exec(className || "");
                const code = String(children).replace(/\n$/, "");
                if (match) {
                  return <CodeBlock code={code} language={match[1]} />;
                }
                return <code className="bg-zinc-100 dark:bg-zinc-800 px-1 py-0.5 rounded text-sm">{children}</code>;
              },
            }}
          >
            {text}
          </ReactMarkdown>
        </div>
      );
    }

    case "thinking": {
      // Claude Opus extended thinking can return "redacted" blocks:
      // empty `thinking` text + non-empty `signature` (encrypted reasoning).
      // Show a compact placeholder so the user knows reasoning happened, but skip the empty expander.
      const text = block.thinking || "";
      if (!text.trim()) {
        return (
          <div key={index} className="text-xs text-zinc-400 italic px-2 py-1">
            (redacted thinking)
          </div>
        );
      }
      return <ThinkingBlock key={index} thinking={text} />;
    }

    case "image": {
      const src = block.source;
      if (src?.sourceType === "base64" && src.data && src.mediaType) {
        return (
          <div key={index} className="my-1">
            <img
              src={`data:${src.mediaType};base64,${src.data}`}
              alt="User image"
              className="max-w-full max-h-96 rounded border border-zinc-200 dark:border-zinc-700"
              loading="lazy"
            />
          </div>
        );
      }
      return null;
    }

    case "toolCall": {
      // Special case: Edit tool -- show diff
      if (block.name === "Edit" && block.input) {
        const input = block.input as { file_path?: string; old_string?: string; new_string?: string };
        if (input.old_string && input.new_string) {
          return (
            <DiffView
              key={index}
              filePath={input.file_path || ""}
              oldString={input.old_string}
              newString={input.new_string}
            />
          );
        }
      }
      const result = block.id ? toolResults?.get(block.id) : undefined;
      return <ToolCallBlock key={index} block={block} subagents={subagents} toolResult={result} />;
    }

    default:
      return null;
  }
}

interface Props {
  message: ViewMessage;
  subagents?: SubagentSummary[];
  toolResults?: Map<string, ToolResult>;
}

export const MessageBubble = memo(function MessageBubble({ message, subagents, toolResults }: Props) {
  if (message.type === "system") {
    // Skip attachment, permissionMode, fileHistorySnapshot subtypes
    if (message.subtype === "attachment" || message.subtype === "permissionMode" || message.subtype === "fileHistorySnapshot") {
      return null;
    }
    if (!message.content) return null;
    return (
      <div className="group flex items-start gap-1.5 text-xs text-zinc-400 italic py-1">
        <div className="flex-1 min-w-0">
          {message.subtype && <span className="font-medium">[{message.subtype}]</span>} {message.content}
        </div>
        <MessageTime timestamp={message.timestamp} />
        <span className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
          <CopyButton text={messageToText(message)} title="Copy message" />
        </span>
      </div>
    );
  }

  const isUser = message.type === "user";

  // Skip user messages that only contain toolResult blocks (automatic tool responses, not real user input)
  if (isUser && message.content.length > 0 && message.content.every((b) => b.type === "toolResult")) {
    return null;
  }

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`group max-w-[85%] rounded-lg p-3 space-y-2 ${
          isUser
            ? "bg-zinc-200 dark:bg-zinc-700"
            : "bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700"
        }`}
      >
        <div className="flex items-center gap-2 mb-1">
          <div className="text-xs font-medium text-zinc-500 flex-1 min-w-0 truncate">
            {isUser ? "You" : `Claude${message.type === "assistant" && message.model ? ` (${message.model})` : ""}`}
          </div>
          <MessageTime timestamp={message.timestamp} />
          <span className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
            <CopyButton text={messageToText(message)} title="Copy message" />
          </span>
        </div>
        {message.content.map((block, i) => renderContentBlock(block, i, subagents, toolResults))}
      </div>
    </div>
  );
});
