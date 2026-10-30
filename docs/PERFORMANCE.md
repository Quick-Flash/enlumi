# Measuring performance

Portable bundles use `cargo build --locked --release` on every platform. For a
local comparison, build with `cargo xtask build-release`; use the same log,
workspace, window size, display scale, and monitor refresh rate for each run.

The FPS readout continuously samples the UI frame cadence and updates every
250 ms. This includes undamaged frames: sampling keeps a heartbeat running but
does not force plot geometry or GPU contents to repaint. The CSV separately
records whether each frame was actually presented. Measure responsiveness while
panning, zooming, or dragging a marker.

## Capture frame costs

Set `ENLUMI_PERF_LOG` to a CSV path before starting the release executable:

```sh
ENLUMI_PERF_LOG=frames.csv ./target/release/enlumi /path/to/log.ulg
cargo xtask perf-summary frames.csv
```

On PowerShell:

```powershell
$env:ENLUMI_PERF_LOG = 'frames.csv'
.\target\release\enlumi.exe C:\logs\flight.ulg
Remove-Item Env:ENLUMI_PERF_LOG
cargo xtask perf-summary frames.csv
```

Recording is optional and writes after each completed frame. It has some I/O
overhead; leave the variable unset for normal use. The summary reports frame
wall times and paint/submission wall times, including driver waits, plus
restyle counts, draw calls, and GPU upload bytes. It does **not** measure GPU
execution time with timestamp queries. Low frame cost with slow presentation
can point toward pacing/compositor behavior; high paint/submission cost needs
a GPU profiler or driver investigation to distinguish its causes.

Record separate files for these 20–30 second scenarios:

1. One normal plot: idle, pointer movement, then pan/zoom.
2. The same actions with many visible normal plots, then one maximized plot.
3. Visit several views, return to the first, and repeat. Inactive views retain
   their data, layout, and appearance but release their UI renderers.
4. Drag the global timeline, with pointer following disabled and enabled.
5. A dense XY plot, separately with solid lines, dashes, and dots. Raw samples
   remain available for hit testing; display geometry uses screen-space detail.

Compare percentile frame costs and upload volume, not just the average FPS.
Keep loading/filter compilation outside the measured interaction interval.

## Linux backend comparison

Use the same release executable and inputs:

```sh
ZGUI_PLATFORM=wayland ENLUMI_PERF_LOG=wayland.csv ./enlumi flight.ulg
ZGUI_PLATFORM=winit ENLUMI_PERF_LOG=winit.csv ./enlumi flight.ulg
```

The portable backend may still use Wayland; this compares backend implementations,
not necessarily Wayland against X11. Include the GPU, driver, compositor, display
scale, refresh rate, Enlumi revision, and both summaries with a performance report.
For finer event/layout/paint boundaries, the existing framework trace is enabled
with `ZGUI_LATENCY=latency.jsonl`.

Summarize a single-window trace with `cargo xtask perf-trace latency.jsonl`.
It reports mean, p95, maximum, and sample count for input dispatch, reactive
flush, layout, scene traversal/emission, scene finalization, atlas uploads,
renderer table preparation, vector rasterization, draw planning, buffer uploads,
command recording, surface acquisition, final submission, and presentation.
Incomplete frames are excluded; absent stages are omitted rather than counted
as zero. Use a fresh trace file for each recording (the framework appends).

These boundaries measure CPU wall time. The renderer's aggregate final
submission row also has individual rows for final copy recording, command-buffer
finishing, queue submission, retirement tracking, and upload reclamation.
Do not add those individual rows to their aggregate. Surface reconfiguration
and acquisition are separate boundaries. They do not isolate every driver stall or measure
GPU execution. Compare untraced runs too, since tracing adds recording overhead.

## Regression checks

`cargo test --locked --workspace` checks that continuous FPS sampling does not
force continuous full redraws, only the active view has plot renderers, state survives view
switches, and dense XY line/marker payloads are reduced while preserving runs and
viewport crossings. These headless checks establish work bounds and correctness;
they do not establish native Linux GPU frame rates.

