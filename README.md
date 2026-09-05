<h1 align="center">BatRing</h1>

<p align="center">
  A small Linux desktop app for starting, stopping, and auto-starting your local development services.
</p>

<p align="center">
  <img alt="Platform: Linux" src="https://img.shields.io/badge/platform-Linux-333">
  <img alt="Built with Tauri" src="https://img.shields.io/badge/built%20with-Tauri%202-24C8DB">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.77%2B-CE422B">
  <img alt="React 19" src="https://img.shields.io/badge/React-19-61DAFB">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue">
</p>

---

PostgreSQL, Docker, and MongoDB all live behind `systemctl`. BatRing puts them
in one window, so you stop memorising unit names and typing your password over
and over.

```text
┌──────────────────────────────────────────────────────────┐
│  B  BatRing                                       LINUX  │
├──────────────────────────────────────────────────────────┤
│  Services   [ Start All ]  [ Stop All ]  [ Restart All ] │
│  Startup    [ Enable All ]  [ Disable All ]              │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │ PostgreSQL                            ● Running    │  │
│  │ postgresql.service               Startup: Enabled  │  │
│  │ [ Stop ]  [ Restart ]  [ Disable Startup ]         │  │
│  └────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────┐  │
│  │ Docker                                ● Running    │  │
│  │ docker.service                   Startup: Enabled  │  │
│  │ [ Stop ]  [ Restart ]  [ Disable Startup ]         │  │
│  └────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────┐  │
│  │ MongoDB                               ○ Stopped    │  │
│  │ mongod.service                  Startup: Disabled  │  │
│  │ [ Start ]  [ Restart ]  [ Enable Startup ]         │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Features

- **One window for every service.** Live status and startup state at a glance.
- **Individual controls.** Start, Stop, Restart, Enable Startup, Disable Startup.
- **Bulk controls.** Do all five to every service at once.
- **Runtime and boot are kept separate.** Stopping a service never disables it,
  and disabling startup never stops it.
- **One password prompt per session, not per click.** See
  [Authorization](#authorization).
- **Partial failures are visible.** If one service fails, the others still run
  and you get a per-service report.
- **Missing services degrade gracefully.** A service that is not installed is
  labelled as such instead of breaking the list.
- **No shell, ever.** The UI cannot name a unit or a command. See
  [Security](#security).

## Supported services

| Service | Internal ID | systemd unit |
| --- | --- | --- |
| PostgreSQL | `postgresql` | `postgresql.service` |
| Docker | `docker` | `docker.service` |
| MongoDB | `mongodb` | `mongod.service` |

Adding another takes one entry in a Rust array and nothing else. See
[Adding a service](#adding-a-service).

## Requirements

- Linux with systemd
- PolicyKit, plus a graphical authentication agent (every mainstream desktop
  ships one)
- A user account in the `sudo` or `admin` group
- GTK 3 and WebKitGTK 4.1

BatRing must **not** be run as root. It is designed to run as you and let
PolicyKit approve each privileged operation.

## Install

There are no published releases yet, so build the bundles first with
`npm run tauri build` and install from
`src-tauri/target/release/bundle/`.

**Debian / Ubuntu**

```bash
sudo apt install ./BatRing_0.1.0_amd64.deb
batring
```

**AppImage** (portable, no install)

```bash
chmod +x BatRing_0.1.0_amd64.AppImage
./BatRing_0.1.0_amd64.AppImage
```

**From source**

```bash
git clone git@github.com:bayramlibahram/BatRing.git
cd BatRing
npm install
npm run tauri build
```

Bundles land in `src-tauri/target/release/bundle/`. Full instructions are in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Usage

### Runtime versus startup

These are two independent things, and BatRing keeps them apart on purpose.

| | Controls | Effect |
| --- | --- | --- |
| **Runtime** | Start, Stop, Restart | Whether the service is running right now |
| **Startup** | Enable, Disable | Whether it comes back after a reboot |

A service can be running while disabled at boot, or stopped while enabled. The
card shows both states, and the bulk section keeps them in separate rows.

`Enable All` will not start anything. `Disable All` will not stop anything.
BatRing never passes `--now` to `systemctl`.

### Bulk actions and partial failure

A bulk action walks the registry and reports on every service independently.
One failure does not cancel the rest:

```text
Start All
2 services started · 1 service failed

  ✓ PostgreSQL   Started
  ✓ Docker       Started
  ✗ MongoDB      MongoDB is not installed or its unit was not found.
