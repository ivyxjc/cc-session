import { useState } from "react";
import { getSubagentMessages } from "../../lib/tauri";
import type { ParsedMessage, SubagentSummary } from "../../lib/types";
import { MessageBubble } from "./MessageBubble";

export function SubagentView({ subagent }: { subagent: SubagentSummary }) {
  const [expanded, setExpanded] = useState(false);
  const [messages, setMessages] = useState<ParsedMessage[]>([]);
  const [loaded, setLoaded] = useState(false);

  const handleExpand = async () => {
    if (!loaded) {
      const msgs = await getSubagentMessages(subagent.id, 0, 200);
      setMessages(msgs);
      setLoaded(true);
    }
    setExpanded(!expanded);
  };

  return (
    <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden my-2">
      <button
        onClick={handleExpand}
        className="w-full text-left px-3 py-2 text-sm bg-zinc-50 dark:bg-zinc-900 hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
      >
        <span className="font-mono text-xs">{expanded ? "\u25BC" : "\u25B6"}</span>
        <span className="font-medium text-purple-600 dark:text-purple-400">Agent</span>
        <span className="text-zinc-500">[{subagent.agentType}]</span>
        <span className="text-zinc-400 truncate">{subagent.description}</span>
      </button>
      {expanded && (
        <div className="p-3 space-y-3 max-h-96 overflow-y-auto bg-zinc-50/50 dark:bg-zinc-900/50">
          {messages.map((msg, i) => (
            <MessageBubble key={i} message={msg} />
          ))}
        </div>
      )}
    </div>
  );
}
