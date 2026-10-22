//! Read-only workspace previews. Authoritative profile resources commit after validation.
use super::{MetadataMap, Profile, Source, resolve};
use crate::{derived, log::LogStore};
use plot::time_series::SeriesData;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Progress {
    pub symbols: BTreeMap<String, String>,
    pub metadata: MetadataMap,
    pub data: BTreeMap<String, SeriesData>,
}
impl Progress {
    pub(super) fn capture(
        profile: &Profile,
        symbols: &BTreeMap<String, String>,
        metadata: &MetadataMap,
        evaluation: &derived::EvaluationCache,
    ) -> Self {
        let mut metadata = metadata.clone();
        for filter in &profile.filters {
            if let Some(source) = symbols.get(&filter.id) {
                metadata.insert(source.clone(), filter.metadata.clone());
            }
        }
        Self {
            symbols: symbols.clone(),
            metadata,
            data: evaluation.series().clone(),
        }
    }
}

pub(super) fn raw_sources(
    profile: &Profile,
    log: &LogStore,
    symbols: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut sources: Vec<_> = profile
        .signals
        .iter()
        .map(|signal| signal.source.clone())
        .chain(
            profile
                .filters
                .iter()
                .flat_map(|filter| filter.inputs.clone()),
        )
        .chain(profile.views.iter().flat_map(|view| view.fields.clone()))
        .collect();
    for plot in profile
        .plot_presets
        .iter()
        .chain(profile.view_presets.iter().flat_map(|view| &view.plots))
        .chain(
            profile
                .workspace_presets
                .iter()
                .flat_map(|workspace| workspace.views.iter().flat_map(|view| &view.plots)),
        )
    {
        sources.extend(
            plot.lines
                .iter()
                .map(|line| Source::Field(line.source.clone())),
        );
        if let Some(condition) = plot
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.condition.as_ref())
        {
            sources.push(Source::Field(condition.clone()));
        }
    }
    sources
        .iter()
        .filter_map(|source| resolve::source(log, source, symbols).ok())
        .filter(|source| derived::id(source).is_none())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
