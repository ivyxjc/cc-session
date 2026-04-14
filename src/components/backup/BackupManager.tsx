import { useEffect, useMemo, useState } from "react";
import { listBackups, listSessions, backupAllSessions, restoreSessionBackup, deleteBackup, getBackupMessages } from "../../lib/tauri";
import type { Backup, ViewMessage, SessionSummary } from "../../lib/types";
import { formatDateTime, formatFileSize, formatRelativeTime } from "../../lib/format";
import { BackupConfigPanel } from "./BackupConfigPanel";
import { MessageBubble } from "../message/MessageBubble";

interface SessionGroup {
  sessionId: number;
  label: string; // project/session from path
  backups: Backup[];
}

export function BackupManager() {
  const [backups, setBackups] = useState<Backup[]>([]);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [backing, setBacking] = useState(false);
  const [expandedSessions, setExpandedSessions] = useState<Set<number>>(new Set());
  const [viewingBackup, setViewingBackup] = useState<Backup | null>(null);
  const [viewMessages, setViewMessages] = useState<ViewMessage[]>([]);
  const [viewLoading, setViewLoading] = useState(false);

  const load = async () => {
    setLoading(true);
    const [data, allSessions] = await Promise.all([
      listBackups(),
      listSessions({ sortBy: "time" }),
    ]);
    setBackups(data);
    setSessions(allSessions);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  // Map session DB id → session info
  const sessionMap = useMemo(() => {
    const map = new Map<number, SessionSummary>();
    for (const s of sessions) map.set(s.id, s);
    return map;
  }, [sessions]);

  // Group by sessionId, newest first
  const groups = useMemo(() => {
    const map = new Map<number, Backup[]>();
    for (const b of backups) {
      const existing = map.get(b.sessionId) || [];
      existing.push(b);
      map.set(b.sessionId, existing);
    }
    const result: SessionGroup[] = [];
    for (const [sessionId, bks] of map) {
      bks.sort((a, b) => b.createdAt - a.createdAt);
      const session = sessionMap.get(sessionId);
      const label = session
        ? session.slug
          ? `${session.projectPath} — ${session.slug}`
          : session.projectPath
        : bks[0].backupPath.split("/").slice(-3, -1).join("/");
      result.push({ sessionId, label, backups: bks });
    }
    result.sort((a, b) => b.backups[0].createdAt - a.backups[0].createdAt);
    return result;
  }, [backups, sessionMap]);

  const handleBackupAll = async () => {
    setBacking(true);
    await backupAllSessions();
    await load();
    setBacking(false);
  };

  const handleRestore = async (backupId: number) => {
    if (!confirm("Restore this backup? It will copy the session back to ~/.claude/projects/.")) return;
    await restoreSessionBackup(backupId);
    alert("Restored successfully. Use `claude -c` in the project directory to resume.");
  };

  const handleDelete = async (backupId: number) => {
    if (!confirm("Delete this backup permanently?")) return;
    await deleteBackup(backupId);
    await load();
  };

  const handleView = async (backup: Backup) => {
    setViewingBackup(backup);
    setViewLoading(true);
    try {
      const msgs = await getBackupMessages(backup.backupPath);
      setViewMessages(msgs);
    } catch (e) {
      console.error("Failed to load backup messages:", e);
      setViewMessages([]);
    }
    setViewLoading(false);
  };

  const toggleExpand = (sessionId: number) => {
    setExpandedSessions((prev) => {
      const next = new Set(prev);
      if (next.has(sessionId)) next.delete(sessionId);
      else next.add(sessionId);
      return next;
    });
  };

  return (
    <div className="p-6 h-full overflow-y-auto space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Backups</h1>
        <button
          onClick={handleBackupAll}
          disabled={backing}
          className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {backing ? "Backing up..." : "Backup All Sessions"}
        </button>
      </div>

      <BackupConfigPanel />

      <div>
        <h2 className="font-medium mb-2">Backup History ({backups.length} backups, {groups.length} sessions)</h2>
        {loading ? (
          <div className="text-zinc-500">Loading...</div>
        ) : groups.length === 0 ? (
          <div className="text-zinc-500">No backups yet.</div>
        ) : (
          <div className="space-y-2">
            {groups.map((group) => {
              const latest = group.backups[0];
              const isExpanded = expandedSessions.has(group.sessionId);
              const hasMore = group.backups.length > 1;

              return (
                <div key={group.sessionId} className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden">
                  {/* Latest backup (always visible) */}
                  <div className="flex items-center justify-between p-3 text-sm">
                    <div className="flex items-center gap-2 min-w-0 flex-1">
                      {hasMore && (
                        <button onClick={() => toggleExpand(group.sessionId)} className="text-xs text-zinc-400 shrink-0">
                          {isExpanded ? "▼" : "▶"}
                        </button>
                      )}
                      <div className="min-w-0">
                        <div className="font-medium truncate">{group.label}</div>
                        <div className="text-xs text-zinc-400">
                          {formatRelativeTime(latest.createdAt)} · {formatFileSize(latest.originalSize)} · {latest.compressed ? "zstd" : "raw"}
                          {hasMore && ` · ${group.backups.length} versions`}
                        </div>
                      </div>
                    </div>
                    <div className="flex gap-2 shrink-0 ml-2">
                      {!latest.compressed && (
                        <button onClick={() => handleView(latest)} className="px-2 py-1 text-xs border border-blue-300 dark:border-blue-700 text-blue-600 rounded hover:bg-blue-50 dark:hover:bg-blue-900/20">
                          View
                        </button>
                      )}
                      <button onClick={() => handleRestore(latest.id)} className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800">
                        Restore
                      </button>
                      <button onClick={() => handleDelete(latest.id)} className="px-2 py-1 text-xs border border-red-300 dark:border-red-700 text-red-600 rounded hover:bg-red-50 dark:hover:bg-red-900/20">
                        Delete
                      </button>
                    </div>
                  </div>

                  {/* Older backups (expanded) */}
                  {isExpanded && group.backups.slice(1).map((b) => (
                    <div key={b.id} className="flex items-center justify-between px-3 py-2 text-sm border-t border-zinc-100 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/50">
                      <div className="text-xs text-zinc-400">
                        {formatDateTime(b.createdAt)} · {formatFileSize(b.originalSize)} · {b.compressed ? "zstd" : "raw"}
                      </div>
                      <div className="flex gap-2 shrink-0 ml-2">
                        {!b.compressed && (
                          <button onClick={() => handleView(b)} className="px-2 py-1 text-xs border border-blue-300 dark:border-blue-700 text-blue-600 rounded hover:bg-blue-50 dark:hover:bg-blue-900/20">
                            View
                          </button>
                        )}
                        <button onClick={() => handleRestore(b.id)} className="px-2 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800">
                          Restore
                        </button>
                        <button onClick={() => handleDelete(b.id)} className="px-2 py-1 text-xs border border-red-300 dark:border-red-700 text-red-600 rounded hover:bg-red-50 dark:hover:bg-red-900/20">
                          Delete
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Backup Viewer Overlay */}
      {viewingBackup && (
        <div className="fixed inset-0 z-50 flex">
          <div className="absolute inset-0 bg-black/50" onClick={() => setViewingBackup(null)} />
          <div className="relative m-8 flex-1 bg-white dark:bg-zinc-950 rounded-lg shadow-xl flex flex-col overflow-hidden">
            <div className="flex items-center justify-between p-4 border-b border-zinc-200 dark:border-zinc-800">
              <div>
                <h2 className="font-semibold">Backup Content</h2>
                <div className="text-xs text-zinc-400 font-mono mt-0.5">
                  {viewingBackup.backupPath.split("/").slice(-3).join("/")} · {formatDateTime(viewingBackup.createdAt)}
                </div>
              </div>
              <button
                onClick={() => setViewingBackup(null)}
                className="px-3 py-1 text-sm border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                Close
              </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4 space-y-3">
              {viewLoading ? (
                <div className="text-zinc-500">Loading messages...</div>
              ) : viewMessages.length === 0 ? (
                <div className="text-zinc-500">No messages found in this backup.</div>
              ) : (
                viewMessages.map((msg, i) => (
                  <MessageBubble key={i} message={msg} />
                ))
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
