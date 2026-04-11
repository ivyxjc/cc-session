import { useState } from "react";
import { toggleFavorite, backupSession } from "../../lib/tauri";

export function FavoriteButton({
  sessionId,
  initialFavorited,
  onToggle,
}: {
  sessionId: number;
  initialFavorited: boolean;
  onToggle?: (favorited: boolean) => void;
}) {
  const [favorited, setFavorited] = useState(initialFavorited);

  const handleClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const result = await toggleFavorite(sessionId);
    setFavorited(result);
    onToggle?.(result);
    // Auto-backup when favorited
    if (result) {
      backupSession(sessionId).catch(console.error);
    }
  };

  return (
    <button onClick={handleClick} className="text-lg hover:scale-110 transition-transform" title={favorited ? "Remove from favorites" : "Add to favorites"}>
      {favorited ? "\u2605" : "\u2606"}
    </button>
  );
}
