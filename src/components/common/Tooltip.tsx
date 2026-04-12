import { useRef, useState } from "react";

interface Props {
  text: string;
  delay?: number;
  children: React.ReactNode;
}

export function Tooltip({ text, delay = 500, children }: Props) {
  const [visible, setVisible] = useState(false);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleEnter = (e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setPos({ x: rect.left, y: rect.bottom + 4 });
    timerRef.current = setTimeout(() => setVisible(true), delay);
  };

  const handleLeave = () => {
    if (timerRef.current) clearTimeout(timerRef.current);
    setVisible(false);
  };

  return (
    <div onMouseEnter={handleEnter} onMouseLeave={handleLeave}>
      {children}
      {visible && (
        <div
          className="fixed z-[9999] px-2 py-1.5 bg-zinc-800 dark:bg-zinc-200 text-white dark:text-zinc-800 text-xs rounded shadow-lg break-all pointer-events-none"
          style={{ left: pos.x, top: pos.y, maxWidth: 400 }}
        >
          {text}
        </div>
      )}
    </div>
  );
}
