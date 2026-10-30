use crate::{PlotPoint, time_series::SeriesData};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Connection {
    #[default]
    Linear,
    Pre,
    Post,
}
/// Pixel-spaced line pattern, independent of sample-marker shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinePattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
    DashDot,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LineDraw {
    pub connection: Connection,
    pub pattern: LinePattern,
    pub markers: bool,
}

pub fn prepare(data: &SeriesData, connection: Connection) -> SeriesData {
    if connection == Connection::Linear {
        return data.clone();
    }
    let mut result = Vec::with_capacity(data.samples().len().saturating_mul(2));
    let mut previous: Option<PlotPoint> = None;
    for &point in data.samples() {
        if let Some(old) = previous
            && old.y.is_finite()
            && point.y.is_finite()
        {
            result.push(match connection {
                Connection::Pre => PlotPoint {
                    x: old.x,
                    y: point.y,
                },
                Connection::Post => PlotPoint {
                    x: point.x,
                    y: old.y,
                },
                Connection::Linear => unreachable!(),
            });
        }
        result.push(point);
        previous = Some(point);
    }
    SeriesData::new(result).expect("steps retain sorted finite timestamps")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steps_jump_before_or_after_interval_and_keep_gaps() {
        let points = vec![
            PlotPoint { x: 1.0, y: 2.0 },
            PlotPoint { x: 3.0, y: 4.0 },
            PlotPoint {
                x: 4.0,
                y: f64::NAN,
            },
            PlotPoint { x: 5.0, y: 8.0 },
        ];
        let data = SeriesData::new(points).unwrap();
        let pre = prepare(&data, Connection::Pre);
        let post = prepare(&data, Connection::Post);
        assert_eq!(pre.samples()[1], PlotPoint { x: 1.0, y: 4.0 });
        assert_eq!(post.samples()[1], PlotPoint { x: 3.0, y: 2.0 });
        assert_eq!(pre.samples().len(), 5);
        assert!(pre.samples()[3].y.is_nan());
        assert_eq!(
            data.samples().len(),
            4,
            "Render geometry must not change source samples"
        );
    }
}
