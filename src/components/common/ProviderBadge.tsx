import type { Provider } from "../../lib/types";

/** Small pill naming the agent CLI a session came from. */
export function ProviderBadge({ provider }: { provider: Provider }) {
  const cls = provider === "codex"
    ? "text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950"
    : "text-zinc-600 dark:text-zinc-300 bg-zinc-100 dark:bg-zinc-800";
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded font-medium uppercase tracking-wide ${cls}`}>
      {provider}
    </span>
  );
}
