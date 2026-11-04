# 2D maps and 3D flight views in Enlumi

Implementation plan and status, updated 2026-11-03. The `mapping/foundation`
branch includes normal-workspace 2D/3D maps with arbitrary coordinate and altitude
fields, plus a standalone terrain demo.

## Map a flight in the normal workspace

Select **Map** to open the same coordinate-pair builder used by 2D plots.
No GPS source is discovered or chosen automatically. Bind longitude/latitude
fields and select degrees, degrees multiplied by 10^7, or radians explicitly.
An optional GPS fix-type field can be supplied. Each pair is a separate trace;
drag it onto an existing map to add it or onto another plot to create a map.
Each map has its own camera and can be docked, duplicated and saved normally.

**Local metres** accepts east in X and north in Y. Give the trace a starting
longitude/latitude in the builder, or drop it onto a map and choose **Set start**
in its legend to click a location. The default places the first valid X/Y
sample at that point; **Anchor: local zero** instead places local (0,0) there.
Each trace retains its own anchor. Local metre displacements use the ground
scale at the anchor, providing a flat local frame rather than a globe model.

Provider selection, 2D/3D mode, offline mode, attribution links, and fit/reset
controls live in the coordinate builder on the right. Open **Map** with an
existing map selected, or use **Edit coordinates**, to change that plot live.
Settings persist with plot/workspace recipes.

Choose **3D map** to orbit the terrain. Each coordinate pair has a **Z / altitude**
slot: drag any sample field there, or enter its source ID and choose **Bind Z**.
Metres, millimetres, and feet are supported. Select **Z increases down** for
down-positive local coordinates such as NED; the default is up-positive. **Local Z** fixes its origin to
terrain elevation at that trace's first valid X/Y position. This height is sampled
once and remains fixed as the trace moves; Z=0 does not follow terrain.
**Global elevation** uses absolute elevations in the terrain dataset's vertical
reference. With no Z binding the trace uses Z=0. Invalid or unavailable altitude
samples break segments rather than extrapolating.

3D traces use depth testing, trace colors/visibility, start/end crosses, and the
shared timeline marker; clicking a projected segment pins timeline time.
3D traces currently render solid lines.

2D maps use the existing plot legend, visibility, ordering, colors, widths,
patterns and markers. 2D screenshot/context-menu/clipboard actions use the normal
zgui-plot snapshot API: SVG retains vector paths and embeds currently loaded
raster tiles; PNG uses the same SVG rasterizer. Exports include the legend,
distance scale and selected-provider attribution; exports perform no tile requests.
Hover near a sample in either map to snap a ring onto it and show its trace,
time and latitude/longitude. 3D hover also shows the original Z in metres and,
for relative Z, the absolute elevation using the fixed starting elevation.
Hover follows the shared timeline only when timeline follow is enabled; pinned
time is preserved. Drag, zoom, hiding a trace and leaving the map clear hover.

3D plot screenshots embed the latest completed GPU frame, including terrain,
traces and timeline marker, in SVG/PNG exports and clipboard captures. Both plot
and dock screenshots retain attribution and legend. Captures make no tile requests.

**Fit traces** (also double-click in 2D) fits visible traces. Drag to pan, wheel to
zoom, and Shift-drag to box zoom. Saved camera state allows zoom 28. Both 2D and 3D interactions reach zoom 22;
raster requests stop at zoom 19, with closer views enlarging existing imagery.
Green/red markers identify the first/last valid positions. The gold markers
follow the shared timeline; clicking near a visible path segment pins its
interpolated time. **Link timeline** controls marker linking and path picking.

Map tracks retain f64 geographic coordinates and recording times, unwrap the
dateline for projection and preserve invalid fixes and time gaps. A horizontal
2D fix is usable without altitude. Normal ULog/tlog field readers, cancellable
trajectory workers and retained path buffers are reused. Moving the timeline
marker does not reprepare paths or request tiles. Map coordinate ranges are
independent of Cartesian XY/time links. Source, camera, provider and offline-only
settings are saved with plot/view/workspace recipes; replacing the log clears
old map data and fits the new track. Hidden/closed maps release visible tile
demand; maps in one Workbench share a lazily created tile service and disk cache.
Provider IDs keep cached imagery separate. Normal maps store tiles in the user
cache directory (`%LOCALAPPDATA%/Enlumi/maps` on Windows,
`~/Library/Caches/Enlumi/maps` on macOS, `$XDG_CACHE_HOME/enlumi/maps` or
`~/.cache/enlumi/maps` on Linux). `ENLUMI_MAP_CACHE` overrides this.

