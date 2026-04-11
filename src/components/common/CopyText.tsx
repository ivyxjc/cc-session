import { useState } from "react";

export function CopyText({ text, display, className }: { text: string; display?: string; className?: string }) {
  const [copied, setCopied] = useState(false);

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 1000);
  };

  return (
    <span className="relative inline-flex items-center">
      <span
        onClick={handleClick}
        title="Click to copy"
        className={`cursor-pointer hover:text-zinc-600 dark:hover:text-zinc-300 ${className || "text-xs text-zinc-400 font-mono"}`}
      >
        {display || text}
      </span>
      {copied && (
        <span className="absolute -top-7 left-1/2 -translate-x-1/2 px-2 py-0.5 bg-zinc-800 dark:bg-zinc-200 text-white dark:text-zinc-800 text-xs rounded shadow whitespace-nowrap">
          Copied!
        </span>
      )}
    </span>
  );
}
