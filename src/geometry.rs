//! Named, session-only geometry. Membership is per plot; definitions are shared.
use kurbo::{BezPath, Point, Rect};
use plot::{PlotGeometry, Ranges};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryKind {
    Rectangle,
    Line,
    Band,
}
impl GeometryKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Line => "Line",
            Self::Band => "Time band",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineOrientation {
    #[default]
    Free,
    Horizontal,
    Vertical,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shade {
    #[default]
    None,
    Above,
    Below,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub id: u64,
    pub name: String,
    pub kind: GeometryKind,
    pub coordinates: [f64; 4],
    pub color: [u8; 3],
    pub opacity: f64,
    pub width: f64,
    pub shade: Shade,
    pub extend: bool,
    pub orientation: LineOrientation,
    pub states: Option<std::sync::Arc<crate::state_bands::StateBands>>,
    pub labels: bool,
}
impl Geometry {
    pub fn new(id: u64, kind: GeometryKind, point: Point) -> Self {
        Self {
            id,
            name: format!("{} {id}", kind.name()),
            kind,
            coordinates: [point.x, point.y, point.x, point.y],
            color: [235, 174, 82],
            opacity: 0.18,
            width: 2.0,
            shade: Shade::None,
            extend: true,
            orientation: LineOrientation::Free,
            states: None,
            labels: true,
        }
    }
    pub fn valid(&self) -> bool {
        self.coordinates.iter().all(|v| v.is_finite())
            && (self.kind != GeometryKind::Line
                || match self.orientation {
                    LineOrientation::Free => true,
                    LineOrientation::Horizontal => self.coordinates[1] == self.coordinates[3],
                    LineOrientation::Vertical => self.coordinates[0] == self.coordinates[2],
                })
            && self.width.is_finite()
            && (0.5..=10.0).contains(&self.width)
            && self.opacity.is_finite()
            && (0.0..=1.0).contains(&self.opacity)
    }
    pub fn nonempty(&self) -> bool {
        let [x1, y1, x2, y2] = self.coordinates;
        match self.kind {
            GeometryKind::Rectangle => x1 != x2 && y1 != y2,
            GeometryKind::Band => self.states.is_some() || x1 != x2,
            GeometryKind::Line => x1 != x2 || y1 != y2,
        }
    }
    pub fn handles(&self) -> Vec<Point> {
        let [x1, y1, x2, y2] = self.coordinates;
        match self.kind {
            GeometryKind::Rectangle => vec![
                (x1, y1).into(),
                (x2, y1).into(),
                (x2, y2).into(),
                (x1, y2).into(),
            ],
            GeometryKind::Line => vec![(x1, y1).into(), (x2, y2).into()],
            GeometryKind::Band => Vec::new(),
        }
    }
    /// Move handle stays on visible line even if defining endpoints are offscreen.
    pub fn move_handle(&self, ranges: Ranges, geometry: PlotGeometry) -> Option<Point> {
        if self.states.is_some() {
            return None;
        }
        if self.kind == GeometryKind::Band {
            let [x1, _, x2, _] = self.coordinates;
            let left = x1.min(x2).max(ranges.x.min);
            let right = x1.max(x2).min(ranges.x.max);
            return (right > left).then(|| {
                Point::new(
                    geometry.x(left * 0.5 + right * 0.5, ranges.x),
                    geometry.height * 0.5,
                )
            });
        }
        let transform = geometry.data_transform(ranges);
        let [x1, y1, x2, y2] = self.coordinates;
        let view = Rect::new(ranges.x.min, ranges.y.min, ranges.x.max, ranges.y.max);
        let center = if self.kind == GeometryKind::Line {
            let (a, b) = if self.extend {
                clip_infinite_line(self.coordinates, view)
            } else {
                clip_line(Point::new(x1, y1), Point::new(x2, y2), view)
            }?;
            a + (b - a) * 0.5
        } else {
            let inside = Rect::new(x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)).intersect(view);
            if inside.width() <= 0. || inside.height() <= 0. {
                return None;
            }
            inside.center()
        };
        Some(transform * center)
    }
    pub fn moved(&self, mut delta: kurbo::Vec2) -> Self {
        if self.states.is_some() {
            return self.clone();
        }
        if self.kind == GeometryKind::Band {
            delta.y = 0.;
        }
        if self.kind == GeometryKind::Line && self.extend {
            match self.orientation {
                LineOrientation::Horizontal => delta.x = 0.,
                LineOrientation::Vertical => delta.y = 0.,
                LineOrientation::Free => {}
            }
        }
        let mut copy = self.clone();
        for pair in copy.coordinates.chunks_exact_mut(2) {
            pair[0] += delta.x;
            pair[1] += delta.y;
        }
        copy
    }
    pub fn resize(&self, handle: usize, point: Point) -> Self {
        if self.states.is_some() {
            return self.clone();
        }
        if self.kind == GeometryKind::Band {
            let mut copy = self.clone();
            copy.coordinates[if handle == 0 { 0 } else { 2 }] = point.x;
            return copy;
        }
        let mut copy = self.clone();
        let c = &mut copy.coordinates;
        match (self.kind, handle) {
            (GeometryKind::Rectangle, 0) | (GeometryKind::Line, 0) => {
                c[0] = point.x;
                c[1] = point.y;
            }
            (GeometryKind::Rectangle, 1) => {
                c[2] = point.x;
                c[1] = point.y;
            }
            (GeometryKind::Rectangle, 2) | (GeometryKind::Line, 1) => {
                c[2] = point.x;
                c[3] = point.y;
            }
            (GeometryKind::Rectangle, 3) => {
                c[0] = point.x;
                c[3] = point.y;
            }
            _ => {}
        }
        if self.kind == GeometryKind::Line {
            match self.orientation {
                LineOrientation::Horizontal => {
                    c[1] = self.coordinates[1];
                    c[3] = self.coordinates[1];
                }
                LineOrientation::Vertical => {
                    c[0] = self.coordinates[0];
                    c[2] = self.coordinates[0];
                }
                LineOrientation::Free => {}
            }
        }
        copy
    }
    pub fn paths(&self, ranges: Ranges, geometry: PlotGeometry) -> (BezPath, BezPath) {
        let view = Rect::new(ranges.x.min, ranges.y.min, ranges.x.max, ranges.y.max);
        let transform = geometry.data_transform(ranges);
        let [x1, y1, x2, y2] = self.coordinates;
        let mut fill = BezPath::new();
        let mut stroke = BezPath::new();
        if !self.valid() || !self.nonempty() {
            return (fill, stroke);
        }
        if self.kind == GeometryKind::Band {
            return (fill, stroke);
        }
        if self.kind == GeometryKind::Rectangle {
            let rect = Rect::new(x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2));
            let inside = rect.intersect(view);
            if inside.width() > 0.0 && inside.height() > 0.0 {
                fill = polygon(&[
                    (inside.x0, inside.y0).into(),
                    (inside.x1, inside.y0).into(),
                    (inside.x1, inside.y1).into(),
                    (inside.x0, inside.y1).into(),
                ]);
            }
            let corners = self.handles();
            for i in 0..4 {
                if let Some((a, b)) = clip_line(corners[i], corners[(i + 1) % 4], view) {
                    stroke.move_to(a);
                    stroke.line_to(b);
                }
            }
        } else {
            let segment = if self.extend {
                clip_infinite_line(self.coordinates, view)
            } else {
                clip_line(Point::new(x1, y1), Point::new(x2, y2), view)
            };
            if let Some((a, b)) = segment {
                stroke.move_to(a);
                stroke.line_to(b);
            }
            if self.shade != Shade::None && x1 != x2 {
                let bounds = if self.extend {
                    view
                } else {
                    view.intersect(Rect::new(x1.min(x2), view.y0, x1.max(x2), view.y1))
                };
                if bounds.width() > 0.0 {
                    let vertices = vec![
                        (bounds.x0, bounds.y0).into(),
                        (bounds.x1, bounds.y0).into(),
                        (bounds.x1, bounds.y1).into(),
                        (bounds.x0, bounds.y1).into(),
                    ];
                    fill = polygon(&clip_half_plane(&vertices, self.coordinates, self.shade));
                }
            }
        }
        (transform * fill, transform * stroke)
    }
}
pub fn polygon(points: &[Point]) -> BezPath {
    let mut points = points.to_vec();
    points.dedup();
    if points.first() == points.last() {
        points.pop();
    }
    let mut path = BezPath::new();
    if points.len() < 3 {
        return path;
    }
    let origin = points[0];
    let area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| (*a - origin).cross(*b - origin))
        .sum::<f64>();
    if !area.is_finite() || area == 0.0 {
        return path;
    }
    path.move_to(points[0]);
    for point in &points[1..] {
        path.line_to(*point);
    }
    path.close_path();
    path
}
/// Implicit line equation: no slope division, stable at almost-vertical angles.
/// Canonical endpoint order makes positive distance mean greater data Y.
fn line_side(point: Point, [x1, y1, x2, y2]: [f64; 4]) -> f64 {
    let (a, b) = if x1 < x2 || (x1 == x2 && y1 <= y2) {
        (Point::new(x1, y1), Point::new(x2, y2))
    } else {
        (Point::new(x2, y2), Point::new(x1, y1))
    };
    let d = b - a;
    let scale = d.x.abs().max(d.y.abs());
    if scale == 0.0 || !scale.is_finite() {
        return f64::NAN;
    }
    (d / scale).cross(point - a)
}
fn intersection(a: Point, b: Point, da: f64, db: f64) -> Point {
    let scale = da.abs().max(db.abs());
    let da = da / scale;
    let db = db / scale;
    a + (b - a) * (da / (da - db)).clamp(0.0, 1.0)
}
fn clip_infinite_line(coordinates: [f64; 4], rect: Rect) -> Option<(Point, Point)> {
    let corners = [
        Point::new(rect.x0, rect.y0),
        Point::new(rect.x1, rect.y0),
        Point::new(rect.x1, rect.y1),
        Point::new(rect.x0, rect.y1),
    ];
    let mut hits = Vec::new();
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        let da = line_side(a, coordinates);
        let db = line_side(b, coordinates);
        if !da.is_finite() || !db.is_finite() {
            return None;
        }
        if da == 0.0 && !hits.contains(&a) {
            hits.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            let hit = intersection(a, b, da, db);
            if !hits.contains(&hit) {
                hits.push(hit);
            }
        }
    }
    (hits.len() >= 2).then(|| (hits[0], hits[1]))
}
pub fn clip_line(a: Point, b: Point, rect: Rect) -> Option<(Point, Point)> {
    if ![a.x, a.y, b.x, b.y].iter().all(|v| v.is_finite()) {
        return None;
    }
    let d = b - a;
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-d.x, a.x - rect.x0),
        (d.x, rect.x1 - a.x),
        (-d.y, a.y - rect.y0),
        (d.y, rect.y1 - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
        }
        if lo > hi {
            return None;
        }
    }
    Some((a + d * lo, a + d * hi))
}
pub fn clip_half_plane(points: &[Point], [x1, y1, x2, y2]: [f64; 4], shade: Shade) -> Vec<Point> {
    if points.is_empty() || x1 == x2 {
        return Vec::new();
    }
    if shade == Shade::None {
        return Vec::new();
    }
    let sign = if shade == Shade::Above { 1.0 } else { -1.0 };
    let distance = |p: Point| sign * line_side(p, [x1, y1, x2, y2]);
    let mut output = Vec::new();
    let mut a = *points.last().expect("nonempty polygon");
    let mut da = distance(a);
    for &b in points {
        let db = distance(b);
        if !da.is_finite() || !db.is_finite() {
            return Vec::new();
        }
        if (da >= 0.0) != (db >= 0.0) {
            output.push(intersection(a, b, da, db));
        }
        if db >= 0.0 {
            output.push(b);
        }
        a = b;
        da = db;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;
    #[test]
    fn axis_locked_lines_keep_direction_and_visible_move_handles() {
        let mut line = Geometry::new(1, GeometryKind::Line, Point::ZERO);
        line.coordinates = [-100., 5., 100., 5.];
        line.orientation = LineOrientation::Horizontal;
        let moved = line.moved((4., 2.).into());
        assert_eq!(moved.coordinates, [-100., 7., 100., 7.]);
        let resized = line.resize(1, (80., 3.).into());
        assert_eq!(resized.coordinates, [-100., 5., 80., 5.]);
        let view = Ranges::new((0., 10.), (0., 10.));
        let geometry = PlotGeometry::with_bounds(0., 0., 100., 100., 100., 100.).unwrap();
        assert_eq!(line.move_handle(view, geometry), Some(Point::new(50., 50.)));
        line.coordinates = [5., -100., 5., 100.];
        line.orientation = LineOrientation::Vertical;
        assert_eq!(
            line.moved((2., 4.).into()).coordinates,
            [7., -100., 7., 100.]
        );
        assert_eq!(
            line.resize(0, (3., -50.).into()).coordinates,
            [5., -50., 5., 100.]
        );
        assert_eq!(line.move_handle(view, geometry), Some(Point::new(50., 50.)));
        line.extend = false;
        assert_eq!(
            line.moved((2., 4.).into()).coordinates,
            [7., -96., 7., 104.]
        );
    }
    #[test]
    fn shading_tracks_data_y_and_endpoint_order() {
        let square = [
            (0., 0.).into(),
            (10., 0.).into(),
            (10., 10.).into(),
            (0., 10.).into(),
        ];
        for coords in [[0., 0., 10., 10.], [10., 10., 0., 0.]] {
            let above = clip_half_plane(&square, coords, Shade::Above);
            let below = clip_half_plane(&square, coords, Shade::Below);
            assert!((polygon(&above).area().abs() - 50.).abs() < 1e-9);
            assert!((polygon(&below).area().abs() - 50.).abs() < 1e-9);
            assert!(above.iter().all(|p| p.y >= p.x));
            assert!(below.iter().all(|p| p.y <= p.x));
        }
        assert!(clip_half_plane(&square, [5., 0., 5., 10.], Shade::Above).is_empty());
    }
    #[test]
    fn steep_reversed_and_offscreen_shading_stays_finite_and_complementary() {
        let view = Ranges::new((0., 10.), (0., 10.));
        let geometry = PlotGeometry::with_bounds(0., 0., 100., 100., 100., 100.).unwrap();
        for coords in [
            [5., 0., 5. + 1e-12, 10.],
            [5. + 1e-12, 10., 5., 0.],
            [-1., 0., 11., 1e12],
            [11., 1e12, -1., 0.],
            [0., -20., 10., -10.],
            [0., 20., 10., 30.],
            [0., 10., 10., 0.],
        ] {
            let mut item = Geometry::new(1, GeometryKind::Line, Point::ZERO);
            item.coordinates = coords;
            item.shade = Shade::Above;
            let (above, stroke) = item.paths(view, geometry);
            item.shade = Shade::Below;
            let (below, _) = item.paths(view, geometry);
            assert!(
                (above.area().abs() + below.area().abs() - 10000.).abs() < 1e-5,
                "{:?} above {} below {}",
                coords,
                above.area(),
                below.area()
            );
            for path in [&above, &below, &stroke] {
                for el in path.elements() {
                    match el {
                        kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => assert!(
                            p.x.is_finite()
                                && p.y.is_finite()
                                && (-1e-8..=100. + 1e-8).contains(&p.x)
                                && (-1e-8..=100. + 1e-8).contains(&p.y)
                        ),
                        _ => {}
                    }
                }
            }
        }
        assert!(polygon(&[(0., 0.).into(), (1., 1.).into(), (1., 1.).into()]).is_empty());
        assert!(polygon(&[(0., 0.).into(), (1., 1.).into(), (2., 2.).into()]).is_empty());
    }
    #[test]
    fn finite_and_extended_lines_shade_correct_view_region() {
        let mut item = Geometry::new(1, GeometryKind::Line, Point::ZERO);
        item.coordinates = [2., 5., 8., 5.];
        item.shade = Shade::Above;
        let view = Ranges::new((0., 10.), (0., 10.));
        let geometry = PlotGeometry::with_bounds(0., 0., 100., 100., 100., 100.).unwrap();
        let (fill, stroke) = item.paths(view, geometry);
        assert_eq!(fill.bounding_box(), Rect::new(0., 0., 100., 50.));
        assert_eq!(stroke.bounding_box(), Rect::new(0., 50., 100., 50.));
        item.extend = false;
        let (fill, stroke) = item.paths(view, geometry);
        assert_eq!(fill.bounding_box(), Rect::new(20., 0., 80., 50.));
        assert_eq!(stroke.bounding_box(), Rect::new(20., 50., 80., 50.));
        item.coordinates = [5., 2., 5., 8.];
        assert!(item.paths(view, geometry).0.is_empty());
        item.coordinates[0] = f64::NAN;
        let (fill, stroke) = item.paths(view, geometry);
        assert!(fill.is_empty() && stroke.is_empty());
    }
    #[test]
    fn clipping_and_transforms_do_not_add_fake_rectangle_edges() {
        let mut item = Geometry::new(1, GeometryKind::Rectangle, Point::ZERO);
        item.coordinates = [-5., 2., 5., 8.];
        let (fill, stroke) = item.paths(
            Ranges::new((0., 10.), (0., 10.)),
            PlotGeometry::with_bounds(0., 0., 100., 100., 100., 100.).unwrap(),
        );
        assert_eq!(fill.bounding_box(), Rect::new(0., 20., 50., 80.));
        assert_eq!(
            stroke.elements().len(),
            6,
            "Only three original edges intersect view"
        );
        let moved = item.moved((2., 3.).into());
        assert_eq!(moved.coordinates, [-3., 5., 7., 11.]);
        assert_eq!(
            item.resize(1, (9., 4.).into()).coordinates,
            [-5., 4., 9., 8.]
        );
        assert!(
            clip_line(
                (-5., -5.).into(),
                (-1., -1.).into(),
                Rect::new(0., 0., 10., 10.)
            )
            .is_none()
        );
        assert!(
            clip_line(
                (f64::NAN, 0.).into(),
                (1., 1.).into(),
                Rect::new(0., 0., 10., 10.)
            )
            .is_none()
        );
    }
}
