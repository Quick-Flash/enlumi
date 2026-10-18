# Betaflight 2026.6 Explorer presets

Six profiles provide **52 view choices for either logging resolution**. Rate
profiles are alternatives: 24 normal-resolution or 24 high-resolution views,
plus 28 shared views. Presets cover Explorer-style field combinations using
Enlumi's existing JSON profiles and Rust filters.

## Load

1. Open log, then **Profiles → Folder…**. Select this directory, or copy entire
   directory into your existing profile library. Keep seven `.rs` helpers beside
   JSON files; copying JSON alone breaks computed outputs.
2. Expand matching **Rates, PID and RC** profile. Normal profile requires
   `blackbox_high_resolution = 0`; high-resolution profile requires `1`.
3. Use **Add**, **New panel**, or drag a view. Fields can also feed filters.
4. Load shared profiles as needed. Use **⋯ → Auto** for profiles you want matched
   when logs open. Recipes default to Auto off. Both rate alternatives can be
   enabled; recorded resolution selects matching one.
5. Choose flight before applying profiles to multi-flight logs.

Firmware guard accepts `Betaflight 2026.6.` only. New firmware needs reviewed
mappings. Missing fields/settings skip affected branches; usable outputs remain.
Eight motor/servo slots and optional sensors/S terms intentionally allow partial
profiles on ordinary quads. Named view appears only if some output is usable.

## Available views

| Profile | Views | Contents |
| --- | ---: | --- |
| `01-rates.json` | 24 | Three-axis gyros, unfiltered gyros, setpoints, tracking error; P/I/D/F/S percentages; complete P+I+D+F sums; per-axis tracking/PID/filter comparisons; mixed gyro/PID overlays; RC commands; mixer throttle |
| `02-rates-high-resolution.json` | 24 | Same views with explicit gyro, setpoint and RC scaling |
| `03-motors.json` | 6 | Configured output percentage, raw motor output, mechanical RPM, rotational Hz, logged eRPM, servos; up to eight channels |
| `04-sensors-power.json` | 13 | Acceleration XYZ/magnitude, combined attitude, roll/pitch/heading, compass raw, voltage, current, power, segment charge, barometer altitude, RSSI |
| `05-modes-rx.json` | 6 | Named RC-mode, failsafe, RX signal, RX validity and GPS-state bands; RX signal/validity traces |
| `06-debug-raw.json` | 3 | All eight debug channels, channels 0–3, channels 4–7 |

Suggested starting layout: **Roll rate tracking**, **Pitch rate tracking**,
**Yaw rate tracking**, **Motors · configured span (%)**. Add per-axis PID
contributions while tuning, then RPM and gyro-filter comparisons for motor/noise
inspection. Add mode/RX bands onto existing plots to correlate transitions.

## Meaning and conversions

Mappings follow [tagged 2026.6.2 Blackbox field definitions and sampling code](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/blackbox/blackbox.c).
Graph choices follow [Explorer's preset definitions](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/graph_config.js).
Displayed quantities follow [Explorer's field conversions](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog_fields_presenter.js)
and [computed fields](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog.js),
inspected 2026-10-17. Helpers implement explicit arithmetic; firmware enums and
source names stay in these files.

| Quantity | Rule |
| --- | --- |
| Gyro/unfiltered gyro and rate setpoint | Raw deg/s normally; multiply by 0.1 in high-resolution logs |
| RC roll/pitch/yaw | Divide high-resolution values by 10, then add 1500 for displayed us |
| RC throttle | Divide high-resolution values by 10; no 1500 offset |
| Mixer throttle | `setpoint[3] * 0.1` percent, independent of resolution |
| PID components | Raw contribution divided by 10, displayed as percent |
| Tracking error | Setpoint minus gyro, after conversion to deg/s |
| PID sum | P + I + D + F, each already percent; requires every declared component |
| Motor configured output | `100 * (raw - motorOutput[0]) / (motorOutput[1] - motorOutput[0])`, clamped 0–100 |
| Mechanical RPM | Logged eRPM × 200 / recorded `motor_poles`; poles must be positive, even integer |
| Rotational frequency | Mechanical RPM / 60 |
| Acceleration | `accSmooth[n] / acc_1G`, using recorded scale |
| Attitude | Logged quaternion XYZ / 32767; reconstruct nonnegative W; existing quaternion-to-Euler filter |
| Heading | Negative quaternion yaw, wrapped into [0, 360) degrees, matching Explorer's convention |
| Voltage/current | `vbatLatest / 100` V; `amperageLatest / 100` A |
| Power | Converted volts × converted amps |
| Segment charge | Integral of amps × 1000/3600 mAh; resets at each finite contiguous segment |
| Barometer altitude | `baroAlt / 100` m |
| RSSI | `rssi * 100/1024` percent |

Raw motor output has no universal physical unit: DShot uses command values, while analog protocols use their configured output scale. Raw motor and debug views therefore leave unit blank. RC roll/pitch/yaw raw values are microsecond offsets from 1500 (scaled by ten in high-resolution logs).

Motor percentage measures recorded configured output span. Explorer's physical
DShot percentage can use protocol minimum instead of configured idle; numbers
can differ. Neither quantity estimates thrust. Motors use recorded channel
numbers, without inferred front/rear positions. One recorded pole count applies
to all RPM channels; mixed-pole installations need edited recipes.

Yaw D can be absent. Yaw tracking and PID-component views retain remaining
signals; complete yaw sum is skipped. No zero D is invented. S contributions
have separate plots and are excluded from explicitly named P+I+D+F sums. Sums
are unclamped contributions, not final mixer output.

Mixed gyro/PID overlays combine deg/s and percent; Enlumi warns about mixed unit
labels. Same-unit tracking and PID views support direct comparisons. These
recipes do not reproduce Explorer's independent display curves or smoothing.

Quaternion attitude needs logged `imuQuaternion` fields; no gyro/accelerometer
attitude estimator runs when those fields are absent. Small int16 rounding
overshoots are tolerated while reconstructing W; invalid vectors produce gaps.
Heading uses IMU reference, without an added geographic/north calibration.
Charge excludes unobserved intervals and prior battery usage.

## Mode and debug semantics

2026.6.2 Blackbox copies first 32 bits of `rcModeActivationMask` into field named
`flightModeFlags`. Rules therefore use [2026.6.2 RC box indices](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc_modes.h),
not `flightModeFlags_e` values. **Arm switch** means RC mode activation, not proof
that motors armed. Rules include modes through crash flip at bit 31. Higher RC
box bits are absent from this field. Simultaneous names join in one band;
unmatched/inactive intervals stay unshaded. No implicit Acro state is fabricated.

GPS-state rules use [recorded state flag bits](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/runtime_config.h).
Failsafe names follow [2026.6.2 failsafe phases](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/flight/failsafe.h).

Debug views preserve raw channel values. Inspect recorded `debug_mode` parameter
before interpreting units or assigning axis names. These generic presets do not
assume one debug mode's channel meanings apply to another mode.

## Scope

Basher example's headers declare rates/PID, all four motor/RPM channels,
acceleration, quaternion, debug and slow status fields. They omit yaw D, S terms,
servos, voltage/current, barometer, compass and RSSI. Those absences produce partial
profiles rather than fabricated signals. This is header inspection, not runtime
validation of these new profiles. Seven Rust helpers compile for
`wasm32-unknown-unknown`; custom Rust execution needs installed Rust WASM target.

FFT/spectrograms, GPS trajectory decoding/maps, video sync, and complete workspace
layout presets require application features beyond these time-series recipes.
