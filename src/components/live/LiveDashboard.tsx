import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { startLiveMonitor, stopLiveMonitor, getLiveSessions } from "../../lib/tauri";
import { useLiveStore } from "../../stores/liveStore";
import { useAppStore } from "../../stores/appStore";
import { LiveSessionCard } from "./LiveSessionCard";
import type { LiveSession } from "../../lib/types";

export function LiveDashboard() {
  const liveSessions = useLiveStore((s) => s.liveSessions);
  const setLiveSessions = useLiveStore((s) => s.setLiveSessions);
  const setView = useAppStore((s) => s.setView);
  const setWatchedSessionId = useLiveStore((s) => s.setWatchedSessionId);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    getLiveSessions().then(setLiveSessions).catch(console.error);
    startLiveMonitor().catch(console.error);

    const unlisten = listen<LiveSession[]>("live-sessions-update", (event) => {
      setLiveSessions(event.payload);
    });

    return () => {
      stopLiveMonitor().catch(console.error);
      unlisten.then((fn) => fn());
    };
  }, [setLiveSessions]);

  const handleSessionClick = (session: LiveSession) => {
    if (session.dbSessionId) {
      setWatchedSessionId(session.sessionId);
      setView("liveConversation");
    }
  };

  // Filter by session ID, slug, project name, or cwd
  const filtered = liveSessions.filter((s) => {
    if (!filter) return true;
    const q = filter.toLowerCase();
    return (
      s.sessionId.toLowerCase().includes(q) ||
      (s.slug || "").toLowerCase().includes(q) ||
      (s.projectName || "").toLowerCase().includes(q) ||
      s.cwd.toLowerCase().includes(q) ||
      String(s.pid).includes(q)
    );
  });

  // Sort: running first (by startedAt desc), then ended (by endedAt desc)
  const sorted = [...filtered].sort((a, b) => {
    if (a.isAlive !== b.isAlive) return a.isAlive ? -1 : 1;
    if (a.isAlive) return b.startedAt - a.startedAt;
    return (b.endedAt || 0) - (a.endedAt || 0);
  });

  const runningCount = liveSessions.filter((s) => s.isAlive).length;
  const endedCount = liveSessions.filter((s) => !s.isAlive).length;

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold">Live Sessions</h1>
        <p className="text-sm text-zinc-500 mt-1">
          {runningCount} running
          {endedCount > 0 && <> &middot; {endedCount} ended</>}
        </p>
      </div>

      {liveSessions.length > 0 && (
        <div className="mb-4 max-w-3xl">
          <input
            type="text"
            placeholder="Filter by session ID, slug, project, PID..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="w-full px-3 py-1.5 rounded border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm placeholder-zinc-400 focus:outline-none focus:border-zinc-500"
          />
        </div>
      )}

      {sorted.length === 0 ? (
        <div className="text-center text-zinc-400 py-12">
          {filter ? (
            <p className="text-lg">No sessions matching &ldquo;{filter}&rdquo;</p>
          ) : (
            <>
              <p className="text-lg">No active Claude Code sessions</p>
              <p className="text-sm mt-1">Sessions will appear here when Claude Code is running</p>
            </>
          )}
        </div>
      ) : (
        <div className="space-y-3 max-w-3xl">
          {sorted.map((session) => (
            <LiveSessionCard
              key={session.pid}
              session={session}
              onClick={() => handleSessionClick(session)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
