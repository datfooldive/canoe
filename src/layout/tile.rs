//! Tile layout - master/stack with configurable master area

use super::{Layout, LayoutArea, LayoutResult, LayoutWindow, WindowGeometry};
use crate::config::{MasterLocation, MUTABLE_CONFIG};

pub struct TileLayout;

impl Layout for TileLayout {
    fn arrange(&self, area: LayoutArea, windows: &[LayoutWindow]) -> LayoutResult {
        let config = MUTABLE_CONFIG.read().unwrap();
        let tile = &config.tile;

        if windows.is_empty() {
            return LayoutResult {
                geometries: Vec::new(),
            };
        }

        let n = windows.len();
        let nmaster = tile.nmaster as usize;
        let mfact = tile.mfact;
        let inner_gap = tile.inner_gap;
        let outer_gap = tile.outer_gap;
        let master_location = tile.master_location;

        // Calculate usable area
        let usable_x = area.x + outer_gap;
        let usable_y = area.y + outer_gap;
        let usable_w = area.width - 2 * outer_gap;
        let usable_h = area.height - 2 * outer_gap;

        let mut geometries = Vec::with_capacity(n);

        if n == 1 {
            // Single window takes full area
            geometries.push((
                windows[0].id,
                WindowGeometry::new(usable_x, usable_y, usable_w, usable_h),
            ));
            return LayoutResult { geometries };
        }

        let master_count = nmaster.min(n);
        let stack_count = n.saturating_sub(nmaster);

        // Calculate master and stack dimensions based on orientation
        let (master_x, master_y, master_w, master_h, stack_x, stack_y, stack_w, stack_h) =
            match master_location {
                MasterLocation::Left => {
                    let master_w = if stack_count > 0 {
                        ((usable_w - inner_gap) as f32 * mfact) as i32
                    } else {
                        usable_w
                    };
                    let stack_w = usable_w - master_w - if stack_count > 0 { inner_gap } else { 0 };
                    (
                        usable_x,
                        usable_y,
                        master_w,
                        usable_h,
                        usable_x + master_w + inner_gap,
                        usable_y,
                        stack_w,
                        usable_h,
                    )
                }
                MasterLocation::Right => {
                    let master_w = if stack_count > 0 {
                        ((usable_w - inner_gap) as f32 * mfact) as i32
                    } else {
                        usable_w
                    };
                    let stack_w = usable_w - master_w - if stack_count > 0 { inner_gap } else { 0 };
                    (
                        usable_x + stack_w + inner_gap,
                        usable_y,
                        master_w,
                        usable_h,
                        usable_x,
                        usable_y,
                        stack_w,
                        usable_h,
                    )
                }
                MasterLocation::Top => {
                    let master_h = if stack_count > 0 {
                        ((usable_h - inner_gap) as f32 * mfact) as i32
                    } else {
                        usable_h
                    };
                    let stack_h = usable_h - master_h - if stack_count > 0 { inner_gap } else { 0 };
                    (
                        usable_x,
                        usable_y,
                        usable_w,
                        master_h,
                        usable_x,
                        usable_y + master_h + inner_gap,
                        usable_w,
                        stack_h,
                    )
                }
                MasterLocation::Bottom => {
                    let master_h = if stack_count > 0 {
                        ((usable_h - inner_gap) as f32 * mfact) as i32
                    } else {
                        usable_h
                    };
                    let stack_h = usable_h - master_h - if stack_count > 0 { inner_gap } else { 0 };
                    (
                        usable_x,
                        usable_y + stack_h + inner_gap,
                        usable_w,
                        master_h,
                        usable_x,
                        usable_y,
                        usable_w,
                        stack_h,
                    )
                }
            };

        // Arrange master windows
        let is_horizontal_master = matches!(
            master_location,
            MasterLocation::Left | MasterLocation::Right
        );
        for (i, window) in windows.iter().take(master_count).enumerate() {
            let (x, y, w, h) = if is_horizontal_master {
                // Stack master windows vertically
                let h = (master_h - (master_count as i32 - 1) * inner_gap) / master_count as i32;
                let y = master_y + i as i32 * (h + inner_gap);
                (master_x, y, master_w, h)
            } else {
                // Stack master windows horizontally
                let w = (master_w - (master_count as i32 - 1) * inner_gap) / master_count as i32;
                let x = master_x + i as i32 * (w + inner_gap);
                (x, master_y, w, master_h)
            };
            geometries.push((window.id, WindowGeometry::new(x, y, w, h)));
        }

        // Arrange stack windows
        if stack_count > 0 {
            for (i, window) in windows.iter().skip(master_count).enumerate() {
                let (x, y, w, h) = if is_horizontal_master {
                    // Stack windows vertically
                    let h = (stack_h - (stack_count as i32 - 1) * inner_gap) / stack_count as i32;
                    let y = stack_y + i as i32 * (h + inner_gap);
                    (stack_x, y, stack_w, h)
                } else {
                    // Stack windows horizontally
                    let w = (stack_w - (stack_count as i32 - 1) * inner_gap) / stack_count as i32;
                    let x = stack_x + i as i32 * (w + inner_gap);
                    (x, stack_y, w, stack_h)
                };
                geometries.push((window.id, WindowGeometry::new(x, y, w, h)));
            }
        }

        LayoutResult { geometries }
    }
}
