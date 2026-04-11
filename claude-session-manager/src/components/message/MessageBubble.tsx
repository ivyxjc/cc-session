import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { ParsedMessage, ContentBlock, SubagentSummary } from "../../lib/types";
import type { ToolResult } from "../../lib/toolResults";
import { ThinkingBlock } from "./ThinkingBlock";
import { ToolCallBlock } from "./ToolCallBlock";
import { DiffView } from "./DiffView";
import { CodeBlock } from "./CodeBlock";
import { extractImagePath, ImageFromPath } from "./ImageFromPath";

function renderContentBlock(
  block: ContentBlock,
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

    case "thinking":
      return <ThinkingBlock key={index} thinking={block.thinking || ""} />;

    case "image": {
      const src = block.source;
      if (src?.type === "base64" && src.data && src.media_type) {
        return (
          <div key={index} className="my-1">
            <img
              src={`data:${src.media_type};base64,${src.data}`}
              alt="User image"
              className="max-w-full max-h-96 rounded border border-zinc-200 dark:border-zinc-700"
              loading="lazy"
            />
          </div>
        );
      }
      return null;
    }

    case "tool_use": {
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
  message: ParsedMessage;
  subagents?: SubagentSummary[];
  toolResults?: Map<string, ToolResult>;
}

export function MessageBubble({ message, subagents, toolResults }: Props) {
  if (message.type === "permissionMode" || message.type === "fileHistorySnapshot" || message.type === "attachment") {
    return null;
  }

  if (message.type === "system") {
    if (!message.content) return null;
    return (
      <div className="text-xs text-zinc-400 italic py-1">
        {message.subtype && <span className="font-medium">[{message.subtype}]</span>} {message.content}
      </div>
    );
  }

  const isUser = message.type === "user";

  // Skip user messages that only contain tool_result blocks (automatic tool responses, not real user input)
  if (isUser && message.content.length > 0 && message.content.every((b) => b.type === "tool_result")) {
    return null;
  }

  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div
        className={`max-w-[85%] rounded-lg p-3 space-y-2 ${
          isUser
            ? "bg-zinc-200 dark:bg-zinc-700"
            : "bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700"
        }`}
      >
        <div className="text-xs font-medium text-zinc-500 mb-1">
          {isUser ? "You" : `Claude${message.type === "assistant" && message.model ? ` (${message.model})` : ""}`}
        </div>
        {message.content.map((block, i) => renderContentBlock(block, i, subagents, toolResults))}
      </div>
    </div>
  );
}