Provider menu offers OpenStreetMap, Google Road/Satellite/Hybrid, Bing
Road/Satellite/Hybrid, Esri Street/Imagery/Terrain, and Mapbox Streets or
Satellite. Bing's zoom 0 world view expands into zoom 1 tiles. Mapbox reads
`ENLUMI_MAPBOX_TOKEN`. Cached tiles load immediately;
expired tiles display while workers revalidate them. Parent tiles provide coarse
coverage while sharper tiles load; adjacent zoom levels are prefetched. **Offline
tiles only** uses saved cache entries and disables requests for that map. Uncached
areas remain empty and appear in status. First visit needs internet or local XYZ tiles.
Blackbox GPS/home records are not imported yet. Any available local metre columns can still be mapped using an explicit anchor.

The standalone demo exercises the same terrain renderer and tile service used
by normal workspace maps.

## Run the demo

```sh
cargo run --locked --release -p enlumi -- --map-preview
```

The demo starts at the Matterhorn. The left pane displays OpenStreetMap raster
tiles; the right drapes those tiles over Mapterhorn elevation data on zgui's
shared GPU device. The first visit needs an internet connection. This remains
in the demo workspace; normal-workspace flight maps currently use 2D.

- **2D:** drag to pan, wheel to zoom, Shift-drag to box zoom.
- **3D:** drag to orbit, **middle-drag or Ctrl-drag to pan** the shared map center, wheel to zoom.
  Drag upward to lower the angle (down to 2 degrees). Horizontal orbit dragging
  follows the corrected direction; vertical dragging keeps its original direction.
- **Distance scale:** metric and imperial rulers appear at bottom left. In 3D,
  the ruler measures horizontal ground distance at the map centre/focus plane;
  perspective means scale varies elsewhere in the view.
- **Matterhorn / Rockies:** jump to a public example area at zoom 12.
- **Reset orbit / Height 1x or 2x:** restore the orbit or change vertical exaggeration.
- **View distance 40 / 80 / 160 km:** select visibility beyond the camera focus,
  independently of zoom. The default is 80 km. Distant coverage uses coarser tiles.
  Resizing changes the aspect ratio without moving the 3D camera. Reversed depth
  preserves distant depth precision when zooming close to the ground.
- **Retry tiles:** retry failed loads; the status row shows loading and failures.
- **Checkerboard:** switch back to the offline rendering fixture and stop further
  map demand. Already-running requests can finish within their timeout.
- Dock, maximize, close and restore panes as before. Attribution links remain
  visible below the workspace.

## Implementation progress

`src/mapping/tiles.rs` owns two bounded I/O workers, canonical XYZ request
identity, shared demand, decoded CPU data and retained image registrations.
Visible tiles and adjacent zoom levels are requested. Current zoom and its
coarser fallback get priority; tiles are then prioritized from the focus point outward,
with separate map/elevation
workers. Shared requests use the highest priority assigned by either pane. Queue
selection checks the latest demand before starting another request. Identical
requests from both panes share one result; stale completions can populate the
bounded cache but do not select an old view. Failed requests remain explicit
until Retry. Rate-limit/service-unavailable responses pause that host for a
minute. Closing the demo wakes workers to stop; in-flight blocking calls have
20-second timeouts.

Encoded data is cached under `target/map-preview-cache` by default. Set
`ENLUMI_MAP_CACHE` to select another directory. The cache retains validators,
uses conditional requests for expired tiles, reads response max-age, and uses
seven days when cache freshness cannot be read. Expired cached images display
immediately while a worker refreshes them. Writes use temporary files
and atomic replacement. CPU tile data has a 384 MiB budget; new disk entries
stop at approximately 2 GiB instead of silently accumulating forever. A full
disk cache still allows downloaded tiles to render in memory; existing offline
tiles remain saved. Cache size is scanned once per directory and tracked during writes.
This cache supports offline revisits of viewed coverage, not offline region
downloads. The OpenStreetMap endpoint's usage policy applies. [Policy][osm]

For deterministic local data, set `ENLUMI_MAP_LOCAL` to a directory containing
`osm/{z}/{x}/{y}.png` (256 pixels) and
`mapterhorn/{z}/{x}/{y}.webp` (512-pixel Terrarium). Local mode makes no HTTP
requests, and missing tiles are reported. The map client verifies HTTPS with
native system trust on Windows/macOS and Rustls on other targets.

The initial raster route remains retained image elements. Pan preserves exact
zoom so floating-point range reconstruction cannot accidentally switch tile
levels. The 2D raster tile set is bounded to 64 visible tiles per zoom level
and source zoom 19. A cached coarser layer stays beneath detailed tiles while
they load.

