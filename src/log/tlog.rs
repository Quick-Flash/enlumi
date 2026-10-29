//! Timestamped MAVLink records written by Mission Planner, QGroundControl and MAVProxy.
use super::{LogStore, RawSignal, SignalInfo};
use px4_ulog::full_parser::SomeVec;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};

const MAX_PACKETS: usize = 5_000_000;
const MAX_VALUES: usize = 20_000_000;
const MAX_STREAMS: usize = 20_000;
const MAX_CHANNELS: usize = 100_000;
const MAX_SKIPPED: usize = 1_000_000;
// Microseconds through year 9999; also accepts logs recorded before a ground-station clock was set.
const MAX_TIMESTAMP: u64 = 253_402_300_799_999_999;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Char,
}
impl Kind {
    fn width(self) -> usize {
        match self {
            Self::U8 | Self::I8 | Self::Char => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }
    fn column(self) -> SomeVec {
        match self {
            Self::U8 => SomeVec::UInt8(Vec::new()),
            Self::I8 => SomeVec::Int8(Vec::new()),
            Self::U16 => SomeVec::UInt16(Vec::new()),
            Self::I16 => SomeVec::Int16(Vec::new()),
            Self::U32 => SomeVec::UInt32(Vec::new()),
            Self::I32 => SomeVec::Int32(Vec::new()),
            Self::U64 => SomeVec::UInt64(Vec::new()),
            Self::I64 => SomeVec::Int64(Vec::new()),
            Self::F32 => SomeVec::Float(Vec::new()),
            Self::F64 => SomeVec::Double(Vec::new()),
            Self::Char => unreachable!("Character fields do not become numeric channels"),
        }
    }
}
struct FieldDef {
    name: &'static str,
    kind: Kind,
    offset: usize,
    count: usize,
    array: bool,
    extension: bool,
    instance: bool,
}
struct MessageDef {
    id: u32,
    name: &'static str,
    crc: u8,
    base_len: usize,
    fields: &'static [FieldDef],
}
include!(concat!(env!("OUT_DIR"), "/mavlink_schema.rs"));

fn definition(id: u32) -> Option<&'static MessageDef> {
    MESSAGES
        .binary_search_by_key(&id, |message| message.id)
        .ok()
        .map(|index| &MESSAGES[index])
}

fn envelope(bytes: &[u8]) -> Option<(u64, usize)> {
    if bytes.first() == Some(&b'-') && bytes.len() >= 21 && bytes[20] == b':' {
        // Original Mission Planner format: signed DateTime.ToBinary ticks and a trailing newline.
        let signed = std::str::from_utf8(&bytes[..20])
            .ok()?
            .parse::<i64>()
            .ok()?;
        let micros = ((i128::from(signed) + (1_i128 << 63)) / 10) - 62_135_596_800_000_000_i128;
        let time = u64::try_from(micros).ok()?;
        return (time <= MAX_TIMESTAMP).then_some((time, 21));
    }
    let time = u64::from_be_bytes(bytes.get(..8)?.try_into().ok()?);
    (time <= MAX_TIMESTAMP).then_some((time, 8))
}

pub(super) fn has_header(bytes: &[u8]) -> bool {
    envelope(bytes).is_some_and(|(_, prefix)| matches!(bytes.get(prefix), Some(0xfe | 0xfd)))
}

#[derive(Default)]
struct Stats {
    records: usize,
    skipped: usize,
    bad_crc: usize,
    truncated: bool,
    unknown: usize,
    unknown_ids: BTreeSet<u32>,
    signed: usize,
    backwards: usize,
    flags: usize,
    limited: bool,
}

struct Record {
    time: u64,
    system: u8,
    component: u8,
    definition: &'static MessageDef,
    payload: [u8; 255],
    v2: bool,
}

