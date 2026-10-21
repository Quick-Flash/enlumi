use crate::{
    profile::{Operation, Scalar, Setting},
    state::Workbench,
};
use std::collections::BTreeSet;
use zgui::prelude::*;

impl Workbench {
    pub fn signal_provenance(self, source: &str) -> String {
        self.derived.with(|fields| {
            self.profiles.instances.with(|instances| {
                self.log.with(|log| {
                    let mut pending = vec![(source.to_owned(), 0)];
                    let mut seen = BTreeSet::new();
                    let mut lines = Vec::new();
                    while let Some((source, depth)) = pending.pop() {
                        if !seen.insert(source.clone()) {
                            continue;
                        }
                        let indent = "  ".repeat(depth.min(16));
                        if let Some(field) = fields.iter().find(|f| f.key() == source) {
                            lines.push(format!(
                                "{indent}{} · {}\n{indent}ID: {source}",
                                field.name,
                                field.filter.label()
                            ));
                            for instance in instances.values() {
                                if let Some(filter) = instance
                                    .definition
                                    .filters
                                    .iter()
                                    .find(|f| instance.ids.fields.get(&f.id) == Some(&field.id))
                                {
                                    let mut settings = Vec::new();
                                    operation_settings(&filter.operation, &mut settings);
                                    for reference in settings {
                                        let value = log
                                            .as_ref()
                                            .map(|log| {
                                                crate::profile::resolve::record(log, reference)
                                                    .map(|(_, s)| {
                                                        format!(
                                                            "{} = {} · {}",
                                                            s.name,
                                                            s.value,
                                                            s.timing()
                                                        )
                                                    })
                                                    .unwrap_or_else(|e| e)
                                            })
                                            .unwrap_or_default();
                                        lines.push(format!(
                                            "{indent}Parameter · flight {} · {value}",
                                            reference.flight
                                        ));
                                    }
                                }
                            }
                            pending.extend(
                                field
                                    .sources
                                    .iter()
                                    .rev()
                                    .map(|source| (source.clone(), depth + 1)),
                            );
                        } else if let Some(setting) =
                            log.as_ref().and_then(|log| log.setting(&source))
                        {
                            lines.push(format!(
                                "{indent}{} = {} · {}\n{indent}ID: {source}",
                                setting.name,
                                setting.value,
                                setting.timing()
                            ));
                        } else {
                            lines.push(format!("{indent}Raw · {source}"));
                        }
                        if seen.len() >= 256 {
                            lines.push("Further inputs omitted (256 shown)".into());
                            break;
                        }
                    }
                    lines.join("\n")
                })
            })
        })
    }
}
fn scalar_settings<'a>(scalar: &'a Scalar, settings: &mut Vec<&'a Setting>) {
    match scalar {
        Scalar::Setting { setting } => settings.push(setting),
        Scalar::Ratio { ratio } => {
            scalar_settings(&ratio[0], settings);
            scalar_settings(&ratio[1], settings);
        }
        Scalar::Number(_) => (),
    }
}
fn operation_settings<'a>(operation: &'a Operation, settings: &mut Vec<&'a Setting>) {
    match operation {
        Operation::Affine { gain, offset } => {
            scalar_settings(gain, settings);
            scalar_settings(offset, settings);
        }
        Operation::LowPass { hz }
        | Operation::HighPass { hz }
        | Operation::ZeroPhaseLowPass { hz }
        | Operation::ZeroPhaseHighPass { hz } => scalar_settings(hz, settings),
        Operation::Integral { initial } => scalar_settings(initial, settings),
        _ => (),
    }
}
