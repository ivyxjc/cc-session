import { useState } from "react";
import type { SessionSummary } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize } from "../../lib/format";
import { backupSession } from "../../lib/tauri";
import { FavoriteButton } from "../common/FavoriteButton";
import { TagBadge } from "../common/TagBadge";
import { TagManager } from "../common/TagManager";
import { useAppStore } from "../../stores/appStore";

export function SessionHeader({ session, onRefresh }: { session: SessionSummary; onRefresh?: () => void }) {
  const selectSession = useAppStore((s) => s.selectSession);
  const [showTagManager, setShowTagManager] = useState(false);
  const [backingUp, setBackingUp] = useState(false);

  const handleBackup = async () => {
    setBackingUp(true);
    try {
      await backupSession(session.id);
    } finally {
      setBackingUp(false);
    }
    onRefresh?.();
  };

  return (
    <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
      <div className="flex items-center gap-2">
        <button
          onClick={() => selectSession(null)}
          className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          &larr; Back
        </button>
        <div className="flex-1" />
        <button
          onClick={handleBackup}
          disabled={backingUp}
          className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
        >
          {backingUp ? "Backing up..." : "Backup"}
        </button>
        <div className="relative">
          <button
            onClick={() => setShowTagManager(!showTagManager)}
            className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
          >
            Tags
          </button>
          {showTagManager && (
            <div className="absolute right-0 top-8 z-10 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-lg">
              <TagManager
                sessionId={session.id}
                currentTags={session.tags}
                onUpdate={() => { setShowTagManager(false); onRefresh?.(); }}
              />
            </div>
          )}
        </div>
        <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
      </div>
      <h1 className="text-lg font-semibold mt-2">
        {session.slug || session.sessionId.slice(0, 8)}
      </h1>
      <div className="text-sm text-zinc-500 mt-0.5">
        {session.projectName} &middot; {session.gitBranch || "\u2014"} &middot; {session.version || "\u2014"} &middot; {session.permissionMode || "default"}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {formatDateTime(session.startedAt)} &middot; {session.messageCount} msgs &middot; {formatTokens(session.totalInputTokens + session.totalOutputTokens)} tokens &middot; {formatFileSize(session.fileSize)}
        {session.isBackedUp && " \u00B7 Backed up"}
      </div>
      {session.tags.length > 0 && (
        <div className="flex gap-1 mt-2">
          {session.tags.map((tag) => <TagBadge key={tag.id} tag={tag} />)}
        </div>
      )}
    </div>
  );
}
