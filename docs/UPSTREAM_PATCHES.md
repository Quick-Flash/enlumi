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
Aggregate: [`enlumi/all-fixes-pr-21`](https://github.com/Quick-Flash/zgui/tree/enlumi/all-fixes-pr-21),
`41324c958d52833127d779b1126654c10b80931b`.
This rebases all nine commits from `enlumi/all-fixes` (`42bb0c2d`) onto PR #21.
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

The band optimization currently consumes the local `../zgui-plot` checkout.
Its default branch is `master`; local commit
`e7e928cd57f3fe05f2007030a7abf8293013f003` adds retained time bands on top of
the merged aggregate. It is intentionally unpushed. After pushing plot master,
replace Enlumi's local plot path with the fork URL and that revision, then
regenerate `Cargo.lock`. The local checkout's existing aspect-selection commit
was preserved when merging `origin/master`.

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
