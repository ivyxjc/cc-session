import type { SessionSummary } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { TagBadge } from "../common/TagBadge";

export function SessionCard({ session }: { session: SessionSummary }) {
  const selectSession = useAppStore((s) => s.selectSession);

  return (
    <button
      onClick={() => selectSession(session.id)}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="font-medium truncate">
            {session.slug || session.sessionId.slice(0, 8)}
          </span>
          <CopyText text={session.sessionId} display={session.sessionId.slice(0, 8)} />
        </div>
        <div className="flex items-center gap-2 ml-2 shrink-0">
          <span className="text-xs text-zinc-400">{formatDateTime(session.lastActive)}</span>
          <OpenTerminalButton path={session.projectPath} />
          <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
        </div>
      </div>
      <div className="text-sm text-zinc-500 mt-0.5">
        {session.projectName} &middot; {session.gitBranch || "\u2014"} &middot; {session.version || "\u2014"}
      </div>
      <div className="text-xs text-zinc-400 mt-0.5 truncate font-mono">
        {session.projectPath}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {session.messageCount} msgs &middot; total {formatTokens(session.totalInputTokens + session.totalOutputTokens + session.totalCacheCreationTokens + session.totalCacheReadTokens)} &middot; in {formatTokens(session.totalInputTokens)} &middot; out {formatTokens(session.totalOutputTokens)} &middot; cache R {formatTokens(session.totalCacheReadTokens)} &middot; cache W {formatTokens(session.totalCacheCreationTokens)} &middot; {formatFileSize(session.fileSize)}
      </div>
      {session.tags.length > 0 && (
        <div className="flex gap-1 mt-2 flex-wrap">
          {session.tags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} />
          ))}
        </div>
      )}
    </button>
  );
}
