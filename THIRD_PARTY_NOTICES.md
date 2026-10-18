# Third-party notices

Enlumi is distributed under [AGPL-3.0-or-later](LICENSE).
Dependencies and third-party material retain their own license terms and notices.

| Component | Source | Declared license |
| --- | --- | --- |
| Zgui and its component crates | [zortax/zgui](https://github.com/zortax/zgui) | Apache-2.0 |
| zgui-plot | [Quick-Flash/zgui-plot](https://github.com/Quick-Flash/zgui-plot) | MIT OR BSD-3-Clause |
| px4-ulog | [PX4/px4-ulog-rs](https://github.com/PX4/px4-ulog-rs) | MIT |
| bbl_parser | [crates.io/bbl_parser](https://crates.io/crates/bbl_parser) | AGPL-3.0-or-later |
| RustFFT | [RustFFT](https://github.com/ejmahler/RustFFT) | MIT OR Apache-2.0 |
| Wasmtime | [bytecodealliance/wasmtime](https://github.com/bytecodealliance/wasmtime) | Apache-2.0 WITH LLVM-exception |
| zgui-editor | [zortax/zgui-editor](https://github.com/zortax/zgui-editor) | Apache-2.0 |

`tests/fixtures/esc_status_log.ulg` comes from PX4's parser tests. Its original
MIT notice remains in [tests/fixtures/LICENSE](tests/fixtures/LICENSE);
[fixture provenance](tests/fixtures/README.md) records the source.

This table highlights framework, plotting, and decoder dependencies, rather than
listing every transitive dependency. `Cargo.lock` identifies resolved versions
and sources; dependency manifests provide license metadata. Binary distributors
must include notices required by their resolved dependency set.

PlotJuggler inspired the interaction model. Enlumi does not bundle its code.

Blackbox support links `bbl_parser` 1.1.1, licensed AGPL-3.0-or-later.
Enlumi's AGPL licensing aligns the application with that decoder dependency.
Distributors must satisfy applicable AGPL source and notice requirements as well
as the retained terms of other dependencies.

## Historical Enlumi notices

Earlier Enlumi code was offered under `MIT OR BSD-3-Clause`. Its original
copyright, permission, conditions, and warranty notices are retained in
[legacy MIT notice](LICENSES/legacy/LICENSE-MIT) and
[legacy BSD notice](LICENSES/legacy/LICENSE-BSD-3-Clause). Those notices apply to
previously licensed material; they are not alternative licenses for all current
Enlumi code. This migration does not revoke previously granted permissions.

The bundled ULog fixture retains its [MIT license](tests/fixtures/LICENSE).
The independently licensed zgui-plot dependency remains `MIT OR BSD-3-Clause`.

## Portable filter compiler

Portable distributions include Rust compiler, LLVM linker, and standard libraries.
Their upstream copyright reports and license texts ship inside `rust-toolchain/`.
These components retain their upstream licenses; Enlumi licensing does not replace them.
Rust source: https://github.com/rust-lang/rust; LLVM source: https://github.com/llvm/llvm-project.
