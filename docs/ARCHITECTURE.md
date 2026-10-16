# Architecture

Enlumi loads one ULog at a time. UI-owned Zgui signals stay on the UI thread;
decoding, filtering, and series preparation run on blocking workers.

| Area | Modules | Responsibility |
| --- | --- | --- |
| Application | `app`, `state`, `startup` | Window composition, log lifecycle, durable pane state |
| Log adapter | `log` | Upstream decoder, header origin, numeric columns, load warnings |
| Field browser | `browser`, `field_tree`, `field_row`, `field_selection` | Search, expansion, selection, group drag/drop |
| Workspace | `workspace`, `panel` | Compose public docking, links, fitted geometry, and equalization APIs |
| Plot adapter | `plot_cursor`, `plot_bounds` | Compose shared series plot; format PX4 elapsed time |
| Membership/style policy | `legend_drag`, `line_colors` | Validate source identity, transfer fields, allocate colors across application panes |
| Filters | `derived`, `derived_state`, `filter_*` | Named dependency graph, batch editor, background recomputation |

Dependency name `plot` aliases upstream `zgui-plot`. Generic plotting and docking
geometry, axis gestures, fit menus, bounded background rendering, steps/markers,
snapped linked pointers, editable legends, appearance controls, color palette,
and links controls live upstream. Enlumi owns PX4 identity, derived fields,
panel records, field-drop fitting policy, duplicate-source validation, and
application callbacks such as Add filters. `Pane::display()` exposes a durable
plot-facing view without giving plotting code access to ULog state.

## Design tokens and themes

`theme::stylesheet()` supplies shared tokens and app styles. `themes/forest.css`
and other palette files define the same semantic color and corner-radius roles;
`themes/common.css` owns typography, control height, and code syntax colors. Components use
`var(--app-...)` references rather than literal palette colors. Add new palette
roles to every theme; register presets once in `Palette::ALL`. Keep layout and interaction rules out of palette files.

`AppTheme` changes palette declarations on window root, including portalled menus,
without rebuilding UI. Zgui UI tokens reference those same roles through
`theme.css`; `plot_theme.css` adapts upstream plot chrome to app tokens. Series
colors and code syntax colors remain distinct from interface accent. Plot canvas
rendering/grid colors still belong to upstream plotting library; current
presets use dark surfaces. Only theme ID is persisted; logs, scripts, plots, and drafts remain session-only.

## UI action buttons

Use `ActionButton` (`src/action_button.rs`) for application commands, including
navigation, toolbars, filter actions, and remove buttons. It fixes the underlying
Zgui variant and shares idle, hover, pressed, selected, disabled, and focus styling
in `src/action_button.css`. Caller classes identify actions and set geometry;
do not add parent-specific button palettes or choose variants at call sites.
Use `setting-active` for persistent toggle state. Ordinary commands do not latch.

Saved-field/new-field rows remain selection controls with their list appearance.
Plot library controls keep upstream behavior and styling. Headless tests compare
actual rendered app-button colors across locations and interaction states.

## Data and lifetime rules

`LogStore` retains typed columns. Plot series keep full f64 samples and shared
renderer buffers. Relative time subtracts integer header timestamp before
conversion to seconds, preserving small intervals around large origins.

Derived fields use stable IDs and cached samples. Validated graphs reject cycles
and duplicate names. Edits invalidate affected descendants; generation counters
reject results from superseded jobs and logs.

Shared zgui-plot renderer reduces visible samples by columns before step expansion and markers.
Filters and cursor lookup use full samples. Each pane coalesces requests to one
preparation worker, publishes compatible results during continuous pans, caches
one extra viewport on each side, and reuses valid cache entries. Hover previews change drawing
visibility and paint order without changing hidden state or viewport.

Pane signals live under the durable workbench owner. Temporary settings and menus
must not own persistent pane or derived-field state.

## Testing

Unit tests cover timestamps, parser recovery, filter math, graph validation,
field ordering, and reduction bounds. Headless tests send keyboard/pointer events
through retained UI and inspect geometry and draw commands. Windows startup smoke
test checks native window and presented GPU frames. Headless tests do not establish
that every desktop and graphics-driver combination works.


Plot geometry uses session-only named definitions (`geometry`, `geometry_state`),
with ID membership and hidden-ID sets per pane. `geometry_library` and `geometry_inspector`
manage shared definitions; `geometry_legend` supplies caller-owned rows through
upstream `SeriesPlot` legend child slot, and `geometry_overlay` projects clipped data-space paths
through public plotting geometry and draws a transparent canvas. Editing exposes small retained handles (`geometry_handle`) over pass-through
canvas; background pan/zoom remains active. Persistent overlay owns gesture
capture, including movable editor (`geometry_editor_drag`), so canceling edits
can unmount controls safely. `geometry_interaction` handles data-space changes
and horizontal/vertical constraints. Geometry never contributes to signal fit bounds.


`state_bands` compresses discrete sample-and-hold values into source-owned
intervals; mappings remain separate from interval timing. `band_state` runs
preparation off UI thread, rejects stale generations/jobs, refreshes derived
sources, and preserves mappings. `band_background` clips visible intervals,
batches fills by color within each layer, and caps labels. Public plot
`background_layer` slot forwards through series/time/data shells and renders
before grid and signals. Generated layers use ordinary geometry membership and
legend controls; manual time bands use same backdrop renderer with editable X
bounds. `geometry_cursor` shares hover/active cursor policy with capture owner.

`band_mapping_file` validates versioned JSON mappings and performs file I/O off
UI thread. Runtime contains no PX4 enum lookup or source-name detection.
Mappings retain unobserved values across refresh; optional firmware examples
live under `examples/state-mappings` and load only through explicit import.
