export function formatRelativeTime(timestampMs: number | null): string {
  if (!timestampMs) return "Unknown";
  const diff = Date.now() - timestampMs;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(timestampMs).toLocaleDateString();
}

export function formatTokens(count: number): string {
  if (count < 1000) return `${count}`;
  if (count < 1_000_000) return `${(count / 1000).toFixed(1)}K`;
  return `${(count / 1_000_000).toFixed(1)}M`;
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

let _locale: string | undefined;

export function setLocale(locale: string | undefined) {
  _locale = locale;
}

export function getLocale(): string | undefined {
  return _locale;
}

export function formatDateTime(timestampMs: number | null): string {
  if (!timestampMs) return "Unknown";
  const locale = _locale || navigator.language || undefined;
  return new Date(timestampMs).toLocaleString(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
