use crate::{derived::FilterInput, state::Workbench};
use zgui::prelude::*;

impl Workbench {
    pub fn select_field(self, source: String, ctrl: bool, shift: bool, visible: &[String]) {
        let family = |id: &str| {
            if id.starts_with("@profile/") {
                1
            } else if id.starts_with("@analysis/") {
                2
            } else {
                0
            }
        };
        let group = family(&source);
        self.selected_fields
            .update(|fields| fields.retain(|id| family(id) == group));
        if self
            .selection_anchor
            .get_untracked()
            .is_some_and(|id| family(&id) != group)
        {
            self.selection_anchor.set(None);
        }
        if shift {
            let anchor = self.selection_anchor.get_untracked();
            let range = anchor.as_ref().and_then(|anchor| {
                Some((
                    visible.iter().position(|field| field == anchor)?,
                    visible.iter().position(|field| field == &source)?,
                ))
            });
            if let Some((start, end)) = range {
                self.selected_fields.update(|fields| {
                    if !ctrl {
                        fields.clear();
                    }
                    fields.extend(visible[start.min(end)..=start.max(end)].iter().cloned());
                });
            } else {
                self.selected_fields
                    .set(std::collections::BTreeSet::from([source.clone()]));
                self.selection_anchor.set(Some(source.clone()));
            }
        } else if ctrl {
            self.selected_fields.update(|fields| {
                if !fields.remove(&source) {
                    fields.insert(source.clone());
                }
            });
            self.selection_anchor.set(Some(source.clone()));
        } else {
            // Keep selected group on press so dragging any member moves group.
            if !self
                .selected_fields
                .with_untracked(|fields| fields.contains(&source))
            {
                self.selected_fields
                    .set(std::collections::BTreeSet::from([source.clone()]));
            }
            self.selection_anchor.set(Some(source.clone()));
        }
        self.selected_field
            .set(self.selected_fields.with_untracked(|fields| {
                if fields.contains(&source) {
                    Some(source)
                } else {
                    fields.first().cloned()
                }
            }));
    }
    pub fn clear_filter_sources(self) {
        self.cancel_filter_input_drag();
        self.filter_inputs.set(Vec::new());
        self.filter_error.set(String::new());
        if self.filter_mode.get_untracked().is_new() {
            self.reset_filter_names();
        }
        self.finish_field_drag();
    }
    pub(crate) fn drag_fields(self, source: String) -> Vec<String> {
        if self
            .selected_fields
            .with_untracked(|fields| fields.contains(&source))
        {
            let mut fields: Vec<_> = self.selected_fields.get_untracked().into_iter().collect();
            if source.starts_with("@profile/") {
                let visible = self.visible_profile_sources();
                fields.sort_by(|a, b| {
                    visible
                        .iter()
                        .position(|id| id == a)
                        .unwrap_or(usize::MAX)
                        .cmp(&visible.iter().position(|id| id == b).unwrap_or(usize::MAX))
                        .then_with(|| crate::field_tree::natural_cmp(a, b))
                });
            } else {
                fields.sort_by(|a, b| crate::field_tree::natural_cmp(a, b));
            }
            if source.starts_with("@analysis/") {
                fields.retain(|id| {
                    id.strip_prefix("@analysis/").is_some_and(|id| {
                        self.analysis
                            .sources
                            .with_untracked(|sources| sources.iter().any(|source| source == id))
                    })
                });
            }
            fields
        } else {
            vec![source]
        }
    }
    pub fn field_drag_move(self, x: f32, y: f32) {
        self.analysis.condition_hover.set(
            self.analysis.open.get_untracked()
                && self.analysis.mode.get_untracked() == "heatmap"
                && Self::inside(self.analysis.condition_drop, x, y),
        );
        self.analysis.hover.set(
            self.analysis.open.get_untracked()
                && !self.filters_open.get_untracked()
                && Self::inside(self.analysis.drop, x, y),
        );
        if self.analysis.hover.get_untracked() || self.analysis.condition_hover.get_untracked() {
            self.drop_target.set(None);
            self.filter_drop_active.set(false);
            self.bands.hover.set(false);
            return;
        }
        if self
            .drag
            .state
            .get_untracked()
            .is_some_and(|drag| drag.0.starts_with("@analysis/"))
        {
            self.drop_target.set(self.hit_any_plot(x, y));
            self.filter_drop_active.set(false);
            self.bands.hover.set(false);
            return;
        }
        if self.drag.state.get_untracked().is_some_and(|drag| {
            crate::profile::identity::Drag::parse(&drag.0)
                .is_some_and(|drag| !matches!(drag.item, crate::profile::identity::Item::Field(_)))
        }) {
            self.drop_target.set(self.hit_plot(x, y));
            return;
        }

        if self.drag.state.get_untracked().is_some_and(|drag| drag.3)
            && Self::inside(self.filter_button, x, y)
        {
            self.filters_open.set(true);
        }
        self.filter_drop_active
            .set(self.filters_open.get_untracked() && Self::inside(self.filter_drop, x, y));
        self.filter_input_ui
            .hover
            .set(if self.filters_open.get_untracked() {
                self.quaternion_slot_at(x, y)
            } else {
                None
            });
        self.bands
            .hover
            .set(self.bands.open.get_untracked() && Self::inside(self.bands.drop, x, y));
        self.drop_target.set(self.hit_plot(x, y));
    }
    pub fn finish_field_drag(self) {
        self.analysis.condition_hover.set(false);
        self.analysis.hover.set(false);
        self.bands.hover.set(false);
        self.filter_drop_active.set(false);
        self.filter_input_ui.hover.set(None);
        self.drop_target.set(None);
    }
    pub fn drop_fields(self, source: String, x: f32, y: f32) {
        if source.starts_with("@analysis/") {
            let sources = self
                .drag_fields(source)
                .into_iter()
                .filter_map(|source| source.strip_prefix("@analysis/").map(str::to_owned))
                .collect::<Vec<_>>();
            if self.analysis.open.get_untracked()
                && self.analysis.mode.get_untracked() == "heatmap"
                && Self::inside(self.analysis.condition_drop, x, y)
            {
                self.set_analysis_condition(sources);
            } else if let Some(target) = self.hit_any_plot(x, y) {
                self.drop_analysis_sources(sources, Some(target));
            }
            self.finish_field_drag();
            return;
        }
        if self.analysis.open.get_untracked()
            && !self.filters_open.get_untracked()
            && (Self::inside(self.analysis.drop, x, y)
                || self.analysis.mode.get_untracked() == "heatmap"
                    && Self::inside(self.analysis.condition_drop, x, y))
        {
            let sources = self.drag_fields(source);
            let conditioning = self.analysis.mode.get_untracked() == "heatmap"
                && Self::inside(self.analysis.condition_drop, x, y);
            if sources
                .first()
                .is_some_and(|source| source.starts_with("@profile/"))
            {
                self.use_profile_items(
                    sources
                        .iter()
                        .filter_map(|source| crate::profile::identity::Drag::parse(source))
                        .collect(),
                    if conditioning {
                        crate::profile_state::Destination::AnalysisCondition
                    } else {
                        crate::profile_state::Destination::Analysis
                    },
                );
            } else {
                if conditioning {
                    self.set_analysis_condition(sources);
                } else {
                    self.add_analysis_sources(sources);
                }
            }
            self.finish_field_drag();
            return;
        }
        if let Some(drag) = crate::profile::identity::Drag::parse(&source) {
            use crate::profile_state::Destination;
            let field = matches!(drag.item, crate::profile::identity::Item::Field(_));
            let target = if field
                && self.bands.open.get_untracked()
                && Self::inside(self.bands.drop, x, y)
            {
                Some(Destination::Bands)
            } else if field
                && (Self::inside(self.filter_button, x, y)
                    || self.filters_open.get_untracked() && Self::inside(self.filter_drop, x, y))
            {
                Some(Destination::Filter)
            } else {
                self.hit_plot(x, y)
                    .map(|pane| Destination::Plot(Some(pane)))
            };
            if let Some(target) = target {
                let items = self
                    .drag_fields(source)
                    .into_iter()
                    .filter_map(|source| crate::profile::identity::Drag::parse(&source))
                    .collect();
                self.use_profile_items(items, target);
            }
            self.finish_field_drag();
            return;
        }

        let sources = self.drag_fields(source);
        if self.bands.open.get_untracked() && Self::inside(self.bands.drop, x, y) {
            if sources.len() == 1 {
                self.generate_bands(sources[0].clone(), None);
            } else {
                self.bands
                    .error
                    .set("Drop one discrete source field".into());
            }
        } else if Self::inside(self.filter_button, x, y)
            || self.filters_open.get_untracked() && Self::inside(self.filter_drop, x, y)
        {
            self.filters_open.set(true);
            if let Err(error) =
                self.accept_filter_sources_at(sources, self.quaternion_slot_at(x, y))
            {
                self.filter_error.set(error);
            }
        } else if let Some(id) = self.hit_plot(x, y) {
            self.activate(id);
            for source in sources {
                self.add(source);
            }
        } else {
            self.status
                .set("Drop outside plots or filter source area cancelled".into());
        }
        self.finish_field_drag();
    }
    pub(crate) fn inside(node: NodeRef, x: f32, y: f32) -> bool {
        let Some(bounds) = node.window_bounds() else {
            return false;
        };
        let scale = node.scale();
        let (x, y) = (x * scale, y * scale);
        bounds.size.width.0 > 0.0
            && bounds.size.height.0 > 0.0
            && x >= bounds.origin.x.0
            && x < bounds.origin.x.0 + bounds.size.width.0
            && y >= bounds.origin.y.0
            && y < bounds.origin.y.0 + bounds.size.height.0
    }
    pub fn accept_filter_sources(self, sources: Vec<String>) -> Result<(), String> {
        self.accept_filter_sources_at(sources, None)
    }
    pub fn accept_filter_sources_at(
        self,
        sources: Vec<String>,
        slot: Option<usize>,
    ) -> Result<(), String> {
        self.cancel_filter_input_drag();
        let log = self.log.get_untracked().ok_or("Open a log first")?;
        let fields = self.derived.get_untracked();
        for source in &sources {
            if !log.catalogue.iter().any(|field| &field.id == source)
                && !fields.iter().any(|field| field.key() == *source)
            {
                return Err("Source field no longer exists".into());
            }
        }
        let mode = self.filter_mode.get_untracked();
        if !mode.is_new()
            && sources.iter().any(|source| {
                mode.ids()
                    .iter()
                    .any(|id| crate::derived::depends_on(&fields, source, *id))
            })
        {
            return Err("This input depends on a field being edited. Finish editing to create another field from it".into());
        }
        let editing = mode.selected().filter(|_| mode.ids().len() == 1);
        let mut inputs = self.filter_inputs.get_untracked();
        if self.combined_filter_untracked() {
            if self.filter_kind.get_untracked() == "rpy" {
                inputs.resize_with(4, crate::filter_input_drag::empty_input);
                let missing = inputs
                    .iter()
                    .filter(|input| input.source.is_empty())
                    .count();
                if sources.len() > 4
                    || slot.is_none()
                        && sources
                            .iter()
                            .filter(|source| !inputs.iter().any(|input| &input.source == *source))
                            .count()
                            > missing
                {
                    return Err(
                        "Quaternion has four slots. Drop onto W, X, Y, or Z to replace an input"
                            .into(),
                    );
                }
                let mut cursor = slot.unwrap_or(0).min(3);
                for source in sources {
                    let existing = inputs.iter().position(|input| input.source == source);
                    let target = if slot.is_some() {
                        let target = cursor;
                        cursor = (cursor + 1) % 4;
                        target
                    } else if existing.is_some() {
                        continue;
                    } else {
                        inputs
                            .iter()
                            .position(|input| input.source.is_empty())
                            .ok_or("Quaternion slots are full")?
                    };
                    if let Some(existing) = existing {
                        inputs.swap(existing, target);
                    } else {
                        inputs[target] = FilterInput {
                            name: format!("{} filtered", self.field_name_untracked(&source)),
                            source,
                        };
                    }
                }
            } else {
                inputs.retain(|input| !input.source.is_empty());
                for source in sources {
                    if !inputs.iter().any(|input| input.source == source) {
                        inputs.push(FilterInput {
                            name: format!("{} filtered", self.field_name_untracked(&source)),
                            source,
                        });
                    }
                }
            }
            self.filter_inputs.set(inputs);
            self.filter_error.set(String::new());
            return Ok(());
        }
        if editing.is_some() {
            if sources.len() == 1 {
                let name = inputs.first().map_or_else(
                    || {
                        fields
                            .iter()
                            .find(|field| Some(field.id) == editing)
                            .map_or_else(String::new, |field| field.name.clone())
                    },
                    |input| input.name.clone(),
                );
                inputs = vec![FilterInput {
                    source: sources[0].clone(),
                    name,
                }];
                self.filter_inputs.set(inputs);
                self.filter_error.set(String::new());
                return Ok(());
            }
            return Err(
                "Editing one saved field requires one source. Finish editing to create a batch"
                    .into(),
            );
        }
        for source in sources {
            if inputs.iter().any(|input| input.source == source) {
                continue;
            }
            if let Some(field) = fields
                .iter()
                .find(|field| mode.ids().contains(&field.id) && field.sources == [source.clone()])
            {
                inputs.push(FilterInput {
                    source,
                    name: field.name.clone(),
                });
                continue;
            }
            let label = fields
                .iter()
                .find(|field| field.key() == source)
                .map_or_else(
                    || self.field_name_untracked(&source),
                    |field| field.name.clone(),
                );
            let stem = format!("{label} filtered");
            let mut name = stem.clone();
            let mut suffix = 2;
            while fields
                .iter()
                .any(|field| field.name.to_lowercase() == name.to_lowercase())
                || inputs
                    .iter()
                    .any(|input| input.name.to_lowercase() == name.to_lowercase())
            {
                name = format!("{stem} {suffix}");
                suffix += 1;
            }
            inputs.push(FilterInput { source, name });
        }
        self.filter_inputs.set(inputs);
        self.filter_error.set(String::new());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_toggles_and_shift_ranges_follow_visible_order() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let visible = ["sensor/0/x", "sensor/0/y", "sensor/0/z", "other/0/x"].map(String::from);
        workbench.select_field(visible[1].clone(), false, false, &visible);
        workbench.select_field(visible[3].clone(), true, false, &visible);
        assert_eq!(workbench.selected_fields.get_untracked().len(), 2);
        workbench.select_field(visible[3].clone(), true, false, &visible);
        assert_eq!(workbench.selected_fields.get_untracked().len(), 1);
        workbench.select_field(visible[1].clone(), false, true, &visible);
        assert_eq!(workbench.selected_fields.get_untracked().len(), 3);
        workbench.select_field(visible[0].clone(), true, true, &visible);
        assert_eq!(workbench.selected_fields.get_untracked().len(), 4);
        workbench.select_field(visible[0].clone(), false, true, &visible[0..1]);
        assert_eq!(workbench.selected_fields.get_untracked().len(), 1);
        assert_eq!(
            workbench.selection_anchor.get_untracked(),
            Some(visible[0].clone())
        );
    }
}
