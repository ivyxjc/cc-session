import { useEffect, useState } from "react";
import { getDailyUsage } from "../../lib/tauri";
import { formatTokens } from "../../lib/format";
import type { DailyUsage } from "../../lib/types";

export function UsagePage() {
  const [usage, setUsage] = useState<DailyUsage[]>([]);
  const [days, setDays] = useState(30);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    getDailyUsage(days).then(setUsage).catch(console.error).finally(() => setLoading(false));
  }, [days]);

  const totals = usage.reduce(
    (acc, d) => ({
      sessions: acc.sessions + d.sessionCount,
      userMsgs: acc.userMsgs + d.userMsgCount,
      input: acc.input + d.totalInputTokens,
      output: acc.output + d.totalOutputTokens,
      cacheR: acc.cacheR + d.totalCacheReadTokens,
      cacheW: acc.cacheW + d.totalCacheCreationTokens,
      total: acc.total + d.totalTokens,
    }),
    { sessions: 0, userMsgs: 0, input: 0, output: 0, cacheR: 0, cacheW: 0, total: 0 },
  );

  return (
    <div className="p-6 h-full overflow-y-auto">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-xl font-semibold">Token Usage</h1>
        <select
          value={days}
          onChange={(e) => setDays(Number(e.target.value))}
          className="text-sm px-2 py-1 rounded border border-zinc-300 dark:border-zinc-700 bg-white dark:bg-zinc-800"
        >
          <option value={7}>Last 7 days</option>
          <option value={30}>Last 30 days</option>
          <option value={90}>Last 90 days</option>
          <option value={365}>Last year</option>
        </select>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-3 mb-6 max-w-3xl">
        <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-800">
          <div className="text-xs text-zinc-400">Total Tokens</div>
          <div className="text-lg font-semibold">{formatTokens(totals.total)}</div>
        </div>
        <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-800">
          <div className="text-xs text-zinc-400">Sessions</div>
          <div className="text-lg font-semibold">{totals.sessions}</div>
        </div>
        <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-800">
          <div className="text-xs text-zinc-400">User Messages</div>
          <div className="text-lg font-semibold">{totals.userMsgs}</div>
        </div>
        <div className="p-3 rounded-lg border border-zinc-200 dark:border-zinc-800">
          <div className="text-xs text-zinc-400">Output Tokens</div>
          <div className="text-lg font-semibold">{formatTokens(totals.output)}</div>
        </div>
      </div>

      {loading ? (
        <div className="text-zinc-500">Loading...</div>
      ) : usage.length === 0 ? (
        <div className="text-zinc-500">No usage data found. Try refreshing the index first.</div>
      ) : (
        <div className="max-w-4xl">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-zinc-200 dark:border-zinc-800 text-left text-xs text-zinc-500">
                <th className="py-2 pr-4">Date</th>
                <th className="py-2 pr-4 text-right">Sessions</th>
                <th className="py-2 pr-4 text-right">User Msgs</th>
                <th className="py-2 pr-4 text-right">Input</th>
                <th className="py-2 pr-4 text-right">Output</th>
                <th className="py-2 pr-4 text-right">Cache R</th>
                <th className="py-2 pr-4 text-right">Cache W</th>
                <th className="py-2 text-right">Total</th>
              </tr>
            </thead>
            <tbody>
              {usage.map((d) => (
                <tr key={d.date} className="border-b border-zinc-100 dark:border-zinc-800/50">
                  <td className="py-2 pr-4 font-mono">{d.date}</td>
                  <td className="py-2 pr-4 text-right">{d.sessionCount}</td>
                  <td className="py-2 pr-4 text-right">{d.userMsgCount}</td>
                  <td className="py-2 pr-4 text-right text-zinc-500">{formatTokens(d.totalInputTokens)}</td>
                  <td className="py-2 pr-4 text-right text-zinc-500">{formatTokens(d.totalOutputTokens)}</td>
                  <td className="py-2 pr-4 text-right text-zinc-500">{formatTokens(d.totalCacheReadTokens)}</td>
                  <td className="py-2 pr-4 text-right text-zinc-500">{formatTokens(d.totalCacheCreationTokens)}</td>
                  <td className="py-2 text-right font-medium">{formatTokens(d.totalTokens)}</td>
                </tr>
              ))}
              <tr className="border-t-2 border-zinc-300 dark:border-zinc-700 font-medium">
                <td className="py-2 pr-4">Total</td>
                <td className="py-2 pr-4 text-right">{totals.sessions}</td>
                <td className="py-2 pr-4 text-right">{totals.userMsgs}</td>
                <td className="py-2 pr-4 text-right">{formatTokens(totals.input)}</td>
                <td className="py-2 pr-4 text-right">{formatTokens(totals.output)}</td>
                <td className="py-2 pr-4 text-right">{formatTokens(totals.cacheR)}</td>
                <td className="py-2 pr-4 text-right">{formatTokens(totals.cacheW)}</td>
                <td className="py-2 text-right">{formatTokens(totals.total)}</td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
