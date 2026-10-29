//! Generate compact wire-field metadata, rather than maintaining message matches by hand.
use mavlink_bindgen::parser::{MavType, extra_crc, parse_profile};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt::Write,
    path::Path,
};

fn attribute(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element
        .attributes()
        .map(Result::unwrap)
        .find(|attribute| attribute.key.as_ref() == name)
        .map(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .unwrap()
                .into_owned()
        })
}

fn primitive(kind: &MavType) -> (&'static str, usize, usize, bool) {
    use MavType::*;
    match kind {
        UInt8 | UInt8MavlinkVersion => ("U8", 1, 1, false),
        Int8 => ("I8", 1, 1, false),
        UInt16 => ("U16", 2, 1, false),
        Int16 => ("I16", 2, 1, false),
        UInt32 => ("U32", 4, 1, false),
        Int32 => ("I32", 4, 1, false),
        UInt64 => ("U64", 8, 1, false),
        Int64 => ("I64", 8, 1, false),
        Float => ("F32", 4, 1, false),
        Double => ("F64", 8, 1, false),
        Char => ("Char", 1, 1, false),
        CharArray(count) => ("Char", 1, *count, true),
        Array(kind, count) => {
            let (kind, size, _, _) = primitive(kind);
            (kind, size, *count, true)
        }
    }
}

pub fn build() {
    println!("cargo:rerun-if-changed=build/tlog.rs");
    let directory = Path::new("vendor/mavlink-dialects");
    let mut files = HashSet::new();
    let profile = parse_profile(directory, Path::new("ardupilotmega.xml"), &mut files)
        .expect("Valid vendored MAVLink definitions");
    let mut instances = BTreeMap::<String, BTreeSet<String>>::new();
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let mut reader = Reader::from_file(file).expect("Readable MAVLink definition");
        let mut buffer = Vec::new();
        let mut message = None;
        loop {
            match reader
                .read_event_into(&mut buffer)
                .expect("Valid MAVLink XML")
            {
                Event::Start(element) if element.name().as_ref() == b"message" => {
                    message = attribute(&element, b"name")
                }
                Event::Start(element) | Event::Empty(element)
                    if element.name().as_ref() == b"field" =>
                {
                    if let Some(message) = &message
                        && attribute(&element, b"instance").as_deref() == Some("true")
                    {
                        instances
                            .entry(message.clone())
                            .or_default()
                            .insert(attribute(&element, b"name").unwrap());
                    }
                }
                Event::End(element) if element.name().as_ref() == b"message" => message = None,
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }
    }
    // Parameter replies interleave different names, although the XML does not mark them as instances.
    for name in ["PARAM_VALUE", "PARAM_EXT_VALUE"] {
        instances
            .entry(name.into())
            .or_default()
            .insert("param_id".into());
    }
    let mut messages = profile.messages.into_values().collect::<Vec<_>>();
    messages.sort_by_key(|message| message.id);
    let mut output = String::from(
        "// Generated from vendored MAVLink XML. See vendor/mavlink-dialects/COPYING.\nstatic MESSAGES: &[MessageDef] = &[\n",
    );
    for message in messages {
        let crc = extra_crc(&message);
        let mut base = message
            .fields
            .iter()
            .filter(|field| !field.is_extension)
            .collect::<Vec<_>>();
        base.sort_by(|a, b| a.mavtype.compare(&b.mavtype));
        let base_len = base
            .iter()
            .map(|field| {
                let (_, size, count, _) = primitive(&field.mavtype);
                size * count
            })
            .sum::<usize>();
        let fields = base
            .into_iter()
            .chain(message.fields.iter().filter(|field| field.is_extension));
        writeln!(
            output,
            "MessageDef {{ id: {}, name: {:?}, crc: {crc}, base_len: {base_len}, fields: &[",
            message.id, message.name
        )
        .unwrap();
        let mut offset = 0;
        for field in fields {
            let name = if field.name == "mavtype" {
                "type"
            } else {
                &field.name
            };
            let instance = instances
                .get(&message.name)
                .is_some_and(|fields| fields.contains(name));
            let (kind, size, count, array) = primitive(&field.mavtype);
            writeln!(output, "FieldDef {{ name: {name:?}, kind: Kind::{kind}, offset: {offset}, count: {count}, array: {array}, extension: {}, instance: {instance} }},", field.is_extension).unwrap();
            offset += size * count;
        }
        assert!(offset <= 255, "MAVLink payload must fit in 255 bytes");
        writeln!(output, "] }},").unwrap();
    }
    output.push_str("];\n");
    let destination =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("mavlink_schema.rs");
    std::fs::write(destination, output).expect("Writable generated schema");
}
