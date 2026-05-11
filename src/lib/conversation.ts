import { useRef } from "react";
import type { ViewMessage } from "./types";
import type { ToolResult } from "./toolResults";

/**
 * Shared helpers for the three conversation views (Claude / Live / Codex).
 * Extracted from inline copies that drifted apart over time.
 */

/** Incrementally accumulate tool results across batched message updates. */
export function useIncrementalToolResults(messages: ViewMessage[]) {
  const mapRef = useRef(new Map<string, ToolResult>());
  const processedRef = useRef(0);

  if (messages.length > processedRef.current) {
    for (let i = processedRef.current; i < messages.length; i++) {
      const msg = messages[i];
      if (msg.type !== "user") continue;
      for (const block of msg.content) {
        if (block.type === "toolResult" && block.toolCallId) {
          const content = extractToolResultContent(block);
          mapRef.current.set(block.toolCallId, {
            content,
            isError: block.isError ?? false,
          });
        }
      }
    }
    processedRef.current = messages.length;
  }
  return mapRef.current;
}

/** Coerce a tool_result `content` field (string | array | other) to a flat string. */
export function extractToolResultContent(block: { content?: unknown }): string {
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

/** Stable React key for a message, prefers UUID, falls back to index. */
export function getMessageKey(msg: ViewMessage, index: number): string {
  if (msg.type === "user" || msg.type === "assistant") return msg.id || `msg-${index}`;
  if (msg.type === "system") return msg.id || `sys-${index}`;
  return `msg-${index}`;
}

/** Find the assistant message containing an `Agent` tool_use whose description matches. */
export function findSubagentMessageIndex(messages: ViewMessage[], description: string): number {
  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    if (msg.type !== "assistant") continue;
    for (const block of msg.content) {
      if (
        block.type === "toolCall" &&
        block.name === "Agent" &&
        (block.input as { description?: string })?.description === description
      ) {
        return i;
      }
    }
  }
  return -1;
}
