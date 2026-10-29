"""Generate synthetic telemetry with an independent encoder: python + pymavlink.

The fixtures are authored for Enlumi under its repository license, not flight data.
Neither Python nor pymavlink is required to run Enlumi or its Rust tests.
"""
from pathlib import Path
import struct
from pymavlink.dialects.v20 import ardupilotmega as mavlink
from pymavlink.dialects.v10 import ardupilotmega as mavlink1

ORIGIN = 1_700_000_000_000_000


def generate():
    encoder = mavlink.MAVLink(None, srcSystem=1, srcComponent=1)
    output = bytearray()

    def append(offset, message, *, v1=False, system=1, component=1):
        encoder.srcSystem, encoder.srcComponent = system, component
        output.extend(struct.pack(">Q", ORIGIN + offset))
        output.extend(message.pack(encoder, force_mavlink1=v1))
        encoder.seq = (encoder.seq + 1) & 255

    append(0, mavlink.MAVLink_heartbeat_message(2, 3, 128, 4, 4, 3), v1=True)
    def attitude(roll):
        return mavlink.MAVLink_attitude_message(123_000, roll, -0.2, 0.3, 0.4, 0.5, 0.6)
    append(100_000, attitude(0.1), v1=True)
    append(200_000, attitude(0.4))
    append(300_000, attitude(8.0), system=2)
    append(400_000, attitude(5.0), component=42, v1=True)
    append(500_000, mavlink.MAVLink_named_value_float_message(1, b"roll", 1.25))
    append(600_000, mavlink.MAVLink_named_value_float_message(2, b"pitch", -2.5))
    append(700_000, mavlink.MAVLink_named_value_float_message(3, b"roll", 4.0))
    append(800_000, mavlink1.MAVLink_distance_sensor_message(1, 10, 1000, 123, 0, 0, 0, 0), v1=True)
    append(900_000, mavlink.MAVLink_distance_sensor_message(1, 10, 1000, 456, 0, 1, 0, 0))
    def battery(remaining=0):
        return mavlink.MAVLink_battery_status_message(0, 0, 0, 2500, [12000, 11900] + [65535] * 8, -1, 100, -1, 85, time_remaining=remaining)
    append(1_000_000, mavlink1.MAVLink_battery_status_message(0, 0, 0, 2500, [12000, 11900] + [65535] * 8, -1, 100, -1, 85), v1=True)
    append(1_100_000, battery(1))
    append(1_200_000, mavlink.MAVLink_system_time_message((1 << 63) + 123, 99))
    append(1_300_000, mavlink.MAVLink_param_value_message(b"GAIN", 3.25, 9, 2, 0))
    append(1_350_000, mavlink.MAVLink_param_value_message(b"MODE", 7.0, 1, 2, 1))
    append(1_400_000, mavlink.MAVLink_statustext_message(6, b"Synthetic \xfd\xfe text"))
    append(1_500_000, mavlink.MAVLink_heartbeat_message(250, 250, 255, 0x80000001, 250, 3))
    target = Path(__file__).with_name("synthetic-telemetry.tlog")
    target.write_bytes(output)
    print(f"Wrote {target.name}: {len(output)} bytes")


if __name__ == "__main__":
    generate()
