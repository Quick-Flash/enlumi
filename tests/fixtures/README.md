`esc_status_log.ulg` comes from PX4's `px4-ulog-rs` test fixtures, retained here
to test nested ESC arrays without depending on sibling decoder checkout.

Source: https://github.com/PX4/px4-ulog-rs/tree/main/tests/fixtures
License: MIT; accompanying `LICENSE` retains original notice.

`synthetic-multi-flight.BBL` is generated test data authored for Enlumi under
repository AGPL-3.0-or-later license. Two four-sample Blackbox sessions use
independent clocks (3s / 6s durations), gyro values 10–13 / 20–23, and flags 0 / 1 / 5 / 0. Headers mimic
Betaflight 2026.6.2 for guard tests; fixture is not evidence of real firmware
field definitions or bit meanings. No private flight data included.

`synthetic-telemetry.tlog` is synthetic MAVLink telemetry authored for Enlumi under
the repository AGPL-3.0-or-later license. Regenerate it with
`python tests/fixtures/generate_tlogs.py` (requires the independent `pymavlink`
encoder). Python/pymavlink is not needed to run Enlumi or its Rust tests.
The file uses the big-endian Unix-microsecond timestamps written by Mission
Planner, QGroundControl and MAVProxy. It includes MAVLink 1/2, multiple senders,
named fields, sensor IDs, numeric arrays, partially truncated extension fields,
large integers, and unknown enum values. These are invented values, not private
flight data or captured client sessions. Recovery, signing-envelope handling and
legacy Mission Planner records are covered by additional generated Rust tests.
