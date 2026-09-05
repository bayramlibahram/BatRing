import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ServiceCard from "./components/ServiceCard";
import BulkControls from "./components/BulkControls";
import BulkSummary from "./components/BulkSummary";

// Per-service commands. Every command takes an internal service ID
// ("postgresql", "docker", "mongodb"); Rust resolves it to the systemd unit.
const COMMANDS = {
  start: "start_service",
  stop: "stop_service",
  restart: "restart_service",
  enable: "enable_service",
  disable: "disable_service",
  refresh: "get_service_status",
};

// Bulk commands take no arguments: Rust iterates its own registry.
const BULK_COMMANDS = {
  start_all: "start_all_services",
  stop_all: "stop_all_services",
  restart_all: "restart_all_services",
  enable_all: "enable_all_services",
  disable_all: "disable_all_services",
};

function getErrorMessage(error) {
  if (error && typeof error === "object" && "message" in error) {
    return error.message;
  }

  return typeof error === "string" ? error : "The service operation failed.";
}

export default function App() {
  const [services, setServices] = useState([]);
  const [busyById, setBusyById] = useState({});
  const [errorsById, setErrorsById] = useState({});
  const [bulkBusy, setBulkBusy] = useState("");
  const [bulkSummary, setBulkSummary] = useState(null);
  const [loadError, setLoadError] = useState("");
  const [loading, setLoading] = useState(true);

  const loadServices = useCallback(async () => {
    setLoading(true);
    setLoadError("");

    try {
      const result = await invoke("get_services");
      setServices(result);
    } catch (error) {
      setLoadError(getErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadServices();
  }, [loadServices]);

  function replaceService(updated) {
    setServices((current) =>
      current.map((service) => (service.id === updated.id ? updated : service)),
    );
  }

  async function handleAction(serviceId, action) {
    setBusyById((current) => ({ ...current, [serviceId]: action }));
    setErrorsById((current) => ({ ...current, [serviceId]: "" }));

    try {
      const updated = await invoke(COMMANDS[action], { serviceId });
      replaceService(updated);
    } catch (error) {
      setErrorsById((current) => ({
        ...current,
        [serviceId]: getErrorMessage(error),
      }));
    } finally {
      setBusyById((current) => ({ ...current, [serviceId]: "" }));
    }
  }

  async function handleBulkAction(action) {
    setBulkBusy(action);
    setBulkSummary(null);
    setErrorsById({});

    try {
      const results = await invoke(BULK_COMMANDS[action]);
      results.forEach((result) => {
        if (result.service) {
          replaceService(result.service);
        }
      });
      setBulkSummary({ action, results });
    } catch (error) {
      setLoadError(getErrorMessage(error));
    } finally {
      setBulkBusy("");
    }
  }

  return (
    <div className="min-h-screen bg-slate-50 text-slate-950">
      <header className="border-b border-slate-200 bg-white">
        <div className="mx-auto flex h-12 max-w-3xl items-center px-5">
          <div className="mr-2.5 flex size-6 items-center justify-center rounded-md bg-slate-900 text-[11px] font-bold text-white">
            B
          </div>
          <h1 className="text-sm font-semibold tracking-tight">BatRing</h1>
          <span className="ml-auto rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide text-slate-500">
            Linux
          </span>
        </div>
      </header>

      <main className="mx-auto max-w-3xl px-5 py-6">
        <div className="mb-3 flex items-end justify-between gap-4">
          <div>
            <h2 className="text-base font-semibold text-slate-900">Services</h2>
            <p className="mt-0.5 text-xs text-slate-500">Manage local development services.</p>
          </div>
          <button
            type="button"
            className="text-xs font-medium text-slate-500 hover:text-slate-900 disabled:opacity-50"
            disabled={loading || Boolean(bulkBusy)}
            onClick={loadServices}
          >
            Refresh all
          </button>
        </div>

        <BulkControls
          busyAction={bulkBusy}
          disabled={loading || services.length === 0}
          onAction={handleBulkAction}
        />

        <BulkSummary summary={bulkSummary} onDismiss={() => setBulkSummary(null)} />

        {loadError ? (
          <div className="mb-4 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
            <p>{loadError}</p>
            <button type="button" className="mt-2 text-xs font-semibold underline" onClick={loadServices}>
              Try again
            </button>
          </div>
        ) : null}

        {loading && services.length === 0 ? (
          <div className="rounded-lg border border-slate-200 bg-white px-4 py-8 text-center text-sm text-slate-500">
            Checking services…
          </div>
        ) : null}

        <div className="space-y-3">
          {services.map((service) => (
            <ServiceCard
              key={service.id}
              service={service}
              busyAction={busyById[service.id]}
              error={errorsById[service.id]}
              disabled={Boolean(bulkBusy)}
              onAction={(action) => handleAction(service.id, action)}
            />
          ))}
        </div>
      </main>
    </div>
  );
}