struct Records<R> {
    input: BufReader<R>,
    pending: VecDeque<u8>,
    synced: bool,
    stats: Stats,
}
impl<R: Read> Records<R> {
    fn new(input: R) -> Self {
        Self {
            input: BufReader::new(input),
            pending: VecDeque::new(),
            synced: true,
            stats: Stats::default(),
        }
    }
    fn ensure(&mut self, count: usize) -> std::io::Result<bool> {
        let mut buffer = [0_u8; 302];
        while self.pending.len() < count {
            let needed = (count - self.pending.len()).min(buffer.len());
            let read = match self.input.read(&mut buffer[..needed]) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            if read == 0 {
                return Ok(false);
            }
            self.pending.extend(&buffer[..read]);
        }
        Ok(true)
    }
    fn skip(&mut self) {
        self.pending.pop_front();
        self.stats.skipped += 1;
        self.synced = false;
    }
    fn newline(&mut self, prefix: usize) -> std::io::Result<()> {
        if prefix == 21 {
            if self.ensure(1)? && self.pending.front() == Some(&b'\r') {
                self.pending.pop_front();
            }
            if self.ensure(1)? && self.pending.front() == Some(&b'\n') {
                self.pending.pop_front();
            }
        }
        Ok(())
    }
    fn next(&mut self) -> std::io::Result<Option<Record>> {
        while self.ensure(9)? {
            if self.stats.skipped >= MAX_SKIPPED || self.stats.records >= MAX_PACKETS {
                self.stats.limited = true;
                break;
            }
            if self.pending.front() == Some(&b'-') {
                self.ensure(22)?;
            }
            let Some((time, prefix)) = envelope(self.pending.make_contiguous()) else {
                self.skip();
                continue;
            };
            let header = match self.pending.get(prefix) {
                Some(0xfe) => 6,
                Some(0xfd) => 10,
                _ => {
                    self.skip();
                    continue;
                }
            };
            if !self.ensure(prefix + header)? {
                self.stats.truncated = true;
                self.skip();
                continue;
            }
            let bytes = self.pending.make_contiguous();
            let payload_len = usize::from(bytes[prefix + 1]);
            let v2 = header == 10;
            let flags = if v2 { bytes[prefix + 2] } else { 0 };
            if flags & !1 != 0 {
                self.stats.flags += 1;
                self.skip();
                continue;
            }
            let id = if v2 {
                u32::from_le_bytes([bytes[prefix + 7], bytes[prefix + 8], bytes[prefix + 9], 0])
            } else {
                u32::from(bytes[prefix + 5])
            };
            let signed = flags & 1 != 0;
            let end = prefix + header + payload_len;
            let length = end + 2 + if signed { 13 } else { 0 };
            if !self.ensure(length)? {
                self.stats.truncated = true;
                self.skip();
                continue;
            }
            let Some(definition) = definition(id) else {
                if self.synced {
                    self.stats.records += 1;
                    self.stats.unknown += 1;
                    if self.stats.unknown_ids.len() < 32 {
                        self.stats.unknown_ids.insert(id);
                    }
                    self.pending.drain(..length);
                    self.newline(prefix)?;
                } else {
                    self.skip();
                }
                continue;
            };
            if !v2 && payload_len != definition.base_len {
                self.skip();
                continue;
            }
            let bytes = self.pending.make_contiguous();
            let expected = u16::from_le_bytes([bytes[end], bytes[end + 1]]);
            if mavlink_core::calculate_crc(&bytes[prefix + 1..end], definition.crc) != expected {
                self.stats.bad_crc += 1;
                self.skip();
                continue;
            }
            let (system, component) = if v2 {
                (bytes[prefix + 5], bytes[prefix + 6])
            } else {
                (bytes[prefix + 3], bytes[prefix + 4])
            };
            let mut payload = [0; 255];
            payload[..payload_len].copy_from_slice(&bytes[prefix + header..end]);
            self.pending.drain(..length);
            self.newline(prefix)?;
            self.synced = true;
            self.stats.records += 1;
            self.stats.signed += usize::from(signed);
            return Ok(Some(Record {
                time,
                system,
                component,
                definition,
                payload,
                v2,
            }));
        }
        self.stats.truncated |= !self.pending.is_empty() && !self.stats.limited;
        Ok(None)
    }
}

