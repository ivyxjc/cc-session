import type { LiveSession } from "../../lib/types";
import { formatTokens, formatRelativeTime, formatFileSize } from "../../lib/format";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { MultiplexerButton } from "../common/MultiplexerButton";
import { LiveStatusBadge } from "./LiveStatusBadge";
import { RunningTimer } from "./RunningTimer";

interface Props {
  session: LiveSession;
  onClick: () => void;
}

export function LiveSessionCard({ session, onClick }: Props) {
  const projectName = session.projectName || session.cwd.split("/").pop() || session.cwd;

  return (
    <button
      onClick={onClick}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      {/* Row 1: status + name + session id + actions */}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <LiveStatusBadge isAlive={session.isAlive} />
          <span className="font-medium truncate">{projectName}</span>
          <CopyText text={session.sessionId} display={session.sessionId.slice(0, 8)} className="text-sm text-zinc-400 font-mono" />
          <span className="text-sm text-zinc-500">{session.gitBranch || "\u2014"}</span>
        </div>
        <div className="flex items-center gap-2 ml-2 shrink-0">
          <span className="text-xs text-zinc-400">
            {session.isAlive ? (
              <RunningTimer startedAt={session.startedAt} />
            ) : (
              formatRelativeTime(session.endedAt)
            )}
          </span>
          <OpenTerminalButton path={session.cwd} />
          <MultiplexerButton path={session.cwd} />
          {session.dbSessionId && (
            <FavoriteButton sessionId={session.dbSessionId} initialFavorited={false} />
          )}
        </div>
      </div>

      <div className="text-xs text-zinc-400 mt-0.5 truncate font-mono">
        {session.cwd}
      </div>

      <div className="text-xs text-zinc-400 mt-1">
        {session.userMsgCount != null && <>{session.userMsgCount} user</>}
        {session.messageCount != null && <>{" | "}{session.messageCount} total</>}
        {" | "}total {formatTokens((session.totalInputTokens || 0) + (session.totalOutputTokens || 0) + (session.totalCacheCreationTokens || 0) + (session.totalCacheReadTokens || 0))}
        {" | "}in {formatTokens(session.totalInputTokens || 0)}
        {" | "}out {formatTokens(session.totalOutputTokens || 0)}
        {" | "}cache R {formatTokens(session.totalCacheReadTokens || 0)}
        {" | "}cache W {formatTokens(session.totalCacheCreationTokens || 0)}
        {session.fileSize != null && <>{" | "}{formatFileSize(session.fileSize)}</>}
        {" | "}PID {session.pid}
        {session.activeSubagentCount != null && session.activeSubagentCount > 0 && (
          <>{" | "}{session.activeSubagentCount} subagent{session.activeSubagentCount > 1 ? "s" : ""}</>
        )}
        {session.version && <>{" | "}{session.version}</>}
      </div>
    </button>
  );
}
