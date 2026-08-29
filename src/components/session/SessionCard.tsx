import type { SessionSummary, LiveSession, Tag, Provider } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize, formatRelativeTime } from "../../lib/format";
import { useAppStore } from "../../stores/appStore";
import { toggleHideSession } from "../../lib/tauri";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { MultiplexerButton } from "../common/MultiplexerButton";
import { TagBadge } from "../common/TagBadge";
import { LiveStatusBadge } from "../live/LiveStatusBadge";
import { RunningTimer } from "../live/RunningTimer";
import { ProviderBadge } from "../common/ProviderBadge";

/** Common subset rendered by the unified card. Both SessionSummary and LiveSession get
 *  normalized into this shape before rendering. */
interface SessionCardModel {
  /** DB row id (null when the live session hasn't been indexed yet). */
  id: number | null;
  provider: Provider;
  sessionId: string;
  projectName: string;
  projectPath: string;
  gitBranch: string | null;
  slug: string | null;
  version: string | null;
  /** Stored title from heuristic / LLM; null when neither has been generated. */
  summary: string | null;
  summarySource: string | null;       // 'heuristic' | 'llm' | null
  /** Curated user tags. */
  tags: Tag[];
  /** Auto-generated LLM tags (rendered differently). */
  aiTags: string[];
  messageCount: number;
  userMsgCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheCreationTokens: number;
  totalCacheReadTokens: number;
  fileSize: number;
  /** Last activity timestamp (ms). null only when truly never active. */
  lastActive: number | null;
  isFavorited: boolean;
  isHidden: boolean;
}

interface LiveOverlay {
  isAlive: boolean;
  startedAt: number;
  endedAt: number | null;
  pid: number;
  activeSubagentCount: number | null;
}

interface Props {
  /** Either a full SessionSummary or a LiveSession enriched with summary/tags. */
  session: SessionSummary | LiveSession;
  /** When provided, the card renders running/ended badge + uptime + PID in the metrics row. */
  live?: LiveOverlay;
  /** Custom click handler (defaults to navigating into the conversation view). */
  onClick?: () => void;
  /** Called after the user toggles the Hide button. Only shown when a hide handler is wanted. */
  onHide?: () => void;
  /** Hide the Hide button (used for live sessions, where hiding doesn't make sense). */
  hideHideButton?: boolean;
}

function isLiveSession(s: SessionSummary | LiveSession): s is LiveSession {
  return (s as LiveSession).pid !== undefined;
}

function normalize(s: SessionSummary | LiveSession): SessionCardModel {
  if (isLiveSession(s)) {
    return {
      id: s.dbSessionId,
      provider: "claude", // live tracking only follows Claude Code processes
      sessionId: s.sessionId,
      projectName: s.projectName || s.cwd.split("/").pop() || s.cwd,
      projectPath: s.projectPath || s.cwd,
      gitBranch: s.gitBranch,
      slug: s.slug,
      version: s.version,
      summary: s.summary,
      summarySource: s.summarySource,
      tags: s.tags,
      aiTags: s.aiTags,
      messageCount: s.messageCount ?? 0,
      userMsgCount: s.userMsgCount ?? 0,
      totalInputTokens: s.totalInputTokens ?? 0,
      totalOutputTokens: s.totalOutputTokens ?? 0,
      totalCacheCreationTokens: s.totalCacheCreationTokens ?? 0,
      totalCacheReadTokens: s.totalCacheReadTokens ?? 0,
      fileSize: s.fileSize ?? 0,
      // Live sessions don't carry lastActive; the live overlay's startedAt/endedAt
      // drives the time display instead.
      lastActive: null,
      isFavorited: false,
      isHidden: false,
    };
  }
  return {
    id: s.id,
    provider: s.provider,
    sessionId: s.sessionId,
    projectName: s.projectName,
    projectPath: s.projectPath,
    gitBranch: s.gitBranch,
    slug: s.slug,
    version: s.version,
    summary: s.summary,
    summarySource: s.summarySource,
    tags: s.tags,
    aiTags: s.aiTags,
    messageCount: s.messageCount,
    userMsgCount: s.userMsgCount,
    totalInputTokens: s.totalInputTokens,
    totalOutputTokens: s.totalOutputTokens,
    totalCacheCreationTokens: s.totalCacheCreationTokens,
    totalCacheReadTokens: s.totalCacheReadTokens,
    fileSize: s.fileSize,
    lastActive: s.lastActive,
    isFavorited: s.isFavorited,
    isHidden: s.isHidden,
  };
}

