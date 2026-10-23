# Using Enlumi

Open ULog (`.ulg`) or Betaflight Blackbox (`.bbl`, `.bfl`, `.cfl`, `.txt`, `.log`) files. Embedded
Blackbox flights have separate field groups and independent elapsed-time origins.
Blackbox values use raw log units. Main I/P fields and merged slow fields support
the same plots and filters as ULog fields; GPS/home records and events are not
shown. File filters include lowercase and uppercase suffixes (such as `.BBL`).
Use **All files** for renamed or mixed-case files; their contents must still be
a supported log format. Decoder failures and frame limits appear as load warnings.

**Workspace** dropdown beside **View** switches collections of views. Saved
workspaces appear automatically, including before opening a log. Selecting one
restores its views and layouts; opening a log populates its plots. Workspace menu
offers **Rename**, **Save**, **Save as...**, and **Duplicate**. Close workspaces
with X in workspace selector. Saved recipes omit axis bounds.

**View** dropdown beside **Equalize panels** switches views in current workspace.
Choose **+ New blank view**, or use adjacent menu to rename, save, or duplicate.
Each view retains its plots, docking layout, zoom, links,
active/maximized plot, and analysis settings/results. Log, derived fields, and
profile resources remain shared. Links stay within their view; background
work finishes in its originating view without switching visible workspace.
Closing final view leaves one blank Untitled view. Open views last for current
application session. View menu offers **Rename**, **Save**, **Save as...**, and **Duplicate**.
Drag presets from Profiles to open views or plots. **Save** overwrites the linked
preset directly; a first save asks for a destination. **Save as...** opens the
editor to create another preset. A captured view previews its plots directly.

Open **Background** in a plot header to edit that plot's background.
On narrow plots, this button shows just the color swatch.
The picker starts with that plot's current color. **Apply to all** copies it
to every open plot, including other views and workspaces. Plots follow the app
theme until you choose a custom color. **Use theme** restores this behavior for
one plot; **Use theme for all plots** restores it everywhere.
Background colors are preserved by duplication, saved presets, and screenshots.
Automatic line colors use the Betaflight Blackbox Explorer palette independently
in each plot; manually chosen and saved line colors remain intact.

Right-click plot background or open plot header
menu to save individual plots. Presets store recipes and layout; zoom, visible
X/Y ranges, and analysis time selections stay in current session.

Sidebar **Save** captures selected raw, derived, and prepared profile fields.
Field right-click **Add to profile** uses same save editor. Choose new file or
loaded destination profile; preview includes required filter dependencies and Rust
source files. Enable **Replace existing entries** explicitly to update conflicting
saved entries. Source preset menu shows **Modified** for recipe changes; zoom and
measurement changes do not mark recipes modified.

Double-click field row to rename inline; Enter or blur saves current-log label,
Escape cancels. Right-click **Edit labels** edits display name, component, and unit;
choose **Current log** or owning profile as save scope. **Edit filter** opens
existing filter editor. Source identity, samples, and plot bounds stay unchanged.
See [profiles](PROFILES.md) for profile editing, saved layouts, and portability.

Press **M**, then click and drag across a plot to measure between data points.
Initial click picks nearest sample. Drag endpoint follows horizontal position on chosen line.
Time plots show elapsed time and reciprocal frequency; FFT/PSD plots show frequency
spacing and reciprocal period. Release mouse clears highlight. Escape has no effect
while measuring.
M leaves text entry alone.

Use each plot's camera menu to save SVG/PNG or **Copy image** to clipboard. Toolbar camera captures all
visible docks together, preserving layout and current X/Y views. PNG renders
the same vector content at twice the SVG dimensions; clipboard images use same resolution.

Expand **Parameters** directly in field tree. **Raw**, **Derived**, and **Profiles**
are independent sections in same browser. Shared search never forces trees open. Search
matches names, values, and record types. Recorded Blackbox flight settings and ULog
parameter/info records appear here; repeated records stay separate. Decoder schema
headers (`Field I/P/S/...` definitions, encoding/predictor tables, Product/Data
version, I/P frame intervals and ratio) stay internal. Firmware, timing/scaling,
motor poles and tuning values remain available. Flight numbers
appear for multi-flight logs. Numeric rows drag into plots and filters as constant
fields. Numeric arrays expand into individually usable elements. Rows show separate
names and values. Select a parameter to read/copy its full value, or use **Add to
plot** / **Use in filter**. Text records remain readable without plot actions. Settings
absent from log cannot be reconstructed.

