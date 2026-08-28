import { useState } from "react";

export function ThinkingBlock({
  thinking,
  reasoningTokens,
  tokensShared,
}: {
  thinking: string;
  reasoningTokens?: number;
  tokensShared?: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  // Codex reports reasoning tokens per API response; when a turn produced several
  // thinking blocks they all share that one number, so label it as a turn total.
  const tokenLabel =
    reasoningTokens === undefined
      ? null
      : tokensShared
        ? ` · ${reasoningTokens} tokens/turn`
        : ` · ${reasoningTokens} tokens`;

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-3 py-2 text-xs text-zinc-500 bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-1"
      >
        <span className="font-mono">{expanded ? "\u25BC" : "\u25B6"}</span>
        Thinking ({thinking.length} chars{tokenLabel})
      </button>
      {expanded && (
        <div className="p-3 text-sm text-zinc-600 dark:text-zinc-400 whitespace-pre-wrap max-h-96 overflow-y-auto bg-zinc-50 dark:bg-zinc-900/50">
          {thinking}
        </div>
      )}
    </div>
  );
}
