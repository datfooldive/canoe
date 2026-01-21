//! Float layout - no automatic arrangement, windows keep their positions

use super::{Layout, LayoutArea, LayoutResult, LayoutWindow};

pub struct FloatLayout;

impl Layout for FloatLayout {
    fn arrange(&self, _area: LayoutArea, _windows: &[LayoutWindow]) -> LayoutResult {
        // Float layout is a no-op - windows maintain their current positions
        LayoutResult {
            geometries: Vec::new(),
        }
    }
}
