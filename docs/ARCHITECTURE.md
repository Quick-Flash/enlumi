# Architecture

Enlumi loads one ULog at a time. UI-owned Zgui signals stay on the UI thread;
decoding, filtering, and series preparation run on blocking workers.

| Area | Modules | Responsibility |
| --- | --- | --- |
| Application | `app`, `state`, `startup` | Window composition, log lifecycle, durable pane state |
| Log adapter | `log` | Upstream decoder, header origin, numeric columns, load warnings |
| Field browser | `browser`, `field_tree`, `field_row`, `field_selection` | Search, expansion, selection, group drag/drop |
| Workspace | `workspace`, `panel`, `dock_layout` | Compose public zgui-plot docking controls |
| Plotting | `plot_cursor`, `plot_lines`, `plot_samples`, `line_draw` | Linked cross, viewport reduction, lines and steps |
| Legend/style | `legend`, `legend_entry`, `legend_drag`, `plot_editor`, `line_style`, `color_picker` | Visibility, preview, reordering, appearance |
| Filters | `derived`, `derived_state`, `filter_*` | Named dependency graph, batch editor, background recomputation |

Dependency name `plot` aliases upstream `zgui-plot`. Generic plotting and docking
geometry belong upstream; Enlumi owns PX4 identity, derived fields, and panel records.

## Data and lifetime rules

`LogStore` retains typed columns. Plot series keep full f64 samples and shared
renderer buffers. Relative time subtracts integer header timestamp before
conversion to seconds, preserving small intervals around large origins.

Derived fields use stable IDs and cached samples. Validated graphs reject cycles
and duplicate names. Edits invalidate affected descendants; generation counters
reject results from superseded jobs and logs.

Rendering reduces visible samples by columns before step expansion and markers.
Filters and cursor lookup use full samples. Each pane coalesces requests to one
preparation worker and reuses valid cache entries. Hover previews change drawing
visibility and paint order without changing hidden state or viewport.

Pane signals live under the durable workbench owner. Temporary settings and menus
must not own persistent pane or derived-field state.

## Testing

Unit tests cover timestamps, parser recovery, filter math, graph validation,
field ordering, and reduction bounds. Headless tests send keyboard/pointer events
through retained UI and inspect geometry and draw commands. Windows startup smoke
test checks native window and presented GPU frames. Headless tests do not establish
that every desktop and graphics-driver combination works.
