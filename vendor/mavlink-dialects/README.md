# MAVLink message definitions

Unmodified `ardupilotmega` XML and its recursive includes, including `common`
used by PX4, from the `mavlink` 0.19.1 crates.io archive.

Source: https://static.crates.io/crates/mavlink/mavlink-0.19.1.crate
Archive SHA-256: `82f4bc8f826928c899e39e30d98eafd57b1c0d992b541a007fadc68126f95303`
Upstream: https://github.com/mavlink/mavlink

`COPYING` is retained verbatim, including the generator-output MIT exception.
`build/tlog.rs` generates metadata locally; ordinary builds need no downloads.

Update definitions, license, provenance, Rust MAVLink dependencies and lockfile,
then run importer tests with one command:

```sh
cargo xtask update-mavlink
```

To select a specific published version: `cargo xtask update-mavlink 0.19.1`.
Failed updates restore the previous files. Review and commit the resulting diff.
