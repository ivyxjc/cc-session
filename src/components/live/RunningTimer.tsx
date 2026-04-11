import { useState, useEffect } from "react";
import { formatDuration } from "../../lib/format";

export function RunningTimer({ startedAt }: { startedAt: number }) {
  const [, setTick] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => setTick((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, []);

  return <span>{formatDuration(startedAt)}</span>;
}
