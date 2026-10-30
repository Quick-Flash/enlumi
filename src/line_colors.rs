use crate::state::{Pane, Workbench};
use plot::plot_data::SeriesStyle;
use zgui::prelude::*;
// Betaflight Blackbox Explorer's GraphConfig.PALETTE, in display order:
// https://github.com/betaflight/blackbox-log-viewer/blob/master/src/graph_config.js
pub const PALETTE: [[u8; 4]; 12] = [
    [251, 128, 114, 255],
    [141, 211, 199, 255],
    [255, 255, 179, 255],
    [190, 186, 218, 255],
    [128, 177, 211, 255],
    [253, 180, 98, 255],
    [179, 222, 105, 255],
    [252, 205, 229, 255],
    [217, 217, 217, 255],
    [188, 128, 189, 255],
    [204, 235, 197, 255],
    [255, 237, 111, 255],
];

pub(crate) fn next_color(local: &[[u8; 4]]) -> [u8; 4] {
    *PALETTE
        .iter()
        .min_by_key(|color| local.iter().filter(|used| used[..3] == color[..3]).count())
        .unwrap()
}

impl Workbench {
    pub fn next_line_color(self, pane: Pane) -> [u8; 4] {
        next_color(&pane.display().colors())
    }
    pub fn ensure_line_colors(self, pane: Pane) {
        let lines = pane.lines.get_untracked();
        pane.plot.styles.update(|styles| {
            let mut used = lines
                .iter()
                .filter_map(|line| styles.get(&line.id.to_string()).map(|style| style.color))
                .collect::<Vec<_>>();
            for line in &lines {
                let key = line.id.to_string();
                styles.entry(key.clone()).or_insert_with(|| {
                    let mut style = SeriesStyle::for_id(&key);
                    style.color = next_color(&used);
                    used.push(style.color);
                    style
                });
            }
        });
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
    fn blackbox_palette_cycles_independently_in_each_plot() {
        let window = zgui_testkit_view::Window::open();
        let workbench = window.scope.with(Workbench::new);
        let first = workbench.pane();
        let second = window
            .scope
            .with(|| workbench.add_plot(first.id, false, false));
        let data = plot::time_series::SeriesData::new(vec![]).unwrap();
        for index in 0..PALETTE.len() + 2 {
            for pane in [first, second] {
                let id = workbench.id();
                workbench.assign_line_color(pane, id);
                assert_eq!(
                    pane.plot.style(&id.to_string()).color,
                    PALETTE[index % PALETTE.len()]
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
        }
        let free = first.lines.with_untracked(|lines| lines[2].id);
        first.remove_line(free);
        assert_eq!(workbench.next_line_color(first), PALETTE[2]);
        assert_eq!(workbench.next_line_color(second), PALETTE[2]);
        // The first plot's removal must not change the second plot's sequence.
        first.remove_line(first.lines.with_untracked(|lines| lines[2].id));
        assert_eq!(workbench.next_line_color(second), PALETTE[2]);
    }
}
