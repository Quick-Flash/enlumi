# Unified fields, workspace views, and presets

Status: unified browser, workspace views, plot/view presets, profile authoring,
and direct field editing implemented.
Workspace collections now add a separate selector, automatic discovery of saved
workspaces, and capture/restore of multiple views as one reusable setup.
Supersedes browser, naming, and
authoring proposals in [earlier profile review](PROFILE_SYSTEM_PLAN.md).

## Implementation progress

- Raw, Derived, and Profiles now share one sidebar search and scroll area.
- Sections collapse independently; search does not force profile folders or
  sections open. Raw/derived rows remain virtualized.
- Ctrl/Shift selection spans source kinds. Mixed drops resolve profile resources
  alongside raw/derived fields before adding them to plots, Filters, or Analyze.
- Individual profile fields show sample counts once their source is prepared.
- View selector beside Equalize supports new, rename, duplicate, switch, and close.
  Open views retain their own layout, bounds, active/maximized plots, links, and
  analysis state. Only current view mounts plot UI.
- Background field/profile/analysis work retains its destination. Shared filter
  and profile changes update hidden plots; closed destinations reject results.
- Plot background and header menus save individual plot presets; view menu saves
  full layouts. Save writes linked recipes directly; Save as reviews a new destination.
- Shared save editor supports new profiles, selected fields, plot/view capture,
  loaded-profile editing, Save as, labels, filter parameters, dependency previews,
  and removal of unreferenced resources. Workspace docking remains layout editor.
- Version 2 recipes restore styles, geometry, tabs, layout, links, and analysis
  settings. Analysis buffers, intervals, and viewport state are excluded.
- Profiles tree opens complete views and appends view plots by drag/drop.
  Plot preset browser inserts plots or selected fields while preserving destination bounds.
- Double-click field rows renames inline; right-click exposes labels, filter
  editing, profile capture, and source copying. Labels offer explicit current-log
  or owning-profile scope, with reset to inherited/raw labels.
- Atomic saves preserve unrelated content, require explicit conflicting replacement,
  reuse shared dependencies, and copy Rust files to portable relative paths.
  Legacy profiles remain readable without an automatic format rewrite.
- Open views remain session state. Presets provide explicit persistence; automatic
  session recovery, thumbnails, and additional management screens remain deferred.

## Goals

- Mix raw, derived, and profile fields without switching sidebar modes.
- Make individual plots and complete workspace layouts reusable presets.
- Create profiles from existing work instead of requiring hand-written JSON.
- Keep configuration entry points visible across selection and editing states.
- Keep zoom and visible X/Y ranges outside profiles and presets.

## Concepts

| Concept | Meaning |
| --- | --- |
| Open view | Live workspace containing plots and their docking layout |
| Plot preset | Saved recipe for one plot: field references, appearance, analysis configuration, geometry |
| View preset | Saved collection of plot recipes, docking layout, tabs, and link relationships |
| Profile | Named collection of labels, filters, geometry, plot presets, and view presets |

An open view is editable working state. A preset is explicitly saved reusable
configuration. Changes to an open view never silently overwrite its source preset.

Existing profile `views` describe individual plots. Present those as plot presets
while retaining compatibility with existing JSON files. Reserve UI term **View**
for complete workspace. Do not rewrite legacy files merely by loading them.

## Bounds stay outside presets

Profiles, plot presets, and view presets must not store or apply zoom, visible X/Y
ranges, axis limits, or auto-fit state. No **Include axis bounds** option and no
saved range policy. Analysis time selections also remain runtime configuration;
loading a preset must not impose a saved interval or change current selection.

Adding fields to an existing plot preserves its current bounds. New plots use
normal initial fitting for their own data, independently of other plots. Explicit
axis links remain supported; loading a preset must not force saved numeric bounds.

Switching between already-open views restores each view's live plot state,
including its current bounds. That is session state, not preset serialization.
Temporary measurements, cursors, and range-picking gestures are not preset data.

## Unified sidebar

```text
FIELDS                       [+] [menu]
[Search fields and presets...          ]

v Raw
  > Topics
  > Parameters

v Derived
    Smoothed RPM
    Current magnitude

v Profiles
  v Motor analysis
    > Fields
    > Plot presets
    > View presets
    > Geometry

---------------------------------------
3 selected
[Plot] [Analyze] [Filter] [Save...]
```

- Raw, Derived, and Profiles are independently collapsible sections in one
  scrollable tree. Remember expansion and scroll state.
- Shared field rows show consistent labels, units, sample counts, selection,
  hover, context menus, and drag behavior.
- Ctrl/Shift selection spans sections. Drag mixed selection into plots, Filters,
  or Analyze using existing compatibility rules.
- Field search matches display names, original source paths, units, and profile
  field labels. Show match counts beside collapsed field sections.
- Search filters field entries without forcing sections open. Saved plots,
  views, workspaces, geometry, and profile files remain visible. Manual expansion
  works during search; clearing query preserves chosen expansion state.
- Parameters remain under Raw and retain normal search/expansion behavior.
- Configuration entry points remain visible. Disable unavailable actions and
  explain missing prerequisites. Detail sections still expand on demand.
- Profile menu exposes New, Edit, Folder, Import, Reload, and existing ownership
  actions. Keep help and long paths out of primary row layout.

## Open workspace views

Place view dropdown beside **Equalize**:

```text
[Equalize] [View: Motor analysis v] [menu]
```

Dropdown lists open views plus **New blank view**. Drag saved views from Profiles.
Adjacent menu provides Rename, Duplicate, Save as preset, and Close.

Each open view owns plots, docking layout, tabs, active plot, maximized state,
appearance, hidden lines, geometry visibility, axis/pointer links, and analysis
results/configuration. Links are scoped to that view and compatible axis domains.

