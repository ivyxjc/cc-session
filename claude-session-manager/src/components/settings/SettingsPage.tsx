import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getBackupConfig, setBackupConfig, migrateBackups } from "../../lib/tauri";
import type { BackupConfig } from "../../lib/types";

export function SettingsPage() {
  const [config, setConfig] = useState<BackupConfig | null>(null);
  const [originalDir, setOriginalDir] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [migrating, setMigrating] = useState(false);

  useEffect(() => {
    getBackupConfig().then((c) => {
      setConfig(c);
      setOriginalDir(c.backupDir);
    });
  }, []);

  const save = async () => {
    if (!config) return;
    setSaving(true);

    // Migrate backups if directory changed
    if (config.backupDir !== originalDir) {
      setMigrating(true);
      try {
        await migrateBackups(originalDir, config.backupDir);
      } catch (e) {
        console.error("Migration failed:", e);
      }
      setMigrating(false);
      setOriginalDir(config.backupDir);
    }

    await setBackupConfig(config);
    setSaving(false);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!config) return <div className="p-6 text-zinc-500">Loading...</div>;

  return (
    <div className="p-6 h-full overflow-y-auto">
      <h1 className="text-xl font-semibold mb-6">Settings</h1>

      {/* Backup Settings */}
      <section className="space-y-4 max-w-lg">
        <h2 className="text-sm font-medium text-zinc-500 uppercase tracking-wide">Backup</h2>

        <div>
          <label className="text-sm font-medium">Backup directory</label>
          <div className="flex gap-2 mt-1">
            <input
              type="text"
              value={config.backupDir}
              onChange={(e) => setConfig({ ...config, backupDir: e.target.value })}
              className="flex-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm font-mono"
            />
            <button
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  title: "Select backup directory",
                  defaultPath: config.backupDir,
                });
                if (selected) {
                  setConfig({ ...config, backupDir: selected });
                }
              }}
              className="px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800 shrink-0"
            >
              Browse
            </button>
          </div>
          <p className="text-xs text-zinc-400 mt-1">Where session backups are stored.</p>
        </div>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={config.autoBackup}
            onChange={(e) => setConfig({ ...config, autoBackup: e.target.checked })}
          />
          <span className="text-sm">Auto-backup on favorite</span>
        </label>

        <label className="flex items-center gap-2">
          <input
            type="checkbox"
            checked={config.compress}
            onChange={(e) => setConfig({ ...config, compress: e.target.checked })}
          />
          <span className="text-sm">Compress backups (zstd)</span>
        </label>

        <div>
          <label className="text-sm font-medium">Max backup copies per session</label>
          <input
            type="number"
            value={config.maxBackupCopies}
            onChange={(e) => setConfig({ ...config, maxBackupCopies: parseInt(e.target.value) || 3 })}
            className="w-20 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            min={1}
            max={99}
          />
        </div>

        <div>
          <label className="text-sm font-medium">Auto-backup interval (hours)</label>
          <input
            type="number"
            value={config.autoBackupIntervalHours}
            onChange={(e) => setConfig({ ...config, autoBackupIntervalHours: parseInt(e.target.value) || 24 })}
            className="w-20 mt-1 px-3 py-1.5 border border-zinc-300 dark:border-zinc-700 rounded bg-white dark:bg-zinc-800 text-sm"
            min={1}
          />
        </div>

        <button
          onClick={save}
          disabled={saving}
          className="px-4 py-1.5 bg-blue-600 text-white rounded text-sm hover:bg-blue-700 disabled:opacity-50"
        >
          {migrating ? "Migrating backups..." : saving ? "Saving..." : saved ? "Saved!" : "Save"}
        </button>
      </section>
    </div>
  );
}
