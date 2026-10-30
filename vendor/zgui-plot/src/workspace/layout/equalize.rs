use crate::workspace::layout::Layout;

impl Layout {
    /// Balance row/column sizes while preserving tab groups and split directions.
    pub fn equalize(&mut self) {
        equalize(self);
    }
}

fn equalize(layout: &mut Layout) {
    if let Layout::Split {
        ratio,
        first,
        second,
        stacked,
        ..
    } = layout
    {
        equalize(first);
        equalize(second);
        let (a, b) = (units(first, *stacked), units(second, *stacked));
        *ratio = a / (a + b);
    }
}
fn units(layout: &Layout, axis: bool) -> f64 {
    match layout {
        Layout::Split {
            first,
            second,
            stacked,
            ..
        } => {
            let (a, b) = (units(first, axis), units(second, axis));
            if *stacked == axis { a + b } else { a.max(b) }
        }
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equalize_preserves_tabs_and_balances_three_columns() {
        let mut layout = Layout::flat(&[(1, 5.0), (2, 1.0), (3, 2.0)], false);
        layout.add_plot_tab(2, 4);
        equalize(&mut layout);
        assert_eq!(layout.tab_ids(2), [2, 4]);
        let mut plots = Vec::new();
        let mut dividers = Vec::new();
        layout.geometry(
            crate::workspace::layout::Rect {
                width: 1500.0,
                height: 800.0,
                ..Default::default()
            },
            &mut plots,
            &mut dividers,
        );
        let widths: Vec<_> = plots.iter().map(|(_, rect)| rect.width).collect();
        assert!(widths.iter().all(|width| (*width - widths[0]).abs() < 4.0));
        assert_eq!(dividers.len(), 2);
    }
    #[test]
    fn mixed_docks_equalize_columns_and_rows() {
        let mut layout = Layout::Plot(1);
        layout.split(1, 2, false);
        layout.split(2, 3, true);
        equalize(&mut layout);
        let mut plots = Vec::new();
        let mut dividers = Vec::new();
        layout.geometry(
            crate::workspace::layout::Rect {
                width: 1800.0,
                height: 1200.0,
                ..Default::default()
            },
            &mut plots,
            &mut dividers,
        );
        assert!((plots[0].1.width - plots[1].1.width).abs() < 1.0);
        assert!((plots[1].1.height - plots[2].1.height).abs() < 1.0);
        assert!(plots[0].1.height > plots[1].1.height * 1.9);
    }
}
