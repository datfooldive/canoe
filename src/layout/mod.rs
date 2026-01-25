//! Layout algorithms for window arrangement

mod float;
mod grid;
mod monocle;
mod scroller;
mod tile;

pub use float::FloatLayout;
pub use grid::GridLayout;
pub use monocle::MonocleLayout;
pub use scroller::ScrollerLayout;
pub use tile::TileLayout;

/// Layout type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutType {
    #[default]
    Tile,
    Grid,
    Monocle,
    Scroller,
    Float,
}

/// Window geometry for layout calculations
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowGeometry {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Area available for layout
#[derive(Debug, Clone, Copy)]
pub struct LayoutArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl LayoutArea {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Window information for layout calculation
#[derive(Debug, Clone)]
pub struct LayoutWindow {
    /// Window identifier
    pub id: usize,
    /// Minimum width
    pub min_width: i32,
    /// Minimum height
    pub min_height: i32,
    /// Whether this window is focused
    pub focused: bool,
    /// Per-window scroller mfact override
    pub scroller_mfact: Option<f32>,
}

/// Result of a layout calculation
#[derive(Debug, Clone)]
pub struct LayoutResult {
    pub geometries: Vec<(usize, WindowGeometry)>,
}

/// Trait for layout implementations
pub trait Layout {
    /// Arrange windows in the given area
    fn arrange(&self, area: LayoutArea, windows: &[LayoutWindow]) -> LayoutResult;
}

/// Arrange windows using the specified layout type
pub fn arrange(
    layout_type: LayoutType,
    area: LayoutArea,
    windows: &[LayoutWindow],
) -> LayoutResult {
    match layout_type {
        LayoutType::Tile => TileLayout.arrange(area, windows),
        LayoutType::Grid => GridLayout.arrange(area, windows),
        LayoutType::Monocle => MonocleLayout.arrange(area, windows),
        LayoutType::Scroller => ScrollerLayout.arrange(area, windows),
        LayoutType::Float => FloatLayout.arrange(area, windows),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_layout() {
        let area = LayoutArea::new(0, 0, 1920, 1080);
        let windows = vec![
            LayoutWindow {
                id: 0,
                min_width: 0,
                min_height: 0,
                focused: true,
                scroller_mfact: None,
            },
            LayoutWindow {
                id: 1,
                min_width: 0,
                min_height: 0,
                focused: false,
                scroller_mfact: None,
            },
        ];

        let result = arrange(LayoutType::Tile, area, &windows);
        assert_eq!(result.geometries.len(), 2);

        // Master should take the left side
        let (id0, geom0) = &result.geometries[0];
        assert_eq!(*id0, 0);
        assert!(geom0.width > 0);

        let (id1, geom1) = &result.geometries[1];
        assert_eq!(*id1, 1);
        assert!(geom1.x >= geom0.x + geom0.width || geom1.y >= geom0.y + geom0.height);
    }

    #[test]
    fn test_grid_layout() {
        let area = LayoutArea::new(0, 0, 1920, 1080);
        let windows: Vec<_> = (0..4)
            .map(|i| LayoutWindow {
                id: i,
                min_width: 0,
                min_height: 0,
                focused: i == 0,
                scroller_mfact: None,
            })
            .collect();

        let result = arrange(LayoutType::Grid, area, &windows);
        assert_eq!(result.geometries.len(), 4);

        // All windows should have positive dimensions
        for (_, geom) in &result.geometries {
            assert!(geom.width > 0);
            assert!(geom.height > 0);
        }
    }

    #[test]
    fn test_monocle_layout() {
        let area = LayoutArea::new(0, 0, 1920, 1080);
        let windows = vec![
            LayoutWindow {
                id: 0,
                min_width: 0,
                min_height: 0,
                focused: true,
                scroller_mfact: None,
            },
            LayoutWindow {
                id: 1,
                min_width: 0,
                min_height: 0,
                focused: false,
                scroller_mfact: None,
            },
        ];

        let result = arrange(LayoutType::Monocle, area, &windows);
        // Only focused window should have geometry
        assert_eq!(result.geometries.len(), 1);
        assert_eq!(result.geometries[0].0, 0);
    }
}
