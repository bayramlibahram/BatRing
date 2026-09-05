# BatRing

BatRing is a Linux-only Tauri desktop utility for managing local development services through systemd.

## Managed services

| ID | Display name | systemd unit |
| --- | --- | --- |
| `postgresql` | PostgreSQL | `postgresql.service` |
| `docker` | Docker | `docker.service` |
| `mongodb` | MongoDB | `mongod.service` |

Every registered service supports:

- status detection: Running, Stopped, Failed, Unknown, or Not installed
- startup detection: Enabled, Disabled, Static, or Masked
- Start, Stop, Restart (runtime state)
- Enable Startup, Disable Startup (boot behaviour, never touches runtime state)
- structured errors returned to the React UI
- rejection of any service ID not present in the Rust registry

A registered service whose unit is missing (for example MongoDB on a machine
without the `mongodb-org` package) is shown as **Not installed** and its
actions are hidden. It never breaks the other cards.

## Bulk controls

The header of the main view offers:

```text
Services   [ Start All ] [ Stop All ] [ Restart All ]
Startup    [ Enable All ] [ Disable All ]
```

Each bulk action iterates the Rust registry and applies the matching
single-service action. One failing service does not stop the others; every
service gets a structured result (`serviceId`, `name`, `success`, `message`,
optional `service` snapshot, optional `error`). The UI renders a summary such
as "2 services started · 1 service failed" plus the per-service rows.

A bulk run prompts for a password at most once, because PolicyKit retains the
authorization for the rest of the window (see Privileges below). The only
short-circuit is an authorization failure: if that single prompt is refused, or
no authentication agent answers it, the remaining services would fail
identically, so they are reported as skipped rather than retried. Every other
kind of failure lets the loop continue.

`Enable All` / `Disable All` run `systemctl enable|disable <unit>` without
`--now`, so they never start or stop anything.

## Architecture

```text
React (ServiceCard, BulkControls, BulkSummary)
      ↓ invoke("start_service", { serviceId: "docker" })   or   invoke("start_all_services")
Tauri command handler (src-tauri/src/commands.rs)
      ↓ resolve ID against the fixed SERVICES registry
Rust systemd adapter (src-tauri/src/systemd.rs)
      ↓
systemctl  ->  systemd over D-Bus  ->  PolicyKit
```

The frontend never sends unit names or shell commands. Rust accepts a service
ID, resolves it against a fixed registry, and invokes `systemctl` directly with
a two-element constant argument vector; no shell is involved. Reads and writes
use the same binary, and privilege is decided by systemd and PolicyKit rather
than by BatRing elevating itself.

## Privileges

Reads (`systemctl show`, `is-active`, `is-enabled`) run as the desktop user and
authorize nothing.

Mutations also run as the desktop user:

```bash
systemctl start|stop|restart|enable|disable <registered-unit>
```

`systemctl` forwards the request to systemd over D-Bus, and systemd asks
PolicyKit to authorize it against:

| Action | PolicyKit action | Typical desktop setting |
| --- | --- | --- |
| start, stop, restart | `org.freedesktop.systemd1.manage-units` | `auth_admin_keep` |
| enable, disable | `org.freedesktop.systemd1.manage-unit-files` | `auth_admin_keep` |

The `_keep` suffix is the important part: PolicyKit retains a successful
authorization for a short window, so the session's authentication agent prompts
once and subsequent actions, including a full bulk run, go through without
another password.

BatRing deliberately does **not** use `pkexec`. That path is checked against
`org.freedesktop.policykit.exec`, which is plain `auth_admin` with no
retention, so every single click would re-prompt. It also grants "run this
program as root" rather than the narrower "manage this unit".

Requirements: BatRing must not be run as root, the user must be a PolicyKit
administrator (on Ubuntu, a member of `sudo` or `admin`), and a graphical
PolicyKit authentication agent must be running. Without an agent, systemd
reports that interactive authentication was never enabled and BatRing surfaces
that as a `permission_denied` error rather than a generic failure.

## Development

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the full guide: prerequisites,
dev mode, production builds, installers, tests, and troubleshooting.

Prerequisites include Node.js, Rust, the Tauri 2 Linux system dependencies, systemd, and PolicyKit.

```bash
npm install
npm run tauri dev
```

Checks:

```bash
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --debug --no-bundle
```

Read-only smoke test against the real systemd on this machine (no prompts, no
mutations):

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_ --nocapture
```

If the Tauri Linux packages (`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev`)
are unavailable, the systemd core can still be checked with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
```

## How to add another service to BatRing

The service flow is generic. To add Redis (unit `redis-server.service` on
Debian/Ubuntu), append one entry to `SERVICES` in `src-tauri/src/commands.rs`:

```rust
ServiceDefinition {
    id: "redis",
    name: "Redis",
    unit: "redis-server.service",
},
```

Then:

1. Add a registry test next to `resolves_mongodb_service_to_mongod_unit`
   asserting `resolve_service("redis")` returns `redis-server.service`.
2. Update the expected order in `keeps_registry_order` and
   `bulk_operations_touch_only_registered_services_in_order`.
3. Run the checks above.

Nothing else changes. `get_services()` returns Redis to React, `App.jsx`
renders another `ServiceCard`, every single-service command resolves `redis`
through the registry, and all five bulk commands pick it up automatically
because they iterate `SERVICES`.
