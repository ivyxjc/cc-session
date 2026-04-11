import type { Tag } from "../../lib/types";

export function TagBadge({ tag }: { tag: Tag }) {
  return (
    <span
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs font-medium"
      style={{
        backgroundColor: tag.color + "20",
        color: tag.color,
      }}
    >
      <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: tag.color }} />
      {tag.name}
    </span>
  );
}
