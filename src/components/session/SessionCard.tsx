import type { SessionSummary } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";
import { toggleHideSession } from "../../lib/tauri";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { MultiplexerButton } from "../common/MultiplexerButton";
import { TagBadge } from "../common/TagBadge";

export function SessionCard({ session, onHide }: { session: SessionSummary; showHidden?: boolean; onHide?: () => void }) {
  const selectSession = useAppStore((s) => s.selectSession);

  const summaryIsHeuristic = session.summarySource === "heuristic";

  return (
    <button
      onClick={() => selectSession(session.id)}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          {session.summary ? (
            <div
              className={`text-sm font-medium truncate ${
                summaryIsHeuristic
                  ? "text-zinc-600 dark:text-zinc-400 italic"
                  : "text-zinc-900 dark:text-zinc-100"
              }`}
              title={session.summary}
            >
              {session.summary}
            </div>
          ) : (
            <div className="text-sm font-medium text-zinc-400 dark:text-zinc-500 italic truncate">
              {session.slug || "[Untitled session]"}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <span className="text-xs text-zinc-400">{formatDateTime(session.lastActive)}</span>
          <OpenTerminalButton path={session.projectPath} sessionId={session.sessionId} />
          <MultiplexerButton path={session.projectPath} />
          <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
          <button
            onClick={(e) => {
              e.stopPropagation();
              toggleHideSession(session.id).then(() => onHide?.());
            }}
            className="px-1.5 py-1 text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
            title={session.isHidden ? "Unhide session" : "Hide session"}
          >
            {session.isHidden ? "Unhide" : "Hide"}
          </button>
        </div>
      </div>
      <div className="flex items-center gap-2 mt-1 text-xs text-zinc-500">
        <span className="truncate">{session.projectName}</span>
        <CopyText text={session.sessionId} display={session.sessionId.slice(0, 8)} className="text-xs text-zinc-400 font-mono" />
        <span className="truncate">{session.gitBranch || "\u2014"}</span>
      </div>
      <div className="text-xs text-zinc-400 mt-0.5 truncate font-mono">
        {session.projectPath}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {session.userMsgCount} user | {session.messageCount} total
        {" | "}total {formatTokens(session.totalInputTokens + session.totalOutputTokens + session.totalCacheCreationTokens + session.totalCacheReadTokens)}
        {" | "}in {formatTokens(session.totalInputTokens)}
        {" | "}out {formatTokens(session.totalOutputTokens)}
        {" | "}cache R {formatTokens(session.totalCacheReadTokens)}
        {" | "}cache W {formatTokens(session.totalCacheCreationTokens)}
        {" | "}{formatFileSize(session.fileSize)}
        {session.version && <>{" | "}{session.version}</>}
      </div>
      {(session.tags.length > 0 || session.aiTags.length > 0) && (
        <div className="flex gap-1 mt-2 flex-wrap">
          {session.tags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} />
          ))}
          {session.aiTags.map((tag) => (
            <span
              key={`ai-${tag}`}
              className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs rounded border border-emerald-300 dark:border-emerald-800 text-emerald-700 dark:text-emerald-400"
              title="AI-generated tag"
            >
              <span className="text-[10px] opacity-70">AI</span>{tag}
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
