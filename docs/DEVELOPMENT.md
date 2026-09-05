# BatRing development guide

Every command below is run from the repository root
(`/mnt/storage/Develop/Projects/BatRing/BatRing`) and was verified on this
machine.

## Prerequisites

| Tool | Verified version here |
| --- | --- |
| Node.js | 24.17.0 |
| npm | 11.13.0 |
| Rust / cargo | 1.97.0 |
| Tauri CLI | 2.11.4 |
| GTK 3 | 3.24.52 |
| WebKitGTK 4.1 | 2.52.6 |

System packages for the Tauri Linux build:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
                 build-essential curl wget file libssl-dev libayatana-appindicator3-dev
```

Runtime requirements: systemd, PolicyKit, a graphical PolicyKit authentication
agent, and a user account in the `sudo` or `admin` group.

## First-time setup

```bash
npm install
```

## Development

```bash
npm run tauri dev
```

This starts Vite on port 1420, compiles the Rust side in debug mode, and opens
the desktop window. React edits hot-reload instantly. Rust edits trigger a
recompile and restart the window automatically. First run takes a few minutes;
later runs start in seconds.

Stop it with Ctrl+C.

### Frontend-only mode

```bash
npm run dev          # http://localhost:1420
```

Useful for pure layout work, but every `invoke(...)` call fails because there
is no Tauri runtime in a plain browser, so no service data appears. Use
`npm run tauri dev` for anything involving real services.

## Production build

```bash
npm run tauri build
```

Builds the optimized binary and both installer bundles. Takes roughly three
minutes with a warm cargo cache and about ten from scratch.

| Output | Size | Path (under `src-tauri/target/release/`) |
| --- | --- | --- |
| Bare binary | 12 MB | `batring` |
| Debian package | 2.8 MB | `bundle/deb/BatRing_0.1.0_amd64.deb` |
| AppImage | 83 MB | `bundle/appimage/BatRing_0.1.0_amd64.AppImage` |

The `.deb` depends on `libwebkit2gtk-4.1-0` and `libgtk-3-0` and installs
`/usr/bin/batring`, a desktop entry, and an icon. The AppImage bundles its own
libraries and runs on any glibc-compatible distribution.

Bundle targets are configured under `bundle.targets` in
`src-tauri/tauri.conf.json`.

### Running the production build

```bash
# straight from the build tree, nothing installed
./src-tauri/target/release/batring

# or install the package system-wide
sudo apt install ./src-tauri/target/release/bundle/deb/BatRing_0.1.0_amd64.deb
batring

# or run the portable AppImage
chmod +x ./src-tauri/target/release/bundle/appimage/BatRing_0.1.0_amd64.AppImage
./src-tauri/target/release/bundle/appimage/BatRing_0.1.0_amd64.AppImage
```

Never launch BatRing with `sudo`. It is designed to run as your normal user and
to let systemd and PolicyKit decide each privileged operation. Running it as
root bypasses that model and skips the authentication prompt entirely.

Uninstall the package with `sudo apt remove bat-ring` (the Debian package name
is `bat-ring`, not `batring`).

### Faster build variants

```bash
# compile and link, skip the installer bundles
npm run tauri -- build --debug --no-bundle

# frontend bundle only, into dist/
npm run build

# preview the built frontend in a browser (http://localhost:4173)
npm run preview
```

## Tests, formatting, and checks

```bash
# Rust unit tests (34 tests)
cargo test --manifest-path src-tauri/Cargo.toml

# same tests without the Tauri desktop feature, so no GTK or WebKit needed
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features

# formatting
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# lints (clippy is not installed on this machine yet:
#   rustup component add clippy)
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
```

The suite is fully mocked and never touches a real service. One test is marked
`#[ignore]` because it queries the real systemd on this machine. It is
read-only, prompts for nothing, and mutates nothing:

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored live_ --nocapture
```

It prints the live registry, for example:

```text
postgresql   postgresql.service   status=Running startup=Enabled
docker       docker.service       status=Running startup=Enabled
mongodb      mongod.service       status=Failed  startup=Enabled
```

Full pre-commit sweep:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --check \
  && cargo test --manifest-path src-tauri/Cargo.toml \
  && npm run build \
  && npm run tauri -- build --debug --no-bundle
```

## Project layout

```text
src/                        React frontend
  App.jsx                   state, command invocation
  components/ServiceCard    one card, reused for every service
  components/BulkControls   Start All / Stop All / ... buttons
  components/BulkSummary    per-service results panel
src-tauri/src/
  commands.rs               SERVICES registry + Tauri commands
  systemd.rs                systemctl adapter, error classification
  models.rs                 serialized types shared with React
  lib.rs                    command registration
src-tauri/tauri.conf.json   window, CSP, bundle configuration
```

## Troubleshooting

**Nothing happens when clicking Start or Enable.** No PolicyKit agent is
running. BatRing reports "No PolicyKit authentication agent answered". Check
with `ps -e | grep polkit`. Most desktop environments start one automatically;
on a bare window manager, launch
`/usr/lib/policykit-1-gnome/polkit-gnome-authentication-agent-1` or the KDE
equivalent.

**It asks for a password on every single click.** That is the old `pkexec`
behavior and should no longer happen. BatRing calls `systemctl` directly so
PolicyKit's `auth_admin_keep` retention applies, meaning one prompt then a
quiet window of a few minutes. If it re-prompts every time, something
reintroduced `pkexec` into `src-tauri/src/systemd.rs`.

**Port 1420 already in use.** Vite is configured with `strictPort`, so it
fails rather than picking another port. Find the process with
`ss -ltnp | grep 1420`.

**Build fails on a missing `.pc` file.** A `-dev` package is absent. Install
the system packages listed under Prerequisites.

**A service shows "Not installed".** Its unit does not exist on this machine.
The card hides its action buttons on purpose. Confirm with
`systemctl status <unit>`.

**`src-tauri/target` is large.** It reaches several gigabytes. Reclaim space
with `cargo clean --manifest-path src-tauri/Cargo.toml`, at the cost of a full
rebuild next time.
