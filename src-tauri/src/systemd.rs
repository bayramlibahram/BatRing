use std::io;
use std::process::{Command, Output};

use crate::models::{
    CommandError, ErrorCode, ServiceDefinition, ServiceState, ServiceStatus, StartupState,
};

const SYSTEMCTL: &str = "/usr/bin/systemctl";

/// A mutation BatRing can apply to a registered unit.
///
/// `Start`, `Stop`, and `Restart` change the current runtime state.
/// `Enable` and `Disable` only change whether the unit starts at boot;
/// they never start or stop the unit (BatRing never passes `--now`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Enable,
    Disable,
}

impl ServiceAction {
    fn systemctl_argument(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }

    fn present_participle(self) -> &'static str {
        match self {
            Self::Start => "starting",
            Self::Stop => "stopping",
            Self::Restart => "restarting",
            Self::Enable => "enabling startup for",
            Self::Disable => "disabling startup for",
        }
    }

    /// Short confirmation used in per-service bulk results.
    pub fn completed_message(self) -> &'static str {
        match self {
            Self::Start => "Started",
            Self::Stop => "Stopped",
            Self::Restart => "Restarted",
            Self::Enable => "Startup enabled",
            Self::Disable => "Startup disabled",
        }
    }
}

/// Read the runtime status and startup state of a registered unit.
///
/// Runs unprivileged `systemctl` queries only; never authorizes anything.
pub fn inspect(service: ServiceDefinition) -> Result<ServiceState, CommandError> {
    ensure_unit_exists(service)?;

    let status = read_status(service)?;
    let startup = read_startup(service)?;

    Ok(ServiceState { status, startup })
}

fn read_status(service: ServiceDefinition) -> Result<ServiceStatus, CommandError> {
    let output = Command::new(SYSTEMCTL)
        .args(["is-active", service.unit])
        .output()
        .map_err(|error| spawn_error("systemctl", error))?;

    classify_status_output(service, &output)
}

fn read_startup(service: ServiceDefinition) -> Result<StartupState, CommandError> {
    let output = Command::new(SYSTEMCTL)
        .args(["is-enabled", service.unit])
        .output()
        .map_err(|error| spawn_error("systemctl", error))?;

    classify_startup_output(service, &output)
}

fn ensure_unit_exists(service: ServiceDefinition) -> Result<(), CommandError> {
    let output = Command::new(SYSTEMCTL)
        .args(["show", "--property=LoadState", "--value", service.unit])
        .output()
        .map_err(|error| spawn_error("systemctl", error))?;

    classify_load_state_output(service, &output)
}

/// Apply a mutation to a registered unit.
///
/// BatRing runs `systemctl` as the desktop user. `systemctl` forwards the
/// request to systemd over D-Bus, and systemd asks PolicyKit to authorize it
/// against `org.freedesktop.systemd1.manage-units` (start, stop, restart) or
/// `manage-unit-files` (enable, disable). Both are configured `auth_admin_keep`
/// on a typical desktop, so the session's PolicyKit agent prompts once and then
/// retains the authorization for a short window. Repeated actions and bulk
/// operations inside that window need no further prompt.
///
/// BatRing deliberately does not use `pkexec`. That would be checked against
/// `org.freedesktop.policykit.exec`, which is plain `auth_admin` and therefore
/// re-prompts on every single action, and it would grant "run this program as
/// root" instead of the narrower "manage this unit".
///
/// The unit is verified first with an unprivileged query so a missing unit
/// fails fast without showing an authorization prompt.
pub fn run_action(service: ServiceDefinition, action: ServiceAction) -> Result<(), CommandError> {
    ensure_unit_exists(service)?;

    let output = Command::new(SYSTEMCTL)
        .args(action_arguments(service, action))
        .output()
        .map_err(|error| spawn_error("systemctl", error))?;

    if output.status.success() {
        return Ok(());
    }

    Err(classify_action_error(service, action, &output))
}

/// The exact argument vector handed to `systemctl`.
///
/// Every element is a compile-time constant from the registry or the action
/// table; nothing from the frontend reaches this list, and no shell is involved.
///
/// `--no-ask-password` is deliberately absent: it would disable interactive
/// PolicyKit authorization and turn every mutation into an "Access denied"
/// failure.
fn action_arguments(service: ServiceDefinition, action: ServiceAction) -> [&'static str; 2] {
    [action.systemctl_argument(), service.unit]
}