`terrain.rs`, `terrain_gpu.rs` and `terrain.wgsl` implement the 3D experiment:
raw Terrarium decoding, local camera-relative coordinates with latitude-adjusted
ground scale, an orbit camera, a depth buffer, textured terrain, slope lighting
and edge skirts. Terrain demand caps at zoom 14; higher map zooms sample the
correct subregion of the parent height tile. A view-frustum quadtree selects
up to 128 nonoverlapping tiles, refining nearby terrain according to projected
size and retaining coarser coverage toward the far plane. The selector remembers
its splits across frames: tiles split above 320 projected pixels and merge below
160 pixels. This deadband prevents small movements from repeatedly changing LOD.
3D camera zoom reaches 22; imagery requests stop at z19 and elevation at z14.
Closer views enlarge those sources without requesting nonexistent detail.
Cached detailed raster descendants refine coarse terrain meshes, including tiles
previously loaded by 2D. Coverage is partitioned into disjoint cells; coarse
parents fill only uncovered regions during pan and orbit changes. Transitions can draw up to 256 tiles. Cached ancestors cover newly
exposed terrain without extra parent requests. Culling conservatively allows
elevation variation around the focus plane. Each selected tile still
uses an indexed 48-by-48 mesh with shared surface and skirt vertices; this is tile-level LOD, without shared-edge
tessellation or geomorphing. Missing elevation tiles remain
missing. Raw DEM bytes never pass through zgui's display-image decoder.
[Source and encoding][terrain]

Terrain textures and material bindings use a 384 MiB GPU LRU cache, including
recently hidden coverage. CPU residency is also byte-bounded, with up to 1,024
entries instead of evicting at 128 entries. These are demo tuning limits, not
firmware memory targets; active tiles are protected, so budgets are soft if the
active working set itself exceeds them. GPU accounting covers tile texture data,
excluding render targets, alignment and driver overhead. Only changed texture
bindings are rebuilt. Orbiting updates uniforms and may request/upload newly
revealed coverage; returning to resident coverage does neither. Ctrl-drag anchors
a cursor ray to the focus-height plane, preserving zoom and orbit; dragging
onto sky or beyond the far plane has no ground anchor. The GPU producer sleeps
when idle, coalesces pending input and renders at most one frame in flight.
Completed camera frames can be presented while newer input is queued, avoiding
starvation during continuous dragging. Device/viewport changes reject obsolete
output. Output textures are capped at four million pixels and are fresh for
each changed frame, so the UI never samples an image being overwritten.
Hidden content unmounts while root camera/orbit settings survive.

This is a local terrain patch, not a globe or a flight viewer. The vertical
origin follows a bilinearly sampled DEM height near the camera center. Its
height source remains stable across orbit-driven LOD changes; zooming in can
upgrade that source. No track-height datum
conversion is implemented. Native performance, neighboring DEM seams, continuous mesh
LOD transitions, raster parent fallback, persistent settings, exports and log overlays are
still work before normal workspace integration.

## Verification

Run `cargo test --locked --workspace mapping::`. GPU tests use actual offscreen
rendering and report a skip only if device discovery fails. Normal tests make
no network requests. They generate local PNG maps and numeric Terrarium tiles,
verify decoding/projection, center-first scheduling, frustum tile budgets,
nonoverlapping LOD, parent/child handover, stable bilinear focus height,
ray-anchored panning and corrected orbit directions; check composed 2D/3D pixels,
verify idle behavior and texture reuse, and exercise pane cleanup. The warm
orbit round trip asserts zero tile loads (including disk reads/decodes) and zero
GPU texture uploads after both views are resident. The local fixture run used
47 MiB of GPU tile textures; this is a residency measurement, not a native
frame-time benchmark. The original
checkerboard tests also cover 1x/2x DPI and retained nodes during pan.

Optional capture variables:

- `ENLUMI_MAP_PREVIEW=target/map-preview.ppm` captures the checkerboard test.
- `ENLUMI_MAP_TERRAIN_PREVIEW=target/map-terrain.ppm` captures the terrain test.
- `ENLUMI_MAP_LIVE=1` opts the terrain test into a single fixed live Matterhorn
  view. Run its exact test by name; it returns before automated orbit/hide
  interactions in live mode. This is a manual provider check, not a CI test
  or a background downloader.

Native Windows/Linux/macOS and physical device-removal checks remain separate
from offscreen coverage. Fixture provenance is in `assets/map-checkerboard.md`;
the terrain test generates its synthetic cartographic grid and hills locally.

