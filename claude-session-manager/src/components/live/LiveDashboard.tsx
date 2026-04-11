import { useEffect } from "react";
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

  useEffect(() => {
    // Initial fetch
    getLiveSessions().then(setLiveSessions).catch(console.error);

    // Start polling
    startLiveMonitor().catch(console.error);

    // Listen for updates
    const unlisten = listen<LiveSession[]>("live-sessions-update", (event) => {
      setLiveSessions(event.payload);
    });

    return () => {
      stopLiveMonitor().catch(console.error);
      unlisten.then((fn) => fn());
    };
  }, [setLiveSessions]);

  const runningCount = liveSessions.filter((s) => s.isAlive).length;
  const endedCount = liveSessions.filter((s) => !s.isAlive).length;

  const handleSessionClick = (session: LiveSession) => {
    if (session.dbSessionId) {
      setWatchedSessionId(session.sessionId);
      setView("liveConversation");
    }
  };

  // Sort: running first (by startedAt desc), then ended (by endedAt desc)
  const sorted = [...liveSessions].sort((a, b) => {
    if (a.isAlive !== b.isAlive) return a.isAlive ? -1 : 1;
    if (a.isAlive) return b.startedAt - a.startedAt;
    return (b.endedAt || 0) - (a.endedAt || 0);
  });

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="mb-4">
        <h1 className="text-xl font-semibold">Live Sessions</h1>
        <p className="text-sm text-zinc-500 mt-1">
          {runningCount} running
          {endedCount > 0 && <> &middot; {endedCount} ended</>}
        </p>
      </div>

      {sorted.length === 0 ? (
        <div className="text-center text-zinc-400 py-12">
          <p className="text-lg">No active Claude Code sessions</p>
          <p className="text-sm mt-1">Sessions will appear here when Claude Code is running</p>
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
