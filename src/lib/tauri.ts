import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  Project, SessionSummary, ParsedMessage, SubagentSummary,
  Tag, Backup, BackupConfig, TerminalConfig, ScanResult, LiveSession,
  LatestMessagesResult,
  MultiplexerConfig, MultiplexerDetectionResult,
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
  showHidden?: boolean;
  sortBy?: string;
}) => invoke<SessionSummary[]>("list_sessions", params);

export const getMessages = (sessionId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_messages", { sessionId, offset, limit });

export const getLatestMessages = (sessionId: number, count = 50) =>
  invoke<LatestMessagesResult>("get_latest_messages", { sessionId, count });

export const getSubagents = (sessionId: number) =>
  invoke<SubagentSummary[]>("get_subagents", { sessionId });

export const getSubagentMessages = (subagentId: number, offset = 0, limit = 50) =>
  invoke<ParsedMessage[]>("get_subagent_messages", { subagentId, offset, limit });

// Favorites
export const toggleFavorite = (sessionId: number, note?: string) =>
  invoke<boolean>("toggle_favorite", { sessionId, note });

export const toggleHideSession = (sessionId: number) =>
  invoke<boolean>("toggle_hide_session", { sessionId });

export const toggleStarProject = (projectId: number) =>
  invoke<boolean>("toggle_star_project", { projectId });

export const getAutoHideConfig = () =>
  invoke<import("./types").AutoHideConfig>("get_auto_hide_config");

export const setAutoHideConfig = (config: import("./types").AutoHideConfig) =>
  invoke<void>("set_auto_hide_config", { config });

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

// Terminal
export const getTerminalConfig = () =>
  invoke<TerminalConfig>("get_terminal_config");

export const setTerminalConfig = (config: TerminalConfig) =>
  invoke<void>("set_terminal_config", { config });

export const openTerminal = (path: string, terminalName?: string) =>
  invoke<void>("open_terminal", { path, terminalName });

export const testTerminalCommand = (command: string) =>
  invoke<void>("test_terminal_command", { command });

// Multiplexer
export const getMultiplexerConfig = () =>
  invoke<MultiplexerConfig>("get_multiplexer_config");

export const setMultiplexerConfig = (config: MultiplexerConfig) =>
  invoke<void>("set_multiplexer_config", { config });

export const detectMultiplexerSessions = (path: string, multiplexer: string) =>
  invoke<MultiplexerDetectionResult>("detect_multiplexer_sessions", { path, multiplexer });

// Settings import/export
export const exportSettings = () =>
  invoke<string>("export_settings");

export const importSettings = (json: string) =>
  invoke<void>("import_settings", { json });

export const exportSettingsToFile = (path: string) =>
  invoke<void>("export_settings_to_file", { path });

export const importSettingsFromFile = (path: string) =>
  invoke<void>("import_settings_from_file", { path });

// Usage
export const getDailyUsage = (days?: number) =>
  invoke<import("./types").DailyUsage[]>("get_daily_usage", { days });

// Session copy
export const copySessionToPath = (sessionId: number, targetPath: string) =>
  invoke<string>("copy_session_to_path", { sessionId, targetPath });

// Images
export const readImageFile = (path: string) =>
  invoke<string>("read_image_file", { path });

// Scanning
export const refreshIndex = () =>
  invoke<ScanResult>("refresh_index");

// Live Monitor
export const getLiveSessions = () =>
  invoke<LiveSession[]>("get_live_sessions");

export const startLiveMonitor = () =>
  invoke<void>("start_live_monitor");

export const stopLiveMonitor = () =>
  invoke<void>("stop_live_monitor");

export const watchSession = (sessionId: string) =>
  invoke<void>("watch_session", { sessionId });

export const unwatchSession = (sessionId: string) =>
  invoke<void>("unwatch_session", { sessionId });
