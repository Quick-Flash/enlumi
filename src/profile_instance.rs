use crate::{
    derived::DerivedField,
    geometry::Geometry,
    profile::{
        MetadataMap, Profile,
        identity::{Item, ProfileKey, ResourceIds},
        prepare::{Prepared, PreparedView},
    },
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub resource: String,
    pub source: Option<String>,
    pub message: String,
    pub dependents: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub pane: u64,
    pub item: Item,
    pub lines: BTreeMap<String, u64>,
}
#[derive(Clone)]
pub struct AppliedProfile {
    pub key: ProfileKey,
    pub definition: Profile,
    pub name: String,
    pub ids: ResourceIds,
    pub fields: Vec<DerivedField>,
    pub geometry: Vec<Geometry>,
    pub metadata: MetadataMap,
    pub groups: BTreeMap<String, String>,
    pub symbols: BTreeMap<String, String>,
    pub views: BTreeMap<String, PreparedView>,
    pub diagnostics: Vec<Diagnostic>,
    pub placements: Vec<Placement>,
}
impl AppliedProfile {
    pub fn from_prepared(key: ProfileKey, definition: Profile, p: Prepared) -> Self {
        Self {
            key,
            definition,
            name: p.name,
            ids: p.ids,
            fields: p.fields,
            geometry: p.geometry,
            metadata: p.metadata,
            groups: p.groups,
            symbols: p.symbols,
            views: p
                .views
                .into_iter()
                .map(|view| (view.name.clone(), view))
                .collect(),
            diagnostics: p.diagnostics,
            placements: Vec::new(),
        }
    }
    pub fn field_ids(&self) -> BTreeSet<u64> {
        self.fields.iter().map(|f| f.id).collect()
    }
    pub fn geometry_ids(&self) -> BTreeSet<u64> {
        self.geometry.iter().map(|g| g.id).collect()
    }
    pub fn line_ids(&self) -> BTreeSet<u64> {
        self.placements
            .iter()
            .flat_map(|p| p.lines.values().copied())
            .collect()
    }
    pub fn resolve(&self, token: &str) -> Option<String> {
        if let Some(id) = token.strip_prefix('$') {
            self.symbols.get(id).cloned()
        } else if self.metadata.contains_key(token)
            || self
                .views
                .values()
                .any(|view| view.fields.iter().any(|(source, _)| source == token))
        {
            Some(token.into())
        } else {
            None
        }
    }
}