Log data, derived fields, and prepared profile resources remain shared. Switching
views must not reload log, recompute unchanged filters, or discard existing FFTs.
Hidden views stop rendering. Background work targets originating view and plot,
even if user switches views before completion. Closed destinations reject stale
results. Finish transient drag/measurement gestures when switching views.

Start with **Untitled** view wrapping current workspace. Closing final view leaves
one blank Untitled view. Full session recovery across application restarts is later
work; saved presets provide initial persistence.

## Plot and preset actions

Right-click plot background opens contextual menu. Plot menu button exposes same
commands for discoverability and keyboard access. Preserve context menus on
specific plot elements such as legend entries.

```text
Add fields...
Add plot preset...
Save plot as preset...
Update source preset        [disabled when unlinked]
Duplicate plot
Screenshot                  >
Close plot
```

| Preset | Available actions |
| --- | --- |
| Plot preset | Add as new plot; Add fields to selected plot... |
| View preset | Open as new view; Add plots to current view |

Adding fields opens selection of preset signals and preserves destination
appearance and bounds. Adding complete plot preserves preset appearance while
fitting new plot normally. Reuse prepared dependencies and skip duplicate sources
within destination plot. Reject incompatible time/FFT combinations before mutation.

Double-click plot preset adds new plot; double-click view preset opens new view.
Individual preset fields remain draggable. Drop plot preset onto existing plot
to add compatible fields; onto workspace docking target to create plot. Whole-view
preset actions use explicit menu commands initially to avoid ambiguous drops.

## Field editing

- Double-click field label to rename inline. Right-click offers Rename,
  Edit labels, Edit filter where applicable, Add to profile, and Copy source path.
  Do not rely on F2 for discovery or access.
- Single click continues selecting; dragging continues moving fields. Recognize
  double-click without accidentally plotting or starting drag.
- Inspector exposes display name, original source, component, unit, and provenance.
- Save scope is explicit: **Current log** or named profile. Show inherited labels
  and local overrides, with reset action.
- Rename updates tree, legends, and search without changing source identity.
- Applying metadata changes preserves plotted values and current bounds.
- Configuration remains reachable regardless of current selection. Unsupported
  editing actions remain disabled rather than disappearing.

## Profile authoring and saving

Three entry points share one save editor:

1. Selected fields -> **Create profile from selection...**
2. Plot menu -> **Save plot as preset...**
3. View menu -> **Save as...** for a new preset; **Save** overwrites the linked preset directly

Editor asks for name and destination profile, including **New profile**.
Expandable preview lists included fields, labels, filters, geometry, plots, and
layout. Include required upstream filter dependencies automatically. Capture only
resources needed by selection, plot, or view.

Save source references and recipes, not sample arrays or computed FFT buffers.
Store analysis type, window function, DC removal, and other algorithm settings,
but no analysis interval, viewport bounds, or range policy. Runtime analysis uses
current analysis selection and existing application behavior.

Rust filters include required source files with portable relative paths. Missing
required save dependencies produce specific errors. Loading on another log shows
missing inputs per affected resource while preserving usable outputs; never bind
to an unrelated field silently.

Existing profiles expose Edit, Save changes, Save as, and Reload. Saving requires
explicit action. Show **Modified** for saved-content changes, excluding zoom,
cursors, and other transient state. Duplicate names require explicit Replace.
Use atomic file writes and preserve existing content not being edited.

Profile builder supports adding selected fields, editing labels, and arranging
plot presets into view presets. Reuse existing filter editor for filter settings.

## Implementation order

1. **Unified browser:** shared rows, independent sections, shared search,
   cross-section selection and drag/drop. Preserve profile loading/ownership.
2. **Workspace views:** move current global panes/layout into per-view state;
   add switching, naming, duplication, closing, and correct background-job routing.
3. **Plot presets:** capture/save/restore single plots, field insertion,
   dependency capture, source-preset association, legacy compatibility.
4. **View presets:** capture layout and plot recipes; open separate views or append
   plots to existing view. Remap runtime IDs and link relationships on creation.
5. **Profile authoring:** shared save editor, profile builder, direct field editing,
   explicit persistence scope, and profile update workflows.

Keep reusable docking and plot configuration mechanisms in `zgui-plot` where
needed; Enlumi owns profile storage, signal resolution, and filter dependencies.
Do not serialize reactive handles, runtime IDs, data buffers, or viewport state.

## Acceptance checks

- Raw, derived, and profile fields select and drag together.
- Search never forces expansion; trees remain manually collapsible.
- Configuration actions remain discoverable across empty, selected, and edit states.
- Double-click/right-click editing coexists with selection and dragging.
- View switching restores live layout/bounds and retains completed analysis.
- Hidden views do not render; asynchronous results reach correct destination.
- Plot and view presets round-trip fields, appearance, geometry, and layout.
- Saved files contain no zoom, X/Y limits, analysis intervals, or range policies.
- Applying presets or metadata never changes existing plot bounds.
- New plots fit their own data; time plots and spectra never mix accidentally.
- Legacy profiles still load; repeated insertion reuses prepared dependencies.
- Profile dependencies, Rust files, missing inputs, and conflicting saves produce
  actionable diagnostics without destroying existing work.

Defer automatic session recovery, thumbnails, and additional management screens
until core workflows work end to end.

## Verification

- `cargo xtask pre-commit`: passed; 153 application tests and 3 xtask tests passed, 6 ignored.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Coverage includes real pointer save/rename/context-menu gestures, preset and
  dependency round trips, portable Rust files, explicit replacement and stale
  saves, shared-resource ownership, legacy editing, nested-layout ID remapping,
  background destinations, existing bounds, clean/modified state, and FFT/map
  configuration without saved intervals or viewport state.
