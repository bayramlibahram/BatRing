import { Button } from "./ui/button";
import { cn } from "../lib/utils";

const STATUS_STYLES = {
  running: {
    label: "Running",
    dot: "bg-emerald-500 ring-emerald-100",
    text: "text-emerald-700",
  },
  stopped: {
    label: "Stopped",
    dot: "bg-slate-400 ring-slate-100",
    text: "text-slate-600",
  },
  failed: {
    label: "Failed",
    dot: "bg-red-500 ring-red-100",
    text: "text-red-700",
  },
  unknown: {
    label: "Unknown",
    dot: "bg-amber-400 ring-amber-100",
    text: "text-amber-700",
  },
  not_installed: {
    label: "Not installed",
    dot: "bg-slate-300 ring-slate-100",
    text: "text-slate-500",
  },
};

const STARTUP_LABELS = {
  enabled: "Enabled",
  disabled: "Disabled",
  static: "Static",
  masked: "Masked",
  unknown: "—",
};

export default function ServiceCard({ service, busyAction, error, onAction, disabled = false }) {
  const status = STATUS_STYLES[service.status] ?? STATUS_STYLES.unknown;
  const isRunning = service.status === "running";
  const isInstalled = service.status !== "not_installed";
  const isBusy = Boolean(busyAction) || disabled;

  const startupLabel = STARTUP_LABELS[service.startup] ?? STARTUP_LABELS.unknown;
  const startupToggle =
    service.startup === "enabled"
      ? { action: "disable", label: "Disable Startup", busyLabel: "Disabling…" }
      : service.startup === "disabled"
        ? { action: "enable", label: "Enable Startup", busyLabel: "Enabling…" }
        : null;

  return (
    <article className="rounded-lg border border-slate-200 bg-white px-4 py-3 shadow-sm">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <h2 className="truncate text-sm font-semibold text-slate-900">{service.name}</h2>
          <p className="mt-0.5 truncate font-mono text-[11px] text-slate-500">{service.unit}</p>
        </div>

        <div className="shrink-0 text-right">
          <div
            className={cn("flex items-center justify-end gap-2 text-xs font-medium", status.text)}
            role="status"
            aria-live="polite"
          >
            <span className={cn("size-2 rounded-full ring-4", status.dot)} aria-hidden="true" />
            {status.label}
          </div>
          <p className="mt-1 text-[11px] text-slate-500">
            Startup:{" "}
            <span
              className={cn(
                "font-medium",
                service.startup === "enabled" ? "text-slate-800" : "text-slate-500",
              )}
            >
              {startupLabel}
            </span>
          </p>
        </div>
      </div>

      {error ? (
        <div className="mt-3 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-800">
          {error}
        </div>
      ) : null}

      <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-slate-100 pt-3">
        {isInstalled ? (
          <>
            {isRunning ? (
              <Button
                variant="destructive"
                disabled={isBusy}
                onClick={() => onAction("stop")}
              >
                {busyAction === "stop" ? "Stopping…" : "Stop"}
              </Button>
            ) : (
              <Button disabled={isBusy} onClick={() => onAction("start")}>
                {busyAction === "start" ? "Starting…" : "Start"}
              </Button>
            )}

            <Button
              variant="outline"
              disabled={isBusy}
              onClick={() => onAction("restart")}
            >
              {busyAction === "restart" ? "Restarting…" : "Restart"}
            </Button>

            {startupToggle ? (
              <Button
                variant="outline"
                disabled={isBusy}
                onClick={() => onAction(startupToggle.action)}
              >
                {busyAction === startupToggle.action ? startupToggle.busyLabel : startupToggle.label}
              </Button>
            ) : null}
          </>
        ) : (
          <p className="text-xs text-slate-500">
            No systemd unit named <span className="font-mono">{service.unit}</span> was found.
          </p>
        )}

        <Button
          className="ml-auto"
          variant="ghost"
          disabled={isBusy}
          onClick={() => onAction("refresh")}
        >
          {busyAction === "refresh" ? "Refreshing…" : "Refresh"}
        </Button>
      </div>
    </article>
  );
}
