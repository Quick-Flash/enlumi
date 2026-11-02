# Contributing to Enlumi

Bug reports, focused pull requests, documentation, and reproducible performance
reports are welcome. Open an issue before large features or dependency migrations.

## Setup and checks

Follow [README setup](README.md#build-and-run). Dependencies are public. Zgui is pinned;
rustup selects the repository's nightly. Commit `Cargo.lock` with dependency changes.

```sh
cargo xtask run-gui
cargo xtask pre-commit
```

Individual checks:

```sh
cargo fmt --all -- --check
cargo check --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Windows startup smoke test, requiring an interactive desktop:

```powershell
powershell -NoProfile -File scripts/check-startup.ps1
```

It builds release, opens its own instance, verifies presented GPU frames, and
closes that instance. Close existing release instances before rebuilding on
Windows: running executables can prevent Cargo from replacing them.

## Zgui fork development

Zgui dependencies and testkits pin a public commit from
[Quick-Flash/zgui, branch `enlumi/all-fixes-pr-21`](https://github.com/Quick-Flash/zgui/tree/enlumi/all-fixes-pr-21).
`Cargo.toml` records the full revision. Plot dependencies also pin a public
commit from [Quick-Flash/zgui-plot, branch `master`](https://github.com/Quick-Flash/zgui-plot/tree/master).
No sibling checkout is required.

Enlumi's framework source patches keep the upstream framework dependencies of
plot and editor on that same fork revision. Update every framework dependency,
testkit, and source patch together, and commit `Cargo.lock` with the changes.
See [fork revisions](docs/UPSTREAM_PATCHES.md) for the consumed branches and patches.

## Pull requests

- Keep changes focused and follow existing module structure and formatting.
- Keep heavy work off the UI thread; UI-owned state belongs in signals.
- Add behavioral tests for parsing, filters, docking, and interactions. Use the
  headless harness when actual layout or pointer events matter.
- Update user documentation when behavior or controls change.
- Contribute plot/framework changes upstream, then update pinned revisions here.
  Keep all Zgui crates on the same revision as `zgui-plot`.
- Describe resulting behavior, validation, and remaining limits. Include screenshots
  for visible UI changes.

## Reports and fixtures

Include OS, Enlumi commit, Rust version, reproduction steps, and relevant errors.
Performance reports should include log size, fields, line modes, and panel count.
Prefer small public or synthetic logs. Flight logs can contain GPS, vehicle IDs,
and operational data; share only material you may redistribute. New fixtures need
source and license notes.

Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Unless explicitly stated
otherwise, contributions submitted for inclusion are available under
[AGPL-3.0-or-later](LICENSE), without additional terms. Third-party material retains
its original license and attribution.
