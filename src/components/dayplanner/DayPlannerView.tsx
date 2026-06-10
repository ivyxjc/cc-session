import { useCallback, useEffect, useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { generateDailySummary, getDayPlanner } from "../../lib/tauri";
import type { DailyReport, DayPlannerBlock } from "../../lib/types";
import { toast } from "../../stores/toastStore";

function todayISO(): string {
  // YYYY-MM-DD in local time (so the date matches what the user sees in clocks).
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

function shiftDate(iso: string, days: number): string {
  const [y, m, d] = iso.split("-").map(Number);
  const dt = new Date(y, m - 1, d);
  dt.setDate(dt.getDate() + days);
  return `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, "0")}-${String(dt.getDate()).padStart(2, "0")}`;
}

/** Local-time [start_ms, end_ms] window covering the entire ISO date. */
function dayWindow(iso: string): { startMs: number; endMs: number } {
  const [y, m, d] = iso.split("-").map(Number);
  const start = new Date(y, m - 1, d, 0, 0, 0, 0);
  const end = new Date(y, m - 1, d, 23, 59, 59, 999);
  return { startMs: start.getTime(), endMs: end.getTime() };
}

/** Format an epoch ms as "HH:MM" in the OS-local timezone. */
function formatHHMM(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** Merge multiple blocks of the same session into one spanning earliest→latest.
 *  Tracks the actual active minutes (sum of source-block durations) so the UI
 *  can show "spans 09:00-22:00 but actually 3h 12m active". */
function mergeBySession(blocks: DayPlannerBlock[]): (DayPlannerBlock & { activeMs: number; fragmentCount: number })[] {
  const map = new Map<number, DayPlannerBlock & { activeMs: number; fragmentCount: number }>();
  for (const b of blocks) {
    const dur = Math.max(0, b.endMs - b.startMs);
    const existing = map.get(b.sessionDbId);
    if (!existing) {
      map.set(b.sessionDbId, { ...b, activeMs: dur, fragmentCount: 1 });
    } else {
      existing.startMs = Math.min(existing.startMs, b.startMs);
      existing.endMs = Math.max(existing.endMs, b.endMs);
      existing.activeMs += dur;
      existing.fragmentCount += 1;
    }
  }
  return Array.from(map.values()).sort((a, b) => a.startMs - b.startMs);
}

/** Render blocks as Day Planner-style Markdown (paste into an Obsidian daily note).
 *  Prefers day-specific summary + tags when available. */
function blocksToMarkdown(date: string, blocks: DayPlannerBlock[]): string {
  if (blocks.length === 0) {
    return `# Day planner — ${date}\n\n_(no Claude Code activity)_\n`;
  }
  const lines = [`# Day planner — ${date}`, ""];
  for (const b of blocks) {
    const title = b.dailySummary || b.title;
    const tags = b.dailyTags.length > 0 ? b.dailyTags : b.aiTags;
    const tagPart = tags.length > 0 ? " " + tags.map((t) => `#${t}`).join(" ") : "";
    lines.push(`- [ ] ${formatHHMM(b.startMs)} - ${formatHHMM(b.endMs)} [${b.projectName}] ${title}${tagPart}`);
  }
  return lines.join("\n") + "\n";
}

export function DayPlannerView() {
  const [date, setDate] = useState<string>(() => todayISO());
  const [blocks, setBlocks] = useState<DayPlannerBlock[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  // Merge same-session blocks by default — gives one row per session per day.
  // Off = show the raw 30-min-gap-split blocks (more granular, more noisy).
  const [mergeSameSession, setMergeSameSession] = useState<boolean>(true);
  // Daily report state (LLM map-reduce)
  const [report, setReport] = useState<DailyReport | null>(null);
  const [reportLoading, setReportLoading] = useState(false);
  const [reportError, setReportError] = useState<string | null>(null);

  // Pure blocks fetch (no report-state side effects). Used both by the
  // date-change effect and by post-generation refresh.
  const fetchBlocks = useCallback(async (d: string) => {
    const { startMs, endMs } = dayWindow(d);
    const result = await getDayPlanner(startMs, endMs, d);
    setBlocks(result);
  }, []);

  // Full reset triggered by date change: clear report state, then fetch blocks.
  const load = useCallback(async (d: string) => {
    setLoading(true);
    setError(null);
    setReport(null);
    setReportError(null);
    try {
      await fetchBlocks(d);
    } catch (e) {
      setError(String(e));
      setBlocks([]);
    } finally {
      setLoading(false);
    }
  }, [fetchBlocks]);

  useEffect(() => { load(date); }, [date, load]);

  const runReport = useCallback(async (force: boolean) => {
    setReportLoading(true);
    setReportError(null);
    try {
      const { startMs, endMs } = dayWindow(date);
      const r = await generateDailySummary(date, startMs, endMs, force);
      setReport(r);
      // Refresh blocks so newly-cached per-session daily summaries flow into
      // block titles. Crucially, do NOT call `load` here — `load` resets the
      // report state we just set.
      await fetchBlocks(date);
    } catch (e) {
      setReportError(String(e));
    } finally {
      setReportLoading(false);
    }
  }, [date, fetchBlocks]);

  // What the UI actually shows: merged-by-session when the toggle is on.
  const displayBlocks = useMemo(
    () => (mergeSameSession ? mergeBySession(blocks) : blocks),
    [blocks, mergeSameSession],
  );

  const markdown = useMemo(() => blocksToMarkdown(date, displayBlocks), [date, displayBlocks]);

  // Active minutes uses the sum of original (un-merged) block durations so the
  // number stays accurate even when display is merged.
  const totalMinutes = useMemo(
    () => blocks.reduce((acc, b) => acc + Math.round((b.endMs - b.startMs) / 60_000), 0),
    [blocks],
  );

  const projectCount = useMemo(
    () => new Set(blocks.map((b) => b.projectName)).size,
    [blocks],
  );

  const copyMarkdown = async () => {
    try {
      await navigator.clipboard.writeText(markdown);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      toast.error(`Copy failed: ${e}`);
    }
  };

  return (
    <div className="h-full overflow-y-auto p-6">
      {/* Header: date nav + summary line */}
      <div className="mb-4 max-w-3xl">
        <h1 className="text-xl font-semibold">Daily Activity</h1>
        <p className="text-sm text-zinc-500 mt-1">
          Day Planner-style timeline of every Claude Code session you touched. Copy the Markdown
          into an Obsidian daily note and the{" "}
          <a href="https://github.com/ivan-lednev/obsidian-day-planner" target="_blank" rel="noopener noreferrer" className="underline">
            day-planner plugin
          </a>{" "}
          renders it as a timetable.
        </p>
      </div>

      <div className="mb-4 max-w-3xl flex items-center gap-2 flex-wrap">
        <button
          onClick={() => setDate(shiftDate(date, -1))}
          className="px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          ← Prev
        </button>
        <input
          type="date"
          value={date}
          onChange={(e) => e.target.value && setDate(e.target.value)}
          className="px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm bg-white dark:bg-zinc-800"
        />
        <button
          onClick={() => setDate(shiftDate(date, 1))}
          className="px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          Next →
        </button>
        <button
          onClick={() => setDate(todayISO())}
          className="px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          Today
        </button>
        <label className="flex items-center gap-1.5 text-xs text-zinc-600 dark:text-zinc-400 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={mergeSameSession}
            onChange={(e) => setMergeSameSession(e.target.checked)}
          />
          Merge same session
        </label>
        <div className="flex-1" />
        {!loading && !error && (
          <span className="text-xs text-zinc-500">
            {displayBlocks.length} row{displayBlocks.length === 1 ? "" : "s"}
            {projectCount > 0 && <> · {projectCount} project{projectCount === 1 ? "" : "s"}</>}
            {totalMinutes > 0 && <> · {Math.floor(totalMinutes / 60)}h {totalMinutes % 60}m active</>}
          </span>
        )}
      </div>

      {/* Body */}
      <div className="max-w-3xl">
        {loading && <div className="text-zinc-500 text-sm">Scanning sessions…</div>}
        {error && <div className="text-red-500 text-sm">Failed: {error}</div>}

        {!loading && !error && (
          <div className="space-y-4">
            {/* AI Daily Report */}
            <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg overflow-hidden">
              <div className="px-4 py-2 bg-zinc-50 dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800 flex items-center gap-2">
                <span className="text-sm font-medium">AI Daily Report</span>
                <span className="text-xs text-zinc-400">two-step LLM: per-session day-slice → unified narrative</span>
                <div className="flex-1" />
                {report && (
                  <button
                    onClick={() => runReport(true)}
                    disabled={reportLoading || blocks.length === 0}
                    className="px-2 py-0.5 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
                    title="Force regenerate ignoring cache"
                  >
                    Regenerate
                  </button>
                )}
                <button
                  onClick={() => runReport(false)}
                  disabled={reportLoading || blocks.length === 0}
                  className="px-3 py-1 text-xs bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                >
                  {reportLoading ? "Generating…" : report ? "Refresh" : "Generate"}
                </button>
              </div>
              <div className="px-4 py-3 text-sm">
                {reportError && (
                  <div className="text-red-500">Generation failed: {reportError}</div>
                )}
                {!report && !reportError && !reportLoading && (
                  <div className="text-zinc-400 italic text-xs">
                    Click Generate to summarize today's work with the LLM. Each session's
                    day-slice is summarized first, then those are combined into a daily narrative.
                    Cached results are reused on the next Generate.
                  </div>
                )}
                {reportLoading && (
                  <div className="text-zinc-500 text-xs">
                    Running the cascade… can take a few seconds × N sessions. Cached sessions
                    will skip the LLM call.
                  </div>
                )}
                {report && (
                  <>
                    <div className="prose prose-sm dark:prose-invert max-w-none">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{report.narrative}</ReactMarkdown>
                    </div>
                    {report.errors.length > 0 && (
                      <details className="mt-3 text-xs">
                        <summary className="cursor-pointer text-amber-600 dark:text-amber-400">
                          ⚠ {report.errors.length} session{report.errors.length === 1 ? "" : "s"} failed to summarize (click to inspect)
                        </summary>
                        <ul className="mt-2 space-y-1 list-disc list-inside text-zinc-500">
                          {report.errors.map((e) => (
                            <li key={e.sessionDbId} className="break-words">
                              <span className="text-zinc-700 dark:text-zinc-300">[{e.projectName}]</span>
                              <span className="font-mono text-[10px] text-zinc-400 ml-1">{e.sessionId.slice(0, 8)}</span>
                              <span className="ml-2 text-red-500">{e.error}</span>
                            </li>
                          ))}
                        </ul>
                      </details>
                    )}
                    <div className="mt-2 text-[11px] text-zinc-400">
                      {report.perSession.length} session{report.perSession.length === 1 ? "" : "s"} summarized
                      {report.errors.length > 0 && <> · {report.errors.length} failed</>}
                    </div>
                  </>
                )}
              </div>
            </div>

            {/* Block list (rich rendering) */}
            <div className="border border-zinc-200 dark:border-zinc-800 rounded-lg divide-y divide-zinc-200 dark:divide-zinc-800">
              {displayBlocks.length === 0 && (
                <div className="px-4 py-6 text-center text-zinc-400 text-sm italic">
                  No Claude Code activity on this day.
                </div>
              )}
              {displayBlocks.map((b, i) => {
                // Prefer day-specific summary/tags when the daily cascade has run.
                const isDaily = !!b.dailySummary;
                const label = b.dailySummary || b.title;
                const tags = b.dailyTags.length > 0 ? b.dailyTags : b.aiTags;
                // When merged, expose how many original fragments folded into this row.
                const merged = b as DayPlannerBlock & { fragmentCount?: number; activeMs?: number };
                const fragCount = merged.fragmentCount ?? 1;
                const activeMs = merged.activeMs;
                return (
                  <div key={i} className="px-4 py-2 flex items-baseline gap-3">
                    <span className="font-mono text-sm text-zinc-700 dark:text-zinc-300 whitespace-nowrap">
                      {formatHHMM(b.startMs)} – {formatHHMM(b.endMs)}
                    </span>
                    <span className="text-xs text-zinc-500 whitespace-nowrap">
                      [{b.projectName}]
                    </span>
                    <span
                      className={`text-sm flex-1 truncate ${isDaily ? "text-zinc-900 dark:text-zinc-100" : "text-zinc-600 dark:text-zinc-400 italic"}`}
                      title={label}
                    >
                      {label}
                    </span>
                    {fragCount > 1 && activeMs != null && (
                      <span
                        className="text-[10px] text-zinc-400 whitespace-nowrap"
                        title={`Spans the time range but only ~${Math.round(activeMs / 60_000)}m of actual messages, split across ${fragCount} fragments`}
                      >
                        ×{fragCount} · {Math.round(activeMs / 60_000)}m
                      </span>
                    )}
                    <div className="flex gap-1 shrink-0">
                      {tags.slice(0, 3).map((t) => (
                        <span
                          key={t}
                          className={`px-1.5 py-0.5 text-[10px] rounded border ${
                            isDaily
                              ? "border-blue-300 dark:border-blue-800 text-blue-700 dark:text-blue-400"
                              : "border-emerald-300 dark:border-emerald-800 text-emerald-700 dark:text-emerald-400"
                          }`}
                        >
                          {t}
                        </span>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Markdown output */}
            <div>
              <div className="flex items-center gap-2 mb-2">
                <h2 className="text-sm font-medium text-zinc-700 dark:text-zinc-300">Markdown</h2>
                <span className="text-xs text-zinc-400">paste into your Obsidian daily note</span>
                <div className="flex-1" />
                <button
                  onClick={copyMarkdown}
                  disabled={blocks.length === 0}
                  className="px-2.5 py-1 text-xs border border-zinc-300 dark:border-zinc-700 rounded hover:bg-zinc-100 dark:hover:bg-zinc-800 disabled:opacity-50"
                >
                  {copied ? "Copied!" : "Copy"}
                </button>
              </div>
              <pre className="text-xs font-mono p-3 rounded border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900 whitespace-pre-wrap break-words overflow-auto max-h-96">
                {markdown}
              </pre>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
