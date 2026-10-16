# State mappings

Import/export JSON from state band's **Mapping** tab. Edit labels/colors live,
then export file for another user or log. No example loads automatically.

No log required: **Geometry > State bands > Blank mapping** opens empty builder.
Add entries and export; import onto source-backed layer when viewing log.

No JSON writing required: drop discrete field into **Geometry > State bands**.
Observed values generate rows automatically. Name/color rows, add custom values
using value/name inputs and **Add**, then **Export mapping**. Existing value IDs
apply with **Set** or Enter; row **X** removes entry and corresponding shading.
Refresh rediscovers observed values; absent custom entries remain available.

```json
{
  "version": 1,
  "states": [
    { "value": 0, "name": "Pilot control", "color": [76, 168, 220] },
    { "value": 42, "name": "Custom hover", "color": [235, 164, 66] }
  ]
}
```

`value` is exact integer (-9007199254740991 to 9007199254740991), unique within
file. `name` is nonblank UTF-8 text, at most 256 bytes. `color` contains three
integers from 0 to 255 (RGB). Version must be 1; unknown properties, duplicate
values, malformed colors and invalid names are rejected without changing layer.
File limit is 1 MiB. At least one entry required.

Import replaces mapping. Observed values absent from imported file receive
numeric labels and automatic colors. Entries absent from current log remain in
mapping; source refresh preserves them. Files contain no source binding, interval
times, opacity or plot layout. Choose source separately; label visibility and
opacity stay local to layer.

[px4-nav-state.json](px4-nav-state.json) contains common PX4 navigation states
from [VehicleStatusV3](https://github.com/PX4/PX4-Autopilot/blob/main/docs/en/msg_docs/VehicleStatusV3.md).
Example deliberately omits version-dependent slots. Verify values against your
firmware; this is editable starting point, not authoritative firmware detection.
Change file for custom firmware, then share alongside log.
