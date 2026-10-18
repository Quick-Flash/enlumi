# Signal names, settings, and units

## Current behavior

Raw field IDs identify columns and filter inputs. Blackbox columns use
`blackbox/<flight>/<field>`; ULog columns use `<topic>/<instance>/<field>`.
Single-flight Blackbox files show field groups directly. Multi-flight files keep
flight groups so distinct clocks and settings remain distinguishable.

Blackbox headers are retained per flight, including firmware revision, craft,
board, numeric settings, field definitions, and original header lines. The raw
lines matter: `bbl_parser`'s numeric settings map cannot represent every header
value. Retaining these headers does not apply aliases or conversions yet.

## Profile implementation

[Log profiles](PROFILES.md) implement the first metadata workflow using editable
JSON files. Profiles declare firmware/setting requirements, stable field
references, names, units and semantic axes, filter graphs (including Rust),
geometry/state rules, and reusable plot views. No firmware-specific naming or
conversion registry lives in Rust code.

Numeric recorded settings are selectable constant fields. Their record identity
and flight scope remain explicit; ambiguous references fail rather than guessing
between initial, changed, or default values. Both log formats use the same path.

Profile loading validates and computes a candidate before committing resources.
It adds fields, metadata, and geometry without creating plot panels. Each view
then offers drag placement and **New panel** / **Add to active**. Missing sources,
cyclic/failed branches, and unavailable geometry are skipped with diagnostics.
Firmware/settings guards and commit conflicts still reject incompatible loads.

**Labels…** now inspects raw identity, supplying profile, transformation chain,
and recorded scalar parameter inputs. Profile defaults and manual label overrides
are separate. Reset can restore profile default or raw label. Different declared
units warn and offer separate plots; no automatic dimension inference occurs.
JSON `flags` rules name bitmask combinations alongside exact `states` rules.
Future work includes timestamped parameter formats, parameter value overrides,
richer field-derived paths, and additional verified firmware profiles. None require a plugin framework. Unknown fields keep
their raw identity. Units are declarations and must match the actual computation;
no automatic unit inference or per-series axis normalization is implemented.

## First conversion: motor RPM

For firmware verified to store `eRPM[n]` in hundreds of electrical RPM:

```text
electrical_rpm = raw_eRPM * 100
pole_pairs = motor_poles / 2
mechanical_rpm = electrical_rpm / pole_pairs
              = raw_eRPM * 200 / motor_poles
rotational_hz = mechanical_rpm / 60
```

Example: raw value 2100 with 14 motor poles gives 30,000 RPM and 500 Hz.
Require a valid, positive, even pole count and a verified field encoding.
An already-mechanical RPM field must not receive this conversion again.
Motor command values and motor order do not establish RPM or physical motor
position; those interpretations require their own evidence.

## Acceptance checks for each step

- Raw IDs and existing filter references remain stable when labels change.
- Single-flight and multi-flight browsing preserve field selection and search.
- Firmware versions, high-resolution modes, and missing settings have fixtures.
- Known numeric examples agree with firmware and official viewer behavior.
- Unknown fields, debug modes, and firmware retain their raw interpretation.
- Overrides affect only the intended flight and invalidate affected derived data.
- Unit labels describe the values actually used by plots and filters. Mixed-unit
  panels must not receive a misleading single-unit Y-axis label.

## References

- [Betaflight Blackbox writer](https://github.com/betaflight/betaflight/blob/master/src/main/blackbox/blackbox.c):
  field definitions, setpoint components, high-resolution scale, eRPM encoding.
- [Official field presenter](https://github.com/betaflight/blackbox-log-viewer/blob/master/src/flightlog_fields_presenter.js):
  axis aliases and RPM conversion.
- [Betaflight motor poles documentation](https://betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering#motor-poles):
  electrical versus mechanical RPM and motor pole count.

These moving references establish investigation targets. Implemented rules must
pin evidence to the firmware revisions they claim to support.

See [profile system review and plan](PROFILE_SYSTEM_PLAN.md) for current gaps,
proposed UI, ownership/lifecycle work, and validation gates.
