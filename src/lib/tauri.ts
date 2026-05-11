import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  Project, SessionSummary, ViewMessage, SubagentSummary,
  Tag, Backup, BackupConfig, TerminalConfig, ScanResult, LiveSession,
  LatestMessagesResult,
  MultiplexerConfig, MultiplexerDetectionResult,
  ContentSearchResult,
  AiSummaryConfig, AiSummaryResult,
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
  invoke<ViewMessage[]>("get_messages", { sessionId, offset, limit });

export const getLatestMessages = (sessionId: number, count = 50) =>
  invoke<LatestMessagesResult>("get_latest_messages", { sessionId, count });

export const getSubagents = (sessionId: number) =>
  invoke<SubagentSummary[]>("get_subagents", { sessionId });

export const getSubagentMessages = (subagentId: number, offset = 0, limit = 50) =>
  invoke<ViewMessage[]>("get_subagent_messages", { subagentId, offset, limit });

// Favorites
export const toggleFavorite = (sessionId: number) =>
  invoke<boolean>("toggle_favorite", { sessionId });

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
  invoke<ViewMessage[]>("get_backup_messages", { backupPath, offset, limit });

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

export const findSessionForPid = (pid: number, multiplexer: string) =>
  invoke<string | null>("find_session_for_pid", { pid, multiplexer });

// Settings import/export (file-based — the string-based variants were unused and removed)
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

// Export
export const exportSession = (sessionId: number, projectPath: string, targetPath: string) =>
  invoke<void>("export_session", { sessionId, projectPath, targetPath });

export const exportCodexSession = (threadId: string, targetPath: string) =>
  invoke<void>("export_codex_session", { threadId, targetPath });

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

// Codex
export const codexGetSession = (threadId: string) =>
  invoke<import("./types").CodexSession>("codex_get_session", { threadId });

export const codexListProjects = (sortBy?: string) =>
  invoke<import("./types").CodexProject[]>("codex_list_projects", { sortBy });

export const codexListSessions = (params: {
  cwd?: string;
  sortBy?: string;
  showArchived?: boolean;
}) => invoke<import("./types").CodexSession[]>("codex_list_sessions", params);

export const codexGetMessages = (threadId: string, offset = 0, limit = 50) =>
  invoke<ViewMessage[]>("codex_get_messages", { threadId, offset, limit });

export const codexGetLatestMessages = (threadId: string, count = 50) =>
  invoke<LatestMessagesResult>("codex_get_latest_messages", { threadId, count });

export const codexGetSubagents = (threadId: string) =>
  invoke<import("./types").CodexSubagent[]>("codex_get_subagents", { threadId });

export const codexGetSubagentMessages = (threadId: string, offset = 0, limit = 200) =>
  invoke<ViewMessage[]>("codex_get_subagent_messages", { threadId, offset, limit });

// Content search
export const searchMessageContent = (query: string, limit = 50) =>
  invoke<ContentSearchResult[]>("search_message_content", { query, limit });

// PTY (embedded multiplexer terminal)
export const ptyAttachMultiplexer = (kind: string, name: string, cwd: string, cols: number, rows: number) =>
  invoke<void>("pty_attach_multiplexer", { kind, name, cwd, cols, rows });
export const ptyCreateMultiplexer = (kind: string, name: string, cwd: string, cols: number, rows: number) =>
  invoke<void>("pty_create_multiplexer", { kind, name, cwd, cols, rows });
export const ptyDetach = () => invoke<void>("pty_detach");

// AI summary
export const getAiSummaryConfig = () =>
  invoke<AiSummaryConfig>("get_ai_summary_config");
export const setAiSummaryConfig = (config: AiSummaryConfig) =>
  invoke<void>("set_ai_summary_config", { config });
export const testAiSummaryConnection = (config: AiSummaryConfig) =>
  invoke<string>("test_ai_summary_connection", { config });
export const generateAiSummary = (sessionDbId: number, force = true) =>
  invoke<AiSummaryResult>("generate_ai_summary", { sessionDbId, force });
export const generateAiSummariesBatch = (force = false, sessionIds?: number[]) =>
  invoke<number>("generate_ai_summaries_batch", { force, sessionIds });
