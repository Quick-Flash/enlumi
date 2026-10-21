# Log profiles

Profiles are versioned JSON files describing signal names/units, existing filter
operations, named geometry and state bands, and reusable plot views. Firmware
field names, aliases, state names and conversion settings live in these files.
No Betaflight alias table or motor pole-count default is built into Enlumi.

## Use

1. Open a log. Expand **Parameters** in field tree for every recorded entry.
   Numeric parameters drag into plots and filters; numeric arrays expand into elements.
   Select a parameter for full, selectable value and direct plot/filter buttons.
   Sidebar contains independently collapsible **Parameters**, **Raw**, **Derived**, and
   **Profiles** sections. Raw starts collapsed when opening a log. Expand Profiles
   alongside other fields to mix their selections without switching modes.
2. Expand **Views** or **Plots** directly under Profiles. Views contain plots;
   plots expand to show individual fields. Drag a view, plot, or field to use it.
   Resources prepare on demand; **Import file…** adds profiles outside the library.
   **Profile files** contains the file/folder tree and file edit/delete actions.
3. **Fields** and **Geometry** contain reusable resources from all profiles.
   Each preset's **…** menu offers actions and identifies its source file.
   Shared sidebar search filters raw,
   derived, and profile results without opening folders or sections. Expand or
   collapse matches manually. Click highlights output; Ctrl-click toggles selection,
   and Shift-click selects range across expanded sections/profiles. Raw, derived,
   and profile fields can be selected and dragged together into plots, Filters,
   or Analyze. Missing profiles prepare before the complete selection is placed.
   Create empty plot with plot-tab **+** first when needed.
   Expand a view preset to see each plot with its fields grouped underneath.
   Drag the whole view to open it as a new workspace view, or drag one nested
   plot to create a separate plot with its saved settings. Individual fields remain draggable.
   Field **⋯ → Use in filter** also accepts individual outputs; fields drag into
   filter inputs or state-band source area. Imports outside library appear under
   **Imported**. Existing plot contents remain; duplicate sources are skipped.
   View geometry attaches too. Search, expansion and scroll survive collapsing
   and reopening Profiles.

Named state bands also appear in the geometry library. Their name and each
state's name/color can be edited with existing geometry controls. Labels render
inside the time bands. Derived band sources refresh after filter recomputation.

Profiles are additive. Conflicting output names, geometry names, or signal
metadata reject the load. Each canonical profile path and selected flight owns one
prepared setup. **⋯ → Reload** prepares replacement in background and preserves previous
setup if validation or computation fails. Stable IDs update existing lines and
recompute dependent filters/state bands. **⋯ → Remove…** previews owned outputs and
blocks removal when manual filters, state bands, or manually added derived lines
depend on them. Existing plots and unrelated raw lines remain. Missing signals, failed filters and their
dependents, and unavailable geometry are skipped with visible warnings. Views keep
remaining lines/geometry; completely empty views are skipped. Raw `requires`
entries report missing data without blocking unrelated resources. Firmware/settings
conditions remain strict to prevent applying wrong units or version mappings.
Invalid JSON/schema, duplicate IDs/view names, and commit conflicts reject load.
Changing logs cancels in-flight preparation. Filters use existing cancellation,
validation, and Rust/WASM execution limits.

## Workspaces

A workspace is a collection of views. Each view contains its own plots and layout.
Use the **Workspace** dropdown beside **View** to switch collections without
mixing their view lists. Existing workspace sessions retain their selected view
and plot bounds when switching.

Saved workspaces in the profile library appear in the Workspace dropdown
automatically. They can be selected before opening a log; opening a log then
populates their plots. Profiles also exposes **Workspaces → Views → Plots →
Fields**, with drag support for whole workspaces, individual views, and plots.

Workspace **… → Save** captures every view in the current
workspace, including its active view, plots, filters, styles, and layouts.
Save overwrites the linked preset without opening an editor. For an unlinked view
or workspace, Save creates its profile in the profile folder. **Save as...**
opens an editor for a separate copy. **Duplicate**
copies all open views and plots into an independent workspace. Workspace names and view
names can be edited in the shared profile editor. Recipes omit zoom bounds and
analysis time intervals. Inactive views do not render in the background.

