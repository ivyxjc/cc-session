import { useState } from "react";
import type { SessionSummary } from "../../lib/types";
import { formatDateTime, formatTokens, formatFileSize } from "../../lib/format";
import { backupSession, copySessionToPath, exportSession, generateAiSummary } from "../../lib/tauri";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "../../stores/toastStore";
import { CopyText } from "../common/CopyText";
import { FavoriteButton } from "../common/FavoriteButton";
import { OpenTerminalButton } from "../common/OpenTerminalButton";
import { MultiplexerButton } from "../common/MultiplexerButton";
import { TagBadge } from "../common/TagBadge";
import { TagManager } from "../common/TagManager";
import { useAppStore } from "../../stores/appStore";

export function SessionHeader({ session, onRefresh }: { session: SessionSummary; onRefresh?: () => void }) {
  const selectSession = useAppStore((s) => s.selectSession);
  const [showTagManager, setShowTagManager] = useState(false);
  const [backingUp, setBackingUp] = useState(false);
  const [copying, setCopying] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [generatingAi, setGeneratingAi] = useState(false);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [exportProjectPath, setExportProjectPath] = useState("");

  const handleBackup = async () => {
    setBackingUp(true);
    try {
      await backupSession(session.id);
      toast.success("Backup successful!");
    } catch (e) {
      toast.error(`Backup failed: ${e}`);
    } finally {
      setBackingUp(false);
    }
    onRefresh?.();
  };

  return (
    <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
      <div className="flex items-center gap-2">
        <button
          onClick={() => selectSession(null)}
          className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          &larr; Back
        </button>
        <div className="flex-1" />
        <button
          onClick={async () => {
            setGeneratingAi(true);
            try {
              const r = await generateAiSummary(session.id, true);
              if (r.summary) {
                toast.success(`AI summary: ${r.summary}`);
              } else {
                toast.success("AI summary unchanged (no new content)");
              }
              onRefresh?.();
            } catch (e) {
              toast.error(`AI summary failed: ${e}`);
            } finally {
              setGeneratingAi(false);
            }
          }}
          disabled={generatingAi}
          className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
          title="Generate AI summary for this session"
        >
          {generatingAi ? "Generating..." : "AI Summary"}
        </button>
        <button
          onClick={handleBackup}
          disabled={backingUp}
          className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
        >
          {backingUp ? "Backing up..." : "Backup"}
        </button>
        <button
          onClick={async () => {
            const dir = await open({ directory: true, multiple: false, title: "Select target project directory" });
            if (dir) {
              setCopying(true);
              try {
                const newUuid = await copySessionToPath(session.id, dir as string);
                alert(`Session copied. New ID: ${newUuid.slice(0, 8)}`);
                onRefresh?.();
              } catch (e) {
                alert(`Copy failed: ${e}`);
              }
              setCopying(false);
            }
          }}
          disabled={copying}
          className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
        >
          {copying ? "Copying..." : "Copy to path"}
        </button>
        <button
          onClick={() => {
            setExportProjectPath(session.projectPath || "");
            setShowExportDialog(true);
          }}
          disabled={exporting}
          className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
        >
          {exporting ? "Exporting..." : "Export zip"}
        </button>
        <div className="relative">
          <button
            onClick={() => setShowTagManager(!showTagManager)}
            className="text-sm px-2 py-0.5 border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800"
          >
            Tags
          </button>
          {showTagManager && (
            <div className="absolute right-0 top-8 z-10 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-lg shadow-lg">
              <TagManager
                sessionId={session.id}
                currentTags={session.tags}
                onUpdate={() => { setShowTagManager(false); onRefresh?.(); }}
              />
            </div>
          )}
        </div>
        <OpenTerminalButton path={session.projectPath} sessionId={session.sessionId} />
        <MultiplexerButton path={session.projectPath} />
        <FavoriteButton sessionId={session.id} initialFavorited={session.isFavorited} />
      </div>
      <h1
        className={`text-lg font-semibold mt-2 break-words ${
          session.summary && session.summarySource === "heuristic" ? "italic text-zinc-600 dark:text-zinc-400" : ""
        }`}
      >
        {session.summary || session.projectName}
      </h1>
      <CopyText text={session.sessionId} className="text-sm text-zinc-400 font-mono" />
      <div className="text-sm text-zinc-500 mt-0.5">
        {session.projectName} &middot; {session.gitBranch || "\u2014"} &middot; {session.version || "\u2014"} &middot; {session.permissionMode || "default"}
      </div>
      <div className="text-xs text-zinc-400 mt-1">
        {formatDateTime(session.startedAt)} &middot; {session.messageCount} msgs &middot; total {formatTokens(session.totalInputTokens + session.totalOutputTokens + session.totalCacheCreationTokens + session.totalCacheReadTokens)} &middot; in {formatTokens(session.totalInputTokens)} &middot; out {formatTokens(session.totalOutputTokens)} &middot; cache R {formatTokens(session.totalCacheReadTokens)} &middot; cache W {formatTokens(session.totalCacheCreationTokens)} &middot; {formatFileSize(session.fileSize)}
        {session.isBackedUp && " \u00B7 Backed up"}
      </div>
      {session.copiedFromSessionId && (
        <div className="text-xs text-zinc-400 mt-1">
          Copied from <CopyText text={session.copiedFromSessionId} display={session.copiedFromSessionId.slice(0, 8)} className="text-xs text-zinc-400 font-mono" />
        </div>
      )}
      {(session.tags.length > 0 || session.aiTags.length > 0) && (
        <div className="flex gap-1 mt-2 flex-wrap">
          {session.tags.map((tag) => <TagBadge key={tag.id} tag={tag} />)}
          {session.aiTags.map((tag) => (
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
      {/* Export path dialog */}
      {showExportDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
          <div className="bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-700 rounded-lg shadow-xl p-4 w-[480px]">
            <div className="text-sm font-medium mb-2">Target project path</div>
            <div className="text-xs text-zinc-400 mb-3">This path will be encoded as the directory name inside the zip. On the target machine, unzip into ~/.claude/projects/ to import.</div>
            <input
              type="text"
              value={exportProjectPath}
              onChange={(e) => setExportProjectPath(e.target.value)}
              onKeyDown={async (e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  setShowExportDialog(false);
                  const slug = session.slug || session.sessionId.slice(0, 8);
                  const filePath = await saveDialog({
                    defaultPath: `${exportProjectPath}/${slug}.zip`,
                    filters: [{ name: "ZIP", extensions: ["zip"] }],
                  });
                  if (filePath) {
                    setExporting(true);
                    try {
                      await exportSession(session.id, exportProjectPath, filePath as string);
                      toast.success("Export successful!");
                    } catch (e2) {
                      toast.error(`Export failed: ${e2}`);
                    }
                    setExporting(false);
                  }
                }
              }}
              autoFocus
              className="w-full px-3 py-1.5 rounded border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-800 text-sm font-mono focus:outline-none focus:border-zinc-500"
            />
            <div className="flex justify-end gap-2 mt-3">
              <button
                onClick={() => setShowExportDialog(false)}
                className="text-sm px-3 py-1 rounded border border-zinc-300 dark:border-zinc-700 hover:bg-zinc-100 dark:hover:bg-zinc-800"
              >
                Cancel
              </button>
              <button
                onClick={async () => {
                  setShowExportDialog(false);
                  const slug = session.slug || session.sessionId.slice(0, 8);
                  const filePath = await saveDialog({
                    defaultPath: `${exportProjectPath}/${slug}.zip`,
                    filters: [{ name: "ZIP", extensions: ["zip"] }],
                  });
                  if (filePath) {
                    setExporting(true);
                    try {
                      await exportSession(session.id, exportProjectPath, filePath as string);
                      toast.success("Export successful!");
                    } catch (e2) {
                      toast.error(`Export failed: ${e2}`);
                    }
                    setExporting(false);
                  }
                }}
                className="text-sm px-3 py-1 rounded bg-zinc-800 dark:bg-zinc-200 text-white dark:text-zinc-900 hover:bg-zinc-700 dark:hover:bg-zinc-300"
              >
                Export
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
