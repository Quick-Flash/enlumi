# Plot

Built for **Enlumi**, whose name comes from Esperanto, meaning **to enlighten**.

`zgui-plot` provides backend-neutral 2D plotting primitives, retained Zgui chart components, SVG/PNG export, and optional docking workspace controls.

The crate lives in the [Quick-Flash/zgui-plot repository](https://github.com/Quick-Flash/zgui-plot). Zgui temporarily pins a public commit from [Quick-Flash/zgui, branch `text-multiclick-selection`](https://github.com/Quick-Flash/zgui/tree/text-multiclick-selection), pending upstream merge. Consumers must use the same Zgui revision recorded in `Cargo.toml`. Zgui currently documents Git installation and is not published on crates.io, so this dependency blocks publishing `zgui-plot` to crates.io.

## Add the crate

While developing in a workspace, use this path from workspace root:

```toml
zgui-plot = { path = "../zgui-plot", features = ["workspace", "raster"] }
```

For consumers outside this workspace, use the public Git source:

```toml
zgui-plot = { git = "https://github.com/Quick-Flash/zgui-plot", features = ["workspace", "raster"] }
```

The `zgui-plot` package imports as `zgui_plot`. Use `default-features = false` to keep Zgui, docking controls, and raster export out of backend-neutral applications.

## Editable series plots

With `workspace`, `zgui_series::SeriesPlot` adds compact editable legends,
per-plot legend collapse/expand (heading and line settings stay accessible),
Ctrl/Shift multi-selection, fixed-layout appearance settings, disabled color
controls for multiple selections, hidden-line hover previews, legend reorder,
cross-pane drag feedback, and snapped linked pointers/readouts.

`SeriesPane::new(id, lines)` owns durable presentation over
`Signal<Vec<SeriesLine>>`; line IDs are stable numeric IDs, while source metadata
stays in consumer. `LegendHost` supplies visible panes, hit testing, and acceptance
callbacks. `SeriesActions` supplies select/remove/move callbacks and optional extra
action label/help/callback; Enlumi uses this hook for **Add filters**. Keep this
state under a durable owner, outside transient dialogs. The controls install their
own stylesheet; applications still install surface/time-series sheets and a
zgui-ui `ThemeProvider`.

`PlotTabs` accepts optional `on_rename: UnsyncCallback<(u64, String)>`. When
provided, double-clicking a tab title or pressing F2 edits it in place. Enter or
focus loss commits; Escape cancels. Empty title lets consumer restore default.

`TimeSeriesPlot::line_drawing` maps ordinary string series IDs to
`line_draw::LineDraw { connection, markers }`. `Connection` supports `Linear`,
`Pre`, and `Post`. Standard time plots now use bounded background preparation,
including column reduction before step expansion/markers, one worker per plot,
compatible completion publishing during continuous pans, and a viewport of cached
data on either side. Full samples remain available for hover and filtering.
Snapshots use the same prepared canvas, preserving interpolation and markers.
`show_legend`, `show_pointer`, and `show_cursor_values` provide explicit composition
controls; consumers no longer need CSS selectors against internal child order.

Backend-neutral helpers are `line_draw::prepare`, `series_samples::{viewport,
pan_viewport, Accuracy}`, and `palette::{DEFAULT_PALETTE, next_color}`. Palette
allocation takes destination and global colors, so each application chooses its
reuse scope. `SeriesPane::copy_appearance` preserves styles/visibility and resolves
destination color clashes before membership transfer; consumer performs transfer
and source cleanup explicitly.

Workspace helpers include `WorkspaceGeometry::fitted` for viewport-sized docks,
`Layout::equalize` for balanced rows/columns, and `PlotLinksMenu` with independent
checkboxes plus All. Group docking and pane-local guides remain available.

Line settings support solid, dashed, dotted, dash-dot, and markers-only rendering. Patterns and sample markers can be edited for multiple selected lines; choosing line type None enables markers. Pattern preparation runs on background workers with bounded geometry and exports through the same retained scene.

`line_pattern::prepare_path` is the shared, backend-neutral pattern engine for
sampled paths in any coordinate order. It preserves loops, vertical segments and
missing-data gaps while sharing clipping, dash phase, spacing and geometry
limits with time plots. `line_pattern::prepare` keeps the existing `SeriesData`
API as a thin adapter. Consumers choose their own sampling policy before calling
the engine: time plots can reduce sorted columns, while XY paths can use
screen-distance reduction without reordering their samples.

## Features

| Feature | Provides |
| --- | --- |
| Default / none | Geometry, ranges, interaction controller, histogram, docking tree and assignment model |
| `zgui` | Retained series renderer, interactive surfaces, legends, linked bounds/pointer, SVG snapshots |
| `raster` | PNG rendering from SVG |
| `clipboard` | Native image clipboard (includes `raster`), with X11/Wayland support |
| `workspace` | `zgui` plus `zgui-ui` docking tabs, pane host, splitters, settings and signal drag/drop |

For native UI consumers, disable default features and enable `workspace` and optional `raster` on the dependency entry. Use the public Git source above for external consumers. Zgui must resolve to same revision as `zgui-plot`.

Use same Zgui revision as this crate. Install `zgui_workspace::stylesheet()` and `zgui_surface::stylesheet()` alongside application styles, inside a `zgui-ui` theme.

## Workspace integration

Application owns data and pane records. Library owns docking geometry and interaction:

1. Create `RwSignal<workspace::layout::Layout>` and optional maximized pane ID.
2. Create `zgui_workspace::DockPane::new(id)` for each stable, unique pane ID. Create pane signals under durable workspace owner, not temporary menu/tab owners.
3. Expose pane records through `Signal<Vec<DockPane>>`; construct `DockState::new(layout, panes)` and `WorkspaceGeometry::new(layout, maximized)`.
4. Render `WorkspaceCanvas`, keyed `WorkspacePane` children, and `PlotTabs` inside each pane's `plot-strip` header. `WorkspaceCanvas` supplies splitters; `WorkspacePane` supplies selection, placement, visibility, and docking overlays.
5. Supply reactive tab titles and add/select/close callbacks. Callbacks update application records and shared layout; renderer/data stay independent of docking.

`Layout` supports splits, close, swap, tab activation, tab grouping/reordering, edge docking, and resize. Divider IDs remain unique when moving plots across surviving splits. Hidden tabs retain caller-owned configuration; only active tabs appear.

`PlotMenu` renders kind, duplicate, split, reorder, maximize/restore, and axis swap controls. `PlotCommands` supplies application factories and selection hooks; pane data remains caller-owned.

`SeriesSource` accepts reactive ID, label, count, selected state, shared `SeriesDrag`, and callbacks. Pointer capture, drag threshold, Escape/cancel, and release live in library. Application resolves drop target and applies assignment. `workspace::drop_axis` and `PlotConfig::assign` preserve existing single-axis assignment behavior. Future ULog tool can use its own multiple-series configuration with same drag source.

`PlotSettings` registers children in caller-provided local settings host (`RwSignal<Option<Rc<ChildrenFn>>, LocalStorage>`). `PlotSettingsButton` renders registered settings plus caller-supplied controls. `LinkControls` edits independent X/Y/pointer options; provide `zgui_link::PlotLinks` as local context around `PlotSurface` to bind links.

## Multiple-series time plots

`time_series::SeriesData` prepares sorted f64 samples once. X must be finite and nondecreasing; nonfinite Y breaks line. Origin-relative f32 renderer buffers preserve small timestamp differences around large epoch values. Original f64 samples remain available for hover and consumer record lookup.

Build `TimeSeries` with stable ID, display label, and prepared data; collect into `TimeSeriesSet::new`. Duplicate/empty IDs return errors before keyed UI rows mount. Repeated labels are allowed. Each series may have its own sample rate and gaps; no resampling required.

Render `zgui_time_series::TimeSeriesPlot` with `Signal<TimeSeriesSet>` and caller-owned `TimeSeriesState`. Install `zgui_time_series::stylesheet()` plus surface stylesheet. Component supplies multiple retained lines, stable ID colors, legend toggles, outlined inside-axis numbers, nearest-sample hover, pan/zoom/box zoom, reset, optional axis formatting, linked interactions, and optional snapshot registry. All visible lines share one Y axis; use matching units.

`TimeSeriesState` defaults to per-column LOD and auto fit. Pan/zoom/manual bounds suspend auto fit; `fit_to` resumes it. `set_view` validates bounds. `duplicate` copies view options into independent signals. Pan/zoom updates canvas transform without rebuilding prepared vertices. Consumer can switch `state.lod` to `Lod::Exact`.

`zgui_workspace::TimeSeriesSettings` supplies auto-fit, LOD, fit, and numeric bounds controls. `state.hover` reports series ID, original sample index, and f64 coordinates; this is hover information, not linked record selection.

Standalone two-panel consumer demo, including different sample rates, gaps, repeated labels, epoch timestamps, shared X/pointer, and independent Y:

```powershell
cargo run --release -p zgui-plot --example time_series --features workspace
```

## Scatter, histograms, styles, and annotations

`zgui_plot::ScatterPlot` and `HistogramPlot` share `DataPlot` with time plots. `PlotState` owns ranges, visibility, LOD, hover, emphasis, styles, annotations, and histogram options. `TimeSeriesState` remains compatibility alias. All kinds share pan/zoom/reset, linked interactions, tick formatting, and snapshot registration.

Prepare `ScatterSeries` from arbitrary-order XY samples, then validate IDs with `ScatterSet::new`. Buffers retain origin-relative coordinates; prepared X index narrows hover searches while preserving original sample indices. Invalid XY samples are omitted from rendering/hover. Prepare `HistogramSeries` from numeric values, then build `HistogramSet`. Nonfinite values are filtered; sorting and moments happen during preparation. Visible groups use common bins and stacked bars. CDF/CCDF use per-column binary searches over prepared groups; normal fit uses cached moments.

`SeriesStyle` controls RGBA color, width, marker shape, and radius. `PlotState.styles` maps stable IDs to overrides; missing IDs use default palette. `SeriesControls` supplies visibility and style widgets, with live legend colors and hover emphasis. `DataPlotSettings` combines shared numeric bounds, annotations toggle, series controls, and histogram bin/CDF/CCDF/normal-fit settings.

`PlotAnnotation` supplies X/Y thresholds, bands, rectangular regions, and fit lines. Consumer supplies values, labels, colors, widths, and dash choice. Paths and painting live in `zgui_plot::annotations`; screen and SVG use same layers. Annotation eligibility and meaning belong to consumer.

`zgui_plot::theme` owns shared tick/legend CSS and export descriptions. `view_state::ViewState<T>` supplies persistent per-kind viewport/settings/source identity for custom adapters. Consumers can combine these APIs with shared scatter submission and histogram backdrop layers while keeping domain-specific grouping, dates, thresholds, diagnostics, and specialized UI in application code.

Standalone consumer demo:

```powershell
cargo run --release -p zgui-plot --example data_plots --features workspace,raster
```

Settings expose shared controls. Export button writes `plot-exports/data-plots.svg` and, with `raster`, PNG. PNG/file work runs on blocking worker.

## Workspace export

Register each rendered plot's snapshot with `zgui_svg::register_snapshot`. Registry cleanup respects replacement callbacks. Pass pane IDs, fallback node bounds, and snapshot sources as `WorkspaceSnapshot` entries to `capture_workspace` with layout, maximized ID, and background color.

Capture uses measured positions, active tabs only, and current ranges. Empty panes receive placeholders. Maximized capture includes maximized pane only. SVG composition isolates gradient IDs; `raster` renders resulting SVG to PNG. Application chooses paths and background work policy.

## Application responsibilities and remaining work

- ULog ingestion, topic/instance/field discovery, timestamp conversion, and signal metadata belong in consumer.
- Domain-specific filters, dates, groups, thresholds, and correlation actions remain in the consuming application.
- Generic time/scatter/histogram plots, state, styling, annotations, settings, and exports live in this library. Consumers retain application-specific data and UI decisions.
- Linked record selection remains TODO. Current links share bounds and data coordinates, not sample/flight identity.
- Workspace serialization/persistence remains TODO.

Existing renderer demo:

```powershell
cargo run --release -p zgui-plot --example waves --features zgui
```

## License

Licensed under either MIT or BSD 3-Clause, at your option. See `LICENSE-MIT` and `LICENSE-BSD-3-Clause`.

## Axis fit and pointer controls

Time, scatter, and histogram plots share full-height Y grab band inside the
left edge. Upper half adjusts Y maximum; lower half adjusts Y minimum.
Bottom X band adjusts the nearer X bound. Wheel over an axis zooms only that axis
around the value under the pointer; the other axis stays unchanged.
Shift still selects a zoom box;
Escape cancels a drag. Axis double-click fits only that axis; data-area
double-click fits both. Manual fits are one-shot and disable continuous auto-fit.
`PlotState::fit_to` and `fit_data` still explicitly enable continuous auto-fit.

With `workspace`, these plots include a right-click menu for **Fit X axis**,
**Fit Y axis**, and **Fit both axes**. The menu uses the active zgui-ui theme.
Pure `zgui` builds retain dragging and axis double-click without pulling in
zgui-ui. Custom surfaces can compose `zgui_workspace::PlotFitMenu` directly.

`PlotState::fit_axes(&PlotData, Axes)` and `fit_axis(&PlotData, Axis)` preserve
unselected axes and ignore hidden series; empty or entirely hidden data leaves
the view unchanged. `SurfaceOptions::on_fit_axis` exposes axis-aware double-click
to custom consumers; omitting it retains full-reset fallback. `HitZones` exposes
`y_axis_width`; `y_min_handle = false` uses the whole bottom band for X bounds.
Right-clicking a legend entry does not toggle it.

## Screenshots

Enable `workspace` and `raster` for shared SVG/PNG screenshot controls.
Set `options.snapshot` to a `zgui_svg::PlotSnapshotSource` on `SeriesPlot`,
`TimeSeriesPlot`, or `DataPlot`. `zgui_workspace::ScreenshotMenu` emits a `ScreenshotFormat`; host
chooses destination and runs `format.save(&svg, &path)` on a worker thread.
`capture_plot(source, true)` captures current view with overlays.
`capture_workspace_with_overlay` composes visible docks at current positions;
maximized and hidden tabs follow the supplied layout. PNG uses the same SVG
geometry at twice its width and height.

Enable `clipboard` and keep a `clipboard::ScreenshotClipboard` in application state.
Pass `on_copy` to `ScreenshotMenu` to show **Copy image**, then capture SVG on the UI
thread and call `clipboard.copy(&svg)` on a blocking worker. Retain clipboard handle
until application closes so Linux can serve paste requests. Clipboard stays lazy;
opening camera menu performs no rendering, font loading, or clipboard access.

Custom plot components call `SnapshotLayers::current_or_new()` before building
children. Register retained `CanvasHandle`s with `register_canvas` as
`Background`, `Foreground`, or `Overlay`. `register_description` adds vector
titles, labels, measurement boxes, and other chrome. Registrations expire with
component ownership, so remounting or closing docks removes stale layers.

## Component options

`DataPlot` and `TimeSeriesPlot` share `zgui_plot::PlotOptions`; `SeriesPlot`
uses `zgui_series::SeriesPlotOptions`. Pass optional formatting, visibility,
snapshots, and line drawing through `options`, using `..Default::default()`
for remaining controls. `SeriesPlotOptions` also owns background/overlay
`ChildrenFn` layers. Typed plot `background_layer` remains a named child prop.

Custom `PlotSurface` consumers pass required geometry/range callbacks through
`SurfaceInteraction`, and optional display/gesture settings through
`SurfaceOptions`. `LegendItem` groups marker and colors in `LegendAppearance`.
This replaces oversized argument lists without Clippy suppressions.

`PlotOptions::lock_selection_aspect` constrains Shift-zoom rectangles to the data
viewport's width/height ratio, so equal-unit views zoom to the rectangle shown.
It defaults to false. Lower-level consumers can set `Controller::selection_aspect`
or call `PlotPointer::set_selection_aspect` to choose a pixel aspect ratio.
