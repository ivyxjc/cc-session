import { useEffect, useState } from "react";
import { readImageFile } from "../../lib/tauri";

const IMAGE_PATH_RE = /^\[Image(?:: source|  source):\s*(.+\.(?:png|jpg|jpeg|gif|webp))\]$/i;

/** Check if a text block is an image path reference like [Image: source: /path/to/file.png] */
export function isImagePathText(text: string): boolean {
  return IMAGE_PATH_RE.test(text.trim());
}

/** Extract the file path from an image reference text */
export function extractImagePath(text: string): string | null {
  const match = text.trim().match(IMAGE_PATH_RE);
  return match ? match[1] : null;
}

export function ImageFromPath({ path }: { path: string }) {
  const [dataUrl, setDataUrl] = useState<string | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    readImageFile(path)
      .then((base64) => {
        const ext = path.split(".").pop()?.toLowerCase() || "png";
        const mime = ext === "jpg" || ext === "jpeg" ? "image/jpeg" : `image/${ext}`;
        setDataUrl(`data:${mime};base64,${base64}`);
      })
      .catch(() => setError(true));
  }, [path]);

  if (error) {
    return (
      <div className="text-xs text-zinc-400 italic py-1">
        Image not found: {path}
      </div>
    );
  }

  if (!dataUrl) {
    return <div className="text-xs text-zinc-400 py-1">Loading image...</div>;
  }

  return (
    <img
      src={dataUrl}
      alt="User image"
      className="max-w-full max-h-96 rounded border border-zinc-200 dark:border-zinc-700"
      loading="lazy"
    />
  );
}
