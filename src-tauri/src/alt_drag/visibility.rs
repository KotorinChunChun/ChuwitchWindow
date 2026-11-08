use crate::monitor::MonitorRect;
use std::collections::HashSet;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR};
use windows::Win32::UI::WindowsAndMessaging::{
    GetTopWindow, GetWindow, GetWindowRect, IsIconic, IsWindowVisible, GW_HWNDNEXT,
};

pub(super) struct Visibility {
    desktop: Vec<MonitorRect>,
    windows: Vec<(HWND, MonitorRect)>,
}

impl Visibility {
    pub fn capture() -> Self {
        let mut desktop: Vec<MonitorRect> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(add_monitor),
                LPARAM(&mut desktop as *mut _ as isize),
            );
        }
        let mut windows = Vec::new();
        let mut visited = HashSet::new();
        let mut hwnd = unsafe { GetTopWindow(HWND::default()) };
        while hwnd.0 != 0 && visited.insert(hwnd.0) {
            if let Some(rect) = occluding_frame(hwnd) {
                windows.push((hwnd, rect));
            }
            hwnd = unsafe { GetWindow(hwnd, GW_HWNDNEXT) };
        }
        Self { desktop, windows }
    }

    pub fn has_visible_half(&self, hwnd: HWND) -> bool {
        let Some(index) = self
            .windows
            .iter()
            .position(|(candidate, _)| *candidate == hwnd)
        else {
            return false;
        };
        at_least_half_visible(
            &self.windows[index].1,
            &self.desktop,
            self.windows[..index].iter().map(|(_, rect)| rect),
        )
    }
}

unsafe extern "system" fn add_monitor(
    _monitor: HMONITOR,
    _dc: HDC,
    bounds: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let desktop = &mut *(data.0 as *mut Vec<MonitorRect>);
    desktop.push((*bounds).into());
    BOOL(1)
}

pub(super) fn visible_frame(hwnd: HWND) -> Option<MonitorRect> {
    let mut rect = RECT::default();
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rect as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    };
    (result.is_ok() && rect.left < rect.right && rect.top < rect.bottom).then(|| rect.into())
}

fn occluding_frame(hwnd: HWND) -> Option<MonitorRect> {
    if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
        return None;
    }
    let mut cloaked = 0u32;
    let result = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    if result.is_ok() && cloaked != 0 {
        return None;
    }
    visible_frame(hwnd).or_else(|| {
        let mut raw = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut raw) }
            .ok()
            .map(|_| raw.into())
    })
}

fn at_least_half_visible<'a>(
    window: &MonitorRect,
    desktop: &[MonitorRect],
    covering: impl IntoIterator<Item = &'a MonitorRect>,
) -> bool {
    let total = area(window);
    if total == 0 {
        return false;
    }
    let mut visible: Vec<MonitorRect> = Vec::new();
    for screen in desktop {
        if let Some(rect) = intersection(window, screen) {
            // モニター領域が重なっても同じ画素を二重に数えない。
            let mut pieces = vec![rect];
            for existing in &visible {
                pieces = pieces
                    .into_iter()
                    .flat_map(|piece| subtract(piece, existing))
                    .collect();
            }
            visible.extend(pieces);
        }
    }
    let half = (total + 1) / 2;
    for cover in covering {
        if intersection(window, cover).is_none() {
            continue;
        }
        if visible.iter().map(area).sum::<i64>() < half {
            return false;
        }
        visible = visible
            .into_iter()
            .flat_map(|piece| subtract(piece, cover))
            .collect();
    }
    visible.iter().map(area).sum::<i64>() >= half
}

pub(super) fn covers_most_of_work_area<'a>(
    work: &MonitorRect,
    frames: impl IntoIterator<Item = &'a MonitorRect>,
) -> bool {
    let total = area(work);
    if total == 0 {
        return false;
    }
    let mut uncovered = vec![work.clone()];
    for frame in frames {
        uncovered = uncovered
            .into_iter()
            .flat_map(|piece| subtract(piece, frame))
            .collect();
    }
    // 重なりを二重計上せず、空きが作業領域の10%以下なら全体配置として扱う。
    uncovered.iter().map(area).sum::<i64>() <= total / 10
}

fn area(rect: &MonitorRect) -> i64 {
    let (width, height) = rect.size();
    i64::from(width.max(0)) * i64::from(height.max(0))
}

fn intersection(rect: &MonitorRect, other: &MonitorRect) -> Option<MonitorRect> {
    let intersection = MonitorRect {
        left: rect.left.max(other.left),
        top: rect.top.max(other.top),
        right: rect.right.min(other.right),
        bottom: rect.bottom.min(other.bottom),
    };
    (area(&intersection) > 0).then_some(intersection)
}

fn subtract(rect: MonitorRect, cover: &MonitorRect) -> impl Iterator<Item = MonitorRect> {
    let strips = match intersection(&rect, cover) {
        Some(overlap) => [
            MonitorRect {
                bottom: overlap.top,
                ..rect.clone()
            },
            MonitorRect {
                top: overlap.bottom,
                ..rect.clone()
            },
            MonitorRect {
                top: overlap.top,
                bottom: overlap.bottom,
                right: overlap.left,
                ..rect.clone()
            },
            MonitorRect {
                top: overlap.top,
                bottom: overlap.bottom,
                left: overlap.right,
                ..rect
            },
        ]
        .map(Some),
        None => [Some(rect), None, None, None],
    };
    strips.into_iter().flatten().filter(|piece| area(piece) > 0)
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
    fn accepts_exactly_half_and_rejects_less() {
        let window = rect(0, 0, 100, 100);
        let desktop = [window.clone()];
        assert!(at_least_half_visible(
            &window,
            &desktop,
            &[rect(0, 0, 50, 100)]
        ));
        assert!(!at_least_half_visible(
            &window,
            &desktop,
            &[rect(0, 0, 51, 100)]
        ));
    }

    #[test]
    fn work_area_coverage_uses_the_union_of_frames_clipped_to_the_screen() {
        let work = rect(0, 0, 100, 100);
        let left = rect(-20, 0, 70, 100);
        assert!(covers_most_of_work_area(
            &work,
            [&left, &rect(60, 0, 90, 100)]
        ));
        assert!(!covers_most_of_work_area(
            &work,
            [&left, &rect(60, 0, 89, 100)]
        ));
        assert!(!covers_most_of_work_area(
            &work,
            [&rect(0, 0, 100, 35), &rect(0, 65, 100, 100)]
        ));
    }

    #[test]
    fn covering_windows_and_displays_are_counted_as_unions() {
        let window = rect(-40, 0, 60, 100);
        let screen = rect(-20, 0, 100, 100);
        let desktop = [screen.clone(), screen];
        let covers = [rect(-20, 0, 0, 100), rect(-5, 0, 10, 100)];
        assert!(at_least_half_visible(&window, &desktop, &covers));
        assert!(!at_least_half_visible(
            &window,
            &desktop,
            covers.iter().chain([&rect(10, 0, 11, 100)])
        ));
    }
}
