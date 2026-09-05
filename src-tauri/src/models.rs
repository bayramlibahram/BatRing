use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    Running,
    Stopped,
    Failed,
    Unknown,
    /// The unit is registered in BatRing but systemd does not know it.
    NotInstalled,
}

/// Whether systemd starts the unit automatically at boot.
///
/// This is independent from [`ServiceStatus`]: a service can be running
/// while disabled at boot, or stopped while enabled at boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupState {
    Enabled,
    Disabled,
    /// The unit has no `[Install]` section and cannot be enabled or disabled.
    Static,
    /// The unit is masked and cannot be started or enabled until unmasked.
    Masked,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub unit: &'static str,
}

/// The live systemd state of a registered service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceState {
    pub status: ServiceStatus,
    pub startup: StartupState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: String,
    pub name: String,
    pub unit: String,
    pub status: ServiceStatus,
    pub startup: StartupState,
}

impl Service {
    pub fn from_definition(definition: ServiceDefinition, state: ServiceState) -> Self {
        Self {
            id: definition.id.to_owned(),
            name: definition.name.to_owned(),
            unit: definition.unit.to_owned(),
            status: state.status,
            startup: state.startup,
        }
    }

    pub fn not_installed(definition: ServiceDefinition) -> Self {
        Self::from_definition(
            definition,
            ServiceState {
                status: ServiceStatus::NotInstalled,
                startup: StartupState::Unknown,
            },
        )
    }
}

/// The outcome of one service inside a bulk operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkServiceResult {
    pub service_id: String,
    pub name: String,
    pub success: bool,
    pub message: String,
    /// The refreshed service snapshot when the action succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Service>,
    /// The structured error when the action failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
}

impl BulkServiceResult {
    pub fn succeeded(service: Service, message: impl Into<String>) -> Self {
        Self {
            service_id: service.id.clone(),
            name: service.name.clone(),
            success: true,
            message: message.into(),
            service: Some(service),
            error: None,
        }
    }

    pub fn failed(definition: ServiceDefinition, error: CommandError) -> Self {
        Self {
            service_id: definition.id.to_owned(),
            name: definition.name.to_owned(),
            success: false,
            message: error.message.clone(),
            service: None,
            error: Some(error),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnknownService,
    UnitNotFound,
    PermissionDenied,
    AuthorizationCancelled,
    SystemdUnavailable,
    CommandFailed,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        let details = details.into();
        if !details.is_empty() {
            self.details = Some(details);
        }
        self
    }
}