export function SessionCard({ session, live, onClick, onHide, hideHideButton }: Props) {
  const selectSession = useAppStore((s) => s.selectSession);
  const m = normalize(session);

  const summaryIsHeuristic = m.summarySource === "heuristic";
  const handleClick = onClick ?? (() => { if (m.id != null) selectSession(m.id); });

  // Time label: live → uptime/ended; non-live → lastActive datetime.
  const timeLabel = live
    ? live.isAlive
      ? <RunningTimer startedAt={live.startedAt} />
      : formatRelativeTime(live.endedAt)
    : formatDateTime(m.lastActive);

  const totalTokens = m.totalInputTokens + m.totalOutputTokens + m.totalCacheCreationTokens + m.totalCacheReadTokens;

  return (
    <button
      onClick={handleClick}
      className="w-full text-left p-4 rounded-lg border border-zinc-200 dark:border-zinc-800 hover:border-zinc-400 dark:hover:border-zinc-600 transition-colors"
    >
      {/* Row 1: title (summary or slug) + actions */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1 flex items-center gap-2">
          {live && <LiveStatusBadge isAlive={live.isAlive} />}
          {m.summary ? (
            <div
              className={`text-sm font-medium truncate ${
                summaryIsHeuristic
                  ? "text-zinc-600 dark:text-zinc-400 italic"
                  : "text-zinc-900 dark:text-zinc-100"
              }`}
              title={m.summary}
            >
              {m.summary}
            </div>
          ) : (
            <div className="text-sm font-medium text-zinc-400 dark:text-zinc-500 italic truncate">
              {m.slug || "[Untitled session]"}
            </div>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <span className="text-xs text-zinc-400">{timeLabel}</span>
          <OpenTerminalButton path={m.projectPath} sessionId={m.sessionId} provider={m.provider} />
          <MultiplexerButton path={m.projectPath} />
          {m.id != null && (
            <FavoriteButton sessionId={m.id} initialFavorited={m.isFavorited} />
          )}
          {!hideHideButton && m.id != null && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleHideSession(m.id!).then(() => onHide?.());
              }}
              className="px-1.5 py-1 text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
              title={m.isHidden ? "Unhide session" : "Hide session"}
            >
              {m.isHidden ? "Unhide" : "Hide"}
            </button>
          )}
        </div>
      </div>

      {/* Row 2: project · sessionId · branch */}
      <div className="flex items-baseline gap-2 mt-1">
        <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300 truncate">{m.projectName}</span>
        {m.provider !== "claude" && <ProviderBadge provider={m.provider} />}
        <CopyText text={m.sessionId} display={m.sessionId.slice(0, 8)} className="text-xs text-zinc-400 font-mono" />
        <span className="text-xs text-zinc-500 truncate">{m.gitBranch || "—"}</span>
      </div>

      {/* Row 3: path */}
      <div className="text-xs text-zinc-400 mt-0.5 truncate font-mono">
        {m.projectPath}
      </div>

      {/* Row 4: metrics */}
      <div className="text-xs text-zinc-400 mt-1">
        {m.userMsgCount} user | {m.messageCount} total
        {" | "}total {formatTokens(totalTokens)}
        {" | "}in {formatTokens(m.totalInputTokens)}
        {" | "}out {formatTokens(m.totalOutputTokens)}
        {" | "}cache R {formatTokens(m.totalCacheReadTokens)}
        {" | "}cache W {formatTokens(m.totalCacheCreationTokens)}
        {m.fileSize > 0 && <>{" | "}{formatFileSize(m.fileSize)}</>}
        {live && <>{" | "}PID {live.pid}</>}
        {live?.activeSubagentCount != null && live.activeSubagentCount > 0 && (
          <>{" | "}{live.activeSubagentCount} subagent{live.activeSubagentCount > 1 ? "s" : ""}</>
        )}
        {m.version && <>{" | "}{m.version}</>}
      </div>

      {/* Row 5: tags */}
      {(m.tags.length > 0 || m.aiTags.length > 0) && (
        <div className="flex gap-1 mt-2 flex-wrap">
          {m.tags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} />
          ))}
          {m.aiTags.map((tag) => (
            <span
              key={`ai-${tag}`}
              className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs rounded border border-emerald-300 dark:border-emerald-800 text-emerald-700 dark:text-emerald-400"
              title="AI-generated tag"
            >
              <span className="text-[10px] opacity-70">AI</span>{tag}
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
