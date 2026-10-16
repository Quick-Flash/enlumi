# Custom Rust filters (prototype)

Choose **Rust function** beside **Built-in** in Filters. Drag raw or derived fields
into Inputs, write a Rust function, name each numbered output row, then
**Compile & create**. **Add output** adds a row; its X removes it. At least one
output remains. Inputs and output rows remain editable in saved groups.
Each output becomes a normal derived field: plot it, chain another filter, or click
its saved row to edit the whole Rust group. **Save changes** preserves field IDs
and recomputes descendants. Removing an output used by another field is blocked;
change or remove that dependent field first. Changes publish together after a successful run.

**Check** compiles without running or saving; no log is required. **Cancel** stops
pending compilation or guest execution. Compilation errors show locations in
`filter.rs`, matching editor source lines. Syntax preflight also shows line/column,
source context, and a caret. Failed runs leave saved fields intact.
Changing inputs/names/editor mode during a run discards its result.
Drag divider above diagnostics to give errors more or less space. Divider also
supports Up/Down keys and shows a resize cursor on hover. Diagnostics always keep
their own panel, including when there are no errors; code and panel sizes remain
stable as messages change.

Starter code loops over each input field, then its samples, summing fields into
`outputs[0]`. One input passes through unchanged. Each sample uses `time[i]` for
its timestamp; NaN in any input leaves a gap. Additional outputs need matching
`OUTPUTS` count and named rows.

## Compiler setup

Install Rust through [rustup](https://rustup.rs/) and add the WASM target:

```sh
rustup target add wasm32-unknown-unknown
```

Enlumi calls `rustc` from PATH, respecting current directory's rustup toolchain.
When running from this repository, its pinned nightly is selected; add the target
from this directory. Outside the repository, rustup's default toolchain applies.
Set `ENLUMI_RUSTC` to a compiler executable path to select a different compiler.
That compiler needs its own `wasm32-unknown-unknown` standard library installed.
Rust 2024 edition support is required.

Compiler is **not bundled**. Enlumi embeds Wasmtime; no separate Wasmtime install
is required. Source compilation uses a temporary directory and direct `rustc`,
without Cargo, build scripts, downloads, or external crates. Windows compiler
processes launch without opening console windows. Same mechanism supports native
Windows, Linux, and macOS builds; this prototype was exercised locally on Windows.

## Function contract

Editor contains a complete Rust module. It must expose a default-constructible
`State` and this function:

```rust
pub const OUTPUTS: usize = 1;

#[derive(Default)]
pub struct State {
    previous: Option<f64>,
}

pub fn filter(
    time: &[f64],
    inputs: &[&[f64]],
    outputs: &mut [&mut [f64]],
    state: &mut State,
) {
    for i in 0..time.len() {
        let value = inputs[0][i];
        if !value.is_finite() {
            state.previous = None;
            continue;
        }
        let previous = state.previous.unwrap_or(value);
        let filtered = previous + 0.1 * (value - previous);
        outputs[0][i] = filtered;
        state.previous = Some(filtered);
    }
}
```

- Declare `pub const OUTPUTS: usize = N;` using a literal from 1 to 16.
  Enlumi checks this against named rows before starting the compiler, including
  **Check**. Arbitrary Rust writes cannot be counted statically: loops and dynamic
  indices must respect this declared count; out-of-range writes trap at runtime.
- Inputs map to displayed `inputs[0]`, `inputs[1]`, etc. Drag whole rows to reorder;
  neighbors animate into insertion positions. Escape or dropping outside cancels.
  Up/Down keys reorder a focused input. Numbered output rows map to `outputs[0]`,
  `outputs[1]`, etc.
- First input supplies timestamps, in elapsed seconds. Other inputs use existing
  timestamp alignment: linear interpolation inside finite short segments, no
  extrapolation. Missing values arrive as `f64::NAN`.
- All slices in a call have the same length. Calls process up to 4096 samples;
  final chunk can be shorter. `time[0]` is chunk start, not necessarily log start.
- `State::default()` runs once per evaluation. State survives chunk boundaries,
  resets when reevaluating. Keep previous time in state when calculating `dt`
  across chunks. Gap/state reset rules belong to your function.
- Outputs start as NaN on every call. Unwritten values remain gaps; nonfinite
  results become NaN. Output timestamps/count match first input.
- Entire output group runs once, even when multiple outputs are consumed.
- `std`, helper functions, structs, collections, and loops work. OS-dependent std
  functions do not provide normal filesystem/thread behavior on this target.
- Code and definitions currently belong to this app session. Save source elsewhere
  before closing; there is no script-file library or session persistence yet.

Source editor uses pinned [zgui-editor](https://github.com/zortax/zgui-editor), with
Rust syntax highlighting, line numbers, indentation, undo/redo, word/line selection,
and caret line/column status. Code uses 14px text; filter labels use at least 12px.
Editor becomes read-only during compilation/execution.
Editor receives saved source changes without resetting its document on each keystroke.
Upstream currently lacks IME composition and soft wrapping; long lines scroll
horizontally. Framework dependencies share Enlumi's pinned Zgui fork through Cargo
patches; no second framework revision is loaded.

## Multiple inputs and outputs

Two-input example with names `Magnitude` and `Difference`:

```rust
pub const OUTPUTS: usize = 2;

#[derive(Default)]
pub struct State;

pub fn filter(
    time: &[f64],
    inputs: &[&[f64]],
    outputs: &mut [&mut [f64]],
    _state: &mut State,
) {
    assert_eq!(inputs.len(), 2, "Drop exactly two input fields");
    assert_eq!(outputs.len(), 2, "Name exactly two outputs");
    for i in 0..time.len() {
        let (x, y) = (inputs[0][i], inputs[1][i]);
        outputs[0][i] = x.hypot(y);
        outputs[1][i] = x - y;
    }
}
```

## Limits and trust

Runtime receives numeric buffers only, with no WASI or host imports. Guest
panics and bounds violations trap instead of unwinding into GUI. Limits:

- 64 inputs, 16 outputs, 256 KiB source.
- 128 MiB guest linear memory.
- 100 million fuel units per chunk (also applied during initialization).
- 30 seconds guest execution per group; 60 seconds Rust compilation.
- Cancellation checked during compilation, between chunks, and through Wasmtime
  epoch interruption while guest code runs. WASM module compilation itself is
  not interruptible; cancellation is checked before and after that step.

These limits protect runtime responsiveness. **Source compilation is trusted
local code:** Rust macros such as `include_str!` can read local files at compile
time. Do not compile untrusted source. Compiler process is not OS-sandboxed.

Last eight compiled modules cache in memory for this app process. Cache is not
persisted to disk. Filtering uses full-resolution f64 samples on background
workers; plot LOD does not change filter inputs.

## Integration tests

Normal tests cover schema/UI behavior without requiring an installed WASM target.
Real compiler/runtime tests run explicitly (and in CI):

```sh
cargo test --locked -p enlumi script_e2e -- --ignored --nocapture
```

Coverage includes state across chunks, multi-output groups, alignment, NaNs,
compiler diagnostics, traps, memory limits, cancellation, atomic edits, chaining,
and million-sample magnitude/low-pass/quaternion checks with printed timings.
