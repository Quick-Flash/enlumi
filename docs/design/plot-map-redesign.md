# Plot and map redesign — handoff notes

`plot-map-redesign.html` is the interactive mockup (open it in a browser; it
needs Google Fonts online but works without them). It is also published at
https://claude.ai/artifact/VpZJ1XAoMEdyH8siT8o299 (private to the owner's account).

## Decisions agreed with the owner

- Defaults on: 1:1 aspect, direction arrows, altitude profile (2D maps, togglable),
  time-range highlight link.
- Measure (M): 3D distance plus horizontal and vertical parts and bearing;
  same-trace samples add Δt, distance flown and average speed.
- Dropping one field fills x/y/z from the same topic (lat/lon/alt or local x/y/z);
  without an altitude partner only x/y are filled. No cross-topic guessing.
- Time-range highlight is a link in the Links pill and uses time-plot X bounds.
- Setup slots are drag and drop only; no field search dropdown. No GPS fix filter UI.
- Local-metre traces: Start location = Default / From GPS / Click map / lat,lon;
  "first sample" vs "local (0, 0)"; a saved default start.
- Timeline marker is a heading arrow in 2D and 3D. Links pill follows theme radius.

## Implemented (branch `plot-map-redesign`)

Icon pane header with ⋯ menu (Background…, Separate by unit), Links pill, arrows
in X/Y, 2D and 3D maps, X/Y start/end dots and timeline readout, map toolbar
(2D/3D + Layers popover) and navigation cluster, measure tool, altitude profile,
time-range highlight and Show path trail (2D, 3D, profile), state markers
(labels in 2D and 3D), legend quick settings, Field color mode with
Auto limits, trace-card setup panel with unit badge, start location, sibling
autofill, and dropping a field onto a map to add its trace.

3D measurements now draw a line and endpoints through the terrain camera.
Measurements and state labels follow orbit, height exaggeration and
resolved terrain elevation. Unprojectable ends hide without shifting the other
endpoint; the measurement readout stays available.
The HTML's annotation details are retained: a 6/4 px dashed measurement line,
8 px endpoints, live second-point preview, midpoint readout in monospace and
top-centred measuring instructions. Mode labels sit 8 px right and 22 px above
their sample, with 18 px stacking for changes within 20 px. Labels fade with
the highlighted time window. Theme colors remain app-controlled; midpoint
readouts clamp inside the viewport to stay readable near edges.

The map now uses the mockup's compact trace legend (eye, swatch, short name,
color mode and inline color/framing controls), with collapse and Options restored.
Options opens the full line editor for color, width, pattern and sample markers. Layers uses labeled checkboxes and radio choices;
Direction arrows changes both 2D and 3D rendering. Basemap includes None (grid),
with extra providers and passive tile status plus Retry tiles in Layers. Navigation order and
spacing follow the mockup. Hover info is a compact, theme-aware chip with mode,
coordinates, altitude and ground speed. Trace cards use stacked coordinate slots,
NED/ENU presentation, four start-location tabs, a GPS-source choice and inline
color controls. NED/ENU changes slot presentation; stored geometry stays normalized
to east/north. Color Auto shares prepared map data with the setup panel.

Direction arrow color is independent for each line, editable inline or in legend
Options and saved with line appearance. Each line also has a State markers toggle.
Legend rows expose X to remove a trace; an open setup panel stays in sync.
New maps default to Esri World Imagery; saved provider choices stay intact. State markers identify their state-band source. Map M
uses shared pane measurement state in 2D and 3D. Timeline heading follows ascent
and descent, including vertical travel. New panels and controls use theme corner
tokens, including live theme changes.

## Known gaps / differences from the mockup

- Highlight uses the most recently zoomed/panned time plot, not all linked plots.
- One color field per map. Profile has no terrain fill (2D loads no elevation data).
- Background picker is now a modal dialog; its perf test allows the scrim/shadow cost.
- Verified with tests and headless renders only, not yet by hand in the app.

## Visual check

    ENLUMI_MAP_CONTROLS_PREVIEW=out.ppm cargo test -- --ignored map_controls_preview

writes `out.ppm` (map, profile, measurement, setup panel) and `out.layers.ppm`.

    ENLUMI_3D_ANNOTATIONS_PREVIEW=annotations.ppm cargo test spatial_annotations_follow_orbit

writes an offline 3D annotation preview and checks camera updates, clipping,
hidden traces and 2D fallback.
