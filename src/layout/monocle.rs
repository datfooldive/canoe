//! Monocle layout - single focused window fills the screen

use super::{Layout, LayoutArea, LayoutResult, LayoutWindow, WindowGeometry};
use crate::config::MUTABLE_CONFIG;

pub struct MonocleLayout;

impl Layout for MonocleLayout {
    fn arrange(&self, area: LayoutArea, windows: &[LayoutWindow]) -> LayoutResult {
        let config = MUTABLE_CONFIG.read().unwrap();
        let gap = config.monocle.gap;

        if windows.is_empty() {
            return LayoutResult {
                geometries: Vec::new(),
            };
        }

        // Find focused window, or use first window
        let focused_window = windows
            .iter()
            .find(|w| w.focused)
            .unwrap_or(&windows[0]);

        let x = area.x + gap;
        let y = area.y + gap;
        let w = area.width - 2 * gap;
        let h = area.height - 2 * gap;

        LayoutResult {
            geometries: vec![(focused_window.id, WindowGeometry::new(x, y, w, h))],
        }
    }
}
