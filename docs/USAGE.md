# Using Enlumi

1. Click **Open ULog** or pass a local log path. File Explorer drag-and-drop is deferred to future work.
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

One log at a time; dynamic docked plots and tabs. Plot area fills available workspace, with a green theme and docked panels. Topic timestamps use elapsed seconds from ULog header. Incomplete logs show recovered samples with load warning. Missing/unsupported timestamps, decreasing timestamps, and mismatched field lengths also produce load warnings. Numeric and boolean fields are plottable; character and padding fields are omitted. Raw typed values remain in memory; plot values use f64. Linear and step rendering use column LOD. Visible samples are reduced before step corners are generated, retaining extrema and gaps. Sparse pre/post steps retain exact corners. Markers use bounded visible samples; cursor snapping uses full original samples. Preparation runs off UI thread with one worker per pane, coalesces viewport changes, publishes compatible completed buffers during continuous pans, and reuses unchanged lines. Preparation includes one extra viewport on either side so nearby pans reveal cached samples immediately. Exact rendering falls back to columns above 65,536 visible samples to limit GPU geometry. Filtering always uses full samples. Large logs retain all decoded fields, plus buffers for selected signals.

Session persistence, exports, and multi-log comparison are not implemented yet.

**Rust function** beside **Built-in** adds programmable filters with loops, persistent per-run
state, multiple inputs, and named outputs. Requires a Rust compiler with WASM
target; see [setup, examples, and limits](RUST_FILTERS.md). Source is local trusted
code, compiled on demand; failed runs preserve existing saved fields.

Legend gear opens line settings. Click selects one line; Ctrl-click toggles lines; Shift-click selects a range. Check marks, borders, and selection count show active lines. Width, marker shape/radius, and connection controls apply to every selected line; color editing requires one selected line. Color controls stay visible but disabled for multiple selections; at least one line remains selected. Hover info icon for selection help. **Add filters** closes settings and opens a fresh filter batch containing every field from that legend, including derived fields.

## Themes

Choose theme from dropdown beside FPS counter: **Forest** (green, soft corners),
**Slate** (blue-gray, sharp), **Ember** (red, crisp), **Amethyst** (purple, rounded),
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

Each plot's legend includes its geometry, even when plot contains no signals.
Click geometry name to show/hide that plot's copy. Pencil opens live editor;
**X** removes geometry from that plot while preserving saved definition.

Live editor sits inside selected plot, leaving field browser available. Name and
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