**Open framework issue:** fault injection restores the embedded GPU producer,
but the pinned zgui offscreen recovery path leaves existing text/image atlas
entries referring to the old device. The capture loses cached text and tiles;
waiting for decoding does not repair it. The integration test explicitly calls
`Window::forget_caches()` after the simulated recovery, which restores the
atlas and allows subsequent DPI/image checks. This is a host reset, not proof
of automatic whole-window recovery. Before closing packet 0, update
`zgui-runtime/src/window/frame.rs` so `FrameOutcome::Recovered` invalidates
paint/content caches and requests a full rebuild, rather than only calling
`renote_chunks`. Add an upstream whole-window text/image recovery test. No
automatic recovery fix is hidden in Enlumi or in an unpublished dependency pin.

The normal 2D flight-map integration now preserves geographic samples,
timestamps, receiver identity and gaps alongside retained Mercator rendering.
Next packets are altitude/source-datum mapping for airborne 3D flight paths,
Blackbox GPS/home ingestion and provider configuration.

## Recommended approach

Add map panes to Enlumi's existing workspace. Reuse log decoding, docking,
the timeline, background preparation and retained track rendering. Ship a
useful 2D map before building a vector-map engine or terrain streamer.

The first release should show ULog/tlog tracks over raster tiles, link a marker
to the existing timeline, and reopen cached coverage offline. Start with a
local XYZ directory for deterministic development. An online raster provider
is explicit and configurable. OpenFreeMap is the preferred eventual keyless
**vector** source; it is not a raster endpoint.

Add 3D in two steps: an orbitable local flight path over a reference plane,
then terrain with an explicit height reference. Keep geospatial processing in
Rust without a GDAL, PROJ, browser or C++ map-engine requirement. This does not
claim the entire existing desktop dependency tree has no native libraries.
Keep Enlumi's AGPL-3.0-or-later license and existing Cargo workspace.

## What already exists

| Area | Existing code | Remaining work |
| --- | --- | --- |
| Log readers | `src/log.rs`, `src/log/tlog.rs`, `src/log/blackbox.rs` | Geographic source metadata; Blackbox GPS ingestion. Reuse the ULog/tlog readers. |
| GPS XY tracks | `src/trajectory.rs`: `gps_pair`, alignment, fix checks, gaps, local East/North coordinates and retained vertices | Preserve geographic samples and heights before projection; add Mercator and 3D products. Local XY is not a basemap projection. |
| Timeline | `src/timeline.rs`, `Workbench::timeline`, `Track::at` | Geographic lookup and map picking that returns a time. |
| Workspace | `src/state.rs`, `src/panel.rs`, `src/workspace.rs` | Map pane/spec and lifecycle integration. |
| Saved views | `src/preset.rs`, capture/apply, profile/workspace modules | Optional map settings, validation and round-trip tests. Existing schemas reject unknown fields. |
| Plot shell | zgui-plot `PlotSurface`, arbitrary XY rendering and child layers | Map camera/input policy and tile presentation; no new docking system. |
| GPU embedding | zgui's `surface` re-export: `SurfaceHandle`, `SurfaceRenderer`, `GpuShare` | Prove rendering into a docked pane with the shared device. No second window/device is required initially. |
| Tiles and terrain | `src/mapping/` raster tile service, local XYZ, camera and terrain prototype | Loading, caching, geographic metadata, map presentation and later terrain preparation. |

The foundation branch pins zgui `41324c95` from
`Quick-Flash/zgui:enlumi/all-fixes-pr-21` and plot `55378525` from
`Quick-Flash/zgui-plot:master`, including the mode-band improvements.
`Cargo.lock` records this pair; sibling checkouts are not required.
The two repositories have different commits; their resolved **framework**
revision must match, not their Git SHAs.

## Rendering decisions to prove first

### 2D map surface

Use one authoritative camera: Mercator center and uniform pixel scale. Derive
ranges from the camera and measured data-area size. Compose tiles below the
track and marker, with attribution and controls above. Hide Cartesian grid
and ticks by default; add coordinate readout and a latitude-aware scale bar.

zgui already has an `image` element and GPU surfaces. `CanvasScene` is a
shape/series API, so it does not by itself establish an image-tile API. First
try a small retained image-element layer in `PlotSurface`'s child stack.
Verify decoded-image loading, clipping, transforms, DPI and disposal. Tiles
and track must use the same camera. Change tile membership only when the
visible tile set changes, not on every pointer event.

