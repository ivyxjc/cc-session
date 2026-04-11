import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  Project, SessionSummary, ParsedMessage, SubagentSummary,
  Tag, Backup, BackupConfig, ScanResult,
} from "./types";

// Safe invoke wrapper — returns empty/default when not in Tauri webview (e.g. browser dev)
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__) {
    console.warn(`[tauri] Not in Tauri webview, skipping invoke("${cmd}")`);
    return Promise.resolve([] as unknown as T);
  }
  return tauriInvoke<T>(cmd, args);
}

// Projects
export const listProjects = (sortBy?: string) =>
  invoke<Project[]>("list_projects", { sortBy });

// Sessions
export const listSessions = (params: {
  projectId?: number;
  tagId?: number;
  favorited?: boolean;
  sortBy?: string;
}) => invoke<SessionSummary[]>("list_sessions", params);

export const getMessages = (sessionId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_messages", { sessionId, offset, limit });

export const getSubagents = (sessionId: number) =>
  invoke<SubagentSummary[]>("get_subagents", { sessionId });

export const getSubagentMessages = (subagentId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_subagent_messages", { subagentId, offset, limit });

// Favorites
export const toggleFavorite = (sessionId: number, note?: string) =>
  invoke<boolean>("toggle_favorite", { sessionId, note });

// Tags
export const createTag = (name: string, color: string) =>
  invoke<Tag>("create_tag", { name, color });

export const deleteTag = (tagId: number) =>
  invoke<void>("delete_tag", { tagId });

export const listTags = () =>
  invoke<Tag[]>("list_tags");

export const tagSession = (sessionId: number, tagId: number) =>
  invoke<void>("tag_session", { sessionId, tagId });

export const untagSession = (sessionId: number, tagId: number) =>
  invoke<void>("untag_session", { sessionId, tagId });

// Backups
export const backupSession = (sessionId: number) =>
  invoke<Backup>("backup_session", { sessionId });

export const backupAllSessions = () =>
  invoke<Backup[]>("backup_all_sessions");

export const restoreSessionBackup = (backupId: number) =>
  invoke<void>("restore_session_backup", { backupId });

export const listBackups = (sessionId?: number) =>
  invoke<Backup[]>("list_backups", { sessionId });

export const deleteBackup = (backupId: number) =>
  invoke<void>("delete_backup", { backupId });

export const getBackupMessages = (backupPath: string, offset = 0, limit = 200) =>
  invoke<ParsedMessage[]>("get_backup_messages", { backupPath, offset, limit });

export const migrateBackups = (oldDir: string, newDir: string) =>
  invoke<number>("migrate_backups_cmd", { oldDir, newDir });

export const getBackupConfig = () =>
  invoke<BackupConfig>("get_backup_config_cmd");

export const setBackupConfig = (config: BackupConfig) =>
  invoke<void>("set_backup_config_cmd", { config });

// Scanning
export const refreshIndex = () =>
  invoke<ScanResult>("refresh_index");
