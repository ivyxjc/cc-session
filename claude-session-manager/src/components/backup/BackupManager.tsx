import { useEffect, useState } from "react";
import { listBackups, backupAllSessions, restoreSessionBackup, deleteBackup } from "../../lib/tauri";
import type { Backup } from "../../lib/types";
import { formatDateTime, formatFileSize } from "../../lib/format";
import { BackupConfigPanel } from "./BackupConfigPanel";

export function BackupManager() {
  const [backups, setBackups] = useState<Backup[]>([]);
  const [loading, setLoading] = useState(true);
  const [backing, setBacking] = useState(false);

  const load = async () => {
    setLoading(true);
    const data = await listBackups();
    setBackups(data);
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

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
        <h2 className="font-medium mb-2">Backup History ({backups.length})</h2>
        {loading ? (
          <div className="text-zinc-500">Loading...</div>
        ) : backups.length === 0 ? (
          <div className="text-zinc-500">No backups yet.</div>
        ) : (
          <div className="space-y-2">
            {backups.map((b) => (
              <div key={b.id} className="flex items-center justify-between p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg text-sm">
                <div>
                  <div className="font-medium truncate max-w-lg">{b.backupPath.split("/").slice(-3).join("/")}</div>
                  <div className="text-xs text-zinc-400">
                    {formatDateTime(b.createdAt)} · {formatFileSize(b.originalSize)} original · {b.compressed ? "compressed" : "raw"} · {b.backupType}
                  </div>
                </div>
                <div className="flex gap-2 shrink-0">
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
        )}
      </div>
    </div>
  );
}
