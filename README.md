# Enlumi

Native desktop viewer for time-series logs. Open ULog (`.ulg`), Betaflight
Blackbox (`.bbl`, `.bfl`, `.cfl`, `.txt`, `.log`), or MAVLink telemetry (`.tlog`)
files from Mission Planner or QGroundControl, browse signals, and drag
fields onto docked plots.

## Features

- Split, tab, resize, and link plots; edit line appearance and reorder legends.
- Set each plot's background or apply its color to all open plots. Automatic line
  colors follow Betaflight Blackbox Explorer's palette independently in each plot.
- Click saved geometry to edit it; drag it onto a plot to place it. Hover a state
  legend entry to see its mode colors. Screenshot legends include geometry.
- Search fields and recorded parameters, with Ctrl/Shift selection and group dragging.
  Field search leaves saved plots, views, workspaces, geometry, and profile files visible.
- Built-in filters and custom Rust functions compiled to WebAssembly.
- JSON profiles for field names/units, filter chains, geometry, flight-mode bands,
  and reusable views, with optional firmware matching.
- FFT, PSD, spectrograms, and field-conditioned heatmaps in docked plots.
- SVG/PNG export and clipboard screenshots of individual plots or visible docks together.

## Downloads

[Nightly builds](https://github.com/Quick-Flash/enlumi/releases) support Windows
x86_64, Linux x86_64, and macOS Intel/Apple Silicon. Extract the whole archive;
portable bundles include example profiles and a Rust-to-WASM compiler for custom
filters. No separate Rust installation is needed to use them.

Each push to `master` starts a nightly build. Downloads publish after all platform
checks pass. Builds are unsigned; macOS builds are not notarized.

Linux archives include Ubuntu 22.04+ (`linux-x86_64`) and rolling Arch Linux
(`arch-linux-x86_64`) builds. Choose matching distribution.

On Linux, Wayland sessions use the native Wayland backend automatically, with
Enlumi's title bar and resize handles. Drag a log onto the title bar or anywhere
in the workbench to open it. Double-click the title bar to maximize or restore;
F11 toggles fullscreen and Escape exits it after any active app interaction is
cancelled. Window buttons also support keyboard focus and activation.
Windows, macOS, and X11 retain winit and their existing window decorations.
For backend troubleshooting, set `ZGUI_PLATFORM=wayland` to force native Wayland
or `ZGUI_PLATFORM=winit` to use winit. External file drops on winit's Wayland
backend remain limited by winit's Wayland support.

To receive update notices, enable **Nightly builds** under **Updates**, then restart.
Checks run at startup and retry every minute while a new build is in progress.
Use **Check for updates** to retry manually. Portable installations offer
**Update & restart** when a newer completed build is available. Restart closes
current log and unsaved work.

## Build and run

Install [Rust through rustup](https://rustup.rs/), Git, and native build tools.
Repository selects its required nightly toolchain; keep `Cargo.lock` for
reproducible dependency versions.

Shared plotting library uses upstream Git revision pinned in `Cargo.toml`.

- **Windows:** Visual Studio Build Tools with **Desktop development with C++**
  and a Windows SDK. Use the MSVC Rust target.
- **macOS:** Xcode Command Line Tools.
- **Linux (Debian/Ubuntu):**

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libx11-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libwayland-dev libxcb1-dev libxcursor-dev libxi-dev \
  libxrandr-dev libfontconfig1-dev fonts-dejavu-core
```

- **Linux (Arch and derivatives):**

```sh
sudo pacman -Syu --needed base-devel rustup git libx11 libxkbcommon \
  libxkbcommon-x11 wayland libxcb libxcursor libxi libxrandr fontconfig ttf-dejavu
```

Set `ENLUMI_BUILD_PLATFORM=arch-linux-x86_64` when making Arch portable bundles
so built-in updater keeps selecting Arch archives.

From repository root:

```sh
cargo run --locked --release -p enlumi
# Or open a log directly:
cargo run --locked --release -p enlumi -- /path/to/flight.BBL
```

A desktop session and GPU supported by wgpu are required.
See [portable build instructions](docs/RUST_FILTERS.md#build-portable-distribution)
to package the app with its offline filter compiler.

## First plot

1. Click **Open log** or drop a log file into the window, then expand a field group
   and drag a field onto a plot.
2. Ctrl/Shift-click to select multiple fields, then drag the selection together.
3. Drag plot tabs onto docking guides to arrange panels. Use **Links** to share
   axes or pointers; double-click a tab title to rename it.

## Limits

Enlumi is under active development. One log file is open at a time; plots and
filters are kept in memory. Full session saving and cross-file comparison are
not implemented. Lines within a plot share one Y axis.

Blackbox values retain raw log units until profiles or filters convert them.
GPS/home records and event annotations are not exposed. Incomplete logs and
parser limits produce load warnings.

## Documentation and development

- [Usage and controls](docs/USAGE.md)
- [Profiles and firmware mappings](docs/PROFILES.md)
- [Frequency analysis](docs/FREQUENCY_ANALYSIS.md)
- [Custom Rust filters](docs/RUST_FILTERS.md)
- [Contributing](CONTRIBUTING.md) and [architecture](docs/ARCHITECTURE.md)
- [Security reporting](SECURITY.md)

Run local checks before submitting changes:

```sh
cargo xtask pre-commit
```

Update MAVLink definitions and their matching Rust dependencies with one command:

```sh
cargo xtask update-mavlink
```

You can also select **update-mavlink** from the interactive `cargo xtask` menu.
This selects the latest stable, non-yanked `mavlink` release. To pin a particular
release, use `cargo xtask update-mavlink 0.19.1`. The command verifies the archive
against its crates.io SHA-256 checksum, copies ArduPilotMega's recursive XML
includes and upstream license, records provenance, updates `Cargo.toml` and
`Cargo.lock`, and runs importer tests. A failed update restores the previous files.
Review and commit the resulting diff. Ordinary builds use the checked-in files
without downloading definitions.

On Linux, test native Wayland parsing and window-state bookkeeping with
`cargo test --locked -p zgui-platform-wayland --lib`. From a running Wayland
desktop, also check compositor-confirmed maximize, restore, fullscreen, and close:

```sh
cargo test --locked -p enlumi native_wayland_compositor_configures_maximize_restore_and_fullscreen -- --ignored --nocapture
```

## License

[GNU AGPL version 3 or later](LICENSE). See [copyright](COPYRIGHT) and
[third-party notices](THIRD_PARTY_NOTICES.md) for dependency and fixture licenses.
