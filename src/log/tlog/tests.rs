use super::*;
use std::io::Cursor;

const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/synthetic-telemetry.tlog");
const ORIGIN: u64 = 1_700_000_000_000_000;

fn packet(time: u64, id: u32, payload: &[u8], v2: bool, flags: u8) -> Vec<u8> {
    let mut bytes = time.to_be_bytes().to_vec();
    if v2 {
        bytes.extend_from_slice(&[
            0xfd,
            payload.len() as u8,
            flags,
            0,
            1,
            1,
            1,
            id as u8,
            (id >> 8) as u8,
            (id >> 16) as u8,
        ]);
    } else {
        bytes.extend_from_slice(&[0xfe, payload.len() as u8, 1, 1, 1, id as u8]);
    }
    bytes.extend_from_slice(payload);
    let crc =
        mavlink_core::calculate_crc(&bytes[9..], definition(id).map_or(0, |message| message.crc));
    bytes.extend_from_slice(&crc.to_le_bytes());
    if v2 && flags & 1 != 0 {
        bytes.extend_from_slice(&[0; 13]);
    }
    bytes
}

fn load(bytes: &[u8]) -> LogStore {
    LogStore::from_tlog("test.tlog".into(), Cursor::new(bytes)).unwrap()
}

#[test]
fn independent_pymavlink_fixture_decodes_mixed_versions_fields_instances_and_clocks() {
    let log = load(FIXTURE);
    assert!(log.warnings.is_empty(), "{:?}", log.warnings);
    assert_eq!(log.origin, ORIGIN);
    assert!((log.duration - 1.5).abs() < 1e-9);
    let roll = log.prepare("mavlink/1.1/ATTITUDE.roll").unwrap();
    assert_eq!(roll.samples().len(), 2);
    assert_eq!(roll.samples()[0].x, 0.1);
    assert!((roll.samples()[0].y - 0.1).abs() < 1e-6);
    assert_eq!(
        log.prepare("mavlink/2.1/ATTITUDE.roll").unwrap().samples()[0].y,
        8.0
    );
    assert_eq!(
        log.prepare("mavlink/1.42/ATTITUDE.roll").unwrap().samples()[0].y,
        5.0
    );
    assert_eq!(
        log.prepare("mavlink/1.1/NAMED_VALUE_FLOAT[roll].value")
            .unwrap()
            .samples()
            .iter()
            .map(|point| point.y)
            .collect::<Vec<_>>(),
        vec![1.25, 4.0]
    );
    assert_eq!(
        log.prepare("mavlink/1.1/NAMED_VALUE_FLOAT[pitch].value")
            .unwrap()
            .samples()[0]
            .y,
        -2.5
    );
    assert_eq!(
        log.prepare("mavlink/1.1/DISTANCE_SENSOR[0].current_distance")
            .unwrap()
            .samples()[0]
            .y,
        123.0
    );
    assert_eq!(
        log.prepare("mavlink/1.1/DISTANCE_SENSOR[1].current_distance")
            .unwrap()
            .samples()[0]
            .y,
        456.0
    );
    assert_eq!(
        log.prepare("mavlink/1.1/PARAM_VALUE[GAIN].param_value")
            .unwrap()
            .samples()[0]
            .y,
        3.25
    );
    assert_eq!(
        log.prepare("mavlink/1.1/PARAM_VALUE[MODE].param_value")
            .unwrap()
            .samples()[0]
            .y,
        7.0
    );
    assert!(log.prepare("mavlink/1.1/STATUSTEXT.text").is_err());
    assert_eq!(
        log.prepare("mavlink/1.1/HEARTBEAT.autopilot")
            .unwrap()
            .samples()[1]
            .y,
        250.0
    );
    assert_eq!(
        log.prepare("mavlink/1.1/HEARTBEAT.custom_mode")
            .unwrap()
            .samples()[1]
            .y,
        0x80000001_u32 as f64
    );
}

#[test]
fn arrays_extensions_and_64_bit_integers_keep_their_wire_values() {
    let log = load(FIXTURE);
    let volts = log
        .prepare("mavlink/1.1/BATTERY_STATUS[0].voltages[0]")
        .unwrap();
    assert_eq!(
        volts
            .samples()
            .iter()
            .map(|point| point.y)
            .collect::<Vec<_>>(),
        vec![12000.0, 12000.0]
    );
    let remaining = log
        .prepare("mavlink/1.1/BATTERY_STATUS[0].time_remaining")
        .unwrap();
    assert_eq!(
        remaining.samples().len(),
        1,
        "MAVLink 1 does not contain extensions"
    );
    assert_eq!(remaining.samples()[0].x, 1.1);
    assert_eq!(
        remaining.samples()[0].y,
        1.0,
        "Truncated high zero bytes must be restored"
    );
    assert_eq!(
        log.prepare("mavlink/1.1/BATTERY_STATUS[0].current_battery")
            .unwrap()
            .samples()[0]
            .y,
        -1.0
    );
    let SomeVec::UInt64(values) = &log.signals["mavlink/1.1/SYSTEM_TIME.time_unix_usec"].values
    else {
        panic!("Expected unsigned 64-bit column")
    };
    assert_eq!(values, &[(1_u64 << 63) + 123]);
}

