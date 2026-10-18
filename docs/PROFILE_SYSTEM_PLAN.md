# Profile system review and next steps

Status: ownership/browser, Auto/Reload/Remove, and metadata changes implemented locally.
Automated validation passed; see implementation notes for remaining release checks.
Historical review follows.

## Immediate fixes

- Keep Fields / Profiles / Filters at fixed 4px gaps. Removing Collapse must not
  redistribute remaining buttons.
- Recognize tab double-click in tab's own pointer handler. Tab dragging captures
  pointer and prevents normal activation, which also suppresses framework's
  synthesized double-click. Listening only on title button cannot work on that path.
- Keep existing drag threshold, single-click selection, F2, Enter, Escape, and
  focus-loss rename behavior.

Compile checks cover API integration. Pointer/focus behavior still needs runtime
interaction validation; these fixes do not establish a finished interaction suite.

## What currently works

- Saved library directory, recursive folder browsing, search, manual imports.
- Firmware/settings guards, opt-in automatic loading, partial resource preparation.
- Raw source identities separate from names, component labels, and unit labels.
- Numeric parameters and array elements as filter inputs; decoder tables hidden.
- Views dragged into plots; named geometry and custom Rust filters.

Keep these capabilities. Improve their state model and presentation before adding
more profile features or firmware packs.

## Problems found during original review

| Problem | Evidence | User impact |
| --- | --- | --- |
| Library and Loaded views duplicate same recipes | `src/profile_library.rs` renders both lists | Extra navigation; unclear which row to use |
| Auto toggle rewrites recipe JSON | `profile::set_auto_apply` serializes file again | Local preference dirties shared repositories; read-only libraries cannot toggle Auto |
| Loaded profiles have no resource ownership record | `ProfileUi` keeps flat fields/geometry, applied paths, and view indices | No reliable Reload/Remove; collisions require resetting log |
| File path loads once per log | `Workbench::load_profile` returns early for applied path | Editing recipe then refreshing list does not update generated outputs |
| Refresh results lack request identity | Library refresh checks log generation and directory only | Repeated refreshes in same directory can finish out of order |
| Disabled view drag has little explanation | `ProfileViewSource` uses `id = None` when incompatible | Gesture appears broken; no reason at point of use |
| Availability and warnings are mostly strings | `Prepared.warnings` and generic load errors | Duplicate missing-source messages; hard to locate affected branch |
| Manual labels lose origin | Label editor and profiles write same metadata map | Cannot explain winning label or restore profile default separately from raw name |
| Unit labels are declarations | No dimension/conversion validation | Mixed units can share Y axis; relabeling can look like conversion |
| Parameters are constants per record | `LogStore::prepare` spans entire log with selected value | Recorded changes do not yet produce time-varying parameter signals |
| Flight references are explicit IDs | Examples bind flight 1; settings default to flight 1 | Automatic matching does not instantiate setup separately for every flight |

## Target interaction

1. Open log. Sidebar shows detected firmware and profiles that match it.
2. Use one tree: folders → profile → Views / Fields / Geometry.
3. Profile status reads **Available**, **Loading**, **Ready**, **Partial**, or
   **Unavailable**. Partial status includes usable/total count; reason expands inline.
4. Drag view to plot, or use keyboard-accessible **Add** / **New plot** actions.
   Loading happens when needed. Repeated placement reuses prepared resources.
5. Use same source rows for individual profile fields as ordinary fields. Geometry
   keeps same attachment gesture as existing geometry library.
6. Profile menu holds **Auto**, **Reload**, and **Remove**. These actions do not
   compete with primary drag/open gesture.
7. Tab name edits in place. Pointer and keyboard paths produce same result.

Keep sidebar header, search placement, row heights, focus rings, spacing, and
empty/loading states consistent. Preserve folder expansion, search, and scroll
when switching Fields / Profiles. Show full paths and detailed diagnostics on
request; do not fill main tree with them.

## Milestone 1 — Profile ownership and one browser

Introduce one applied-profile record per canonical recipe path and flight. Store
owned field IDs, geometry IDs, view IDs, preparation status, and diagnostics.
This record replaces separate path/index bookkeeping; no plugin framework needed.

- Give views stable identities; avoid vector positions as drag identity.
- Add request token to library scans. Discard superseded scans even when directory
  and log generation are unchanged.
- Keep library state outside transient sidebar component so mode switches preserve it.
- Render one tree from library records plus applied state. Imported files outside
  library appear in one Imported group, not a duplicate global view list.
- Expose individual fields, views, and geometry under their owning profile.
- Structured diagnostics identify profile, resource, source, and failure. Show
  one cause with dependent outputs grouped underneath.

Acceptance: repeated drops do not duplicate resources; partial profiles show
usable outputs; switching mode preserves position; stale refresh cannot replace
newer results; every drag has keyboard alternative and visible failure reason.

## Milestone 2 — Predictable Auto, Reload, and Remove

- Store user's Auto choices in application preferences, keyed by canonical path.
  Keep firmware matching rules in recipe. Do not rewrite recipe to toggle local UI
  preference. Specify precedence between recipe default and user override.
