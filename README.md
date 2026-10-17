# Enlumi

**Enlumi** comes from Esperanto, meaning **to enlighten**.

A native desktop workbench for time-series logs, including ULog (`.ulg`) and
Betaflight Blackbox (`.bbl`, `.bfl`, `.cfl`, `.txt`, `.log`). Browse fields, drag them
onto docked plots, and build named filter chains with built-in operations or custom Rust functions.

Enlumi is an early-stage project inspired by
[PlotJuggler](https://github.com/PlotJuggler/PlotJuggler). It uses
[Zgui](https://github.com/zortax/zgui),
[zgui-plot](https://github.com/Quick-Flash/zgui-plot), and log decoders
[px4-ulog](https://crates.io/crates/px4-ulog) and
[bbl_parser](https://crates.io/crates/bbl_parser). Zgui dependencies are pinned.

Plotting code is fetched from a pinned public Git revision of `zgui-plot`.
Zgui temporarily pins a public text-selection fork; see [Contributing](CONTRIBUTING.md#zgui-fork-development).

## Features

- Searchable field tree with nested arrays and numeric instance ordering.
- Searchable recorded ULog parameters/metadata and Blackbox headers; numeric
  parameters and numeric array elements can be dragged into plots/filters as
  constant fields, with selectable full values and direct action buttons.
- Editable JSON [profiles](docs/PROFILES.md) for names/units, built-in and Rust
  filter graphs, named geometry/state bands, and reusable plot views. Add views
  through sidebar folder tree. Drag views onto plots; missing fields leave usable
  parts intact. Folder picker remembers library location. Auto toggles select
  profiles by recorded firmware/settings.
- Profile field groups and per-field labels name axes/components without changing source IDs.
- Double-click/F2 tab rename, with Enter to save and Escape to cancel.
- Per-plot legend collapse/expand preserves plotted lines and geometry.
- Ctrl/Shift selection and drag/drop of field groups onto plots or filters.
- Split, tab, duplicate, resize, and equalize docks; dock beside groups of panels.
- Independent links for pointers and X/Y bounds.
- Movable legend entries and hover previews of hidden lines.
- Contrasting automatic colors across panels; per-line colors, widths, markers, and step connections.
- Fixed bounds on legend toggles; independent X/Y fit from axis double-click or right-click menu.
- Named low/high-pass (optional zero phase), moving average, derivative, integral,
  and gain/offset fields with shared batch settings.
- Multi-input magnitude and quaternion-to-RPY transforms, with named outputs
  that can feed later filter stages.
- Custom Rust filters compiled to WebAssembly, with loops, state, multiple inputs,
  and named outputs. See [Rust filter setup and examples](docs/RUST_FILTERS.md).
- Bounded column rendering; filters and cursor lookup use full samples.

## Build and run

Install [Rust through rustup](https://rustup.rs/), Git, and native build tools.
`rust-toolchain.toml` selects **nightly-2026-04-16**, matching the pinned Zgui
revision. Stable Rust is not supported by that framework revision. First build
downloads the toolchain and dependencies.

**Windows:** Visual Studio Build Tools with **Desktop development with C++** and
a Windows SDK. Use the MSVC Rust target.

**Linux (Debian/Ubuntu):**

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libx11-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libwayland-dev libxcb1-dev libxcursor-dev libxi-dev \
  libxrandr-dev libfontconfig1-dev fonts-dejavu-core
```

Run from repository root:

```sh
cargo run --locked --release -p enlumi
cargo run --locked --release -p enlumi -- /path/to/flight.ulg
```

Or use the workspace helper:

```sh
cargo xtask run-gui-release
cargo xtask run-gui-release -- /path/to/flight.ulg
```

Windows is the currently verified desktop platform. Linux has a desktop backend
and CI configuration; interactive Linux and macOS behavior still needs verification.
The GUI requires a desktop session and a graphics adapter supported by wgpu.

## First plot

1. Click **Open log**, then expand a topic in the left field browser.
2. Drag a field into the plot. Ctrl-click toggles selection; Shift-click selects a
   visible range. Drag any selected field to add the group.
3. Use **Duplicate** or tab **+** to add panels. Drag tabs onto docking guides to
   arrange them. **Links** controls pointer and bounds sharing.
4. Click a legend entry to hide its line; hover to preview. The legend gear opens
   line appearance settings.
5. Click **Filters** beside **Collapse all**. Drag source fields into the selector,
   name outputs, choose settings, and create the batch. **Add next filter** chains
   outputs; **Plot outputs** returns to plots.

See [the usage guide](docs/USAGE.md) for docking, keyboard controls, filtering
details, and rendering behavior.

## Log formats

- **ULog (`.ulg`):** numeric and boolean fields, nested arrays, topic instances,
  and recovered samples from incomplete logs.
- **Betaflight Blackbox (`.bbl`, `.bfl`, `.cfl`, `.txt`, `.log`):** main I/P fields and slow fields merged
  by `bbl_parser`. Each embedded flight appears under `blackbox.N` when a file
  contains multiple flights. Flights start at zero independently; duration is
  the longest flight. Values retain raw log units, without engineering-unit
  conversion until a profile/filter supplies it. Recorded firmware metadata is
  available under Parameters. GPS/home records and event annotations are not
  exposed. Missing values produce gaps rather than zeroes.

Blackbox decoding currently stops at the parser's safety limit of more than
1,000,000 processed frames or 10,000 failed frames per flight. Enlumi reports a
load warning when that limit is reached. Real-flight compatibility depends on
the firmware and encodings supported by `bbl_parser` 1.1.1.

`bbl_parser` is AGPL-3.0-or-later; see [third-party notices](THIRD_PARTY_NOTICES.md)
for distribution implications.

## Current limits

One log file is open at a time. Plots and filter definitions live in memory;
loading another file resets them. Profiles recreate declared setups; full session
saving, exports, file-explorer drops,
and comparison across files are not implemented. All plotted lines in a panel
share its Y axis.

Incomplete logs display recovered samples with a warning. Invalid timestamps and
mismatched field lengths can cause topics or fields to be skipped. Numeric and
boolean fields are plottable; character and padding fields are omitted. Large logs
retain decoded columns and selected-series buffers in memory.

## Development

```sh
cargo xtask pre-commit
```

This runs formatting, workspace checks, Clippy, and tests. Headless tests cover
real layout and pointer interactions, docking, legends, linked plots, and filters.
`cargo xtask` also offers an interactive command menu.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and pull requests,
[architecture](docs/ARCHITECTURE.md) for the code map, and
[SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

Enlumi source is dual-licensed under [MIT](LICENSE-MIT) or
[BSD-3-Clause](LICENSE-BSD-3-Clause), at your option (`MIT OR BSD-3-Clause`).
Dependencies and the bundled test fixture retain their own licenses; see
[third-party notices](THIRD_PARTY_NOTICES.md).


Reusable plot geometry: draw translucent rectangles or lines with above/below
shading, edit bounds directly on plots, and share named shapes across panels.
See [geometry controls](docs/USAGE.md#geometry-bounds).

State bands map discrete raw or derived fields to named, colored time sections
behind signals using editable, [shareable JSON mappings](examples/state-mappings/README.md).
Firmware mappings auto-apply only when explicitly enabled in profile files. Manual time bands provide reusable labeled highlights alongside
automatic layers. [State band controls](docs/USAGE.md#state-bands-and-manual-time-highlights).
