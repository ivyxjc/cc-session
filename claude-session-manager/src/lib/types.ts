export interface Project {
  id: number;
  encodedPath: string;
  originalPath: string;
  displayName: string;
  sessionCount: number;
  lastActive: number | null;
}

export interface SessionSummary {
  id: number;
  sessionId: string;
  projectId: number;
  projectName: string;
  projectPath: string;
  slug: string | null;
  version: string | null;
  permissionMode: string | null;
  gitBranch: string | null;
  startedAt: number | null;
  lastActive: number | null;
  messageCount: number;
  userMsgCount: number;
  assistantMsgCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheCreationTokens: number;
  totalCacheReadTokens: number;
  fileSize: number;
  isFavorited: boolean;
  isBackedUp: boolean;
  tags: Tag[];
}

export interface Tag {
  id: number;
  name: string;
  color: string;
}

export interface Backup {
  id: number;
  sessionId: number;
  backupPath: string;
  backupType: string;
  originalSize: number;
  compressed: boolean;
  createdAt: number;
}

export interface BackupConfig {
  enabled: boolean;
  backupDir: string;
  autoBackup: boolean;
  autoBackupIntervalHours: number;
  compress: boolean;
  maxBackupCopies: number;
}

export interface SubagentSummary {
  id: number;
  sessionId: number;
  agentId: string;
  agentType: string;
  description: string;
}

export interface TerminalEntry {
  name: string;
  command: string;
}

export interface TerminalConfig {
  terminals: TerminalEntry[];
  defaultTerminal: string;
}

export interface ScanResult {
  projectsFound: number;
  sessionsFound: number;
  sessionsUpdated: number;
  sessionsRemoved: number;
  durationMs: number;
}

// Message types from parser
export interface ContentBlock {
  type: "text" | "thinking" | "tool_use" | "tool_result";
  // text
  text?: string;
  // thinking
  thinking?: string;
  // tool_use
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  // tool_result
  toolUseId?: string;
  content?: unknown;
  isError?: boolean;
}

export interface Usage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

export type ParsedMessage =
  | { type: "user"; uuid: string; parentUuid: string | null; timestamp: string | null; content: ContentBlock[] }
  | { type: "assistant"; uuid: string; parentUuid: string | null; timestamp: string | null; model: string | null; content: ContentBlock[]; usage: Usage | null; stopReason: string | null }
  | { type: "system"; uuid: string | null; timestamp: string | null; subtype: string | null; content: string | null }
  | { type: "attachment"; attachmentType: string }
  | { type: "permissionMode"; mode: string }
  | { type: "fileHistorySnapshot" };