If that path cannot place/cache/crop images efficiently, choose an embedded
GPU surface for textured quads or add a minimal generic image/UV API to zgui.
The spike must select one working route. Keep tile providers out of zgui.

Uniform scale must hold for wheel, box zoom, fit, resize and restoration.
`lock_selection_aspect` alone does not cover all of these. Reuse plot callbacks
and constrain ranges before publishing them through
`SurfaceInteraction::on_update`. Avoid a second reactive effect repeatedly
correcting an already-published viewport. Add generic controller options to
zgui-plot only where existing hooks are insufficient.

### 3D GPU integration

The inspected zgui revision already exposes `zgui::surface`. `GpuShare` supplies
the device/queue. `SurfaceHandle` reports attachment, resize, visibility, device
loss and detachment; `present` hands over a texture and wakes the UI.

Prefer a stable surface handle and a producer driven by camera, track and tile
changes. Forward lifecycle events; do not draw or block inside their callback.
Coalesce requests, pause hidden panes and submit nothing when idle. Use zgui's
re-exported `wgpu` so textures use the same device and API version. Do not read
frames back to CPU just to display them. Follow the handle's texture-reuse
contract with a small pool and submission completion tracking.

The callback `SurfaceRenderer` also exists, but its documented redraw triggers
are initial draw, resize, device loss and requested animation frames. Verify
explicit invalidation for camera/tile changes before selecting that route.
An always-running animation loop is not the solution. Device generation changes
must recreate GPU resources; pane detachment releases them.

## Geographic track contract

Prepare a geographic track from existing numeric columns, separately from
`trajectory::Track`, which currently stores projected local XY points.

| Record | Information to retain |
| --- | --- |
| Edit coordinates | Log/flight, vehicle/system, component, topic/instance, raw/fused role, field IDs, scale/units, time domain and validity rules |
| Geographic sample | Log-relative time, f64 latitude/longitude, optional height with reference, horizontal/vertical quality and segment identity |
| Height reference | WGS84 ellipsoid; named orthometric/geoid datum; MSL with unspecified model; relative-to-home with a documented origin; or unknown |
| Prepared track | Shared samples, valid segment ranges, time index, source revision and cached projection/LOD products |

Bind coordinates, validity and height from the same source instance. Never
silently combine different GPS receivers or vehicles. Suggest fused position
when available, with explicit selection of raw GNSS or user/profile mappings.
A field's display name is not enough to infer its semantics.

Reuse generation/cancellation checks and workers. Preserve recording order,
loops, gaps and original time/sample indices. Do not sort tracks by longitude
or treat them as monotonic-X time series. Keep the raw time index even when
rendering simplified geometry; picking must resolve back to a real segment.

### Format-specific requirements

| Format | First implementation | Constraints |
| --- | --- | --- |
| ULog | First 2D release | Existing pairing covers older scaled GNSS fields and newer degree fields. Validate the actual schema. Fused global position has separate MSL/ellipsoid heights and validity/reset fields. [PX4][px4] |
| tlog | First 2D release | Reuse MAVLink source identities and normalized ground-station record time. Global coordinates use degE7; global/relative height use millimetres. Keep raw/fused position and receiver selection explicit. [MAVLink][mavlink] |
| BBL | Separate follow-up | `read_blackbox` currently retains I/P frames and discards GPS/home records. Audit decoder G/H output, predictors, units and clocks, then expose independent GPS columns. Do not expand every fix to main-loop frequency. |

Do not switch one tlog channel to payload `time_usec` or `time_boot_ms`: these
can be different clock domains. Sensor-time alignment needs an explicit mapping
and reboot handling, consistent with the existing plots.

Keep horizontal and vertical validity separate: a usable 2D fix need not have
usable height. Handle protocol-specific unknown values and invalid flags;
`(0, 0)` alone is not an invalid coordinate. Split tracks at outages, stale
quality, time gaps and applicable reset events. Interpolate markers only inside
valid segments and bounded intervals; do not bridge gaps or extrapolate forever.
Quality expiry must reflect its source's update rate, which may be slower than
position updates.

## Projection and height rules

Implement the supported math in a small tested module. Keep coordinates, time
and transforms in f64; subtract a local/render origin before GPU f32 conversion.
For Web Mercator, with WGS84 radius 6,378,137 m:

```text
x = R * longitude_radians
y = R * asinh(tan(latitude_radians))
```

Clamp the display projection to approximately +/-85.05112878 degrees. Preserve
the original sample and show when it cannot be displayed. Test the inverse.
Mercator metres are projected units, not uniform ground distance; use
latitude-aware/geodesic math for scale bars and measurements.

