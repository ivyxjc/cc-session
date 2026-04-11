import { useState } from "react";
import { getSubagentMessages } from "../../lib/tauri";
import type { ParsedMessage, SubagentSummary } from "../../lib/types";
import { MessageBubble } from "./MessageBubble";

interface Props {
  subagent: SubagentSummary;
  onLocate?: () => void;
}

export function SubagentView({ subagent, onLocate }: Props) {
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
      <div className="flex items-center bg-zinc-50 dark:bg-zinc-900">
        <button
          onClick={handleExpand}
          className="flex-1 text-left px-3 py-2 text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 flex items-center gap-2"
        >
          <span className="font-mono text-xs">{expanded ? "\u25BC" : "\u25B6"}</span>
          <span className="font-medium text-purple-600 dark:text-purple-400">Agent</span>
          <span className="text-zinc-500">[{subagent.agentType}]</span>
          <span className="text-zinc-400 truncate">{subagent.description}</span>
        </button>
        {onLocate && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onLocate();
            }}
            className="px-2 py-2 text-zinc-400 hover:text-blue-500 dark:hover:text-blue-400 shrink-0 transition-colors"
            title="Scroll to this agent call in conversation"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M8 2L8 11M8 2L4.5 5.5M8 2L11.5 5.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
              <path d="M3 13.5H13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </button>
        )}
      </div>
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