- Show which recorded firmware/settings caused match and which flight it targets.
- Reload prepares replacement in background, then swaps owned resources together.
  Failure preserves previous working setup. Unrelated/manual plots stay intact.
- Remove affects only owned resources. Detect manual filter/view dependencies;
  present concrete dependent items before destructive removal.
- Define explicit conflict policy for two profiles naming same signal or geometry.
  Display conflict instead of silently choosing last loaded file.
- Reuse prepared resources when placing views; refresh missing/deleted resources
  deliberately instead of asking user to reload entire log.

Acceptance: Auto works with read-only recipe folder; recipe files stay unchanged;
Reload updates plots without resetting log; failed reload preserves old outputs;
Remove cannot silently break unrelated work.

## Milestone 3 — Trustworthy signal and parameter meaning

- Inspector shows raw ID, current name/component/unit, supplying profile, conversion
  pipeline, and recorded parameter inputs. Separate label override from data transform.
- Retain raw metadata, profile defaults, and manual overrides separately so Reset
  can restore profile label or raw label intentionally.
- Warn about incompatible units in one panel; offer separate plots. Do not guess
  engineering units or invent conversions for unknown fields.
- Select flight before matching/instantiating profiles for multi-flight files.
- Preserve parameter event times where available. Distinguish initial values,
  changes, defaults, and user overrides; never assume last recorded value applies
  across entire flight.
- Add JSON-defined bitmask state rules for flight-mode flags, alongside exact-value
  state rules. Name active bands without embedding firmware enum tables in code.
- Expand verified example packs only after source names, scaling, high-resolution
  modes, pole counts, and optional fields have explicit evidence.

Acceptance: user can trace plotted value to raw field plus transformations and
settings; unknown firmware remains raw; partial/multi-flight logs stay scoped;
invalid conversion inputs skip affected result with clear reason.

## Milestone 4 — Interaction and data validation

Required validation matrix before calling system solid:

- Real pointer events: double-click rename, F2, selection replacement, Enter,
  Escape, blur, drag after rename, rename after drag, tab close, overflow, docking.
- Compare header button positions in Fields and Profiles at narrow/wide widths.
- Folder selection, cancellation, restart persistence, deleted/read-only folders,
  repeated refresh, mode changes during scan, and log change during preparation.
- Full/partial/incompatible profiles; missing sources; failed Rust branch; cycles;
  repeated view drops; Reload/Remove dependencies; conflicting metadata.
- Provided Basher log, high-resolution Blackbox log, multi-flight Blackbox file,
  and ULog with parameter changes. Keep private logs out of public repository;
  use small shareable fixtures for repeatable checks.
- Large field/profile lists, long names, keyboard-only navigation, themes, and
  visible loading/cancellation states. Measure load/interaction times before
  optimizing implementation.

## Recommended next implementation

Start Milestone 1 with applied-profile record and single browser. Pair model change
with one complete flow: expand profile → inspect available fields → drag view →
see partial reasons. Then implement Auto/Reload/Remove on those owned resources.
Avoid another round of independent buttons over flat global state.


## Implementation notes

- One applied record per canonical path + selected flight owns fields, geometry,
  named views, diagnostics, and created lines. Index-based view transport removed.
- One browser includes directory profiles and Imported entries; views/fields/
  geometry have drag and button actions. Mode switches preserve mounted state.
- Scan tokens reject superseded refreshes. Auto overrides save separately from
  recipes, with explicit override taking precedence over recipe default.
- Reload stages computation and dependent filters/state geometry, validates conflicts
  and concurrent edits, then updates stable IDs and placed views. Failed reload
  preserves usable setup. Remove previews owned counts and names manual blockers.
- Inspector traces sources, filters and recorded scalar settings; manual labels
  stay separate from defaults. Mixed unit labels can split into separate panels.
- Flight selector scopes template references; JSON bitmask state rules supported.
- ULog parameter messages have no event timestamps. Changed records remain explicit
  untimed snapshots; no fabricated parameter history. Other timestamped parameter
  formats remain future work.
- Pointer/keyboard rename test reproduced missing focus after mounting. Local
  zgui-plot now waits for input binding before requesting focus/select-all.
  Enlumi pins published zgui-plot revision `abc99220c129bf6e85e8b370aa0b42d1994295cc`.

Validation covers profile ownership, partial preparation, optional-output recovery,
failed reload preservation, removal dependencies, read-only recipe Auto choices,
flight reference rebinding, bitmask combinations, browser mode persistence/header
spacing, and pointer/F2/Enter/Escape/blur rename. Provided Basher log verified six
PID views and RPM output, with missing yaw D skipped. Synthetic high-resolution and Rust-filter cases pass. Added public synthetic
multi-flight fixture verifying distinct IDs/names, scoped settings, independent
clocks, and combined bitmask bands. Full release matrix above
still requires platform file-picker/restart checks, additional real high-resolution
and multi-flight logs, and large-library timing measurements. Private log stays
outside repository.

Automated results: 109 default tests passed (6 opt-in tests skipped); 11 profile
data tests passed with opt-in Rust/high-resolution/Basher checks enabled.
`cargo check --offline`, formatting, and diff whitespace checks passed.
