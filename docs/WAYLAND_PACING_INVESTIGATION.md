# Native Wayland presentation

Tested native Wayland candidate prepares frames independently of compositor callbacks and
uses a bounded FIFO presentation worker. V-sync belongs to the graphics driver;
no fixed 60 Hz timer or second application clock controls presentation.

## Cause and fix

Framework revision `41324c958d52833127d779b1126654c10b80931b` withheld redraws
while the previous commit's compositor callback was outstanding. This serialized
CPU preparation with display waiting. Winit deliberately avoided that gate.
Removing it recovered display throughput, but synchronous FIFO presentation
blocked the UI for 1,001 ms when minimized.

The fix keeps scene preparation on the UI thread and moves swapchain configure,
acquisition, final blit and presentation to one worker per native window. One
pending composition bounds work and preserves GPU queue ordering: the composed
texture cannot be overwritten until its previous blit has been submitted.
Backpressure retains damage and waits for a worker wake rather than polling.

A dedicated Wayland callback queue waits for compositor readiness outside wgpu
locks. Moving only `present()` to a thread was insufficient: the driver can hold
shared device locks while waiting, blocking UI GPU submissions too. Cancellation
ends callback waits; window shutdown never joins a blocked display-driver call.

Resize generations discard stale frames. Buffer scale, viewport destination and
window geometry are applied for the exact presented buffer. Control commits are
deferred during that transaction. Stale callbacks cannot release a new mapping.
Other platform backends keep their existing presentation path.

Framework commit: `3a71f689db1132f78923f541ab86438fc11c6975`.
Normal candidate executable (without replay feature):
`target/native-backend-comparison/enlumi-wayland-fifo`.
Rebuilding with the unchanged dependency pin will use the original framework.

Remote push was rejected by automatic approval review pending explicit user
permission. Enlumi's committed dependency pins remain unchanged until publication.
The measurements below use the local candidate, not the original dependency pin.

## Reproduction

```sh
cargo build --locked --release --features backend-replay
ZGUI_PLATFORM=wayland ENLUMI_REPLAY_LOG="$HOME/Downloads/btfl_001.bbl" \
  ENLUMI_PERF_LOG=native.csv ./target/release/enlumi
ZGUI_PLATFORM=winit ENLUMI_REPLAY_LOG="$HOME/Downloads/btfl_001.bbl" \
  ENLUMI_PERF_LOG=winit.csv ./target/release/enlumi
```

Commands exercise the framework revision currently pinned in `Cargo.toml`.
Replay opens four linked plots with ten raw eRPM, gyroADC and gyroUnfilt signals.
First 15 seconds allow placement on the intended display. Pointer movement runs
at seconds 15–25, primary-button pan at 25–35, then the window exits. Events go
directly to this application's handler; replay does not inject desktop input.
Normal builds omit this entry point.

Optional checks:

- `ENLUMI_REPLAY_MINIMIZE=1`: minimize at second 28.
- `ENLUMI_REPLAY_RESTORE=1`: request focus/restore at second 30.
- `ENLUMI_REPLAY_STRESS=1`: resize five times per second at seconds 16–24,
  enter fullscreen at 25 and leave at 27.

For physical display counts, run with `WAYLAND_DEBUG=client` and count
`wp_presentation_feedback.presented`, using compositor timestamps. CSV
`presented` means queued renderer output on the asynchronous path; UI FPS also
includes undamaged frames. Neither alone proves actual screen delivery.

## Measurements, 2026-11-03

COSMIC Wayland, HP X32 DP-3, 2560 × 1440, scale 1, 164.834 Hz.
Same recording, raw plots and release profile; no compilation during replay.
Protocol logging and CSV recording enabled. Warmup excluded; pan measured over
seconds 26–34.

| Backend | Actual displayed pan FPS |
| --- | ---: |
| Native, original callback gate | 102.9 |
| Winit, original framework | 163.5 |
| Native, bounded FIFO worker | 162.1 |

Final native pan processing p95: 4.212 ms. Native hover delivered 161.8 FPS.
Resize/fullscreen/minimize/restore replay exited cleanly, restored successfully
and reported worst UI timer delay of 24.964 ms after second 27. Earlier
minimize/restore-only replay measured 9.381 ms. These replace the measured
1,001 ms synchronous FIFO stall; timer delays include scheduling and UI work.

A display-switch replay exited cleanly with a 6.130 ms worst late timer, but
protocol feedback stayed at 164.834 Hz. That run does **not** establish 60↔165 Hz
switching; a verified mixed-refresh move remains outstanding.

Framework validation: 193 native Wayland, 36 render-contract and 173 wgpu unit
tests pass with desktop GPU access. Strict Clippy passes for those three crates.
Enlumi regression suite: 246 passed, 12 ignored.
Regression coverage includes pending-damage retention, bounded composition,
resize generations, cancellation, stale callbacks and deferred control commits.

Local executables, CSVs and protocol logs live under
`target/native-backend-comparison/` and are excluded from Git.

## V-sync and variable refresh

FIFO synchronizes buffer delivery to vertical blank and works with either fixed
or variable refresh. No Mesa override is needed. See
[Vulkan WSI](https://docs.vulkan.org/spec/latest/chapters/VK_KHR_surface/wsi.html).

DP-3 advertised adaptive sync and was configured `automatic` during testing.
That does not prove windowed or fullscreen VRR activation. G-Sync/FreeSync
activation remains a compositor, driver and display decision; this fix removes
the application's extra pacing gate without claiming VRR is active.
