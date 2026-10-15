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
