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
- `zgui-platform-wayland`: zortax/zgui `951a0ea3b113247da6dff439fccbd9097aa3187f`.
  Preserve UTF-8 and Unix filename bytes in file-drop URI decoding, accept local
  `localhost` URIs, reject remote authorities and NULs, and avoid malformed-escape
  panics. Isolate asynchronous reads between drag sessions and retain a dropped
  offer through the subsequent `leave` while its transfer is pending.
  Remember normal window dimensions when maximizing or entering fullscreen so
  restoring works when the compositor leaves its requested size unspecified.
- `zgui-editor`: zortax/zgui-editor `1f7aaec6e4e2dfaee4d26b049a719d9e88d16f8f`.
  Add horizontal selection autoscroll and Shift-wheel scrolling.

Focused source diffs are in `text-scrolling.patch`, `text-input-focus.patch`,
`file-drop-routing.patch`, `window-state.patch`, `wayland-file-drop.patch`, and
`wayland-window-state.patch`.
Sources and Apache-2.0 licenses are included. Runtime and Wayland manifests are expanded
from upstream workspace metadata; framework dependencies still resolve upstream.
Remove these patch entries and directories after upstream fixes are adopted.
