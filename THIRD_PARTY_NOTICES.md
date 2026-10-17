# Third-party notices

Enlumi's `MIT OR BSD-3-Clause` license applies to this project's original code.
Dependencies keep their own license terms.

| Component | Source | Declared license |
| --- | --- | --- |
| Zgui and its component crates | [zortax/zgui](https://github.com/zortax/zgui) | Apache-2.0 |
| zgui-plot | [Quick-Flash/zgui-plot](https://github.com/Quick-Flash/zgui-plot) | MIT OR BSD-3-Clause |
| px4-ulog | [PX4/px4-ulog-rs](https://github.com/PX4/px4-ulog-rs) | MIT |
| bbl_parser | [crates.io/bbl_parser](https://crates.io/crates/bbl_parser) | AGPL-3.0-or-later |
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
Enlumi's source license does not override that dependency's copyleft terms.
Distributing a combined binary requires addressing those terms or obtaining
an appropriate alternative license from the dependency author.