fn classify_status_output(
    service: ServiceDefinition,
    output: &Output,
) -> Result<ServiceStatus, CommandError> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined = format!("{stdout}\n{stderr}").to_lowercase();

    if indicates_no_systemd(&combined) {
        return Err(systemd_unavailable().with_details(stderr));
    }

    if indicates_missing_unit(&combined) {
        return Err(unit_not_found(service).with_details(stderr));
    }

    match stdout.as_str() {
        "active" => Ok(ServiceStatus::Running),
        "inactive" | "deactivating" => Ok(ServiceStatus::Stopped),
        "failed" => Ok(ServiceStatus::Failed),
        _ if output.status.success() => Ok(ServiceStatus::Unknown),
        _ => Err(CommandError::new(
            ErrorCode::CommandFailed,
            format!("Could not read the status of {}.", service.name),
        )
        .with_details(non_empty_output(&stdout, &stderr))),
    }
}

fn classify_startup_output(
    service: ServiceDefinition,
    output: &Output,
) -> Result<StartupState, CommandError> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined = format!("{stdout}\n{stderr}").to_lowercase();

    if indicates_no_systemd(&combined) {
        return Err(systemd_unavailable().with_details(stderr));
    }

    if indicates_missing_unit(&combined) {
        return Err(unit_not_found(service).with_details(stderr));
    }

    // `systemctl is-enabled` exits non-zero for disabled and masked units,
    // so the printed state matters more than the exit code.
    Ok(match stdout.as_str() {
        "enabled" | "enabled-runtime" | "alias" | "linked" | "linked-runtime" => {
            StartupState::Enabled
        }
        // `indirect` units are not enabled themselves but can be enabled.
        "disabled" | "indirect" => StartupState::Disabled,
        "static" | "generated" | "transient" => StartupState::Static,
        "masked" | "masked-runtime" => StartupState::Masked,
        _ => StartupState::Unknown,
    })
}

fn classify_load_state_output(
    service: ServiceDefinition,
    output: &Output,
) -> Result<(), CommandError> {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined = format!("{stdout}\n{stderr}").to_lowercase();

    if indicates_no_systemd(&combined) {
        return Err(systemd_unavailable().with_details(stderr));
    }

    if stdout == "not-found" || indicates_missing_unit(&combined) {
        return Err(unit_not_found(service).with_details(stderr));
    }

    if output.status.success() {
        Ok(())
    } else {
        Err(CommandError::new(
            ErrorCode::CommandFailed,
            format!("Could not inspect the {} systemd unit.", service.name),
        )
        .with_details(non_empty_output(&stdout, &stderr)))
    }
}

/// Turn a failed `systemctl` mutation into a structured error.
///
/// The interesting cases come from PolicyKit and are distinguished by whether
/// interactive authorization was available:
///
/// * "requires interactive authentication ... has not been enabled" means no
///   PolicyKit agent answered, so nobody was ever asked.
/// * A plain "Access denied" means the agent did ask and the answer was no,
///   either because the dialog was dismissed or the user is not an admin.
fn classify_action_error(
    service: ServiceDefinition,
    action: ServiceAction,
    output: &Output,
) -> CommandError {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let combined = format!("{stdout}\n{stderr}").to_lowercase();

    if indicates_no_systemd(&combined) {
        return systemd_unavailable().with_details(stderr);
    }

    if indicates_no_auth_agent(&combined) {
        return CommandError::new(
            ErrorCode::PermissionDenied,
            format!(
                "No PolicyKit authentication agent answered while {} {}.",
                action.present_participle(),
                service.name
            ),
        )
        .with_details(stderr);
    }

    if indicates_authorization_refused(&combined) {
        return CommandError::new(
            ErrorCode::AuthorizationCancelled,
            format!(
                "Authorization was cancelled or denied while {} {}.",
                action.present_participle(),
                service.name
            ),
        )
        .with_details(stderr);
    }

    if indicates_missing_unit(&combined) {
        return unit_not_found(service).with_details(stderr);
    }

    CommandError::new(
        ErrorCode::CommandFailed,
        format!(
            "BatRing failed while {} {}.",
            action.present_participle(),
            service.name
        ),
    )
    .with_details(non_empty_output(&stdout, &stderr))
}

fn unit_not_found(service: ServiceDefinition) -> CommandError {
    CommandError::new(
        ErrorCode::UnitNotFound,
        format!(
            "{} is not installed or its unit was not found.",
            service.name
        ),
    )
}

fn systemd_unavailable() -> CommandError {
    CommandError::new(
        ErrorCode::SystemdUnavailable,
        "systemd is not available on this Linux system.",
    )
}

/// systemd could not ask anyone, because interactive authorization was not
/// offered or no PolicyKit agent is registered for the session.
fn indicates_no_auth_agent(output: &str) -> bool {
    output.contains("interactive authentication has not been enabled")
        || output.contains("interactive authentication required")
        || output.contains("no authentication agent")
}

