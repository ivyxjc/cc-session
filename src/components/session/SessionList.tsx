import { useEffect, useState } from "react";
import { listSessions } from "../../lib/tauri";
import type { SessionSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { useFilterStore } from "../../stores/filterStore";
import { SessionCard } from "./SessionCard";

export function SessionList({ favoritesOnly }: { favoritesOnly?: boolean }) {
  const { selectedProjectId, refreshCounter } = useAppStore();
  const { sortBy, selectedTagId } = useFilterStore();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [showHidden, setShowHidden] = useState(false);

  useEffect(() => {
    setLoading(true);
    listSessions({
      projectId: selectedProjectId ?? undefined,
      tagId: selectedTagId ?? undefined,
      favorited: favoritesOnly || undefined,
      showHidden,
      sortBy,
    })
      .then(setSessions)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [selectedProjectId, selectedTagId, favoritesOnly, sortBy, refreshCounter, showHidden]);

  const title = favoritesOnly ? "Favorites" : "Sessions";

  const { setSortBy } = useFilterStore();

  const reload = () => {
    listSessions({
      projectId: selectedProjectId ?? undefined,
      tagId: selectedTagId ?? undefined,
      favorited: favoritesOnly || undefined,
      showHidden,
      sortBy,
    }).then(setSessions).catch(console.error);
  };

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <h1 className="text-xl font-semibold">{title}</h1>
          {!favoritesOnly && (
            <label className="flex items-center gap-1.5 text-xs text-zinc-500 cursor-pointer">
              <input
                type="checkbox"
                checked={showHidden}
                onChange={(e) => setShowHidden(e.target.checked)}
                className="rounded"
              />
              Show hidden
            </label>
          )}
        </div>
        <select
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value)}
          className="text-sm px-2 py-1 rounded border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-800"
        >
          <option value="time">Last active</option>
          <option value="messages">Most messages</option>
          <option value="tokens">Most tokens</option>
          <option value="size">Largest size</option>
        </select>
      </div>
      {loading ? (
        <div className="text-zinc-500">Loading...</div>
      ) : sessions.length === 0 ? (
        <div className="text-zinc-500">{favoritesOnly ? "No favorited sessions yet. Click the star on a session to add it." : "No sessions found."}</div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <SessionCard
              key={s.id}
              session={s}
              showHidden={showHidden}
              onHide={reload}
            />
          ))}
        </div>
      )}
    </div>
  );
}
