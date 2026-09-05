import { Button } from "./ui/button";

const BULK_ACTIONS = {
  services: [
    { id: "start_all", label: "Start All", busyLabel: "Starting all…" },
    { id: "stop_all", label: "Stop All", busyLabel: "Stopping all…", variant: "destructive" },
    { id: "restart_all", label: "Restart All", busyLabel: "Restarting all…", variant: "outline" },
  ],
  startup: [
    { id: "enable_all", label: "Enable All", busyLabel: "Enabling all…", variant: "outline" },
    { id: "disable_all", label: "Disable All", busyLabel: "Disabling all…", variant: "outline" },
  ],
};

function ActionRow({ title, hint, actions, busyAction, disabled, onAction }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <div className="w-20 shrink-0">
        <p className="text-xs font-semibold text-slate-900">{title}</p>
        <p className="text-[11px] text-slate-500">{hint}</p>
      </div>
      <div className="flex flex-wrap gap-2">
        {actions.map((action) => (
          <Button
            key={action.id}
            variant={action.variant ?? "default"}
            disabled={disabled}
            onClick={() => onAction(action.id)}
          >
            {busyAction === action.id ? action.busyLabel : action.label}
          </Button>
        ))}
      </div>
    </div>
  );
}

export default function BulkControls({ busyAction, disabled, onAction }) {
  const isDisabled = disabled || Boolean(busyAction);

  return (
    <section
      aria-label="Bulk controls"
      className="mb-4 space-y-3 rounded-lg border border-slate-200 bg-white px-4 py-3 shadow-sm"
    >
      <ActionRow
        title="Services"
        hint="Runtime state"
        actions={BULK_ACTIONS.services}
        busyAction={busyAction}
        disabled={isDisabled}
        onAction={onAction}
      />
      <ActionRow
        title="Startup"
        hint="At boot"
        actions={BULK_ACTIONS.startup}
        busyAction={busyAction}
        disabled={isDisabled}
        onAction={onAction}
      />
    </section>
  );
}