Saving preserves deleted derived outputs. Deleted filter recipes are removed from
the saved file only when no other preset or resource references them; saving does
not reactivate deleted outputs retained for other presets.

Saved presets can share inputs with different captured labels. Existing input
labels stay active; label differences do not block loading or dragging plots.

Equivalent filter copies with the same name, operation and resolved inputs share
one output during preparation, including copies that use raw aliases. Saving
recognizes raw aliases and numeric operation defaults to avoid adding those copies
again. Raw decoding and multi-output Rust results are cached for the preparation;
matching Rust functions and inputs run once even across separately saved groups.
Independent filter branches and raw input decoding run on a shared Rayon pool,
capped at four workers (leaving one CPU available when possible). Dependent filters
wait for their inputs. Results and diagnostics retain deterministic ordering;
failed branches do not stop unrelated outputs. Completed filters are validated
once per preparation, rather than again for every subsequent branch.

Workspace recipes use `workspace_presets`: each entry contains `name`, `views`
(view preset objects), and `active` (zero-based initial view index).

## Create and edit profiles

Profile actions include **Delete profile file…**. Confirmation removes the JSON
file from disk; loaded fields and plots remain available in the current session.
Plot and view preset action menus include **Delete plot preset…** and **Delete
view preset…**. These remove only the saved preset, keeping plotted copies and
shared fields. **Unload outputs…** removes loaded profile resources without
deleting the saved file.

- Sidebar **Save** captures selected fields into a new or existing profile.
  Field right-click **Add to profile** includes that field and its dependencies.
- Plot background/header **Save plot as preset** captures one plot. View menu
  **Save view as preset** captures all plots, tabs, splits, and link settings.
- Shared editor asks for name and destination file or loaded profile. Expand
  dependency preview to inspect included fields, labels, filters, geometry, and
  recipes. Edit names, component/unit labels, and filter parameters there; use
  normal filter editor for building/changing filter stages. Arrange layouts with
  normal workspace docking, then save view preset.
- Profile **Edit profile** opens saved contents. **Save** writes changes;
  **Save as / choose file** changes destination and copies Rust dependencies.
  **Reload** explicitly refreshes prepared resources from file.
- **Replace existing entries** must be enabled for conflicting named entries.
  Saving preserves unrelated entries. Writes are atomic and stale editor writes
  reject changed files rather than overwriting external changes.
- Plot **Update source preset** opens same review editor with source destination and
  replacement selected. **Modified** tracks recipe content, excluding viewport
  changes, cursors, measurements, and analysis time selections.

Plot presets offer **Add as new plot** and **Add fields to selected plot**. Second
option opens field checkboxes, skips duplicates, and preserves destination style
and current bounds. Time fields and spectra cannot mix. View presets offer
**Open as new view** and **Add plots**; both remap runtime plot IDs. Double-click
plot preset opens new plot; double-click view preset name opens new view.
Legacy `views` appear as plot presets and remain unchanged until explicitly saved.

Saved recipes contain field references and filter definitions, not samples or
computed spectra. FFT/PSD/heatmap/spectrogram recipes recompute using current
Analyze selection. Window, DC removal, window size, overlap, and conditioning
field are retained; no zoom, axis limits, auto-fit state, or analysis interval is
serialized. New plots fit normally. Existing plots keep bounds when fields are
inserted. Axis links are explicit relationships, not saved numeric ranges.

Rust source dependencies live beside JSON under content-addressed `sources/*.rs`
paths. Save as copies dependencies, so move JSON and its `sources` directory
together. Shared inputs are deduplicated when adding presets to one profile.
Identical live filters/geometry can be reused when opening version 2 recipes in
originating workspace; removing profile preserves borrowed resources. Removing
resources still referenced by another profile is blocked.

Field **Edit labels** offers explicit **Current log** or owning profile scope.
Inline rename affects current log. Saving profile labels clears local override
for that field; **Profile default** restores inherited metadata and **Raw label**
suppresses it. Labels never change source identity or plotted samples.

## Profile directory

