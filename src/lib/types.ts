/** Which agent CLI a project/session came from. Mirrors `sources::Provider` in Rust. */
export type Provider = "claude" | "codex";

export interface Project {
  id: number;
  provider: Provider;
  encodedPath: string;
  originalPath: string;
  displayName: string;
  sessionCount: number;
  lastActive: number | null;
  isStarred: boolean;
}

export interface SessionSummary {
  id: number;
  provider: Provider;
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
  isHidden: boolean;
  isBackedUp: boolean;
  copiedFromSessionId: string | null;
  copiedAt: number | null;
  summary: string | null;
  summarySource: string | null;   // 'heuristic' | 'llm'
  summaryAt: number | null;
  aiTags: string[];
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

export interface LiveSession {
  pid: number;
  sessionId: string;
  cwd: string;
  startedAt: number;
  kind: string;
  entrypoint: string;
  isAlive: boolean;
  endedAt: number | null;
  dbSessionId: number | null;
  slug: string | null;
  projectName: string | null;
  projectPath: string | null;
  gitBranch: string | null;
  messageCount: number | null;
  userMsgCount: number | null;
  totalInputTokens: number | null;
  totalOutputTokens: number | null;
  totalCacheCreationTokens: number | null;
  totalCacheReadTokens: number | null;
  version: string | null;
  fileSize: number | null;
  lastMessagePreview: string | null;
  activeSubagentCount: number | null;
  summary: string | null;
  summarySource: string | null;
  aiTags: string[];
  tags: Tag[];
}

export interface SessionMessagesUpdate {
  sessionId: string;
  newMessages: ViewMessage[];
}

export interface LatestMessagesResult {
  messages: ViewMessage[];
  totalCount: number;
}

export interface AutoHideConfig {
  enabled: boolean;
  minMessageCount: number;
}

export interface DayPlannerBlock {
  sessionDbId: number;
  sessionId: string;
  projectName: string;
  /** Session-level summary (whole arc). Fallback when daily summary not yet generated. */
  title: string;
  /** Session-level AI tags. */
  aiTags: string[];
  /** Day-specific summary from the daily map-reduce, when cached. Prefer over `title`. */
  dailySummary: string | null;
  /** Day-specific tags. */
  dailyTags: string[];
  /** Day-specific Jira/PR references; url present only when seen in the session. */
  dailyRefs: { label: string; url: string | null }[];
  /** Absolute timestamps in UTC ms. Render labels with `new Date(...)` for OS-local HH:MM. */
  startMs: number;
  endMs: number;
}

export interface DailySessionSummary {
  sessionDbId: number;
  sessionId: string;
  projectName: string;
  summary: string;
  tags: string[];
  startMs: number;
  endMs: number;
  /** Sum of gap-split block durations — actual engaged time, not span. */
  activeMs: number;
  refs: { label: string; url: string | null }[];
}

export interface DailySessionError {
  sessionDbId: number;
  sessionId: string;
  projectName: string;
  error: string;
}

export interface DailyReport {
  date: string;
  narrative: string;       // Markdown
  perSession: DailySessionSummary[];
  errors: DailySessionError[];
}

export interface DailyUsage {
  date: string;
  sessionCount: number;
  userMsgCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheCreationTokens: number;
  totalCacheReadTokens: number;
  totalTokens: number;
}

export interface MultiplexerConfig {
  multiplexer: string; // "none" | "zellij" | "tmux"
}

export interface MultiplexerSession {
  name: string;
  status: string;
  cwd: string | null;
  matchesPath: boolean;
  attachCmd: string;
}

export interface MultiplexerDetectionResult {
  multiplexer: string;
  sessions: MultiplexerSession[];
  newSessionCmd: string;
}

/** Effective grid of a session's external clients (per-dimension minimum). */
export interface ExternalClientSize {
  cols: number;
  rows: number;
  clients: number;
}

export interface AiSummaryConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
}

export interface AiSummaryResult {
  generated: boolean;
  summary: string | null;
  tags: string[] | null;
}

export interface AiSummaryProgress {
  current: number;
  total: number;
  sessionDbId: number;
  status: "ok" | "skipped" | "error";
  error: string | null;
  summary: string | null;
}

export interface ContentSearchResult {
  sessionDbId: number;
  sessionId: string;
  slug: string | null;
  projectName: string;
  projectPath: string;
  messageUuid: string;
  role: string;
  timestampMs: number;
  snippet: string;
}

export interface ScanResult {
  projectsFound: number;
  sessionsFound: number;
  sessionsUpdated: number;
  sessionsRemoved: number;
  durationMs: number;
}

// View model types — provider-agnostic
export interface ViewContentBlock {
  type: "text" | "thinking" | "toolCall" | "toolResult" | "image";
  // text
  text?: string;
  // thinking
  thinking?: string;
  reasoningTokens?: number;
  tokensShared?: boolean;
  // toolCall
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  // toolResult
  toolCallId?: string;
  content?: unknown;
  isError?: boolean;
  // image
  source?: {
    sourceType: string;
    mediaType?: string;
    data?: string;
  };
}

export interface ViewUsage {
  inputTokens: number;
  outputTokens: number;
  cacheCreationInputTokens: number;
  cacheReadInputTokens: number;
}

export type ViewMessage =
  | { type: "user"; id: string; parentId: string | null; timestamp: string | null; content: ViewContentBlock[] }
  | { type: "assistant"; id: string; parentId: string | null; timestamp: string | null; model: string | null; content: ViewContentBlock[]; usage: ViewUsage | null; stopReason: string | null }
  | { type: "system"; id: string | null; timestamp: string | null; subtype: string | null; content: string | null };
