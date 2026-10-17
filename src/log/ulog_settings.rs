use super::LogSetting;
use px4_ulog::stream_parser::{
    Message, SimpleCallbackResult,
    model::{LogStage, ParameterDefaultMessage, ParameterMessage},
    read_file_with_simple_callback,
};

pub(super) fn read(path: &str) -> Result<Vec<LogSetting>, String> {
    let mut settings = Vec::new();
    read_file_with_simple_callback(path, &mut |message| {
        let setting = match message {
            Message::ParameterMessage(parameter) => {
                let (name, value, stage, data_type) = match parameter {
                    ParameterMessage::Int32(name, value, stage) => {
                        (*name, value.to_string(), stage, "int32_t")
                    }
                    ParameterMessage::Float(name, value, stage) => {
                        (*name, value.to_string(), stage, "float")
                    }
                };
                let stage = match stage {
                    LogStage::Definitions => "Initial parameter",
                    LogStage::Data => "Parameter change",
                };
                Some(LogSetting {
                    name: name.into(),
                    value,
                    kind: format!("{stage} ({data_type})"),
                })
            }
            Message::ParameterDefaultMessage(parameter) => {
                let (name, value, flags, data_type) = match parameter {
                    ParameterDefaultMessage::Int32(name, value, flags) => {
                        (*name, value.to_string(), flags, "int32_t")
                    }
                    ParameterDefaultMessage::Float(name, value, flags) => {
                        (*name, value.to_string(), flags, "float")
                    }
                };
                Some(LogSetting {
                    name: name.into(),
                    value,
                    kind: format!("Default ({data_type}, flags {flags:#04x})"),
                })
            }
            Message::InfoMessage(info) => Some(LogSetting {
                name: info.key.into(),
                value: format_value(info.type_str, info.value),
                kind: format!("Info ({})", info.type_str),
            }),
            Message::MultiInfoMessage(info) => Some(LogSetting {
                name: info.key.into(),
                value: format_value(info.type_str, info.value),
                kind: format!(
                    "Info fragment ({}, continued={})",
                    info.type_str, info.is_continued
                ),
            }),
            _ => None,
        };
        if let Some(setting) = setting {
            settings.push(setting);
        }
        SimpleCallbackResult::KeepReading
    })
    .map_err(|error| format!("Cannot read ULog settings: {error}"))?;
    Ok(settings)
}

fn format_value(data_type: &str, bytes: &[u8]) -> String {
    let base = data_type.split('[').next().unwrap_or(data_type);
    if base == "char" {
        return String::from_utf8_lossy(bytes).trim_end_matches('\0').into();
    }
    let width = match base {
        "int8_t" | "uint8_t" | "bool" => 1,
        "int16_t" | "uint16_t" => 2,
        "int32_t" | "uint32_t" | "float" => 4,
        "int64_t" | "uint64_t" | "double" => 8,
        _ => 0,
    };
    if width == 0 || bytes.is_empty() || !bytes.len().is_multiple_of(width) {
        return format!(
            "0x{}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
    }
    let values: Vec<_> = bytes
        .chunks_exact(width)
        .map(|chunk| match base {
            "int8_t" => (chunk[0] as i8).to_string(),
            "uint8_t" => chunk[0].to_string(),
            "bool" => (chunk[0] != 0).to_string(),
            "int16_t" => i16::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "uint16_t" => u16::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "int32_t" => i32::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "uint32_t" => u32::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "int64_t" => i64::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "uint64_t" => u64::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "float" => f32::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            "double" => f64::from_le_bytes(chunk.try_into().unwrap()).to_string(),
            _ => unreachable!(),
        })
        .collect();
    if data_type.contains('[') || values.len() != 1 {
        format!("[{}]", values.join(", "))
    } else {
        values.into_iter().next().unwrap()
    }
}