Wrap tile X modulo `2^z`, clamp Y and bound zoom before shifts. Unwrap track
longitudes by segment at the antimeridian, and render the correct world copy.
MVT tile Y points south; plot Y points north. Tile pixel size (256/512), tile
numbering and vector extent are different concepts. Remove the draft's
"0.6 m precision at z14" accuracy claim: overzoom and coordinate quantization
are not ground accuracy.

For 3D use WGS84 geographic-to-ECEF-to-local-ENU, with Z up. Track and terrain
share an origin, axis convention and compatible height reference. Do not use
Mercator metres as local 3D ground metres. Start with a bounded flight area;
globe rendering and very long routes need a separate precision/curvature design.

For a known datum/model, ellipsoid height `h`, orthometric height `H` and geoid
undulation `N` satisfy `h = H + N`. An MSL field does not automatically identify
the geoid. Prefer a compatible logged height; otherwise use a tested conversion
grid and explicit source references. Mapterhorn's upstream
[vertical-reference issue][vertical] means we must verify regional metadata,
not label every tile EGM96 or EGM2008 by assumption.

Unknown height remains unknown. Show 2D or a clearly relative 3D path without
claiming absolute terrain registration. A user-entered display offset can help
visual alignment, but "snap first point to ground" is not a geoid conversion:
a log may start airborne and a constant offset cannot fix a varying datum.

## Modules, tile service and cache

Start inside `src/mapping/`, not six new workspace crates. Extract a library
when there is a second consumer.

| Proposed modules | Responsibility |
| --- | --- |
| `geo`, `sources`, `track` | Projection, bindings, validity, geographic samples and time lookup |
| `state`, `pane`, `camera` | Durable settings, lifecycle, camera and selection |
| `tiles`, `cache` | Source metadata, bounded I/O, request deduplication and caches |
| `render2d` | Tile presentation and retained geographic overlays |
| `terrain`, `render3d` | Later: DEM decoding, meshes, orbit camera and GPU producer |

A source descriptor needs more than `url(z,x,y)`: stable ID, content/version
identity, raster/vector/elevation kind, TileJSON, XYZ/TMS addressing, pixel size,
zoom/coverage bounds, attribution/license links, known height reference,
cache policy and offline-extraction permission. HTTP, local XYZ and PMTiles
are backends behind that descriptor.

Visible panes subscribe to one shared tile service. Deduplicate source-version,
kind and coordinate requests. Separate encoded-byte, decoded-content and GPU
caches; rendered vector products also depend on style revision and scale.
Device loss invalidates GPU resources, not disk data or the geographic track.

Reuse existing `ureq`/rustls and worker infrastructure. Blocking HTTP belongs on
bounded I/O workers, not the UI thread or the plot geometry pool. Validate
status, encoded/decompressed size, dimensions and archive offsets. Honor cache
validators/expiry, write disk entries atomically, and back off on 429/outages.
Negative-cache missing coverage with an expiry instead of retrying every frame.

Proposed initial limits, to measure: four in-flight requests per host (or its
lower policy limit), eight total, 128 queued requests, 128 MiB decoded tiles,
256 MiB GPU map/terrain resources and a configurable 2 GiB disk cache. Budget
bytes, not only tile counts. Hidden panes drop their subscriptions; shared
requests can continue for other visible panes.

While a tile is missing, crop the correct UV quadrant of a cached ancestor.
Stretching an entire parent into each child repeats the map incorrectly.
Do not request nonexistent overzoom levels. Offline mode must make no network
requests, including TileJSON, fonts, sprites or terrain. A few cached views are
not a complete region pack: packs need a manifest and coverage/completion checks.

### Providers and offline data

| Source | Decision |
| --- | --- |
| Local XYZ images | First rendering/cache fixture; user supplies content and attribution metadata. |
| OSM standard raster | Optional online smoke test. Attribution, Enlumi User-Agent and HTTP caching are required; no bulk/prefetch or offline-pack feature for this endpoint. [Policy][osm] |
| OpenFreeMap | Preferred eventual keyless vector source. Load TileJSON/style resources and retain version/cache metadata. It cannot be substituted for a PNG endpoint. [Service][ofm] [Integration][ofm-start] |
| Local raster PMTiles | First portable offline archive. Check tile type/compression: PMTiles is a container, and a vector archive still needs a vector renderer. [Format][pmtiles] |
| Mapterhorn | Terrain candidate: 512-pixel Terrarium RGB WebP, TileJSON and PMTiles extracts. Check coverage and datum metadata. [Access][terrain] [Attribution][terrain-attribution] |
| Imagery / aviation charts | Later, provider-specific work. Verify current offline/export and commercial-use terms per layer; do not promise a universal keyless satellite layer. |

