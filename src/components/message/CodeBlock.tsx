import { useEffect, useState } from "react";
import { codeToHtml } from "shiki";

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

export function CodeBlock({ code, language }: { code: string; language?: string }) {
  const [html, setHtml] = useState<string>("");

  useEffect(() => {
    const theme = isDarkMode() ? "github-dark" : "github-light";
    codeToHtml(code, {
      lang: language || "text",
      theme,
    })
      .then((result) => {
        // Strip inline background-color from shiki's <pre> so our CSS controls it
        const cleaned = result.replace(/background-color:\s*#[0-9a-fA-F]+;?/g, "");
        setHtml(cleaned);
      })
      .catch(() => setHtml(`<pre><code>${escapeHtml(code)}</code></pre>`));
  }, [code, language]);

  if (!html) {
    return (
      <pre className="p-3 bg-zinc-100 dark:bg-zinc-900 rounded text-sm text-zinc-700 dark:text-zinc-300 overflow-x-auto border border-zinc-200 dark:border-zinc-700">
        <code>{code}</code>
      </pre>
    );
  }

  return (
    <div
      className="rounded overflow-x-auto text-sm [&_pre]:p-3 [&_pre]:bg-zinc-100 [&_pre]:dark:bg-zinc-900 [&_pre]:rounded [&_pre]:border [&_pre]:border-zinc-200 [&_pre]:dark:border-zinc-700"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
