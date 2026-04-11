import ReactDiffViewer, { DiffMethod } from "react-diff-viewer-continued";

export function DiffView({
  oldString,
  newString,
  filePath,
}: {
  oldString: string;
  newString: string;
  filePath: string;
}) {
  return (
    <div className="rounded-lg overflow-hidden border border-zinc-200 dark:border-zinc-800 my-1">
      <div className="px-3 py-1.5 text-xs text-zinc-500 bg-zinc-50 dark:bg-zinc-900 border-b border-zinc-200 dark:border-zinc-800">
        {filePath}
      </div>
      <ReactDiffViewer
        oldValue={oldString}
        newValue={newString}
        splitView={false}
        compareMethod={DiffMethod.WORDS}
        useDarkTheme={true}
        styles={{
          contentText: { fontSize: "0.8rem", lineHeight: "1.4" },
        }}
      />
    </div>
  );
}