Show attribution in the pane and exports. Region packages must retain data,
style, font and sprite attribution/license metadata. Online fetches request
map tiles; they do not upload flight logs. Keep providers configurable instead
of hard-coding dated imagery URLs.

## Vector maps: reuse before writing an engine

An MVT decoder plus `lyon` is not a renderer for OpenFreeMap's style. Usable
vector maps need layer order/filtering, zoom expressions, polygon holes,
clipping/seams, road styling, glyph/sprite resources, label placement and
collision handling. A UI text shaper does not solve cartographic placement.
A small custom unlabeled style is a limited experiment, not Liberty-style
compatibility. [Style specification][style]

Evaluate a Rust engine in a bounded spike before building those systems.
Galileo is a candidate, not a selected dependency. Its inspected upstream
`main` uses wgpu 30 while Enlumi's inspected zgui line uses 29; released versions
need separate checking. It also brings runtime/font dependencies and windowing
defaults requiring an audit against our constraints. Require shared-device
rendering, controlled scheduling, usable labels/style coverage and cleanup
before adopting a pinned revision or a small compatibility patch. Do not
upgrade Enlumi's graphics stack merely to run an example. [Manifest][galileo]
[GPU API][galileo-render] [Features][galileo-features]

Walkers is an egui widget: use it as a licensed reference, not a drop-in zgui
component. maplibre-rs labels itself experimental and documents missing features;
it is not equivalent to MapLibre Native. If no candidate passes, retain raster
maps and explicitly scope a vector-style subset. [Walkers][walkers]
[maplibre-rs][maplibre-rs]

## Terrain and 3D work

Share the geographic track, selected source, tile service and timeline with
2D. Keep the 3D orbit camera separate from 2D camera state. First render a
simple vehicle marker and time-preserving ribbon over a reference plane.
Aircraft models, globe navigation, buildings and photogrammetry come later.

DEM decoding is numeric processing, not display-image loading:

```text
height_metres = R_byte * 256 + G_byte + B_byte / 256 - 32768
```

Do not use zgui's display-image decoder for heights: it can premultiply and
resize pixels. Preserve numeric channels, decode heights first, then interpolate
heights. Distinguish Terrarium from Mapbox Terrain-RGB and honor dataset no-data
rules. Verify decoder output against known values. [Encoding][terrarium]

Convert tile-grid positions to longitude/latitude and place terrain in the
same local ENU frame as the track. Start with shaded terrain. Texture draping
later needs raster imagery or a rasterized vector basemap; MVT bytes are not
an image. Use neighbor samples for normals and consistent edges, skirts or
stitching for LOD transitions, projected-error LOD selection and parent meshes
until children are ready. Bound visible meshes, uploads/frame and submissions.
Test depth precision and track occlusion. Tile zoom/sample spacing does not
establish vertical accuracy or obstacle clearance.

## Enlumi and upstream integration

Add optional `MapSpec` and prepared-track state to `Pane`, using the durable
owner and visibility rules. Pane actions must prevent map, analysis and XY
modes being active simultaneously. Add `MapPane` routing in `panel.rs` and
reuse docking and existing settings conventions.

Persist bindings, provider IDs, 2D/3D cameras, mode, track style, height choices
and explicit offsets. Do not serialize tiles, buffers, credentials or
machine-specific cache paths. Add optional serde fields and round-trip coverage
for capture/apply, profiles, duplication, save/load and log replacement.

The existing timeline remains the time authority. Scrubbing updates a small
marker overlay, not track geometry or tile demand. Map picking needs a spatial
segment index and returns time within a valid segment. At crossings, choose
deterministically, for example the hit closest to current timeline time.
Do not join Mercator maps to the existing local-XY spatial range group simply
because both have metre axes. Time linking is immediate; map-camera linking
is a separate option.

Default export decision: PNG includes the displayed map, track, marker and
attribution. Offer a track-only/vector export. Initially SVG embeds a raster
basemap beneath vector overlays; 3D exports are raster captures. GPU surfaces
are not automatically captured by `CanvasScene` exports, so add a map snapshot
adapter and make incomplete/unsupported export explicit.

| Repository | Expected work |
| --- | --- |
| Enlumi | Sources/tracks, tile service, provider policy, map/3D state, time picking, pane/preset integration, attribution and export. Most work is here. |
| zgui-plot | Reuse `PlotSurface` and child layers. Add only missing generic camera/gesture constraints or layer/export hooks demonstrated by the spike. |
| zgui | Use existing image/surface support. Add stable image/UV handles or callback invalidation only if a demonstrated gap requires it; test lifetime, clipping and damage upstream. |