fn escaped(bytes: &[u8]) -> String {
    let bytes = &bytes[..bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len())];
    if bytes.is_empty() {
        return "%00".into();
    }
    let mut name = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
            name.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(name, "%{byte:02X}").unwrap();
        }
    }
    name
}

fn number(kind: Kind, bytes: &[u8]) -> String {
    let mut padded = [0; 8];
    padded[..bytes.len()].copy_from_slice(bytes);
    match kind {
        Kind::U8 | Kind::U16 | Kind::U32 | Kind::U64 => u64::from_le_bytes(padded).to_string(),
        Kind::I8 => (padded[0] as i8).to_string(),
        Kind::I16 => i16::from_le_bytes(padded[..2].try_into().unwrap()).to_string(),
        Kind::I32 => i32::from_le_bytes(padded[..4].try_into().unwrap()).to_string(),
        Kind::I64 => i64::from_le_bytes(padded).to_string(),
        Kind::F32 => f32::from_le_bytes(padded[..4].try_into().unwrap()).to_string(),
        Kind::F64 => f64::from_le_bytes(padded).to_string(),
        Kind::Char => escaped(bytes),
    }
}

fn instance(record: &Record) -> String {
    let values = record
        .definition
        .fields
        .iter()
        .filter(|field| field.instance)
        .map(|field| {
            let length = field.count * field.kind.width();
            let bytes = &record.payload[field.offset..field.offset + length];
            if field.kind == Kind::Char {
                escaped(bytes)
            } else {
                escaped(number(field.kind, &bytes[..field.kind.width()]).as_bytes())
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        format!("[{}]", values.join(","))
    }
}

struct Column {
    name: String,
    offset: usize,
    width: usize,
    extension: bool,
    values: SomeVec,
}
impl Column {
    fn push(&mut self, payload: &[u8; 255]) {
        let mut bytes = [0; 8];
        bytes[..self.width].copy_from_slice(&payload[self.offset..self.offset + self.width]);
        match &mut self.values {
            SomeVec::UInt8(values) => values.push(bytes[0]),
            SomeVec::Int8(values) => values.push(bytes[0] as i8),
            SomeVec::UInt16(values) => {
                values.push(u16::from_le_bytes(bytes[..2].try_into().unwrap()))
            }
            SomeVec::Int16(values) => {
                values.push(i16::from_le_bytes(bytes[..2].try_into().unwrap()))
            }
            SomeVec::UInt32(values) => {
                values.push(u32::from_le_bytes(bytes[..4].try_into().unwrap()))
            }
            SomeVec::Int32(values) => {
                values.push(i32::from_le_bytes(bytes[..4].try_into().unwrap()))
            }
            SomeVec::UInt64(values) => values.push(u64::from_le_bytes(bytes)),
            SomeVec::Int64(values) => values.push(i64::from_le_bytes(bytes)),
            SomeVec::Float(values) => {
                values.push(f32::from_le_bytes(bytes[..4].try_into().unwrap()))
            }
            SomeVec::Double(values) => values.push(f64::from_le_bytes(bytes)),
            _ => unreachable!("Schema generates numeric columns only"),
        }
    }
}

struct Stream {
    timestamps: Vec<u64>,
    extensions: Vec<u64>,
    columns: Vec<Column>,
}
impl Stream {
    fn new(message: &MessageDef) -> Self {
        let mut columns = Vec::new();
        for field in message
            .fields
            .iter()
            .filter(|field| field.kind != Kind::Char && !field.name.starts_with("_padding"))
        {
            for index in 0..field.count {
                let name = if field.array {
                    format!("{}[{index}]", field.name)
                } else {
                    field.name.to_owned()
                };
                columns.push(Column {
                    name,
                    offset: field.offset + index * field.kind.width(),
                    width: field.kind.width(),
                    extension: field.extension,
                    values: field.kind.column(),
                });
            }
        }
        Self {
            timestamps: Vec::new(),
            extensions: Vec::new(),
            columns,
        }
    }
}

impl LogStore {
    pub(super) fn read_tlog(path: &Path, name: String) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|error| format!("Cannot read telemetry log: {error}"))?;
        Self::from_tlog(name, file)
    }

    fn from_tlog(name: String, input: impl Read) -> Result<Self, String> {
        let mut reader = Records::new(input);
        let mut streams = BTreeMap::<String, Stream>::new();
        let mut origin = None;
        let mut last = 0;
        let mut values = 0;
        let mut channels = 0;
        while let Some(record) = reader
            .next()
            .map_err(|error| format!("Cannot read telemetry log: {error}"))?
        {
            if origin.is_some() && record.time < last {
                reader.stats.backwards += 1;
                continue;
            }
            let key = format!(
                "mavlink/{}.{}/{}{}",
                record.system,
                record.component,
                record.definition.name,
                instance(&record)
            );
            if !streams.contains_key(&key) {
                let stream = Stream::new(record.definition);
                if streams.len() >= MAX_STREAMS || channels + stream.columns.len() > MAX_CHANNELS {
                    reader.stats.limited = true;
                    break;
                }
                channels += stream.columns.len();
                streams.insert(key.clone(), stream);
            }
            let stream = streams.get_mut(&key).unwrap();
            let added = stream
                .columns
                .iter()
                .filter(|column| record.v2 || !column.extension)
                .count();
            if values + added > MAX_VALUES {
                reader.stats.limited = true;
                break;
            }
            values += added;
            origin.get_or_insert(record.time);
            last = record.time;
            stream.timestamps.push(record.time);
            if record.v2 {
                stream.extensions.push(record.time);
            }
            for column in &mut stream.columns {
                if record.v2 || !column.extension {
                    column.push(&record.payload);
                }
            }
        }
        let mut warnings = Vec::new();
        let stats = reader.stats;
        if stats.skipped > 0 {
            warnings.push(format!(
                "MAVLink: skipped {} damaged/unrecognized bytes ({} CRC failures)",
                stats.skipped, stats.bad_crc
            ));
        }
        if stats.truncated {
            warnings.push(
                "MAVLink: incomplete record or trailing bytes; showing recovered samples".into(),
            );
        }
        if stats.unknown > 0 {
            warnings.push(format!(
                "MAVLink: skipped {} messages from unsupported dialects (message IDs {:?})",
                stats.unknown, stats.unknown_ids
            ));
        }
        if stats.flags > 0 {
            warnings.push(format!(
                "MAVLink: skipped {} candidates with unsupported incompatibility flags",
                stats.flags
            ));
        }
        if stats.backwards > 0 {
            warnings.push(format!(
                "MAVLink: skipped {} records with decreasing ground-station timestamps",
                stats.backwards
            ));
        }
        if stats.signed > 0 {
            warnings.push(format!(
                "MAVLink: decoded {} signed packets; signatures were not authenticated",
                stats.signed
            ));
        }
        if stats.limited {
            warnings
                .push("MAVLink: decoder safety limit reached; showing recovered samples".into());
        }
        let mut signals = BTreeMap::new();
        for (prefix, stream) in streams {
            let timestamps: Arc<[u64]> = stream.timestamps.into();
            let extensions: Arc<[u64]> = stream.extensions.into();
            for column in stream.columns {
                let timestamps = if column.extension {
                    &extensions
                } else {
                    &timestamps
                };
                if timestamps.is_empty() {
                    continue;
                }
                signals.insert(
                    format!("{prefix}.{}", column.name),
                    RawSignal {
                        timestamps: timestamps.clone(),
                        values: column.values,
                    },
                );
            }
        }
        if signals.is_empty() {
            return Err(format!(
                "Telemetry log contains no supported numeric MAVLink messages. {}",
                warnings.join("; ")
            ));
        }
        let catalogue = signals
            .iter()
            .map(|(id, signal)| SignalInfo {
                id: id.clone(),
                count: signal.timestamps.len(),
            })
            .collect();
        Ok(Self {
            name,
            catalogue,
            duration: last.saturating_sub(origin.unwrap_or(last)) as f64 / 1_000_000.0,
            warnings,
            blackbox_headers: BTreeMap::new(),
            settings: BTreeMap::new(),
            origin: origin.unwrap_or(last),
            signals,
        })
    }
}

#[cfg(test)]
mod tests;