The timeline marker retains its empty canvas while it is outside the visible
time range. XY preparation retains the current geometry while replacements run;
an obsolete result is rejected when it cannot cover the newest queued request.
When a completed result already covers a queued nearby pan, that request is
consumed without rebuilding the same geometry again.
Normal solid and patterned lines retain their vertex allocations during nearby
pans. Patterned lines reuse only at the same pixel scale on both axes; zoom,
resize, width changes, and leaving prepared coverage invalidate that geometry.
Completed preparation must cover the current viewport at adequate detail before
it can replace displayed geometry, including when a drag returns to a cached view.

## Replay a local Attitude view

The ignored replay uses a local log and the saved PX4 Flight Review profile.
It prepares the actual Attitude signals, styles, layout, links, and mode bands,
then scripts 60 Hz drag input. It compares full-range/zoomed dragging, linked
and unlinked X bounds, mode bands on/off, and a maximized chart.

```sh
ENLUMI_REPLAY_LOG=/path/to/flight.ulg \
ENLUMI_REPLAY_PROFILE='/path/to/PX4 Flight Review.json' \
ENLUMI_REPLAY_GPU=1 \
cargo test --locked --release replay_attitude_drag -- --ignored --nocapture
```

The default prepares only resources reachable from Attitude. Add
`ENLUMI_REPLAY_FULL=1` to load the entire workspace before replay. Leave
`ENLUMI_REPLAY_GPU` unset to use scene capture instead of GPU rendering.
`ENLUMI_REPLAY_OUT` selects the CSV output path (default
`target/attitude-replay.csv`). No private recordings or profiles are copied into
the repository.

Set `ZGUI_COALESCE_DAMAGE=0` for the control run, then unset it for the optimized
run. Optional `ENLUMI_REPLAY_SCREENSHOTS` selects a directory for final GPU image
readbacks (PPM) so the two runs can be compared for rendering correctness.
Add `ZGUI_LATENCY=target/attitude-stages.jsonl` to capture the detailed stage
trace, then run `cargo xtask perf-trace target/attitude-stages.jsonl`.
Replay begin/end marks separate each measured drag case from warmup frames;
the summary lists initial setup frames separately under `all frames`.

The GPU replay waits for submitted GPU work to complete after each frame and
reports that wait separately from frame processing and paint/submission time.
This is a real offscreen GPU workload, not a native compositor/presentation test
or a GPU timestamp-query measurement. Compare it with native frame recordings
to distinguish application costs from surface acquisition and frame pacing.

### Measured Attitude replay

On Windows/DX12 with an RTX 4070 Laptop GPU, the release replay loaded
`00000-2026-10-16_22-32-14.ulg` (1,907 seconds) and the complete saved PX4 Flight
Review workspace (152 filters, 29 views). Each case used 180 drag steps at
1500 x 850. The same executable ran with damage coalescing disabled and enabled:

| Case | Control frame p95 | Coalesced frame p95 | Mean draw calls, control / coalesced |
| --- | ---: | ---: | ---: |
| Three linked charts, full range | 32.68 ms | 19.98 ms | 580 / 218 |
| Three linked charts, zoomed | 29.26 ms | 19.07 ms | 554 / 211 |
| One unlinked chart, zoomed | 7.00 ms | 7.90 ms | 68 / 66 |
| Three linked charts without mode bands | 27.64 ms | 16.05 ms | 537 / 209 |
| One maximized chart | 10.44 ms | 10.51 ms | 69 / 69 |

Dense damage regions previously replayed the scene's GPU batches repeatedly.
Joining nearby regions reduces that work; isolated chart updates mostly retain
their original behavior. These are individual controlled runs, not statistical
confidence intervals. Frame processing excludes the separately recorded GPU
completion wait and does not predict native Linux presentation FPS.