Expand **Profiles**, then click **Folder…** to choose profile library directory. Choice persists across
launches and controls both browsing and automatic firmware matching. Nested
folders are supported. **Load file…** imports one profile without changing library.
Profile sections and **Details** expand on demand. Info icon beside profile search shows full directory path, recorded firmware, and library help. Superseded directory scans cannot replace newer results.

Without a selected folder, library defaults to:

| Platform | Directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/enlumi/profiles`, otherwise `~/.config/enlumi/profiles` |
| Windows | `%APPDATA%\Enlumi\profiles` |
| macOS | `~/Library/Application Support/Enlumi/profiles` |

Create the directory and copy JSON files there. Rust source files referenced by a
profile travel beside it or in a subdirectory. Sidebar shows selectable directory path.
JSON files in nested directories form expandable tree. Symlinks are not followed;
scan allows up to 32 directory levels and 4096 files. **Load file…** works from any
directory. No directory watcher runs. Example organization:

```text
profiles/
  betaflight/
    2026.6/
      pid.json
      rate_error.rs
      motor-rpm.json
  px4/
    modes.json
```

## Firmware detection and defaults

Open **⋯** beside profile, then **Auto** to enable automatic firmware selection. **Auto ✓**
means enabled; click again to disable. Choices save in application configuration
as `profile-auto.json`, keyed by canonical recipe path. Recipe stays untouched,
including in read-only libraries. Auto scans chosen library; imports outside it
must move into library before enabling Auto. Explicit local true/false overrides recipe's
`auto_apply` default; absent local choice uses recipe default (false if omitted). Each successful log load scans files in filename order and matches
`requires_settings` against recorded metadata, including firmware revision/version.
At least one setting condition is required for auto-apply. Unmatched firmware
profiles are skipped. Matching profiles apply serially and open Profiles sidebar
with available views ready to drag. Missing fields do not block them. Broken files
or resources report warnings and do not stop unrelated defaults. No panels open
automatically; drag a view or choose its panel action.

Use narrow version prefixes, e.g. `Betaflight 2026.6.`, rather than guessing across
firmware releases. Choose **Flight** before applying profiles to multi-flight files.
Flight-1 Blackbox paths and setting references act as template scope and bind to
selected flight; explicit references to other flights stay explicit. Each flight
gets independent resource IDs. Recorded matching values appear under profile name.
Selecting flight also checks configured Auto profiles for that flight.
Scanning reads all JSON profiles; malformed files report errors. Rust filters in
enabled matching profiles run through existing WASM engine. Only enable profiles
you intend to run. Refresh updates discovery; **Reload** applies edited recipe to existing setup.

Latest-release examples target [Betaflight 2026.6.2](https://github.com/betaflight/betaflight/releases/tag/2026.6.2):
`betaflight-2026.6-pid.json`, `betaflight-2026.6-pid-high-resolution.json`, and
`betaflight-2026.6-motor-rpm.json`. Copy `rate_error.rs` beside PID profiles.
PID examples require matching `blackbox_high_resolution` setting; high-resolution
example scales gyro/setpoint by 0.1 before error calculation. RPM example uses
recorded motor pole count and requires motor 1 eRPM telemetry. Field definitions
and scaling checked against [tagged firmware source](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/blackbox/blackbox.c).
Examples remain disabled for auto-apply until configured. Normal-resolution PID and RPM examples were exercised against user-provided
Betaflight 2026.6.2 Basher log. Yaw D appears only when `axisD[2]` exists. Firmware
can omit it when yaw D is disabled; remaining PID terms still load. No zero-valued
field is fabricated.
High-resolution example requires `blackbox_high_resolution = 1`; normal logs use
`betaflight-2026.6-pid.json`. Older 4.5 and ULog examples intentionally reject
unrelated logs. Profile list shows compatibility and requirement failures.

Numeric array settings accept zero-based `element`, e.g.
`{"setting":{"name":"rollPID","element":0}}` references recorded roll P.
`occurrence` selects repeated records; `element` selects numeric item inside that
record. Non-numeric arrays remain text. Copy Rust source beside PID JSON when
moving examples; compiler requires installed `wasm32-unknown-unknown` target.

## Fields, settings, and metadata

A source is either a raw field ID, a profile reference (`"$roll_gyro"`), or a
setting reference:

```json
{"setting":{"name":"motor_poles","flight":1}}
```

`flight` defaults to 1. If the same setting name occurs more than once, specify
`"occurrence":0` for the first, 1 for the next, and so on. Entries follow recorded
order, including ULog initial values, changes and defaults. There is no implicit
"last wins" rule. Values must parse as finite numbers for filter inputs.

Numeric settings appear as stable-within-log sources `@setting/<flight>/<record>`.
They are constant series across the log's time range. A recorded parameter change
is a separate selectable snapshot, never implicitly last-wins flight history.
ULog parameter messages contain no timestamp: UI marks changed records as untimed,
initial records as log-start values, and defaults separately. Profiles must select
explicit `occurrence` for untimed changes. No event timestamp is fabricated. In multi-input
filters, put a sampled signal first to establish output timestamps; settings can
then be additional inputs. Numeric array elements use sources
`@setting/<flight>/<record>/<element>`. String and non-numeric array records remain
inspectable; they cannot supply numeric filters.

Signal definitions bind an ID and metadata to an existing source. Raw sample IDs
stay unchanged; profile IDs exist within the file. Aliases may refer to earlier
aliases. Filter references can point forward; cyclic branches are skipped with warnings. A filter ID
represents one named output.

```json
{
  "id":"roll_gyro",
  "source":"blackbox/1/gyroADC[0]",
  "metadata":{"name":"Roll gyro","unit":"deg/s","axis":"roll"}
}
```

`name` is required; `unit` and `axis` are optional strings. Names/units appear in
field labels, legends, and filter source labels. Search accepts friendly and raw
names. Name alone is enough. Editor calls optional `axis` **Component**: it supplies
a short field-tree leaf label, such as Roll or Pitch, while the full name stays in
legends. It does not assign a plot X or Y axis.
Metadata alone does not scale values. Declare units only after verifying encoding
and scaling. Manual edits to derived filter operations invalidate profile metadata
for affected outputs/dependents, so old unit labels do not outlive computations.

## A small complete profile

```json
{
  "version":1,
  "name":"Motor 1 RPM",
  "requires_settings":[
    {"setting":{"name":"Firmware revision"},"starts_with":"Betaflight 4.5."}
  ],
  "signals":[
    {"id":"electrical","source":"blackbox/1/eRPM[0]",
     "metadata":{"name":"Motor 1 logged eRPM","unit":"100 eRPM"}}
  ],
  "filters":[
    {"id":"rpm","metadata":{"name":"Motor 1 mechanical RPM","unit":"rpm"},
     "inputs":["$electrical"],
     "operation":{"kind":"affine",
       "gain":{"ratio":[200,{"setting":{"name":"motor_poles"}}]}}}
  ],
  "geometry":[
    {"id":"interval","name":"First second",
     "shape":{"kind":"band","start":0,"end":1}}
  ],
  "views":[
    {"name":"Motor RPM","fields":["$rpm"],"geometry":["interval"]}
  ]
}
```

This example requires the referenced field and recorded motor pole count. It
assumes the stated firmware's logged field is in hundreds of electrical RPM;
mechanical RPM is `raw * 200 / poles`. Electrical RPM from mechanical RPM instead
uses gain `{"ratio":[{"setting":{"name":"motor_poles"}},2]}`. Conversion data
and prerequisites remain explicit in the file. Enlumi does not infer encoding,
read missing motor settings, or validate hardware pole counts from telemetry.

## Schema reference (versions 1 and 2)

Top-level keys: `version`, `name`, optional `requires` (raw IDs),
`requires_settings`, `signals`, `filters`, `geometry`, `views`; version 2 also
adds `plot_presets` and `view_presets` (described below). Unknown keys are
errors. A setting requirement selects a record and uses exactly one `equals` or
nonempty `starts_with` string. Requirements must all match. Use these to constrain
firmware versions and settings such as high-resolution logging.

A scalar is a number, a setting reference, or `{"ratio":[numerator,denominator]}`.
Ratios nest; zero denominators and nonfinite results are rejected. This limited
arithmetic resolves scalar parameters, not sample expressions. Use existing Rust
filters for sample-wise computations.

Filter entries contain `id`, `metadata`, `inputs`, and `operation`:

| Operation kind | Additional keys |
| --- | --- |
| `affine` | `gain` scalar, optional `offset` scalar (0) |
| `low_pass`, `high_pass`, `zero_phase_low_pass`, `zero_phase_high_pass` | `hz` scalar |
| `moving_average` | `samples` integer |
| `derivative`, `magnitude` | none |
| `integral` | optional `initial` scalar (0) |
| `quaternion` | `axis`: `roll`/`pitch`/`yaw`, optional `degrees` (false); four w/x/y/z inputs |
| `rust` | relative `path`, optional `outputs` (1), optional zero-based `output` (0) |

Rust files implement the existing [Rust filter contract](RUST_FILTERS.md).
Paths must stay within the profile directory, including symlink resolution.
Multiple nodes can select different outputs of a script. The current runner may
execute that script separately for each node. Install the WASM target described
in the filter guide before applying profiles containing Rust filters.

Geometry entries contain `id`, `name`, optional RGB `color`, optional `opacity`,
and `shape`:

Geometry IDs identify resources; names are labels and may repeat. Identical
definitions with matching plot styles share one runtime resource. Different
definitions or plot styles stay separate even when names match. Reload can split
previously shared aliases and updates their plot attachments.

Invalid geometry is skipped with its attachments, leaving usable plot fields
available. This also applies during reload. Valid geometry borrowed by another
profile remains available to that profile. Loading status shows preparation stages;
successful partial loads report warnings under **Profiles > Skipped items**.

| Shape kind | Additional keys |
| --- | --- |
| `line`, `rectangle` | `coordinates`: four scalars `[x1,y1,x2,y2]` |
| `band` | `start`, `end`: time scalars in seconds |
| `states` | `source`, `rules`: `[{"value":3,"name":"Mission","color":[40,160,90]}]` |
| `flags` | `source`, `rules`: `[{"mask":4,"name":"Example flag","color":[40,160,90]}]` |

State sources can be raw fields, numeric settings or derived fields. Values must
be exact integers. Exact `states` retain numeric names for unmapped values.
`flags` matches `(value & mask) == mask`; simultaneous names join with ` + `,
using first matching rule's color. Inactive samples leave gaps. Supply 1–64 unique,
nonzero masks within exact integer range (2^53−1). Mask meanings live in recipe;
Enlumi does not guess firmware mode bits. Static geometry coordinates may use recorded
settings. General sample-to-path geometry is not part of version 1.

Views contain `name`, optional `fields`, optional geometry IDs in `geometry`,
optional `tab` (false) and `stacked` (true). Placement flags affect **New panel**;
**Add to active plot** uses the active destination. Views use current derived
results after edits. Deleting a required output/geometry makes the view unavailable
rather than silently using old data. Plots retain the application's shared-Y-axis
behavior; unit labels do not normalize differently-scaled signals. Different
nonempty unit labels show warning and **Separate plots** action. Unknown units
stay unknown. **Labels…** inspector shows raw ID, supplying profile/flight,
transformation chain and recorded scalar settings. Manual label overrides survive
Reload; **Profile default** and **Raw label** reset independently. Labels never
convert values.

Limits: 4 MiB profile JSON, 4096 aliases, 2048 filters/geometries, 32 view presets,
256 state rules per band, and the existing filter and geometry limits.

## Examples

[Betaflight 2026.6 Explorer pack](../examples/profiles/betaflight/2026.6/README.md)
adds 52 view choices for either logging resolution: rates/PID, RC commands,
motors/RPM, sensors/power, named mode/RX bands, and raw debug channels. Select
pack directory with **Folder…**, or copy entire folder into existing library.
Keep Rust helpers beside JSON files. Pack README lists conversions and limits.

See [examples/profiles](../examples/profiles):

- `ulog-esc.json`: usable with the bundled ESC fixture; aliases, named band, view.
- `ulog-mode-bands.json`: navigation-state names from the existing PX4 mapping
  example. Check that the mapping matches your firmware, especially forks.
- `betaflight-4.5-motor-rpm.json`: explicit pole-count conversion and RPM/Hz views.
- `betaflight-4.5-pid.json` plus `rate_error.rs`: roll/pitch/yaw aliases, custom
  Rust rate-error filters, separate rate and PID-term views. Requires normal
  resolution (`blackbox_high_resolution=0`) and all listed fields. Other firmware
  or high-resolution logs require an adapted profile.

Betaflight examples are templates checked with synthetic matching data, not a
claim of real-flight validation. Review required settings, units and state names
against the actual firmware/log before using their interpretations.

## Field groups and axis names

Choose a raw field, then **Labels…** to edit its plot/legend name, tree component
label, and unit. Changes affect current log only and do not scale values. Search
continues matching raw IDs and full display names. **Reset** removes that field's
display override.

For reusable mappings, add `field_groups` to profile. Source prefixes and exact
component suffixes are explicit; Enlumi does not assume every vector is roll/pitch/yaw.

```json
"field_groups": [
  {
    "source": "blackbox/1/gyroADC",
    "name": "Gyroscope",
    "unit": "deg/s",
    "components": {"[0]":"Roll", "[1]":"Pitch", "[2]":"Yaw"}
  },
  {
    "source": "rates/0",
    "name": "Angular rates",
    "unit": "rad/s",
    "components": {"/p":"Roll", "/q":"Pitch", "/r":"Yaw"}
  }
]
```

Array suffixes (`[0]`), nested members (`.x`), and individual fields (`/p`) all
use same mechanism. Group name replaces browser branch label; components replace
leaf labels. Source IDs remain unchanged. Group-generated legend names include
group and component; explicit `signals` metadata overrides generated names. Missing
components are skipped. Existing Betaflight PID examples map gyro, setpoint, and
PID-term components. High-resolution examples retain raw scaling in unit labels.

## Tab names

Double-click tab title, or focus it and press F2, to edit directly. Existing text
is selected. Enter or clicking elsewhere saves; Escape cancels. Clear text to
restore default name. Dragging a view onto unnamed empty plot uses view title.
Names stay attached to plot IDs through docking; titles remain session-local.

## Individual view fields and analysis

Arrow beside each view prepares profile if needed and exposes its available member fields. Members support selection and dragging individually. Drag whole view into **Analyze** to list every available member separately, then enable desired inputs. See [frequency analysis](FREQUENCY_ANALYSIS.md).

## Version 2 plot and view recipes

Version 1 remains supported. Version 2 adds `plot_presets` and `view_presets`;
existing signal/filter/geometry/firmware definitions retain their format.
Application writes version 2 when capturing presets. Loading never rewrites files.

Example plot preset inside a profile with signal alias `$rpm`:

```json
{
  "name": "Motor RPM",
  "lines": [{
    "source": "$rpm",
    "color": [48, 180, 220, 255],
    "width": 1.5,
    "radius": 3.0,
    "marker": "circle",
    "connection": "linear",
    "pattern": "solid",
    "markers": false,
    "hidden": false
  }],
  "links": {"x": false, "y": false, "pointer": true},
  "analysis": null
}
```

Optional plot properties: `geometry` contains geometry IDs, `geometry_style`
contains presentation per ID, and `legend_order` contains `line:$alias` or
`geometry:id` entries. Analysis object contains `mode` (`fft`, `psd`, `heatmap`,
`spectrogram`), `window` (`hann`, `hamming`, `blackman`, `rectangular`), `size`,
`overlap`, `heatmap_bins`, `remove_dc`, and optional `condition` field reference.
Windowed maps require one input; conditioned heatmaps also require conditioning
field. Viewport and interval properties are rejected by recipe schema.

Each `view_presets` entry contains `name`, `plots` (same plot recipe format), and
`layout`. Layout references zero-based plot indexes exactly once:

| Layout | Properties |
| --- | --- |
| `plot` | `plot`: plot index |
| `tabs` | `plots`: indexes, `active`: selected plot index |
| `split` | `stacked`: rows if true, `ratio`, `first` and `second`: nested layouts |

Example layout: `{"kind":"split","stacked":true,"ratio":0.5,"first":{"kind":"plot","plot":0},"second":{"kind":"tabs","plots":[1,2],"active":1}}`.