```

The one exception is authorization. If the single password prompt is refused,
or no authentication agent answers it, the remaining services would fail
identically, so they are reported as skipped rather than retried.

## Authorization

BatRing asks for your password **once**, then stays quiet for the next few
minutes, including across bulk actions.

That is a deliberate design choice, not a default. There are two ways a desktop
app can run a privileged `systemctl` command, and they behave very differently:

| Approach | PolicyKit action consulted | Setting | Prompts |
| --- | --- | --- | --- |
| `pkexec systemctl ...` | `org.freedesktop.policykit.exec` | `auth_admin` | **every single time** |
| `systemctl ...` over D-Bus | `org.freedesktop.systemd1.manage-units` and `manage-unit-files` | `auth_admin_keep` | **once, then cached** |

BatRing takes the second path. It runs `systemctl` as your normal user;
`systemctl` forwards the request to systemd over D-Bus, and systemd asks
PolicyKit to authorize it. The `_keep` suffix means PolicyKit retains a
successful authorization for a short window.

The security win matters as much as the convenience one. `pkexec` grants "run
this program as root". The D-Bus path grants only "manage this systemd unit".

## Security

The frontend never names a unit and never builds a command.

```text
React
  │  invoke("start_service", { serviceId: "mongodb" })
  ▼
Tauri command handler          src-tauri/src/commands.rs
  │  resolve the ID against a fixed SERVICES registry
  │  postgresql → postgresql.service
  │  docker     → docker.service
  │  mongodb    → mongod.service
  ▼
systemd adapter                src-tauri/src/systemd.rs
  │  Command::new("/usr/bin/systemctl").args(["start", "mongod.service"])
  ▼
systemd over D-Bus  →  PolicyKit  →  authentication agent
```

Concretely, that means:

- Unit names are `&'static str` compile-time constants. An ID that is not in
  the registry is rejected before systemd is touched.
- Arguments are a fixed two-element array. No shell is spawned, so there is
  nothing to inject into.
- Reads (`is-active`, `is-enabled`, `show`) are unprivileged and prompt for
  nothing.
- The unit is checked for existence before any mutation, so a missing service
  fails fast without a pointless password prompt.
- BatRing is never root and holds no elevated privileges of its own.

## Adding a service

The whole flow is generic. To add Redis, append one entry to `SERVICES` in
`src-tauri/src/commands.rs`:

```rust
ServiceDefinition {
    id: "redis",
    name: "Redis",
    unit: "redis-server.service",
},
```

That is the entire feature. No React component, Tauri command, or systemd code
changes. `get_services()` returns Redis, `App.jsx` renders another
`ServiceCard`, every single-service command resolves the new ID through the
registry, and all five bulk commands pick it up because they iterate the same
list.

Then update the two tests that assert registry contents and run the checks:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

## Architecture

```text
src/                             React frontend
  App.jsx                        state and command invocation
  components/ServiceCard.jsx     one card, reused for every service
  components/BulkControls.jsx    Start All / Stop All / Restart All / ...
  components/BulkSummary.jsx     per-service results
src-tauri/src/
  commands.rs                    SERVICES registry, 12 Tauri commands
  systemd.rs                     systemctl adapter, error classification
  models.rs                      types serialized to React
  lib.rs                         command registration
docs/DEVELOPMENT.md              build, run, test, troubleshoot
```

Twelve commands are exposed: `get_services`, `get_service_status`, five
single-service actions, and five bulk actions.

Errors reach React as structured values, never strings, with a `code` of
`unknown_service`, `unit_not_found`, `permission_denied`,
`authorization_cancelled`, `systemd_unavailable`, or `command_failed`.

## Development

```bash
npm install
npm run tauri dev
```

Checks:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml      # 34 tests
npm run build
npm run tauri -- build --debug --no-bundle
```

The test suite is fully mocked and never touches a real service. A separate
read-only test inspects the real systemd on your machine without prompting or
mutating anything:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_ --nocapture
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for the full guide.

## Roadmap

- Recent journal output per service
- Redis and Nginx in the default registry
- A dedicated PolicyKit action so the prompt can be scoped to BatRing itself
- Tray icon with at-a-glance status

## License

MIT. See [LICENSE](LICENSE).
