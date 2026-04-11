import type { ParsedMessage, ContentBlock } from "./types";

export interface ToolResult {
  content: string;
  isError: boolean;
}

/**
 * Build a map of tool_use_id -> tool result from a list of messages.
 * tool_result blocks live inside user messages as responses to assistant tool_use blocks.
 */
export function buildToolResultsMap(messages: ParsedMessage[]): Map<string, ToolResult> {
  const map = new Map<string, ToolResult>();

  for (const msg of messages) {
    if (msg.type !== "user") continue;
    for (const block of msg.content) {
      if (block.type === "tool_result" && block.tool_use_id) {
        const content = extractToolResultContent(block);
        map.set(block.tool_use_id, {
          content,
          isError: block.is_error ?? false,
        });
      }
    }
  }

  return map;
}

function extractToolResultContent(block: ContentBlock): string {
  // tool_result content can be a string or an array of content blocks
  const raw = block.content;
  if (typeof raw === "string") return raw;
  if (Array.isArray(raw)) {
    return raw
      .filter((b: Record<string, unknown>) => b.type === "text")
      .map((b: Record<string, unknown>) => b.text || "")
      .join("\n");
  }
  return String(raw ?? "");
}