**Profiles → Folder…** chooses saved library directory. Nested folders and
auto firmware matching use same location. **Profiles** groups saved **Views** and **Plots** directly.
Views contain plots, and plots contain fields. **Profile files** holds file edit/delete actions.
Saved resources use the same tree connectors, disclosure arrows, and row spacing as
raw fields. Large profile field lists build only nearby rows as you scroll in the
shared sidebar; Shift-selection still includes off-screen matches. Use Left/Right
to collapse or expand a focused branch, or Enter/Space to toggle it.
Drag a view or plot to use it; profile loads on demand. Missing
fields skip affected outputs while remaining view works. Views, fields and geometry offer **Add** / **New panel**; fields also offer
**Filter**. **Reload** updates owned outputs without resetting log; **Remove…**
previews affected outputs and dependencies. **Auto** saves local firmware-matching
preference without editing recipe. Select flight for multi-flight files. Double-click tab title (or focus it and press F2) to rename in place. See [profile setup and format](PROFILES.md).

Raw field selected: **Labels…** edits display name, axis/component, and unit.
Reusable group labels live in profiles; source IDs stay unchanged.

1. Click **Open log** or pass a local log path. File Explorer drag-and-drop is deferred to future work.
2. Expand plain text topic-instance roots (for example `vehicle_imu.0`; single-instance topics omit suffix) and named field/array groups in scrollable left field tree. Numeric instances and array indices sort naturally. Search keeps matching ancestor paths in tree without opening branches. Matching topic-instance root or field group keeps its whole subtree; leaf matches prune unrelated fields. Expand results manually. Sidebar stays inside visible window; wheel scrolls fields independently.
3. Drag a field onto any visible plot to add a line. Drag legend labels to move lines between plots or reorder within legend. Drop above/below legend row to insert there; drop onto plot area to append. Appearance and filters follow line. Up/Down on focused legend label reorders it. Ctrl-click toggles individual fields; Shift-click selects visible range; Ctrl+Shift-click adds range. Select several, then drag any selected field to move whole selection into a plot or filter source area. Clicking an unselected field selects it alone; Enter plots focused field. Escape cancels dragging; release outside plots changes nothing.
4. Each plot's **Duplicate** button copies its lines and appearance into new pane. Tab **+** adds a tab in same pane; tab X closes it.
5. Each dock keeps original left/right/top/bottom/center targets: center groups tabs, edges split that pane. Additional four-arrow guides target contiguous multi-pane row/column bands, including nested regions. Hover near a band to reveal its guide; center number counts visible panes. With three rows, top two and bottom two both have guides. Whole-layout preview labels every resulting dock at its future position, including neighbors that shift or resize. Preview and final drop use same layout transaction. Unselected bands keep their positions. Drag splitters to resize: other edges stay fixed until neighboring dock reaches 220 x 160 px minimum, then further dragging pushes its far edge and any adjacent minimum-size docks. Workspace border stops pushing; reversing within drag restores original edges. Perpendicular edges stay fixed; **Maximize**/**Restore** changes workspace view. **Equalize panels** balances columns and rows while keeping tabs and arrangement. Sidebar icon toggles fields; right grip resizes it.
6. Gear inside legend box opens centered line-settings dialog. Legend and gear appear once plot has lines. Select legend entry in dialog to edit HSV color square, rainbow hue strip, hex RGB, width, sample markers (dots, square, triangle, diamond, cross), marker size, and linear/pre-step/post-step connection. Filters workspace manages derived fields separately from line appearance. Legend X removes its line; clicking label toggles visibility. Hovering hidden legend field previews its line above other lines until pointer leaves, without changing visibility or bounds.
7. Pan, wheel/box zoom (release Shift to cancel selection), hover samples, toggle legend visibility. Wheel over bottom X band or left Y axis zooms only that axis around pointer; other axis stays fixed. Double-click inside plot fits both axes. Double-click bottom X axis or full-height left Y edge fits only that axis; right-click menu also offers Fit X axis, Fit Y axis, and Fit both axes. Drag upper/lower half of left Y edge to adjust that bound; Escape cancels axis drag. Legend toggles and removals preserve bounds; adding a field or moving a line into another panel fits destination. Automatic line colors use 16 contrasting colors, avoid reuse across panels until palette is exhausted, then prefer least-used colors. Each plot header's **Links** dropdown has checkboxes for All, X bounds, Y bounds, and pointer links. All enables or clears every link; toggling an individual link leaves other selections intact and updates All indicator. Only pointer links by default; opt into X/Y bounds independently. Linked pointer and coordinates below legend follow snapped sample cross; away from samples they follow free cross position. FPS appears in top-right toolbar and updates at 4 Hz (every 250 ms) from frame timestamps.

**Filters** beside Collapse all in field browser switches plot area to filtering workspace while keeping left field tree visible. Drag raw or derived fields into source area; hover Filters button during a drag to open workspace, or drop directly onto button. Ctrl-click or Shift-click fields, then drag any selected field to add whole group. Clear fields empties source selector without changing editor mode or deleting saved fields. Sources have individually editable output names; low/high-pass, moving average, derivative, integral, or gain/offset settings apply identically to every source. Create batch, edit shared settings without changing output identities, or use **Add next filter** to chain all batch outputs. **Plot outputs** returns to plots and adds outputs to active dock. Sources can be removed from batch individually. Outputs appear under **Derived fields** and support normal plotting and drag/drop.

Choose **Magnitude** to combine all source fields into one named output, or
**Quaternion → RPY** to create three named outputs. Choose radians (default) or
degrees in **Output units**; editing a saved group restores its unit setting. Quaternion
requires exactly four source fields, assigned **w, x, y, z** from top to bottom.
W/X/Y/Z slots remain visible when empty. Drop a field directly into its slot, or
drag quaternion array elements together to fill slots in order. Drag field rows
to swap quaternion assignments. Neighbors slide into preview positions, and rows
settle into place on release. Entire quaternion row is draggable, including padding.
Magnitude and standard filters do not reorder: standard rows label their source
as **Input** and their editable result name as **Output**. Escape or
dropping outside source list cancels reorder. Removing a quaternion input leaves
its slot empty without shifting other assignments.
The supplied DCM formulas determine roll, pitch, and yaw; quaternion values must
already be unit length and are not normalized. Pitch clamps its asin argument to
[-1, 1]; all-zero quaternions and nonfinite inputs produce missing output values.
Roll/yaw remain wrapped to atan2 range.

For multi-input transforms, first source supplies output timestamps. Other sources
with identical timestamps retain sample-by-sample alignment, including duplicates.
Other timelines interpolate linearly between finite neighboring values; there is
no extrapolation or interpolation across gaps exceeding 10 times that source's
median positive sample interval. Missing inputs remain gaps. Magnitude uses first
added input as reference timeline; quaternion uses **w**.

Click any RPY output in filter library to edit its group's inputs and all three
output names together. Saving preserves output identities; **Add next filter**
starts a low-pass batch over RPY outputs. Magnitude keeps its filter type for
chaining. Creation is the default: drag raw or derived fields into an empty draft,
name outputs, and create. After creating fields, inputs clear automatically for
the next draft; **Plot outputs**, **Add next filter**, and **Edit** refer to the
last-created results. Magnitudes can feed later magnitudes without entering edit
mode. The result's **Edit** action can reopen a whole newly created batch.

Click a saved row to edit its field (or quaternion group). Selected rows highlight,
heading names the edit target, and action changes to **Save changes**. Click a
selected row again, or choose **Done editing**, to return to an empty creation
draft. Blue **New field** row follows saved fields at bottom of list and selects a fresh
creation draft when leaving an edit. It highlights while creating fields; clicking
it again preserves current draft. Saved rows have an **X** for removal; fields with
dependents must be removed after their dependents. Saved list, including **New field**,
scrolls independently.
Cycle checks apply only to edit targets: an existing field cannot depend on its
own output or a descendant. Finish editing to create a separate field from either.

Filter library lets you edit or rename individual stages. Upstream edits recompute descendants in background and update every plotted copy while preserving line appearance. Unrelated stages reuse cached samples. Batch validation is atomic: cycles, duplicate names, and invalid parameters leave existing definitions intact. Remove dependent stages before deleting their source. Definitions belong to current log and reset when loading another log.

PT1 uses alpha = 1 - exp(-2*pi*cutoff*dt), using actual sample timing. Low/high-pass
are causal by default; **Zero phase (forward/backward)** applies each filter twice,
with odd-reflected segment edges. High-pass performs high-pass on each pass rather
than subtracting a zero-phase low-pass. No uniform resampling is used. As with
[forward/backward filtering](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.filtfilt.html),
combined attenuation is stronger and edge transients can remain. Reflection covers
up to three time constants, capped at 65536 samples and segment length minus one.

**Moving average** uses a configurable trailing sample-count window (partial windows
at segment starts). **Derivative** uses nonuniform centered differences inside each
segment and one-sided differences at its ends; a single sample has no derivative.
**Integral** uses trapezoidal integration with configurable initial value.
Nonfinite samples, duplicate timestamps, and gaps exceeding 10 times median
positive interval separate segments; integration and averaging restart there.

Gain/offset applies y = input * gain + offset. Its **rad → deg** and
**deg → rad** presets fill gain and reset offset to zero; both remain editable.
Timestamps and gaps are preserved.

One log file at a time; dynamic docked plots and tabs. Plot area fills available workspace, with a green theme and docked panels. ULog timestamps use elapsed seconds from file header. Blackbox flights use elapsed seconds from each flight’s first main frame. Incomplete logs show recovered samples with load warning. Missing/unsupported timestamps, decreasing timestamps, and mismatched field lengths also produce load warnings. Numeric and boolean fields are plottable; character and padding fields are omitted. Raw typed values remain in memory; plot values use f64. Linear and step rendering use column LOD. Visible samples are reduced before step corners are generated, retaining extrema and gaps. Sparse pre/post steps retain exact corners. Markers use bounded visible samples; cursor snapping uses full original samples. Preparation runs off UI thread with one worker per pane, coalesces viewport changes, publishes compatible completed buffers during continuous pans, and reuses unchanged lines. Preparation includes one extra viewport on either side so nearby pans reveal cached samples immediately. Exact rendering falls back to columns above 65,536 visible samples to limit GPU geometry. Filtering always uses full samples. Large logs retain all decoded fields, plus buffers for selected signals.

Automatic session recovery and multi-log comparison are not implemented yet.

**Rust function** beside **Built-in** adds programmable filters with loops, persistent per-run
state, multiple inputs, and named outputs. Requires a Rust compiler with WASM
target; see [setup, examples, and limits](RUST_FILTERS.md). Source is local trusted
code, compiled on demand; failed runs preserve existing saved fields.

Legend gear opens line settings. Click selects one line; Ctrl-click toggles lines; Shift-click selects a range. Check marks, borders, and selection count show active lines. Width, marker shape/radius, and connection controls apply to every selected line; color editing requires one selected line. Color controls stay visible but disabled for multiple selections; at least one line remains selected. Hover info icon for selection help. **Add filters** closes settings and opens a fresh filter batch containing every field from that legend, including derived fields.

## Themes

Choose theme from dropdown beside FPS counter: **Slate** (default; blue-gray, sharp),
**Forest** (green, soft corners), **Ember** (red, crisp), **Amethyst** (purple, rounded),
or **Dune** (warm amber, gently rounded). Palette
updates toolbar, Fields, Filters, code editor surfaces, and plot chrome together.
Switching preserves open plots, selection, and filter draft. Theme choice is saved automatically across launches; line colors remain unchanged.

Filter creation uses larger button pinned below settings. Editing existing fields
keeps **Save changes** and **Done editing** beside top actions.

Theme preference stores one ID in `Enlumi/theme` under Windows `%APPDATA%`,
`~/Library/Application Support/Enlumi/theme` on macOS, or
`$XDG_CONFIG_HOME/enlumi/theme` (default `~/.config/enlumi/theme`) on Linux.
No plots, log paths, scripts, or filter drafts are restored across launches.


## Geometry bounds

**Geometry** sits above derived fields in Fields sidebar. Click **Rectangle** or
**Line**, then drag inside plot. Shapes use data coordinates: pan/zoom changes
view, while bounds stay anchored. Geometry does not change automatic axis fitting.

Each plot's legend includes its geometry, even when plot contains no signals. Drag fields, geometry, and time bands into any legend position; Up/Down reorders focused entries. Geometry order controls overlapping shapes and band label rows. State bands list their named state colors beneath legend entry. Mixed unit labels and **Separate plots** live under **Units** in plot header.
Click geometry name to show/hide that plot's copy. Pencil opens live editor;
**X** removes geometry from that plot while preserving saved definition.

Live editor floats above the plot workspace, leaving the field browser available.
Drag its title across plot boundaries to reposition controls. The title and footer
remain visible in smaller windows while the editor body scrolls. Name and
**Coordinates** update immediately. Drag arrow handle to move geometry; square
handles resize corners/endpoints. Move handle stays on visible section of line
when its defining endpoints are offscreen. Background drag pans, including
rectangle interiors; scrolling zooms while editor stays open. Drag editor title
to reposition controls, or collapse editor to expose covered handles. **Appearance** uses same color picker as signals, plus fill opacity and
stroke width sliders. **Done** closes editor and handles;
**Escape** cancels current drag and exits editing. Invalid coordinates leave
last valid shape intact.

Line direction controls choose **Free**, **Horizontal**, or **Vertical**.
Horizontal/vertical modes expose single Y/X position input and preserve direction
during endpoint drags. Across-plot horizontal/vertical move handles slide only
along relevant axis; finite segments can also translate along their length.
Focused handles support arrow-key nudging; Shift increases step to 10 pixels.

Lines support **Above**/**Below** translucent shading in data Y coordinates.
**Across plot** extends line across viewport; **Segment** keeps finite line and
shading between endpoint X coordinates. Near-vertical lines use stable implicit
clipping. Exactly vertical lines do not support above/below shading.

