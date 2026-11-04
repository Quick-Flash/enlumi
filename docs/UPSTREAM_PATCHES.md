# UI fork changes

Enlumi consumes pinned Git commits from the Quick-Flash forks. UI source copies
and local path patches are no longer needed. MAVLink XML remains vendored for
reproducible offline code generation.

Each fork has an `enlumi/all-fixes` branch containing its combined changes.
Individual branches below start at the base revision and contain only their
focused changes, so they can be submitted as separate upstream PRs. Aggregate
branches retain the same changes as a commit series. New commits use Kevin
Plaizier's configured Git identity.

## Framework

Fork: [Quick-Flash/zgui](https://github.com/Quick-Flash/zgui).
Base: [upstream PR #21](https://github.com/zortax/zgui/pull/21),
`be5596501fa92877485879e5ba184c9b63e5ebf6`.
Aggregate: [`enlumi/wayland-dnd-actions`](https://github.com/Quick-Flash/zgui/tree/enlumi/wayland-dnd-actions),
`abc660ae5d6d39953412c8f5c32801bfb35ef341`. This includes the prior aggregate changes, bounded asynchronous
FIFO presentation for native Wayland, and choosing the copy action when a
Wayland drag enters.
The focused branches below retain their original base, `951a0ea3`.

| Branch | Change |
| --- | --- |
| `enlumi/text-scrolling` | Reveal native field caret; autoscroll horizontal selection drags. |
| `enlumi/text-input-focus` | Enable native text input after newly focused fields receive layout bounds. |
| `enlumi/file-drop-routing` | Route external drops to the element under the pointer without requiring focus. |
| `enlumi/window-state` | Refresh maximized/fullscreen signals even when content size stays unchanged. |
| `enlumi/wayland-file-drop` | Decode local file URIs safely; preserve filename bytes and isolate asynchronous drag sessions. |
| `enlumi/wayland-window-state` | Restore normal dimensions when the compositor supplies no restore size. |
| `enlumi/frame-reporting` | Report completed frame timings/counters to passive probes and coalesce dense paint damage. |
| `enlumi/renderer-timing` | Expose opt-in surface, submission, retirement, and upload latency boundaries. |
| `enlumi/wayland-async-fifo` | Present native Wayland frames on a bounded worker; keep UI responsive while FIFO pacing waits for compositor readiness. |
| `enlumi/wayland-dnd-actions` | Set the drag action on `wl_data_device.enter`; compositors send `source_actions` first, so file drops were cancelled. |

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
`5537852574b7838c56a553b3debffc8b12ef3c43`. This includes the merged aggregate
and retained time bands, including rectangle fills and fixed-width boundaries.
The published commit has the same source tree as local `e7e928cd`, used for
the retained-band and latest PR #21 benchmarks. Enlumi pins the published Git
revision; no sibling checkout is required.

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
