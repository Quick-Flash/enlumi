use super::*;

#[derive(Clone)]
struct Part {
    layout: Layout,
    span: f64,
}

impl Layout {
    /// Dock against any contiguous band, regrouping parallel splits without moving other bands.
    pub fn dock_region(
        &mut self,
        source: u64,
        members: &[u64],
        zone: DropZone,
        viewport: (f64, f64),
    ) -> bool {
        let (stacked, before) = match zone {
            DropZone::Left => (false, true),
            DropZone::Right => (false, false),
            DropZone::Top => (true, true),
            DropZone::Bottom => (true, false),
            _ => return false,
        };
        let mut members = members
            .iter()
            .copied()
            .filter(|id| *id != source)
            .collect::<Vec<_>>();
        members.sort_unstable();
        members.dedup();
        if members.is_empty()
            || !self.contains(source)
            || members.iter().any(|id| !self.contains(*id))
        {
            return false;
        }
        let split_id = self
            .max_split_id()
            .checked_add(1)
            .expect("split IDs exhausted");
        let mut updated = self.clone();
        updated.remove(source);
        if !updated.wrap_group(&members, source, stacked, before, split_id) {
            let minimum = updated.minimum();
            let mut plots = vec![];
            updated.geometry(
                Rect {
                    width: viewport.0.max(minimum.0),
                    height: viewport.1.max(minimum.1),
                    ..Rect::default()
                },
                &mut plots,
                &mut vec![],
            );
            if !regroup(
                &mut updated,
                &members,
                source,
                (stacked, before, split_id),
                &plots,
            ) {
                return false;
            }
        }
        *self = updated;
        true
    }
}

fn regroup(
    layout: &mut Layout,
    members: &[u64],
    source: u64,
    insertion: (bool, bool, u64),
    plots: &[(u64, Rect)],
) -> bool {
    let Layout::Split { stacked, .. } = layout else {
        return false;
    };
    let axis = *stacked;
    let mut parts = vec![];
    let mut boundaries = vec![];
    flatten(layout, axis, plots, &mut parts, &mut boundaries);
    for start in 0..parts.len() {
        let mut ids = vec![];
        for end in start..parts.len() {
            ids.extend(parts[end].layout.plot_ids());
            ids.sort_unstable();
            if ids == members && end > start {
                let span = parts[start..=end].iter().map(|part| part.span).sum::<f64>()
                    + 6.0 * (end - start) as f64;
                let band = assemble(&parts[start..=end], &boundaries[start..end], axis);
                let (stacked, before, id) = insertion;
                let mut new_parts = parts[..start].to_vec();
                new_parts.push(Part {
                    layout: split_pair(band, source, stacked, before, id),
                    span,
                });
                new_parts.extend_from_slice(&parts[end + 1..]);
                let mut new_boundaries = boundaries[..start].to_vec();
                new_boundaries.extend_from_slice(&boundaries[end..]);
                *layout = assemble(&new_parts, &new_boundaries, axis);
                return true;
            }
            if ids.len() > members.len() {
                break;
            }
        }
    }
    if let Layout::Split { first, second, .. } = layout {
        regroup(first, members, source, insertion, plots)
            || regroup(second, members, source, insertion, plots)
    } else {
        false
    }
}

fn flatten(
    layout: &Layout,
    axis: bool,
    plots: &[(u64, Rect)],
    parts: &mut Vec<Part>,
    boundaries: &mut Vec<u64>,
) {
    if let Layout::Split {
        id,
        stacked,
        first,
        second,
        ..
    } = layout
        && *stacked == axis
    {
        flatten(first, axis, plots, parts, boundaries);
        boundaries.push(*id);
        flatten(second, axis, plots, parts, boundaries);
    } else {
        let ids = layout.plot_ids();
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for (_, rect) in plots.iter().filter(|(id, _)| ids.contains(id)) {
            let (at, size) = if axis {
                (rect.y, rect.height)
            } else {
                (rect.x, rect.width)
            };
            min = min.min(at);
            max = max.max(at + size);
        }
        parts.push(Part {
            layout: layout.clone(),
            span: (max - min).max(0.0),
        });
    }
}

fn assemble(parts: &[Part], boundaries: &[u64], stacked: bool) -> Layout {
    if parts.len() == 1 {
        return parts[0].layout.clone();
    }
    let span = parts.iter().map(|part| part.span).sum::<f64>() + 6.0 * (parts.len() - 2) as f64;
    Layout::Split {
        id: boundaries[0],
        stacked,
        ratio: parts[0].span / span.max(1.0),
        first: Box::new(parts[0].layout.clone()),
        second: Box::new(assemble(&parts[1..], &boundaries[1..], stacked)),
    }
}
