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
time-range highlight and Show path trail (2D, 3D, profile), flight-mode markers
(labels in 2D, colored dots in 3D), legend quick settings, Field color mode with
Auto limits, trace-card setup panel with unit badge, start location, sibling
autofill, and dropping a field onto a map to add its trace.

## Known gaps / differences from the mockup

- No NED/ENU switch: slots read East (X) / North (Y); autofill maps NED; Swap covers the rest.
- 3D mode markers have no text labels; 3D measure shows the readout but no line.
- Highlight uses the most recently zoomed/panned time plot, not all linked plots.
- One color field per map. Profile has no terrain fill (2D loads no elevation data).
- Background picker is now a modal dialog; its perf test allows the scrim/shadow cost.
- Verified with tests and headless renders only, not yet by hand in the app.

## Visual check

    ENLUMI_MAP_CONTROLS_PREVIEW=out.ppm cargo test -- --ignored map_controls_preview

writes `out.ppm` (map, profile, measurement, setup panel) and `out.layers.ppm`.