Drag saved shape onto another plot to reuse definition. Edits update every copy;
visibility and legend removal affect only one plot. **Duplicate** creates
independent definition on active plot; editor's delete control removes saved
definition and all copies. Duplicating plot includes geometry and visibility.
Shapes remain available when opening another log, but are not saved across app
restarts.


## State bands and manual time highlights

In **Geometry**, click **State bands**, then drag one discrete field onto source
area. Each integer value starts with numeric label (`State N`) and automatic
color. No field names or firmware enums select a mapping automatically.
To build file without log, choose **Geometry > State bands > Blank mapping**.
Add values/names/colors and export, then import file onto source-backed layer
when plotting log. Blank mappings have no time intervals.

Build mapping entirely in **Mapping**: source values already have rows. Type
names and pick colors; use **Add** with integer value/name for states absent from
log. Edit existing numeric value, then press **Enter** or **Set**. Row **X** removes
mapping and its shading. Invalid or duplicate values leave existing rows intact.
Source **Refresh** rediscovers observed values, restoring numeric rows for any
observed values removed earlier. **Export mapping** saves shareable JSON file.
**Import mapping** applies any user's mapping to current layer. Imported entries
replace previous mapping; observed values missing from file keep numeric labels.
Entries absent from current log stay available for editing/export and survive
source refresh, so mapping remains useful across flights.

