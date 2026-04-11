export function LiveStatusBadge({ isAlive }: { isAlive: boolean }) {
  if (isAlive) {
    return (
      <span className="inline-flex items-center gap-1 text-xs font-medium text-green-600 dark:text-green-400">
        <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
        Running
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 text-xs font-medium text-zinc-400">
      <span className="w-2 h-2 rounded-full bg-zinc-400" />
      Ended
    </span>
  );
}
