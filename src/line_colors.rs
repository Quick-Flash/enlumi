use crate::state::{Pane, Workbench};
#[cfg(test)]
use plot::palette::DEFAULT_PALETTE as PALETTE;
use plot::plot_data::SeriesStyle;
use zgui::prelude::*;
impl Workbench {
    pub fn next_line_color(self, pane: Pane) -> [u8; 4] {
        let global = self.panes.with_untracked(|panes| {
            panes
                .iter()
                .flat_map(|pane| pane.display().colors())
                .collect::<Vec<_>>()
        });
        plot::palette::next_color(&pane.display().colors(), &global)
    }
    pub fn assign_line_color(self, pane: Pane, id: u64) {
        let color = self.next_line_color(pane);
        let key = id.to_string();
        let mut style = SeriesStyle::for_id(&key);
        style.color = color;
        pane.plot.styles.update(|styles| {
            styles.insert(key, style);
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Line;

    #[test]
    fn palette_avoids_global_reuse_then_reuses_least_used_colors() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let first = workbench.pane();
        let second = window
            .scope
            .with(|| workbench.add_plot(first.id, false, false));
        let data = plot::time_series::SeriesData::new(vec![]).unwrap();
        for index in 0..PALETTE.len() + 2 {
            let id = index as u64 + 1;
            let pane = if index % 2 == 0 { first } else { second };
            workbench.assign_line_color(pane, id);
            let color = pane
                .plot
                .styles
                .with_untracked(|styles| styles[&id.to_string()].color);
            assert_eq!(
                color,
                PALETTE[if index < PALETTE.len() {
                    index
                } else {
                    1 - index % 2
                }]
            );
            pane.lines.update(|lines| {
                lines.push(Line {
                    id,
                    source: id.to_string(),
                    name: id.to_string(),
                    data: data.clone(),
                    revision: 0,
                })
            });
        }
        // Free a color still used only once; new series prefers it over reused colors.
        first.remove_line(3);
        workbench.assign_line_color(second, 20);
        assert_eq!(
            second
                .plot
                .styles
                .with_untracked(|styles| styles["20"].color),
            PALETTE[2]
        );
    }
}