Optional [PX4 example](../examples/state-mappings/px4-nav-state.json) provides
starting point only: verify integer values against firmware used to record log.
Copy/edit file for custom PX4 forks or other discrete fields. File contains no
source paths and can be applied to any discrete raw or derived source. Changes
to firmware enum require mapping edits, not application changes.
See [mapping file format](../examples/state-mappings/README.md).

State transitions produce labeled time spans along X, filled across plot height
behind grid and signals. Names/colors edit live in **Mapping**; **Appearance**
controls opacity and labels. Legend toggles/removal and reuse across plots work
like other geometry. Source controls interval times; generated bands have no
manual handles. Derived source changes regenerate bands while preserving names
and colors. Invalid derived outputs hide stale shading and show error.

Values hold until next update or log end. Duplicate timestamps use last value;
missing/nonfinite values leave unshaded gaps. No state is inferred before first
sample. Continuous values, more than 256 distinct states, and over 100000
intervals are rejected. This complements numeric filters; Rust filters can emit
integer states for custom classifications.

Use **Time band** for manual highlights: drag start/end along X, name section,
and choose color/opacity. Full height stays tied to viewport while start/end
remain fixed. Move handle slides band along time; edge handles resize interval.
Manual highlights coexist with data-driven layers. Successful log replacement
clears generated layers to prevent stale source data; manual geometry remains.
Geometry stays session-only; exported mapping files persist independently.

During edits, cursor follows action: drawing uses crosshair; free move uses
open/closed hand; axis locks and time-band edges use directional resize arrows;
rectangle corner arrows follow actual corner after flips. Locked-line endpoint
handles change length; move handle or position input changes line's X/Y level.

## Legend

Click **▾ Legend** to minimize legend to its heading. Click **▸ Legend** to expand
again. Plotted fields, geometry, and visibility remain unchanged. Each plot has
its own collapsed state; line settings remain available while collapsed.

## Frequency analysis

Open toolbar **Analyze**, drag fields or profile views into inputs, choose analysis mode, then drag signals onto a plot. Drop heatmap conditioning field into its dedicated target. **Select on plot** chooses time bounds by dragging; **Use visible range** copies active time plot bounds. **Remove DC** toggles mean subtraction. Expand profile views to drag individual members. See [frequency analysis](FREQUENCY_ANALYSIS.md) for scaling, sampling, units, and limits.