#[test]
fn corruption_bad_lengths_and_truncated_tails_recover_following_records() {
    let heartbeat = [0, 0, 0, 0, 2, 3, 128, 4, 3];
    let first = packet(ORIGIN, 0, &heartbeat, false, 0);
    let mut damaged = packet(ORIGIN + 100_000, 0, &heartbeat, true, 0);
    damaged[20] ^= 0x40;
    let mut bytes = first.clone();
    bytes.extend_from_slice(&[0x33; 13]);
    bytes.extend_from_slice(&damaged);
    let mut wrong_length = packet(ORIGIN + 200_000, 0, &heartbeat, false, 0);
    wrong_length[9] = 250;
    bytes.extend_from_slice(&wrong_length);
    bytes.extend_from_slice(&packet(ORIGIN + 300_000, 0, &heartbeat, true, 0));
    bytes.extend_from_slice(&first[..14]);
    let log = load(&bytes);
    assert_eq!(
        log.prepare("mavlink/1.1/HEARTBEAT.type")
            .unwrap()
            .samples()
            .len(),
        2
    );
    assert!((log.duration - 0.3).abs() < 1e-9);
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("CRC failures"))
    );
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("incomplete"))
    );
}

#[test]
fn unknown_dialects_signed_frames_and_clock_regressions_are_reported() {
    let payload = [0, 0, 0, 0, 2, 3, 128, 4, 3];
    let mut bytes = packet(ORIGIN, 0, &payload, true, 1);
    bytes.extend_from_slice(&packet(ORIGIN + 100, 16_000_000, &[1, 2, 3], true, 0));
    bytes.extend_from_slice(&packet(ORIGIN - 100, 0, &payload, true, 0));
    bytes.extend_from_slice(&packet(ORIGIN + 200, 0, &payload, false, 0));
    let log = load(&bytes);
    assert_eq!(
        log.prepare("mavlink/1.1/HEARTBEAT.type")
            .unwrap()
            .samples()
            .len(),
        2
    );
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("unsupported dialects"))
    );
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("signatures were not authenticated"))
    );
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("decreasing"))
    );
}

#[test]
fn legacy_mission_planner_timestamps_are_supported() {
    let payload = [0, 0, 0, 0, 2, 3, 128, 4, 3];
    let mut bytes = Vec::new();
    for offset in [0, 500_000] {
        let signed =
            ((i128::from(ORIGIN + offset) + 62_135_596_800_000_000_i128) * 10) - (1_i128 << 63);
        bytes.extend_from_slice(format!("{signed}:").as_bytes());
        bytes.extend_from_slice(&packet(ORIGIN + offset, 0, &payload, false, 0)[8..]);
        bytes.extend_from_slice(b"\r\n");
    }
    assert!(has_header(&bytes));
    let log = load(&bytes);
    assert_eq!(log.duration, 0.5);
    assert!(log.warnings.is_empty());
}

#[test]
fn extensive_corruption_stops_at_the_recovery_budget() {
    let mut bytes = packet(ORIGIN, 0, &[0, 0, 0, 0, 2, 3, 128, 4, 3], false, 0);
    bytes.resize(bytes.len() + MAX_SKIPPED + 400, 0x55);
    let log = load(&bytes);
    assert!(
        log.warnings
            .iter()
            .any(|warning| warning.contains("safety limit"))
    );
    assert_eq!(
        log.prepare("mavlink/1.1/HEARTBEAT.type")
            .unwrap()
            .samples()
            .len(),
        1
    );
}

#[test]
fn detection_accepts_case_variants_and_renamed_files_and_rejects_invalid_input() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["flight.tlog", "flight.TLOG", "flight.tLoG", "flight.data"] {
        let path = directory.path().join(name);
        std::fs::write(&path, FIXTURE).unwrap();
        assert!(
            LogStore::read(&path)
                .unwrap()
                .prepare("mavlink/1.1/ATTITUDE.roll")
                .is_ok()
        );
    }
    for bytes in [b"".as_slice(), b"not telemetry", &[0; 100], &FIXTURE[..10]] {
        assert!(LogStore::from_tlog("bad.tlog".into(), Cursor::new(bytes)).is_err());
    }
}

#[test]
fn escaped_named_instances_and_bytewise_reads_do_not_merge_channels() {
    let message = definition(251).unwrap();
    let named = |name: &[u8], value: f32, time| {
        let mut payload = vec![0; message.base_len];
        payload[4..8].copy_from_slice(&value.to_le_bytes());
        payload[8..8 + name.len()].copy_from_slice(name);
        packet(time, 251, &payload, true, 0)
    };
    let mut bytes = named(b"a/b.c", 1.0, ORIGIN);
    bytes.extend(named(b"a%2Fb.c", 2.0, ORIGIN + 100));
    struct Bytewise<'a>(&'a [u8]);
    impl Read for Bytewise<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(&mut buffer[..1])
        }
    }
    let log = LogStore::from_tlog("tiny reads".into(), Bytewise(&bytes)).unwrap();
    assert_eq!(
        log.prepare("mavlink/1.1/NAMED_VALUE_FLOAT[a%2Fb%2Ec].value")
            .unwrap()
            .samples()[0]
            .y,
        1.0
    );
    assert_eq!(
        log.prepare("mavlink/1.1/NAMED_VALUE_FLOAT[a%252Fb%2Ec].value")
            .unwrap()
            .samples()[0]
            .y,
        2.0
    );
}
