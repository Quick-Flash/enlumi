# Log profiles

Profiles are versioned JSON files describing signal names/units, existing filter
operations, named geometry and state bands, and reusable plot views. Firmware
field names, aliases, state names and conversion settings live in these files.
No Betaflight alias table or motor pole-count default is built into Enlumi.

## Use

1. Open a log. Expand **Parameters** in field tree for every recorded entry.
   Numeric parameters drag into plots and filters; numeric arrays expand into elements.
   Select a parameter for full, selectable value and direct plot/filter buttons.
   Click **Profiles** directly in sidebar heading. **Fields** switches back.
2. Browse folders and expand a profile. Drag one of its views onto a plot.
   Enlumi loads profile if needed, computes available outputs, and adds that view.
   **Load** prepares resources without placing plots; **Load file…** supports
   profiles outside library directory. Existing work is preserved.
3. Under **Loaded views**, drag a view onto any plot, or use **New panel** /
   **Add to active**. Existing plot
   contents remain; fields already present are skipped. View geometry is attached
   too. Adding a view fits the destination axes. Views can be used repeatedly.

Named state bands also appear in the geometry library. Their name and each
state's name/color can be edited with existing geometry controls. Labels render
inside the time bands. Derived band sources refresh after filter recomputation.

Profiles are additive. Conflicting output names, geometry names, or signal
metadata reject the load. Each profile path loads once per log; reload the log
to apply a revised file from scratch. Missing signals, failed filters and their
dependents, and unavailable geometry are skipped with visible warnings. Views keep
remaining lines/geometry; completely empty views are skipped. Raw `requires`
entries report missing data without blocking unrelated resources. Firmware/settings
conditions remain strict to prevent applying wrong units or version mappings.
Invalid JSON/schema, duplicate IDs/view names, and commit conflicts reject load.
Changing logs cancels in-flight preparation. Filters use existing cancellation,
validation, and Rust/WASM execution limits.

## Profile directory

Sidebar scans this directory when opened, when log changes, or when **Refresh** is used:

| Platform | Directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/enlumi/profiles`, otherwise `~/.config/enlumi/profiles` |
| Windows | `%APPDATA%\Enlumi\profiles` |
| macOS | `~/Library/Application Support/Enlumi/profiles` |

Create the directory and copy JSON files there. Rust source files referenced by a
profile travel beside it or in a subdirectory. The chooser shows the exact path.
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

Click **Auto** beside a profile to enable automatic firmware selection. **Auto ✓**
means enabled; click again to disable. Toggle saves `"auto_apply"` in that JSON file.
Reload log to use changed defaults. Editing `"auto_apply": true` manually also works;
default is false. Each successful log load scans files in filename order and matches
`requires_settings` against recorded metadata, including firmware revision/version.
At least one setting condition is required for auto-apply. Unmatched firmware
profiles are skipped. Matching profiles apply serially and open Profiles sidebar
with available views ready to drag. Missing fields do not block them. Broken files
or resources report warnings and do not stop unrelated defaults. No panels open
automatically; drag a view or choose its panel action.

Use narrow version prefixes, e.g. `Betaflight 2026.6.`, rather than guessing across
firmware releases. Conditions include explicit flight number (default 1); examples
configure first flight only. Additional flights need separately configured profiles.
Scanning reads all JSON profiles; malformed files report errors. Rust filters in
enabled matching profiles run through existing WASM engine. Only enable profiles
you intend to run. Refresh updates discovery; reload log to reapply defaults.

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
is a separate selectable value; its event time is not reconstructed. In multi-input
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
names. Semantic `axis` is stored as metadata; it does not assign a plot Y axis.
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

## Schema reference (version 1)

Top-level keys: `version`, `name`, optional `requires` (raw IDs),
`requires_settings`, `signals`, `filters`, `geometry`, `views`. Unknown keys are
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

| Shape kind | Additional keys |
| --- | --- |
| `line`, `rectangle` | `coordinates`: four scalars `[x1,y1,x2,y2]` |
| `band` | `start`, `end`: time scalars in seconds |
| `states` | `source`, `rules`: `[{"value":3,"name":"Mission","color":[40,160,90]}]` |

State sources can be raw fields, numeric settings or derived fields. Values must
be exact integers. Unmapped values retain numeric names. Bitmask fields need an
explicit extraction/filter step or rules for exact combined values; Enlumi does
not guess firmware mode bits. Static geometry coordinates may use recorded
settings. General sample-to-path geometry is not part of version 1.

Views contain `name`, optional `fields`, optional geometry IDs in `geometry`,
optional `tab` (false) and `stacked` (true). Placement flags affect **New panel**;
**Add to active plot** uses the active destination. Views use current derived
results after edits. Deleting a required output/geometry makes the view unavailable
rather than silently using old data. Plots retain the application's shared-Y-axis
behavior; unit labels do not normalize differently-scaled signals.

Limits: 4 MiB profile JSON, 4096 aliases, 2048 filters/geometries, 32 view presets,
256 state rules per band, and the existing filter and geometry limits.

## Examples

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

## Tab names

Use **Rename** in plot strip, enter title, and press Enter or **Save**. **Reset**
restores default name. Dragging a view onto unnamed empty plot uses view title.
Names stay attached to plot IDs through docking; titles remain session-local.
