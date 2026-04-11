import { useEffect, useState } from "react";
import { listTags, createTag, tagSession, untagSession } from "../../lib/tauri";
import type { Tag } from "../../lib/types";

const PRESET_COLORS = ["#ef4444", "#f97316", "#eab308", "#22c55e", "#3b82f6", "#8b5cf6", "#ec4899"];

export function TagManager({
  sessionId,
  currentTags,
  onUpdate,
}: {
  sessionId: number;
  currentTags: Tag[];
  onUpdate: () => void;
}) {
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState("");
  const [newColor, setNewColor] = useState(PRESET_COLORS[0]);

  useEffect(() => {
    listTags().then(setAllTags);
  }, []);

  const currentIds = new Set(currentTags.map((t) => t.id));

  const handleToggle = async (tag: Tag) => {
    if (currentIds.has(tag.id)) {
      await untagSession(sessionId, tag.id);
    } else {
      await tagSession(sessionId, tag.id);
    }
    onUpdate();
  };

  const handleCreate = async () => {
    if (!newName.trim()) return;
    const tag = await createTag(newName.trim(), newColor);
    await tagSession(sessionId, tag.id);
    setNewName("");
    setShowCreate(false);
    setAllTags((prev) => [...prev, tag]);
    onUpdate();
  };

  return (
    <div className="p-2 space-y-2 min-w-48">
      {allTags.map((tag) => (
        <button
          key={tag.id}
          onClick={() => handleToggle(tag)}
          className="w-full text-left flex items-center gap-2 px-2 py-1 rounded text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
        >
          <span className="w-3 h-3 rounded-full" style={{ backgroundColor: tag.color }} />
          <span className="flex-1">{tag.name}</span>
          {currentIds.has(tag.id) && <span className="text-blue-500">&#10003;</span>}
        </button>
      ))}
      {showCreate ? (
        <div className="space-y-2 pt-2 border-t border-zinc-200 dark:border-zinc-800">
          <input
            type="text"
            placeholder="Tag name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleCreate()}
            className="w-full px-2 py-1 border border-zinc-300 dark:border-zinc-700 rounded text-sm bg-transparent"
            autoFocus
          />
          <div className="flex gap-1">
            {PRESET_COLORS.map((c) => (
              <button
                key={c}
                onClick={() => setNewColor(c)}
                className={`w-5 h-5 rounded-full ${newColor === c ? "ring-2 ring-offset-1 ring-blue-500" : ""}`}
                style={{ backgroundColor: c }}
              />
            ))}
          </div>
          <div className="flex gap-1">
            <button onClick={handleCreate} className="px-2 py-1 bg-blue-600 text-white rounded text-xs">Create</button>
            <button onClick={() => setShowCreate(false)} className="px-2 py-1 text-xs text-zinc-500">Cancel</button>
          </div>
        </div>
      ) : (
        <button
          onClick={() => setShowCreate(true)}
          className="w-full text-left px-2 py-1 text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
        >
          + New tag
        </button>
      )}
    </div>
  );
}
