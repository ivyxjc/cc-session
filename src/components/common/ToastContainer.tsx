import { useToastStore, type ToastType } from "../../stores/toastStore";

const typeStyles: Record<ToastType, string> = {
  success: "bg-emerald-600 dark:bg-emerald-700 text-white",
  error: "bg-red-600 dark:bg-red-700 text-white",
  info: "bg-zinc-700 dark:bg-zinc-600 text-white",
};

const typeIcons: Record<ToastType, string> = {
  success: "\u2713",
  error: "\u2717",
  info: "\u2139",
};

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);
  const removeToast = useToastStore((s) => s.removeToast);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-[100] flex flex-col gap-2 pointer-events-none">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`pointer-events-auto flex items-center gap-2 px-4 py-2.5 rounded-lg shadow-lg text-sm font-medium animate-in slide-in-from-right ${typeStyles[t.type]}`}
          style={{ animation: "slideIn 0.2s ease-out" }}
        >
          <span className="text-base leading-none">{typeIcons[t.type]}</span>
          <span className="flex-1">{t.message}</span>
          <button
            onClick={() => removeToast(t.id)}
            className="ml-2 opacity-70 hover:opacity-100 text-base leading-none"
          >
            &times;
          </button>
        </div>
      ))}
    </div>
  );
}
