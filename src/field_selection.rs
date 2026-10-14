use crate::{derived::FilterInput, state::Workbench};
use zgui::prelude::*;

impl Workbench {
    pub fn select_field(self, source: String, ctrl: bool, shift: bool, visible: &[String]) {
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
        self.filter_inputs.set(Vec::new());
        self.filter_outputs.set(Vec::new());
        self.filter_edit.set(None);
        self.filter_error.set(String::new());
        self.finish_field_drag();
    }
    fn drag_fields(self, source: String) -> Vec<String> {
        if self
            .selected_fields
            .with_untracked(|fields| fields.contains(&source))
        {
            let mut fields: Vec<_> = self.selected_fields.get_untracked().into_iter().collect();
            fields.sort_by(|a, b| crate::field_tree::natural_cmp(a, b));
            fields
        } else {
            vec![source]
        }
    }
    pub fn field_drag_move(self, x: f32, y: f32) {
        if self.drag.state.get_untracked().is_some_and(|drag| drag.3)
            && Self::inside(self.filter_button, x, y)
        {
            self.filters_open.set(true);
        }
        self.filter_drop_active
            .set(self.filters_open.get_untracked() && Self::inside(self.filter_drop, x, y));
        self.drop_target.set(self.hit_plot(x, y));
    }
    pub fn finish_field_drag(self) {
        self.filter_drop_active.set(false);
        self.drop_target.set(None);
    }
    pub fn drop_fields(self, source: String, x: f32, y: f32) {
        let sources = self.drag_fields(source);
        if Self::inside(self.filter_button, x, y)
            || self.filters_open.get_untracked() && Self::inside(self.filter_drop, x, y)
        {
            self.filters_open.set(true);
            if let Err(error) = self.accept_filter_sources(sources) {
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
    fn inside(node: NodeRef, x: f32, y: f32) -> bool {
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
        let log = self.log.get_untracked().ok_or("Open a log first")?;
        let fields = self.derived.get_untracked();
        for source in &sources {
            if !log.catalogue.iter().any(|field| &field.id == source)
                && !fields.iter().any(|field| field.key() == *source)
            {
                return Err("Source field no longer exists".into());
            }
            if let Some(editing) = self.filter_edit.get_untracked()
                && sources.len() == 1
                && crate::derived::depends_on(&fields, source, editing)
            {
                return Err("Source would create a filter cycle".into());
            }
        }
        let editing = self.filter_edit.get_untracked();
        let mut inputs = self.filter_inputs.get_untracked();
        if editing.is_some() {
            if sources.len() == 1 {
                let name = inputs
                    .first()
                    .map_or_else(String::new, |input| input.name.clone());
                inputs = vec![FilterInput {
                    source: sources[0].clone(),
                    name,
                }];
                self.filter_inputs.set(inputs);
                self.filter_error.set(String::new());
                return Ok(());
            }
            self.filter_edit.set(None);
            inputs.clear();
            self.filter_outputs.set(Vec::new());
        }
        for source in sources {
            if inputs.iter().any(|input| input.source == source) {
                continue;
            }
            let label = fields
                .iter()
                .find(|field| field.key() == source)
                .map_or_else(|| source.clone(), |field| field.name.clone());
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
