//! Scroller layout - focused window centered with surrounding windows on sides

use super::{Layout, LayoutArea, LayoutResult, LayoutWindow, WindowGeometry};
use crate::config::MUTABLE_CONFIG;

pub struct ScrollerLayout;

impl Layout for ScrollerLayout {
    fn arrange(&self, area: LayoutArea, windows: &[LayoutWindow]) -> LayoutResult {
        let config = MUTABLE_CONFIG.read().unwrap();
        let scroller = &config.scroller;

        if windows.is_empty() {
            return LayoutResult {
                geometries: Vec::new(),
            };
        }

        let n = windows.len();
        let outer_gap = scroller.outer_gap;
        let inner_gap = scroller.inner_gap;
        let snap_to_left = scroller.snap_to_left;

        // Calculate usable area
        let usable_x = area.x + outer_gap;
        let usable_y = area.y + outer_gap;
        let usable_w = area.width - 2 * outer_gap;
        let usable_h = area.height - 2 * outer_gap;

        if n == 1 {
            return LayoutResult {
                geometries: vec![(
                    windows[0].id,
                    WindowGeometry::new(usable_x, usable_y, usable_w, usable_h),
                )],
            };
        }

        // Find focused window index
        let focused_idx = windows
            .iter()
            .position(|w| w.focused)
            .unwrap_or(0);

        let mut geometries = Vec::with_capacity(n);

        // Get mfact for focused window (use per-window override if available)
        let mfact = windows[focused_idx]
            .scroller_mfact
            .unwrap_or(scroller.mfact);

        // Calculate main window width
        let main_w = ((usable_w - inner_gap) as f32 * mfact) as i32;
        let side_w = usable_w - main_w - inner_gap;

        // Calculate main window position
        let main_x = if snap_to_left {
            usable_x
        } else {
            // Center the main window
            usable_x + (usable_w - main_w) / 2
        };

        // Arrange focused window
        geometries.push((
            windows[focused_idx].id,
            WindowGeometry::new(main_x, usable_y, main_w, usable_h),
        ));

        // Arrange windows before focused (to the left)
        let mut left_x = main_x - inner_gap;
        for i in (0..focused_idx).rev() {
            let w_width = calculate_side_width(side_w, focused_idx - i);
            left_x -= w_width;
            geometries.push((
                windows[i].id,
                WindowGeometry::new(left_x, usable_y, w_width, usable_h),
            ));
            left_x -= inner_gap;
        }

        // Arrange windows after focused (to the right)
        let mut right_x = main_x + main_w + inner_gap;
        for i in (focused_idx + 1)..n {
            let w_width = calculate_side_width(side_w, i - focused_idx);
            geometries.push((
                windows[i].id,
                WindowGeometry::new(right_x, usable_y, w_width, usable_h),
            ));
            right_x += w_width + inner_gap;
        }

        LayoutResult { geometries }
    }
}

/// Calculate width for side windows (they get progressively smaller)
fn calculate_side_width(base_width: i32, distance: usize) -> i32 {
    // Windows further from focus are smaller
    let factor = 1.0 / (distance as f32 + 1.0);
    (base_width as f32 * factor).max(100.0) as i32
}
