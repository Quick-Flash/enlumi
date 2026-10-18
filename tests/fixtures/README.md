`esc_status_log.ulg` comes from PX4's `px4-ulog-rs` test fixtures, retained here
to test nested ESC arrays without depending on sibling decoder checkout.

Source: https://github.com/PX4/px4-ulog-rs/tree/main/tests/fixtures
License: MIT; accompanying `LICENSE` retains original notice.

`synthetic-multi-flight.BBL` is generated test data authored for Enlumi under
repository AGPL-3.0-or-later license. Two four-sample Blackbox sessions use
independent clocks (3s / 6s durations), gyro values 10–13 / 20–23, and flags 0 / 1 / 5 / 0. Headers mimic
Betaflight 2026.6.2 for guard tests; fixture is not evidence of real firmware
field definitions or bit meanings. No private flight data included.
