use resvg::tiny_skia;

pub struct Renderer<'a> {
    pixmap: tiny_skia::PixmapMut<'a>,
}

impl<'a> Renderer<'a> {
    pub fn new(pixels: &'a mut [u8], width: i32, height: i32) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let pixmap = tiny_skia::PixmapMut::from_bytes(pixels, width as u32, height as u32)?;
        Some(Self { pixmap })
    }

    pub fn width(&self) -> i32 {
        self.pixmap.width() as i32
    }

    pub fn height(&self) -> i32 {
        self.pixmap.height() as i32
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        self.pixmap.data_mut()
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color_argb: u32) {
        if width <= 0 || height <= 0 {
            return;
        }

        let buffer_width = self.width();
        let buffer_height = self.height();
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + width).min(buffer_width);
        let y1 = (y + height).min(buffer_height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let color_bytes = color_argb.to_ne_bytes();
        let pixels = self.data_mut();
        for row in y0..y1 {
            for col in x0..x1 {
                let offset = ((row * buffer_width + col) * 4) as usize;
                if offset + 4 <= pixels.len() {
                    pixels[offset..offset + 4].copy_from_slice(&color_bytes);
                }
            }
        }
    }

    pub fn blit_pixmap(&mut self, icon: &tiny_skia::Pixmap, x: i32, y: i32) {
        let buffer_width = self.width();
        let buffer_height = self.height();
        if buffer_width <= 0 || buffer_height <= 0 {
            return;
        }

        let icon_w = icon.width() as i32;
        let icon_h = icon.height() as i32;
        if icon_w <= 0 || icon_h <= 0 {
            return;
        }

        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + icon_w).min(buffer_width);
        let y1 = (y + icon_h).min(buffer_height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let icon_data = icon.data();
        let pixels = self.data_mut();
        for row in y0..y1 {
            let src_y = row - y;
            for col in x0..x1 {
                let src_x = col - x;
                let src_offset = ((src_y * icon_w + src_x) * 4) as usize;
                if src_offset + 4 <= icon_data.len() {
                    let src_r = icon_data[src_offset];
                    let src_g = icon_data[src_offset + 1];
                    let src_b = icon_data[src_offset + 2];
                    let src_a = icon_data[src_offset + 3];
                    if src_a == 0 {
                        continue;
                    }
                    let dst_offset = ((row * buffer_width + col) * 4) as usize;
                    if dst_offset + 4 <= pixels.len() {
                        blend_pixel_premul(
                            &mut pixels[dst_offset..dst_offset + 4],
                            src_r,
                            src_g,
                            src_b,
                            src_a,
                        );
                    }
                }
            }
        }
    }

    pub fn blit_argb(&mut self, src: &[u8], src_width: i32, src_height: i32, x: i32, y: i32) {
        if src_width <= 0 || src_height <= 0 {
            return;
        }

        let dst_width = self.width();
        let dst_height = self.height();
        if dst_width <= 0 || dst_height <= 0 {
            return;
        }

        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + src_width).min(dst_width);
        let y1 = (y + src_height).min(dst_height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }

        let dst = self.data_mut();
        for row in y0..y1 {
            let src_y = row - y;
            for col in x0..x1 {
                let src_x = col - x;
                let src_offset = ((src_y * src_width + src_x) * 4) as usize;
                let dst_offset = ((row * dst_width + col) * 4) as usize;
                if src_offset + 4 <= src.len() && dst_offset + 4 <= dst.len() {
                    dst[dst_offset..dst_offset + 4]
                        .copy_from_slice(&src[src_offset..src_offset + 4]);
                }
            }
        }
    }
}

/// Alpha blend a premultiplied RGBA source over an ARGB destination.
fn blend_pixel_premul(bg: &mut [u8], src_r: u8, src_g: u8, src_b: u8, src_a: u8) {
    let inv_a = 255u16.saturating_sub(src_a as u16);

    // Read background (native endian ARGB)
    let bg_val = u32::from_ne_bytes([bg[0], bg[1], bg[2], bg[3]]);
    let bg_r = ((bg_val >> 16) & 0xff) as u16;
    let bg_g = ((bg_val >> 8) & 0xff) as u16;
    let bg_b = (bg_val & 0xff) as u16;

    let out_r = (src_r as u16 + (bg_r * inv_a) / 255) as u8;
    let out_g = (src_g as u16 + (bg_g * inv_a) / 255) as u8;
    let out_b = (src_b as u16 + (bg_b * inv_a) / 255) as u8;

    let out_argb = 0xFF000000 | ((out_r as u32) << 16) | ((out_g as u32) << 8) | (out_b as u32);
    bg.copy_from_slice(&out_argb.to_ne_bytes());
}
