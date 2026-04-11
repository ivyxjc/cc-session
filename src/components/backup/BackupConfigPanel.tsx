import { useState, useEffect } from "react";
import { getBackupConfig, setBackupConfig } from "../../lib/tauri";
import type { BackupConfig } from "../../lib/types";

export function BackupConfigPanel() {
  const [config, setConfig] = useState<BackupConfig | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getBackupConfig().then(setConfig);
  }, []);

  const save = async () => {
    if (!config) return;
    setSaving(true);
    await setBackupConfig(config);
    setSaving(false);
  };

  if (!config) return <div className="text-zinc-500">Loading config...</div>;

  return (
    <div className="space-y-4 p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg">
      <h3 className="font-medium">Backup Configuration</h3>

      <label className="flex items-center gap-2">
        <input type="checkbox" checked={config.autoBackup} onChange={(e) => setConfig({ ...config, autoBackup: e.target.checked })} />
        <span className="text-sm">Auto-backup every {config.autoBackupIntervalHours}h</span>
      </label>

      <label className="flex items-center gap-2">
        <input type="checkbox" checked={config.compress} onChange={(e) => setConfig({ ...config, compress: e.target.checked })} />
        <span className="text-sm">Compress backups (zstd)</span>
      </label>

      <div>
        <label className="text-sm text-zinc-500">Backup directory</label>
        <input
          type="text"
          value={config.backupDir}
          onChange={(e) => setConfig({ ...config, backupDir: e.target.value })}
          className="w-full mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-transparent text-sm"
        />
      </div>

      <div>
        <label className="text-sm text-zinc-500">Max backup copies per session</label>
        <input
          type="number"
          value={config.maxBackupCopies}
          onChange={(e) => setConfig({ ...config, maxBackupCopies: parseInt(e.target.value) || 3 })}
          className="w-24 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-transparent text-sm"
          min={1}
          max={99}
        />
      </div>

      <button
        onClick={save}
        disabled={saving}
        className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
      >
        {saving ? "Saving..." : "Save Config"}
      </button>
    </div>
  );
}