## Work packets and acceptance gates

| Order | Deliverable | Acceptance |
| --- | --- | --- |
| 0 | Lock dependencies; checkerboard image and GPU-surface spikes in a pane | Clipping/order at two DPI scales, resize/hide/reopen, no idle producer loop, device-loss recovery. Select one tile presentation route. |
| 1 | Geographic sources/tracks and math | ULog/tlog bindings preserve identity, time, validity, altitude reference and gaps; projection/antimeridian fixtures pass without network. |
| 2 | Map pane with debug tile grid | Uniform pan/zoom/fit/resize, retained track, timeline marker, click-to-time and world wrapping. |
| 3 | Local XYZ tiles and shared bounded cache | Landmark alignment, parent UV fallback and stale completion rejection after log/pane replacement. |
| 4 | Optional online raster, cached offline reopening, presets and PNG | Provider controls/attribution; network-disabled reopening; missing tiles explicit; exports contain basemap. First usable release. |
| 5 | Local raster PMTiles and region manifests | Correct type/compression/offsets, interrupted-pack handling and coverage checks. |
| 6 | Rust vector-engine experiment, then OpenFreeMap if viable | Shared-device compatibility, representative labels/styles, bounded idle/pan cost; otherwise retain raster and document the limited alternative. |
| 7 | Flat 3D flight pane | Same timeline/track, stable orbit, correct axes/heights and GPU lifecycle. Can follow packet 4 independently of vector maps. |
| 8 | Mapterhorn DEM/mesh pipeline | Raw-height fixtures, declared datum behavior, seams/LOD/fallback and bounded resources; unknown heights stay unknown. |
| 9 | BBL GPS; further formats/providers | Decoder G/H predictor/time fixtures and real GPS-equipped logs; imagery, charts and other formats are separate follow-ups. |

Each packet should be a reviewable change with a visible result or tested
contract. Do not start terrain streaming before proving surface integration.

## Verification and performance

Use generated local tiles, synthetic tracks/terrain and source-attributed
fixtures. Keep private logs outside Git and remote providers out of CI.
Test projection inverse/clamping, antimeridian/tile edges, XYZ/TMS and 256/512
sizes, fix loss, gaps, resets, different clocks, missing height, known ENU
points, datum conversion, parent crops, cache expiry/offline/errors, bounded
queues and stale completion rejection.

Headless tests cover docking, hidden panes, map/plot time linking, loop/gap
picking, log replacement, saved state and attribution. Offscreen GPU tests
compare known tile/track/terrain placement through overlap and resize. Check
native integration on Windows, Linux and macOS separately.

Add deterministic pan/zoom, scrub and orbit replays with cold/warm caches
separate. Record CPU p50/p95, GPU completion wait separately, uploads, draws,
resident bytes, queue depth and frame requests. Require retained allocations
during nearby pans, no track rebuild/tile fetch for marker-only movement,
bounded caches and no map GPU submissions when idle/hidden. The existing FPS
heartbeat is not a reason to render continuously. Derive frame-time budgets
from these replays rather than promise a frame rate now.

## Sources checked

Resolve exact library revisions during implementation; upstream branches and
provider policies change. Sources support the facts above, not a claim that
these unbuilt integrations have already passed their acceptance gates.

[ofm]: https://openfreemap.org/
[ofm-start]: https://openfreemap.org/quick_start/
[osm]: https://operations.osmfoundation.org/policies/tiles/
[pmtiles]: https://docs.protomaps.com/pmtiles/
[terrain]: https://mapterhorn.com/data-access/
[terrain-attribution]: https://mapterhorn.com/attribution/
[vertical]: https://github.com/mapterhorn/mapterhorn/issues/255
[terrarium]: https://github.com/tilezen/joerd/blob/master/docs/formats.md
[px4]: https://docs.px4.io/main/en/msg_docs/VehicleGlobalPosition
[mavlink]: https://mavlink.io/en/messages/common.html
[style]: https://maplibre.org/maplibre-style-spec/
[galileo]: https://github.com/galileo-map/galileo/blob/main/Cargo.toml
[galileo-render]: https://github.com/galileo-map/galileo/blob/main/galileo/src/render/wgpu/mod.rs
[galileo-features]: https://github.com/galileo-map/galileo/blob/main/galileo/Cargo.toml
[walkers]: https://github.com/podusowski/walkers
[maplibre-rs]: https://github.com/maplibre/maplibre-rs