/// Somebody was asked and the answer was no.
fn indicates_authorization_refused(output: &str) -> bool {
    output.contains("access denied")
        || output.contains("not authorized")
        || output.contains("permission denied")
        || output.contains("authentication failed")
        || output.contains("dismissed")
        || output.contains("cancelled")
        || output.contains("canceled")
}

fn indicates_missing_unit(output: &str) -> bool {
    output.contains("could not be found")
        || output.contains("not found")
        || output.contains("not-found")
        || output.contains("not loaded")
        || output.contains("no such file or directory")
}

fn indicates_no_systemd(output: &str) -> bool {
    output.contains("system has not been booted with systemd")
        || output.contains("failed to connect to bus")
}

fn non_empty_output(stdout: &str, stderr: &str) -> String {
    if !stderr.is_empty() {
        stderr.to_owned()
    } else {
        stdout.to_owned()
    }
}

fn spawn_error(program: &str, error: io::Error) -> CommandError {
    CommandError::new(
        ErrorCode::CommandFailed,
        format!("Could not run {program}."),
    )
    .with_details(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    const POSTGRESQL: ServiceDefinition = ServiceDefinition {
        id: "postgresql",
        name: "PostgreSQL",
        unit: "postgresql.service",
    };

    const MONGODB: ServiceDefinition = ServiceDefinition {
        id: "mongodb",
        name: "MongoDB",
        unit: "mongod.service",
    };

    /// The exact text systemd emits when no PolicyKit agent can be consulted,
    /// captured from `systemctl --no-ask-password stop docker.service`.
    const NO_AGENT_STDERR: &str = concat!(
        "Failed to stop docker.service: Access denied as the requested operation ",
        "requires interactive authentication. However, interactive authentication ",
        "has not been enabled by the calling program.\n",
        "See system logs and 'systemctl status docker.service' for details."
    );

    fn output(code: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn maps_active_to_running() {
        let result = classify_status_output(POSTGRESQL, &output(0, "active\n", ""));
        assert_eq!(result, Ok(ServiceStatus::Running));
    }

    #[test]
    fn maps_not_found_load_state_to_structured_error() {
        let result = classify_load_state_output(POSTGRESQL, &output(0, "not-found\n", ""));
        assert_eq!(
            result.expect_err("expected an error").code,
            ErrorCode::UnitNotFound
        );
    }

    #[test]
    fn maps_inactive_exit_code_to_stopped() {
        let result = classify_status_output(POSTGRESQL, &output(3, "inactive\n", ""));
        assert_eq!(result, Ok(ServiceStatus::Stopped));
    }

    #[test]
    fn maps_failed_to_failed() {
        let result = classify_status_output(POSTGRESQL, &output(3, "failed\n", ""));
        assert_eq!(result, Ok(ServiceStatus::Failed));
    }

    #[test]
    fn reports_missing_units_as_structured_errors() {
        let result = classify_status_output(
            POSTGRESQL,
            &output(
                4,
                "unknown\n",
                "Unit postgresql.service could not be found.",
            ),
        );

        assert_eq!(
            result.expect_err("expected an error").code,
            ErrorCode::UnitNotFound
        );
    }

    #[test]
    fn maps_is_enabled_output_to_startup_state() {
        let cases = [
            (0, "enabled\n", StartupState::Enabled),
            (0, "enabled-runtime\n", StartupState::Enabled),
            (0, "alias\n", StartupState::Enabled),
            (1, "disabled\n", StartupState::Disabled),
            (0, "indirect\n", StartupState::Disabled),
            (0, "static\n", StartupState::Static),
            (0, "generated\n", StartupState::Static),
            (1, "masked\n", StartupState::Masked),
            (0, "something-new\n", StartupState::Unknown),
        ];

        for (code, stdout, expected) in cases {
            let result = classify_startup_output(POSTGRESQL, &output(code, stdout, ""));
            assert_eq!(result, Ok(expected), "is-enabled printed {stdout:?}");
        }
    }

    #[test]
    fn missing_unit_in_is_enabled_is_a_structured_error() {
        // Observed on this machine for an uninstalled MongoDB: exit 4, stdout "not-found".
        let result = classify_startup_output(MONGODB, &output(4, "not-found\n", ""));
        assert_eq!(
            result.expect_err("expected an error").code,
            ErrorCode::UnitNotFound
        );
    }

    #[test]
    fn action_arguments_are_fixed() {
        assert_eq!(ServiceAction::Start.systemctl_argument(), "start");
        assert_eq!(ServiceAction::Stop.systemctl_argument(), "stop");
        assert_eq!(ServiceAction::Restart.systemctl_argument(), "restart");
        assert_eq!(ServiceAction::Enable.systemctl_argument(), "enable");
        assert_eq!(ServiceAction::Disable.systemctl_argument(), "disable");
    }

    #[test]
    fn mutations_run_systemctl_directly_without_pkexec() {
        // The D-Bus path is what makes PolicyKit's auth_admin_keep caching
        // apply. Reintroducing pkexec would re-prompt on every action.
        assert_eq!(
            action_arguments(MONGODB, ServiceAction::Restart),
            ["restart", "mongod.service"]
        );
        assert!(!SYSTEMCTL.contains("pkexec"));
    }

    #[test]
    fn mutations_never_disable_interactive_authorization() {
        // `--no-ask-password` would suppress the PolicyKit prompt entirely and
        // make every mutation fail with "Access denied".
        for action in [
            ServiceAction::Start,
            ServiceAction::Stop,
            ServiceAction::Restart,
            ServiceAction::Enable,
            ServiceAction::Disable,
        ] {
            let arguments = action_arguments(POSTGRESQL, action);
            assert_eq!(arguments.len(), 2);
            assert!(arguments.iter().all(|argument| !argument.starts_with("--")));
        }
    }

    #[test]
    fn enable_and_disable_never_touch_runtime_state() {
        // `systemctl enable --now` would also start the unit and
        // `systemctl disable --now` would also stop it. BatRing must never
        // pass `--now`, so Enable All cannot start and Disable All cannot stop.
        for action in [ServiceAction::Enable, ServiceAction::Disable] {
            let arguments = action_arguments(POSTGRESQL, action);
            assert!(!arguments.contains(&"--now"));
            assert!(!arguments.contains(&"start"));
            assert!(!arguments.contains(&"stop"));
        }
    }

    #[test]
    fn reports_a_missing_polkit_agent_as_permission_denied() {
        let error = classify_action_error(
            POSTGRESQL,
            ServiceAction::Stop,
            &output(1, "", NO_AGENT_STDERR),
        );

        assert_eq!(error.code, ErrorCode::PermissionDenied);
        assert!(error.message.contains("No PolicyKit authentication agent"));
    }

    #[test]
    fn reports_a_refused_prompt_as_cancelled() {
        // Interaction was offered and the answer was no, so systemd reports a
        // bare "Access denied" with no "interactive authentication" clause.
        let error = classify_action_error(
            POSTGRESQL,
            ServiceAction::Start,
            &output(1, "", "Failed to start postgresql.service: Access denied"),
        );

        assert_eq!(error.code, ErrorCode::AuthorizationCancelled);
    }

    #[test]
    fn reports_a_dismissed_prompt_as_cancelled() {
        let error = classify_action_error(
            POSTGRESQL,
            ServiceAction::Start,
            &output(1, "", "Error executing operation: Request dismissed"),
        );

        assert_eq!(error.code, ErrorCode::AuthorizationCancelled);
    }

    #[test]
    fn recognises_the_unit_not_found_message_systemctl_actually_prints() {
        // Captured from `systemctl start batring-nonexistent-probe.service`.
        let error = classify_action_error(
            MONGODB,
            ServiceAction::Start,
            &output(
                5,
                "",
                "Failed to start mongod.service: Unit mongod.service not found.",
            ),
        );

        assert_eq!(error.code, ErrorCode::UnitNotFound);
    }

    #[test]
    fn authorization_failures_are_not_mistaken_for_missing_units() {
        // The no-agent message mentions neither "not found" nor "not-found",
        // but it is checked first regardless so ordering cannot regress.
        let error = classify_action_error(
            MONGODB,
            ServiceAction::Enable,
            &output(1, "", NO_AGENT_STDERR),
        );

        assert_eq!(error.code, ErrorCode::PermissionDenied);
    }

    #[test]
    fn enable_errors_describe_startup_not_runtime() {
        let error = classify_action_error(
            POSTGRESQL,
            ServiceAction::Enable,
            &output(1, "", "Failed to enable unit: File exists"),
        );

        assert_eq!(error.code, ErrorCode::CommandFailed);
        assert!(error.message.contains("enabling startup for PostgreSQL"));
    }

    #[test]
    fn sysv_synchronisation_noise_does_not_change_classification() {
        // `systemctl enable postgresql.service` prints SysV chatter on stdout
        // before the real failure arrives on stderr.
        let error = classify_action_error(
            POSTGRESQL,
            ServiceAction::Enable,
            &output(
                1,
                "Synchronizing state of postgresql.service with SysV service script.",
                "Failed to enable unit: Access denied",
            ),
        );

        assert_eq!(error.code, ErrorCode::AuthorizationCancelled);
    }
}
