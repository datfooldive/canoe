//! Grid layout - automatic grid arrangement

use super::{Layout, LayoutArea, LayoutResult, LayoutWindow, WindowGeometry};
use crate::config::{GridDirection, MUTABLE_CONFIG};

pub struct GridLayout;

impl Layout for GridLayout {
    fn arrange(&self, area: LayoutArea, windows: &[LayoutWindow]) -> LayoutResult {
        let config = MUTABLE_CONFIG.read().unwrap();
        let grid = &config.grid;

        if windows.is_empty() {
            return LayoutResult {
                geometries: Vec::new(),
            };
        }

        let n = windows.len();
        let outer_gap = grid.outer_gap;
        let inner_gap = grid.inner_gap;
        let direction = grid.direction;

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

        // Calculate grid dimensions
        let (cols, rows) = calculate_grid_dimensions(n, direction);

        let cell_w = (usable_w - (cols as i32 - 1) * inner_gap) / cols as i32;
        let cell_h = (usable_h - (rows as i32 - 1) * inner_gap) / rows as i32;

        let mut geometries = Vec::with_capacity(n);

        for (i, window) in windows.iter().enumerate() {
            let (col, row) = match direction {
                GridDirection::Horizontal => (i % cols, i / cols),
                GridDirection::Vertical => (i / rows, i % rows),
            };

            let x = usable_x + col as i32 * (cell_w + inner_gap);
            let y = usable_y + row as i32 * (cell_h + inner_gap);

            // Handle last row/column which may have fewer windows
            let w = if direction == GridDirection::Horizontal
                && row == rows - 1
                && i == n - 1
                && n % cols != 0
            {
                // Last window in incomplete row - expand to fill
                usable_w - (col as i32 * (cell_w + inner_gap))
            } else {
                cell_w
            };

            let h = if direction == GridDirection::Vertical
                && col == cols - 1
                && i == n - 1
                && n % rows != 0
            {
                // Last window in incomplete column - expand to fill
                usable_h - (row as i32 * (cell_h + inner_gap))
            } else {
                cell_h
            };

            geometries.push((window.id, WindowGeometry::new(x, y, w, h)));
        }

        LayoutResult { geometries }
    }
}

/// Calculate grid dimensions (columns, rows) for n windows
fn calculate_grid_dimensions(n: usize, direction: GridDirection) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }

    // Start with square root approximation
    let sqrt = (n as f64).sqrt().ceil() as usize;

    match direction {
        GridDirection::Horizontal => {
            // Prefer more columns
            let cols = sqrt;
            let rows = (n + cols - 1) / cols;
            (cols, rows)
        }
        GridDirection::Vertical => {
            // Prefer more rows
            let rows = sqrt;
            let cols = (n + rows - 1) / rows;
            (cols, rows)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_dimensions() {
        assert_eq!(
            calculate_grid_dimensions(1, GridDirection::Horizontal),
            (1, 1)
        );
        assert_eq!(
            calculate_grid_dimensions(2, GridDirection::Horizontal),
            (2, 1)
        );
        assert_eq!(
            calculate_grid_dimensions(4, GridDirection::Horizontal),
            (2, 2)
        );
        assert_eq!(
            calculate_grid_dimensions(5, GridDirection::Horizontal),
            (3, 2)
        );
        assert_eq!(
            calculate_grid_dimensions(9, GridDirection::Horizontal),
            (3, 3)
        );
    }
}