Final image comparisons across all five cases, excluding the FPS readout,
differed by at most one level per 8-bit color channel; no geometry was missing.
The replay also asserts that dragging preserves the configured time span.

A subsequent traced diagnostic run split the zoomed linked-chart costs further:

| Stage | Mean CPU wall time | p95 |
| --- | ---: | ---: |
| Reactive flush | 3.29 ms | 4.38 ms |
| Scene traversal/emission | 3.20 ms | 4.66 ms |
| Buffer uploads | 0.37 ms | 0.54 ms |
| Command recording | 1.76 ms | 2.72 ms |
| Command-buffer finishing | 4.58 ms | 7.12 ms |
| Queue submission | 0.45 ms | 0.64 ms |

This is a diagnostic stage breakdown, not a controlled before/after speedup
comparison. Other compilation activity was observed on the machine. In this
run, the previously broad submission block was dominated by command-buffer
finishing; attributing its entire cost to queue submission would be incorrect.
The five final images matched the preceding optimized replay exactly outside
the FPS readout. Native compositor timings and GPU execution remain separate.

Independent profile branches use a bounded Rayon pool. Plot geometry prepares
on zgui's worker executor. UI state/layout and ordered render submission retain
serial work, so this work does not guarantee a fixed native presentation rate.

Normal time-series, XY, and heatmap geometry preparation share zgui-plot's preparation
helper on zgui's default pool (at most four workers). Detailed traces separately
report per-job queue delay and computation time for `time-series`, `xy-paths`,
`xy-alignment`, and `heatmap`. Job timing is separate from UI frame costs; completed jobs
are grouped by the replay case active when they finish. A warmup job can finish
during a measured case. Worker marks may interleave with UI marks; frame-stage
ordering is validated within each stage pair.
Queue delay starts when the helper submits the job to the worker executor; it
excludes time held in a chart's replaceable pending slot. Completed job samples
also include invocations that stop after cooperative cancellation.

With the shared helper enabled, the full Attitude replay recorded 11 geometry
jobs during setup/warmups and none during its 900 measured drag frames. All five
final images matched the preceding optimized replay exactly outside the FPS
readout. This verifies steady-pan geometry reuse for that input; it does not
establish native Linux FPS or eliminate the measured command-buffer finishing
cost.

XY alignment keeps one active job and one replaceable pending input snapshot per
chart. Superseded jobs check cancellation before alignment and every 4096 samples;
results are also checked on the UI thread before publication. The displayed
track survives replacement preparation. The controlled-worker regression checks
that intermediate input changes never start additional jobs or replace displayed
geometry. Input-burst regressions verify that 32 queued pointer moves or fractional
wheel events produce one bounds update per linked chart per frame. The wheel
regression exercises normal and XY link groups separately and
checks that all 32 zoom deltas contribute to the final range. The shared plot
surface accumulates wheel factors in a non-reactive pending slot and publishes
them from its frame callback, before the frame's reactive flush; linking then
propagates the final range. Event dispatch can still settle unrelated controls
between events without preparing intermediate plot ranges.

XY path canvases update their view matrix during a nearby pan, preserving the
canvas revision as well as the vertex allocation. Timeline markers use a separate
small scene. The regression covers solid, dashed, dotted, and dash-dot traces.

Heatmaps prepare visible cell tiles on the same bounded workers, with one active
and one replaceable pending request per chart. They retain cell-coordinate paths
while pan, zoom, and resize update one matrix. Missing bins remain transparent;
adjacent equal-color cells merge into runs. Extra cached cells are limited to
262,144; larger visible regions retain all their data without additional padding.
Cancelled or hidden-chart completions cannot replace current geometry. Export
tests compare the rendered PNG against clipped cell references at full range,
pan, zoom, and data boundaries; SVG shape exports honor the canvas view matrix.

Headless app fixtures use an isolated update state so the machine's nightly-update
preferences and asynchronous notices cannot change layout mid-test.
