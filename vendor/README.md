# Temporary UI fixes

Direct framework dependencies remain upstream. Dependency crates
are patched locally until upstream carries these behaviors:

- `zgui-runtime`: zortax/zgui `951a0ea3b113247da6dff439fccbd9097aa3187f`.
  Reveal native field caret after keyboard edits/selection changes and scroll
  horizontally while dragging selection past either edge.
  Enable native text input after layout when a newly mounted field received focus
  before it had bounds (for example, the plot tab rename input on Linux).
  Route external file drops to the element under the drop position rather than
  to the focused element, including when no control has focus.
  Refresh maximized/fullscreen signals on every frame, including compositor
  state changes that retain the same content size.
  Expose completed-frame wall timings and renderer counters to passive frame
  probes independently of Enlumi's continuous FPS sampling.
  Coalesce three or more densely packed dirty regions before paint emission,
  avoiding repeated GPU batch-stream replay during linked-plot dragging. Sparse
  cursor/caret damage stays partial. `ZGUI_COALESCE_DAMAGE=0` disables this policy
  for controlled performance comparisons.
- `zgui-platform-wayland`: zortax/zgui `951a0ea3b113247da6dff439fccbd9097aa3187f`.
  Preserve UTF-8 and Unix filename bytes in file-drop URI decoding, accept local
  `localhost` URIs, reject remote authorities and NULs, and avoid malformed-escape
  panics. Isolate asynchronous reads between drag sessions and retain a dropped
  offer through the subsequent `leave` while its transfer is pending.
  Remember normal window dimensions when maximizing or entering fullscreen so
  restoring works when the compositor leaves its requested size unspecified.
- `zgui-editor`: zortax/zgui-editor `1f7aaec6e4e2dfaee4d26b049a719d9e88d16f8f`.
  Add horizontal selection autoscroll and Shift-wheel scrolling.
- `zgui-render-wgpu`: zortax/zgui `951a0ea3b113247da6dff439fccbd9097aa3187f`.
  Add opt-in latency boundaries around surface reconfiguration, command-buffer
  finishing, queue submission, retirement tracking, and upload reclamation.
  These split the existing aggregate submission measurement without changing
  rendering or introducing a GPU wait. Source, tests, license, and notice are
  copied from that revision; the manifest expands upstream workspace metadata.
- `zgui-plot`: Quick-Flash/zgui-plot `aef14a9ce0c8097f8347239a423c151e1641077c`.
  Share geometry preparation through the existing bounded zgui worker executor,
  with opt-in queue-delay and computation timings for normal and XY plots.
  Reuse patterned time-series geometry during translation at unchanged pixel
  scale, and reject completed preparation outside current coverage/detail.
  Regression tests verify retained vertex identity for solid and patterned pans.
  Honor canvas view transforms on exported SVG shapes, including retained
  heatmap cells during pan/zoom.
  Source, tests, examples, and both upstream licenses are included. The focused
  changes are recorded in `plot-preparation.patch`.

Focused source diffs are in `text-scrolling.patch`, `text-input-focus.patch`,
`file-drop-routing.patch`, `window-state.patch`, `wayland-file-drop.patch`, and
`wayland-window-state.patch`, `frame-reporting.patch`, and `renderer-timing.patch`.
Sources and upstream licenses are included (Apache-2.0 for zgui; MIT/BSD-3-Clause
for zgui-plot). Runtime, renderer, and Wayland manifests are expanded
from upstream workspace metadata; framework dependencies still resolve upstream.
Remove these patch entries and directories after upstream fixes are adopted.

For standalone runtime regression tests, seed its ignored test lockfile from the
application's `Cargo.lock` so tests use the same upstream revisions, then run
`cargo test --offline --manifest-path vendor/zgui-runtime/Cargo.toml --target-dir target --lib`.
The standalone manifest includes the arena dependency used by upstream unit tests.

For plot-library regression tests, likewise seed its ignored lockfile from the
application lockfile, then run `cargo test --offline --manifest-path
vendor/zgui-plot/Cargo.toml --target-dir target --features workspace,raster,clipboard --lib`.
Its standalone workspace uses the same local runtime and renderer patches.
