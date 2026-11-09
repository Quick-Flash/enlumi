# UI fork changes

Enlumi consumes Git branches from the Quick-Flash forks, with exact commits pinned in Cargo.lock. UI source copies
and local path patches are no longer needed. MAVLink XML remains vendored for
reproducible offline code generation.

Each fork has an `enlumi/all-fixes` branch containing its combined changes.
Individual branches below start at the base revision and contain only their
focused changes, so they can be submitted as separate upstream PRs. Aggregate
branches retain the same changes as a commit series. New commits use Kevin
Plaizier's configured Git identity.

## Framework

Fork: [Quick-Flash/zgui](https://github.com/Quick-Flash/zgui).
Consumed branch: [`enlumi/pr-stack`](https://github.com/Quick-Flash/zgui/tree/enlumi/pr-stack),
`dd7fadf2a25f3acb285a0d8d955ac9414ecf811e`. It is
[upstream PR #33](https://github.com/zortax/zgui/pull/33) (`wayland-speedup`, native Wayland
presentation pacing) with the focused branches below applied on top.

Each focused branch starts at upstream `main` (`a73861ea`) and is submitted as its own upstream PR, pushed to the fork as `pr/<name>`.

| Branch | Change |
| --- | --- |
| [#38](https://github.com/zortax/zgui/pull/38) `enlumi/text-scrolling` | Reveal native field caret; autoscroll horizontal selection drags. |
| [#39](https://github.com/zortax/zgui/pull/39) `enlumi/text-input-focus` | Enable native text input after newly focused fields receive layout bounds. |
| [#40](https://github.com/zortax/zgui/pull/40) `enlumi/file-drop-routing` | Route external drops to the element under the pointer without requiring focus. |
| [#41](https://github.com/zortax/zgui/pull/41) `enlumi/wayland-file-drop` | Decode local file URIs safely; preserve filename bytes and isolate asynchronous drag sessions. |
| [#42](https://github.com/zortax/zgui/pull/42) `enlumi/window-state` | Refresh maximized/fullscreen signals even when content size stays unchanged. |
| [#43](https://github.com/zortax/zgui/pull/43) `enlumi/wayland-window-state` | Restore normal dimensions when the compositor supplies no restore size. |
| [#44](https://github.com/zortax/zgui/pull/44) `enlumi/wayland-dnd-actions` | Set the drag action on `wl_data_device.enter`; compositors send `source_actions` first, so file drops were cancelled. |
| [#45](https://github.com/zortax/zgui/pull/45) `enlumi/input-event-timestamp` | Dispatch pointer, wheel and key events with their own timestamp instead of the queue drain time. |
| [#46](https://github.com/zortax/zgui/pull/46) `enlumi/resizable-untracked` | Read resizable divider travel untracked in input handlers. |
| [#47](https://github.com/zortax/zgui/pull/47) `enlumi/toggle-group-untracked` | Read toggle group state untracked in the click handler. |
| [#48](https://github.com/zortax/zgui/pull/48) `enlumi/frame-reporting` | Report completed frame timings to probes (used by `ENLUMI_PERF_LOG`); coalesce dense paint damage. |

The renderer latency marks (`enlumi/renderer-timing`) and the bounded FIFO worker
(`enlumi/wayland-async-fifo`) are no longer consumed; PR #33 replaced the worker.

## Editor

Fork: [Quick-Flash/zgui-editor](https://github.com/Quick-Flash/zgui-editor).
Base: `1f7aaec6e4e2dfaee4d26b049a719d9e88d16f8f`.
Aggregate: [`enlumi/all-fixes`](https://github.com/Quick-Flash/zgui-editor/tree/enlumi/all-fixes),
`a5f84d76cd3f7e28860ea637f1e244501b524844`.

| Branch | Change |
| --- | --- |
| `enlumi/text-scrolling` | Horizontal selection autoscroll and Shift-wheel scrolling. |
| `enlumi/shaped-run-test-instance` | Initialize variation instance in the existing shaped-run test fixture for the pinned framework API. |

## Plots

Fork: [Quick-Flash/zgui-plot](https://github.com/Quick-Flash/zgui-plot).
Base: `40757c406d0384a36434a8baf0fd3fad512b33ed`.
Aggregate: [`enlumi/all-fixes`](https://github.com/Quick-Flash/zgui-plot/tree/enlumi/all-fixes),
`0087b2b2efb5666a26eaf4f1ee8779c61caa04b3`.

Consumed branch: [`master`](https://github.com/Quick-Flash/zgui-plot/tree/master),
`668686f4ff7c69482a234beb2550ded282c37123`. This includes the merged aggregate,
retained time bands, tracked range linking, and snapshot reads for styles, pointer
state, tab renaming, and divider events. Tab double-click detection uses input-event
timestamps instead of processing time. Cargo.lock pins the published commit;
no sibling checkout is required.

The old vendoring notes named `aef14a9ce0c8097f8347239a423c151e1641077c`;
that object was unavailable in the local fork, Cargo cache, and GitHub fork.
Changes were recovered by comparing vendored sources with the current fork head.

| Branch | Change |
| --- | --- |
| `enlumi/selection-aspect` | Keep box-zoom selection and applied zoom at the viewport aspect ratio. |
| `enlumi/svg-transforms` | Apply retained canvas transforms during SVG export, including heatmap cells. |
| `enlumi/geometry-preparation` | Share bounded geometry workers, record optional preparation timings, and retain patterned pan geometry. |

## Dependency resolution

Direct dependencies pin fork commits. Cargo patches map upstream framework
packages used by editor and plots to that same framework commit, preventing
duplicate framework types. Editor and plot PR branches retain upstream framework
URLs, so upstream PRs do not depend on personal fork URLs.

After upstream merges, update dependency pins and remove the framework source
patch table when all consumers resolve the same upstream revision. No UI source
vendoring needs to return.

Vendoring-only workspace manifests, line-ending normalization, and removed Clippy
allowances were not ported. Forks retain upstream workspace metadata and lint
allowances. Redundant pointer-event clones were replaced by copies to satisfy
framework Clippy checks. Vendoring-only license copies retain their upstream
workspace locations. Plot module declarations were formatted with rustfmt.
