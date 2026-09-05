use crate::models::{
    BulkServiceResult, CommandError, ErrorCode, Service, ServiceDefinition, ServiceState,
};
use crate::systemd::{self, ServiceAction};

/// The only services BatRing will ever talk to systemd about.
///
/// The frontend sends the `id`; Rust resolves it to the trusted `unit`.
/// Order here is the order the React UI renders the cards.
const SERVICES: &[ServiceDefinition] = &[
    ServiceDefinition {
        id: "postgresql",
        name: "PostgreSQL",
        unit: "postgresql.service",
    },
    ServiceDefinition {
        id: "docker",
        name: "Docker",
        unit: "docker.service",
    },
    ServiceDefinition {
        id: "mongodb",
        name: "MongoDB",
        unit: "mongod.service",
    },
];

fn resolve_service(service_id: &str) -> Result<ServiceDefinition, CommandError> {
    SERVICES
        .iter()
        .copied()
        .find(|service| service.id == service_id)
        .ok_or_else(|| {
            CommandError::new(
                ErrorCode::UnknownService,
                "The requested service is not registered in BatRing.",
            )
        })
}

fn read_service(service: ServiceDefinition) -> Result<Service, CommandError> {
    service_from_inspection(service, systemd::inspect(service))
}

/// A registered service whose unit is missing is reported as "not installed"
/// instead of failing, so one absent service never hides the others.
fn service_from_inspection(
    service: ServiceDefinition,
    inspection: Result<ServiceState, CommandError>,
) -> Result<Service, CommandError> {
    match inspection {
        Ok(state) => Ok(Service::from_definition(service, state)),
        Err(error) if error.code == ErrorCode::UnitNotFound => Ok(Service::not_installed(service)),
        Err(error) => Err(error),
    }
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn get_services() -> Result<Vec<Service>, CommandError> {
    SERVICES.iter().copied().map(read_service).collect()
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn get_service_status(service_id: String) -> Result<Service, CommandError> {
    read_service(resolve_service(&service_id)?)
}

fn apply_action(
    service: ServiceDefinition,
    action: ServiceAction,
) -> Result<Service, CommandError> {
    systemd::run_action(service, action)?;
    read_service(service)
}

fn perform_action(service_id: &str, action: ServiceAction) -> Result<Service, CommandError> {
    apply_action(resolve_service(service_id)?, action)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn start_service(service_id: String) -> Result<Service, CommandError> {
    perform_action(&service_id, ServiceAction::Start)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn stop_service(service_id: String) -> Result<Service, CommandError> {
    perform_action(&service_id, ServiceAction::Stop)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn restart_service(service_id: String) -> Result<Service, CommandError> {
    perform_action(&service_id, ServiceAction::Restart)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn enable_service(service_id: String) -> Result<Service, CommandError> {
    perform_action(&service_id, ServiceAction::Enable)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn disable_service(service_id: String) -> Result<Service, CommandError> {
    perform_action(&service_id, ServiceAction::Disable)
}

/// Apply `action` to every registered service and report each outcome.
///
/// One failing service does not stop the others. The single exception is an
/// authorization failure. PolicyKit caches a successful authorization for a
/// short window, so a bulk run prompts at most once; if that one attempt is
/// refused, or no agent answers it, every remaining service would fail the
/// same way. Those are reported as skipped rather than retried.
fn perform_bulk_action(action: ServiceAction) -> Vec<BulkServiceResult> {
    perform_bulk_with(action, |service| apply_action(service, action))
}

/// Authorization outcomes that make the remaining services pointless to try.
fn aborts_bulk(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::AuthorizationCancelled | ErrorCode::PermissionDenied
    )
}

fn perform_bulk_with<F>(action: ServiceAction, mut apply: F) -> Vec<BulkServiceResult>
where
    F: FnMut(ServiceDefinition) -> Result<Service, CommandError>,
{
    let mut results = Vec::with_capacity(SERVICES.len());
    let mut cancelled = false;

    for service in SERVICES.iter().copied() {
        if cancelled {
            results.push(BulkServiceResult::failed(
                service,
                CommandError::new(
                    ErrorCode::AuthorizationCancelled,
                    "Skipped because authorization did not succeed.",
                ),
            ));
            continue;
        }

        match apply(service) {
            Ok(updated) => {
                results.push(BulkServiceResult::succeeded(
                    updated,
                    action.completed_message(),
                ));
            }
            Err(error) => {
                cancelled = aborts_bulk(error.code);
                results.push(BulkServiceResult::failed(service, error));
            }
        }
    }

    results
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn start_all_services() -> Vec<BulkServiceResult> {
    perform_bulk_action(ServiceAction::Start)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn stop_all_services() -> Vec<BulkServiceResult> {
    perform_bulk_action(ServiceAction::Stop)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn restart_all_services() -> Vec<BulkServiceResult> {
    perform_bulk_action(ServiceAction::Restart)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn enable_all_services() -> Vec<BulkServiceResult> {
    perform_bulk_action(ServiceAction::Enable)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn disable_all_services() -> Vec<BulkServiceResult> {
    perform_bulk_action(ServiceAction::Disable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ServiceStatus, StartupState};

    fn running(service: ServiceDefinition) -> Service {
        Service::from_definition(
            service,
            ServiceState {
                status: ServiceStatus::Running,
                startup: StartupState::Enabled,
            },
        )
    }

    fn failure(code: ErrorCode) -> CommandError {
        CommandError::new(code, "mocked failure")
    }

    #[test]
    fn resolves_postgresql_service() {
        let service = resolve_service("postgresql").expect("postgresql should be registered");
        assert_eq!(service.name, "PostgreSQL");
        assert_eq!(service.unit, "postgresql.service");
    }

    #[test]
    fn resolves_docker_service() {
        let service = resolve_service("docker").expect("docker should be registered");
        assert_eq!(service.name, "Docker");
        assert_eq!(service.unit, "docker.service");
    }

    #[test]
    fn resolves_mongodb_service_to_mongod_unit() {
        let service = resolve_service("mongodb").expect("mongodb should be registered");
        assert_eq!(service.name, "MongoDB");
        assert_eq!(service.unit, "mongod.service");
    }

    #[test]
    fn keeps_registry_order() {
        let service_ids: Vec<_> = SERVICES.iter().map(|service| service.id).collect();
        assert_eq!(service_ids, ["postgresql", "docker", "mongodb"]);
    }

    #[test]
    fn registry_ids_and_units_are_unique() {
        for (index, service) in SERVICES.iter().enumerate() {
            for other in &SERVICES[index + 1..] {
                assert_ne!(service.id, other.id, "duplicate id");
                assert_ne!(service.unit, other.unit, "duplicate unit");
            }
        }
    }

    #[test]
    fn rejects_arbitrary_service_input() {
        for input in [
            "postgresql.service; reboot",
            "mongod.service",
            "mongod",
            "redis",
            "",
            "Docker",
        ] {
            let error = resolve_service(input).expect_err("arbitrary input should be rejected");
            assert_eq!(error.code, ErrorCode::UnknownService, "input {input:?}");
        }
    }

    #[test]
    fn missing_unit_becomes_not_installed_instead_of_an_error() {
        let mongodb = resolve_service("mongodb").unwrap();
        let service =
            service_from_inspection(mongodb, Err(failure(ErrorCode::UnitNotFound))).unwrap();

        assert_eq!(service.status, ServiceStatus::NotInstalled);
        assert_eq!(service.startup, StartupState::Unknown);
        assert_eq!(service.unit, "mongod.service");
    }

    #[test]
    fn other_inspection_errors_still_propagate() {
        let postgresql = resolve_service("postgresql").unwrap();
        let error =
            service_from_inspection(postgresql, Err(failure(ErrorCode::SystemdUnavailable)))
                .expect_err("systemd errors must surface");

        assert_eq!(error.code, ErrorCode::SystemdUnavailable);
    }

    #[test]
    fn bulk_operations_touch_only_registered_services_in_order() {
        let mut seen = Vec::new();
        let results = perform_bulk_with(ServiceAction::Start, |service| {
            seen.push(service.id);
            Ok(running(service))
        });

        assert_eq!(seen, ["postgresql", "docker", "mongodb"]);
        assert_eq!(results.len(), SERVICES.len());
        assert!(results.iter().all(|result| result.success));
        assert!(results.iter().all(|result| result.message == "Started"));
        assert!(results.iter().all(|result| result.service.is_some()));
    }

    #[test]
    fn one_failure_does_not_stop_the_other_services() {
        let results = perform_bulk_with(ServiceAction::Start, |service| {
            if service.id == "docker" {
                Err(failure(ErrorCode::CommandFailed))
            } else {
                Ok(running(service))
            }
        });

        let outcomes: Vec<_> = results
            .iter()
            .map(|result| (result.service_id.as_str(), result.success))
            .collect();
        assert_eq!(
            outcomes,
            [("postgresql", true), ("docker", false), ("mongodb", true)]
        );

        let docker = &results[1];
        assert_eq!(docker.message, "mocked failure");
        assert_eq!(
            docker.error.as_ref().unwrap().code,
            ErrorCode::CommandFailed
        );
        assert!(docker.service.is_none());

        assert_eq!(results.iter().filter(|result| result.success).count(), 2);
        assert_eq!(results.iter().filter(|result| !result.success).count(), 1);
    }

    #[test]
    fn a_missing_unit_in_a_bulk_operation_is_just_one_failed_row() {
        let results = perform_bulk_with(ServiceAction::Restart, |service| {
            if service.id == "mongodb" {
                Err(failure(ErrorCode::UnitNotFound))
            } else {
                Ok(running(service))
            }
        });

        assert_eq!(results[2].service_id, "mongodb");
        assert!(!results[2].success);
        assert_eq!(
            results[2].error.as_ref().unwrap().code,
            ErrorCode::UnitNotFound
        );
        assert!(results[0].success && results[1].success);
    }

    #[test]
    fn cancelled_authorization_skips_the_remaining_services() {
        let mut attempts = 0;
        let results = perform_bulk_with(ServiceAction::Stop, |_| {
            attempts += 1;
            Err(failure(ErrorCode::AuthorizationCancelled))
        });

        assert_eq!(attempts, 1, "must not prompt again after a cancel");
        assert_eq!(results.len(), SERVICES.len());
        assert!(results.iter().all(|result| !result.success));
        assert!(
            results
                .iter()
                .all(|result| result.error.as_ref().unwrap().code
                    == ErrorCode::AuthorizationCancelled)
        );
        assert_eq!(
            results[1].message,
            "Skipped because authorization did not succeed."
        );
    }

    #[test]
    fn a_missing_polkit_agent_also_skips_the_remaining_services() {
        let mut attempts = 0;
        let results = perform_bulk_with(ServiceAction::Enable, |_| {
            attempts += 1;
            Err(failure(ErrorCode::PermissionDenied))
        });

        assert_eq!(attempts, 1, "a missing agent will not answer the next one");
        assert!(results.iter().all(|result| !result.success));
    }

    #[test]
    fn ordinary_failures_never_abort_the_bulk_loop() {
        for code in [
            ErrorCode::CommandFailed,
            ErrorCode::UnitNotFound,
            ErrorCode::SystemdUnavailable,
            ErrorCode::UnknownService,
        ] {
            assert!(!aborts_bulk(code), "{code:?} must not stop the loop");
        }
        assert!(aborts_bulk(ErrorCode::AuthorizationCancelled));
        assert!(aborts_bulk(ErrorCode::PermissionDenied));
    }

    #[test]
    fn every_service_is_still_attempted_when_failures_are_ordinary() {
        let mut attempts = 0;
        let results = perform_bulk_with(ServiceAction::Start, |_| {
            attempts += 1;
            Err(failure(ErrorCode::CommandFailed))
        });

        assert_eq!(attempts, SERVICES.len());
        assert_eq!(results.len(), SERVICES.len());
        assert!(results.iter().all(|result| !result.success));
    }

    #[test]
    fn enable_and_disable_bulk_results_use_startup_wording() {
        let enabled = perform_bulk_with(ServiceAction::Enable, |service| Ok(running(service)));
        assert!(enabled
            .iter()
            .all(|result| result.message == "Startup enabled"));

        let disabled = perform_bulk_with(ServiceAction::Disable, |service| Ok(running(service)));
        assert!(disabled
            .iter()
            .all(|result| result.message == "Startup disabled"));
    }

    /// Read-only smoke test against the real systemd on this machine.
    /// Run with `cargo test -- --ignored live_`. It never mutates anything.
    #[test]
    #[ignore]
    fn live_registry_inspection_is_read_only() {
        let services = get_services().expect("get_services should not fail as a whole");
        assert_eq!(services.len(), SERVICES.len());
        for service in &services {
            println!(
                "{:<12} {:<20} status={:?} startup={:?}",
                service.id, service.unit, service.status, service.startup
            );
        }
        let unknown = get_service_status("mongod.service".into())
            .expect_err("raw unit names must be rejected");
        assert_eq!(unknown.code, ErrorCode::UnknownService);
    }
}
