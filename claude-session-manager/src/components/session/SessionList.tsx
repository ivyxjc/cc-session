import { useEffect, useState } from "react";
import { listSessions } from "../../lib/tauri";
import type { SessionSummary } from "../../lib/types";
import { useAppStore } from "../../stores/appStore";
import { useFilterStore } from "../../stores/filterStore";
import { SessionCard } from "./SessionCard";

export function SessionList({ favoritesOnly }: { favoritesOnly?: boolean }) {
  const { selectedProjectId } = useAppStore();
  const { sortBy, selectedTagId } = useFilterStore();
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    listSessions({
      projectId: selectedProjectId ?? undefined,
      tagId: selectedTagId ?? undefined,
      favorited: favoritesOnly || undefined,
      sortBy,
    })
      .then(setSessions)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [selectedProjectId, selectedTagId, favoritesOnly, sortBy]);

  const title = favoritesOnly ? "Favorites" : "Sessions";

  return (
    <div className="p-6 h-full overflow-y-auto">
      <h1 className="text-xl font-semibold mb-4">{title}</h1>
      {loading ? (
        <div className="text-zinc-500">Loading...</div>
      ) : sessions.length === 0 ? (
        <div className="text-zinc-500">{favoritesOnly ? "No favorited sessions yet. Click the star on a session to add it." : "No sessions found."}</div>
      ) : (
        <div className="space-y-2">
          {sessions.map((s) => (
            <SessionCard key={s.id} session={s} />
          ))}
        </div>
      )}
    </div>
  );
}
