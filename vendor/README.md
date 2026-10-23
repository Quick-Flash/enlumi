# Temporary UI fixes

Direct framework dependencies remain upstream. Dependency crates
are patched locally until upstream carries these behaviors:

- `zgui-runtime`: zortax/zgui `f6badfb1af02b08ed1924687237028fb98a97bd8`.
  Reveal native field caret after keyboard edits/selection changes and scroll
  horizontally while dragging selection past either edge.
  Enable native text input after layout when a newly mounted field received focus
  before it had bounds (for example, the plot tab rename input on Linux).
- `zgui-editor`: zortax/zgui-editor `1f7aaec6e4e2dfaee4d26b049a719d9e88d16f8f`.
  Add horizontal selection autoscroll and Shift-wheel scrolling.

Focused source diffs are in `text-scrolling.patch` and `text-input-focus.patch`.
Runtime/editor sources and Apache-2.0 licenses are included. Runtime manifest is expanded from
upstream workspace metadata; framework dependencies still resolve upstream.
Remove these patch entries and directories after upstream fixes are adopted.
