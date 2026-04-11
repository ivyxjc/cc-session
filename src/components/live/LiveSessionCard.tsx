import type { LiveSession } from "../../lib/types";
import { formatTokens, formatRelativeTime } from "../../lib/format";
import { CopyText } from "../common/CopyText";
import { LiveStatusBadge } from "./LiveStatusBadge";
import { RunningTimer } from "./RunningTimer";

interface Props {
  session: LiveSession;
  onClick: () => void;
}

export function LiveSessionCard({ session, onClick }: Props) {
  const displayName = session.slug || session.sessionId.slice(0, 8);
  const projectName = session.projectName || session.cwd.split("/").pop() || session.cwd;

  const totalTokens =
    (session.totalInputTokens || 0) +
    (session.totalOutputTokens || 0) +
    (session.totalCacheCreationTokens || 0) +
    (session.totalCacheReadTokens || 0);

  return (
    <button
      onClick={onClick}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <LiveStatusBadge isAlive={session.isAlive} />
          <span className="font-medium truncate">{displayName}</span>
          <CopyText text={session.sessionId} display={session.sessionId.slice(0, 8)} />
        </div>
        <div className="text-xs text-zinc-400 shrink-0 ml-2">
          {session.isAlive ? (
            <RunningTimer startedAt={session.startedAt} />
          ) : (
            formatRelativeTime(session.endedAt)
          )}
        </div>
      </div>

      <div className="text-sm text-zinc-500 mt-1">
        {projectName}
        {session.gitBranch && <> &middot; {session.gitBranch}</>}
        {" "}&middot; {session.kind}
      </div>

      <div className="text-xs text-zinc-400 mt-0.5 truncate font-mono">
        {session.cwd}
      </div>

      <div className="text-xs text-zinc-400 mt-1">
        {session.messageCount != null && <>{session.messageCount} msgs &middot; </>}
        {totalTokens > 0 && <>{formatTokens(totalTokens)} tokens &middot; </>}
        PID {session.pid}
        {session.activeSubagentCount != null && session.activeSubagentCount > 0 && (
          <> &middot; {session.activeSubagentCount} subagent{session.activeSubagentCount > 1 ? "s" : ""}</>
        )}
      </div>

      {session.lastMessagePreview && (
        <div className="text-xs text-zinc-500 mt-1 truncate italic">
          &gt; {session.lastMessagePreview}
        </div>
      )}
    </button>
  );
}
