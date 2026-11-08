use crate::monitor::{work_area_scale, MonitorRect};

const DESKTOP_SNAP_DISTANCE: i32 = 20;

pub(super) struct MonitorArea {
    pub bounds: MonitorRect,
    pub work: MonitorRect,
}

pub(super) struct DragLayout {
    pub bounds: MonitorRect,
    pub origin: (i32, i32),
    pub source: Option<MonitorArea>,
    pub snap: bool,
    pub resize: bool,
}

pub(super) struct Placement {
    origin: (i32, i32),
    cursor: (i32, i32),
    scale: (f32, f32),
    shift: (i32, i32),
}

impl Placement {
    pub fn rect(&self, rect: &MonitorRect) -> MonitorRect {
        let map_x = |x| {
            self.cursor.0
                + ((x - self.origin.0) as f32 * self.scale.0).round() as i32
                + self.shift.0
        };
        let map_y = |y| {
            self.cursor.1
                + ((y - self.origin.1) as f32 * self.scale.1).round() as i32
                + self.shift.1
        };
        MonitorRect {
            left: map_x(rect.left),
            right: map_x(rect.right),
            top: map_y(rect.top),
            bottom: map_y(rect.bottom),
        }
    }

    pub fn resizes(&self) -> bool {
        self.scale != (1.0, 1.0)
    }
}

impl DragLayout {
    pub fn placement(
        &self,
        cursor: (i32, i32),
        destination: Option<&MonitorArea>,
        finished: bool,
    ) -> Placement {
        let mut placement = Placement {
            origin: self.origin,
            cursor,
            scale: (1.0, 1.0),
            shift: (0, 0),
        };
        let Some(destination) = destination else {
            return placement;
        };
        let unscaled = placement.rect(&self.bounds);
        // ドラッグ中は元の大きさで追従し、ボタンを離したときだけ拡縮する。
        if let Some(source) = &self.source {
            if finished && self.resize && source.bounds.size() != destination.bounds.size() {
                placement.scale = work_area_scale(&source.work, &destination.work);
            }
        }
        if self.snap {
            let work = &destination.work;
            let (preview_x, preview_y) = snap_edges(&unscaled, work);
            let final_bounds = placement.rect(&self.bounds);
            let (final_x, final_y) = snap_edges(&final_bounds, work);
            // 拡縮時も、ドラッグ中に吸着していた辺を同じ端に合わせる。
            placement.shift = (
                preview_x.or(final_x).map_or(0, |edge| {
                    edge.shift(final_bounds.left, final_bounds.right, work.left, work.right)
                }),
                preview_y.or(final_y).map_or(0, |edge| {
                    edge.shift(final_bounds.top, final_bounds.bottom, work.top, work.bottom)
                }),
            );
        }
        placement
    }
}

#[derive(Clone, Copy)]
enum Edge {
    Start,
    End,
}

impl Edge {
    fn shift(self, start: i32, end: i32, work_start: i32, work_end: i32) -> i32 {
        match self {
            Self::Start => work_start - start,
            Self::End => work_end - end,
        }
    }
}

fn snap_edges(rect: &MonitorRect, work: &MonitorRect) -> (Option<Edge>, Option<Edge>) {
    let closest = |start, end, work_start, work_end| {
        [Edge::Start, Edge::End]
            .into_iter()
            .map(|edge| (edge, edge.shift(start, end, work_start, work_end).abs()))
            .filter(|(_, distance)| *distance <= DESKTOP_SNAP_DISTANCE)
            .min_by_key(|(_, distance)| *distance)
            .map(|(edge, _)| edge)
    };
    let x = if rect.top < work.bottom && work.top < rect.bottom {
        closest(rect.left, rect.right, work.left, work.right)
    } else {
        None
    };
    let y = if rect.left < work.right && work.left < rect.right {
        closest(rect.top, rect.bottom, work.top, work.bottom)
    } else {
        None
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> MonitorRect {
        MonitorRect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn snaps_group_outer_edges_to_the_work_area_within_twenty_pixels() {
        let monitor = MonitorArea {
            bounds: rect(-1200, -100, 0, 900),
            work: rect(-1200, -100, 0, 860),
        };
        let layout = DragLayout {
            bounds: rect(-1180, 440, -700, 840),
            origin: (-1100, 480),
            source: None,
            snap: true,
            resize: true,
        };
        let snapped = layout.placement(layout.origin, Some(&monitor), false);
        assert_eq!(snapped.rect(&layout.bounds), rect(-1200, 460, -720, 860));
        assert_eq!(
            snapped.rect(&rect(-940, 440, -700, 840)),
            rect(-960, 460, -720, 860)
        );
        let outside_threshold = layout.placement((-1099, 479), Some(&monitor), false);
        assert_eq!(
            outside_threshold.rect(&layout.bounds),
            rect(-1179, 439, -699, 839)
        );
    }

    #[test]
    fn release_scales_sizes_and_gaps_about_the_grab_point_only_on_different_resolution() {
        let layout = DragLayout {
            bounds: rect(100, 100, 740, 500),
            origin: (150, 200),
            source: Some(MonitorArea {
                bounds: rect(0, 0, 1920, 1080),
                work: rect(0, 0, 1920, 1000),
            }),
            snap: false,
            resize: true,
        };
        let smaller = MonitorArea {
            bounds: rect(-960, 0, 0, 560),
            work: rect(-960, 0, 0, 500),
        };
        let first = rect(100, 100, 400, 500);
        let second = rect(440, 100, 740, 500);
        let drag = layout.placement((-900, 200), Some(&smaller), false);
        assert_eq!(drag.rect(&first).size(), (300, 400));
        let drop = layout.placement((-900, 200), Some(&smaller), true);
        assert_eq!(drop.rect(&first), rect(-925, 150, -775, 350));
        assert_eq!(drop.rect(&second), rect(-755, 150, -605, 350));
        let same = MonitorArea {
            bounds: rect(-1920, 0, 0, 1080),
            work: rect(-1920, 0, 0, 1040),
        };
        assert!(!layout.placement((-900, 200), Some(&same), true).resizes());
    }

    #[test]
    fn resizing_on_drop_keeps_the_edge_that_was_snapped_during_drag() {
        let layout = DragLayout {
            bounds: rect(100, 100, 500, 400),
            origin: (150, 150),
            source: Some(MonitorArea {
                bounds: rect(0, 0, 1920, 1000),
                work: rect(0, 0, 1920, 1000),
            }),
            snap: true,
            resize: true,
        };
        let smaller = MonitorArea {
            bounds: rect(-960, 0, 0, 500),
            work: rect(-960, 0, 0, 500),
        };
        let drag = layout.placement((-340, 200), Some(&smaller), false);
        let drop = layout.placement((-340, 200), Some(&smaller), true);
        assert_eq!(drag.rect(&layout.bounds).right, 0);
        assert_eq!(drop.rect(&layout.bounds).right, 0);
        assert_eq!(drop.rect(&layout.bounds).size(), (200, 150));
    }
}
