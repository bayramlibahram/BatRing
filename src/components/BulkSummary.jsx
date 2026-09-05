import { cn } from "../lib/utils";

const NOUNS = {
  start_all: { title: "Start All", done: "started" },
  stop_all: { title: "Stop All", done: "stopped" },
  restart_all: { title: "Restart All", done: "restarted" },
  enable_all: { title: "Enable All", done: "enabled at startup" },
  disable_all: { title: "Disable All", done: "disabled at startup" },
};

function plural(count, word) {
  return `${count} ${word}${count === 1 ? "" : "s"}`;
}

export default function BulkSummary({ summary, onDismiss }) {
  if (!summary) {
    return null;
  }

  const nouns = NOUNS[summary.action] ?? { title: "Bulk action", done: "updated" };
  const succeeded = summary.results.filter((result) => result.success);
  const failed = summary.results.filter((result) => !result.success);
  const allFailed = succeeded.length === 0;

  return (
    <section
      aria-live="polite"
      className={cn(
        "mb-4 rounded-lg border px-4 py-3 text-sm",
        allFailed
          ? "border-red-200 bg-red-50 text-red-900"
          : failed.length > 0
            ? "border-amber-200 bg-amber-50 text-amber-900"
            : "border-emerald-200 bg-emerald-50 text-emerald-900",
      )}
    >
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="font-semibold">{nouns.title}</p>
          <p className="mt-0.5 text-xs">
            {plural(succeeded.length, "service")} {nouns.done}
            {failed.length > 0 ? ` · ${plural(failed.length, "service")} failed` : ""}
          </p>
        </div>
        <button
          type="button"
          className="text-xs font-medium underline opacity-80 hover:opacity-100"
          onClick={onDismiss}
        >
          Dismiss
        </button>
      </div>

      <ul className="mt-2 space-y-1 text-xs">
        {summary.results.map((result) => (
          <li key={result.serviceId} className="flex gap-2">
            <span className="w-4 shrink-0 font-semibold" aria-hidden="true">
              {result.success ? "✓" : "✗"}
            </span>
            <span className="w-24 shrink-0 font-medium">{result.name}</span>
            <span className="min-w-0">
              {result.message}
              {!result.success && result.error?.details ? (
                <span className="block truncate font-mono text-[11px] opacity-80">
                  {result.error.details}
                </span>
              ) : null}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
