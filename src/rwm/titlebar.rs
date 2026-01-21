//! Titlebar rendering for windows

use crate::protocol::RiverDecorationV1;
use fontdue::{Font, FontSettings};
use std::os::fd::AsFd;
use std::sync::OnceLock;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::QueueHandle;

/// Titlebar height in pixels
pub const TITLEBAR_HEIGHT: i32 = 24;

/// Total border width in pixels (1px black, 3px gray, 1px black)
pub const BORDER_WIDTH: i32 = 5;

/// Font size for titlebar text
const FONT_SIZE: f32 = 14.0;

/// Background color for active window titlebar (blue)
const BG_COLOR_ACTIVE: u32 = 0x2F6BFFFF;

/// Background color for inactive window titlebar (gray)
const BG_COLOR_INACTIVE: u32 = 0x666666FF;

/// Text color for titlebar (white)
const TEXT_COLOR: u32 = 0xFFFFFFFF;

/// Border colors
const BORDER_COLOR_OUTER: u32 = 0x000000FF;
const BORDER_COLOR_MID: u32 = 0x888888FF;
const BORDER_COLOR_INNER: u32 = 0x000000FF;

const BORDER_OUTER: i32 = 1;
const BORDER_MID: i32 = 3;
const BORDER_INNER: i32 = 1;

/// Horizontal padding for title text
const TITLE_PADDING: i32 = 8;

/// Get or initialize the titlebar font
fn get_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();

    FONT.get_or_init(|| {
        // Try common system font paths
        let font_paths = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        ];

        for path in font_paths {
            if let Ok(font_data) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(font_data, FontSettings::default()) {
                    log::info!("Loaded titlebar font from {}", path);
                    return Some(font);
                }
            }
        }

        log::warn!("Could not load any system font for titlebar");
        None
    })
    .as_ref()
}

/// Titlebar state for a window
pub struct Titlebar {
    /// The wl_surface for the titlebar
    pub surface: wl_surface::WlSurface,
    /// The river decoration object
    pub decoration: RiverDecorationV1,
    /// Current buffer (if any)
    pub buffer: Option<wl_buffer::WlBuffer>,
    /// Shared memory pool
    pub pool: Option<wl_shm_pool::WlShmPool>,
    /// Memory-mapped file for the buffer
    pub memfile: Option<std::fs::File>,
    /// Memory map pointer
    pub mmap: Option<memmap2::MmapMut>,
    /// Current buffer width
    pub width: i32,
    /// Current buffer height
    pub height: i32,
    /// Current content width
    pub content_width: i32,
    /// Current content height
    pub content_height: i32,
    /// Whether titlebar needs redraw
    pub dirty: bool,
}

impl Titlebar {
    /// Create a new titlebar
    pub fn new(surface: wl_surface::WlSurface, decoration: RiverDecorationV1) -> Self {
        Self {
            surface,
            decoration,
            buffer: None,
            pool: None,
            memfile: None,
            mmap: None,
            width: 0,
            height: 0,
            content_width: 0,
            content_height: 0,
            dirty: true,
        }
    }

    /// Ensure buffer is allocated for the given width
    pub fn ensure_buffer<D: 'static>(
        &mut self,
        content_width: i32,
        content_height: i32,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
    ) where
        D: wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
            + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>,
    {
        if content_width <= 0 || content_height <= 0 {
            log::info!("ensure_buffer: invalid content size, skipping");
            return;
        }

        let width = content_width + BORDER_WIDTH * 2;
        let height = content_height + BORDER_WIDTH * 2;

        // Check if we need a new buffer
        if self.width != width || self.height != height || self.buffer.is_none() {
            log::info!(
                "ensure_buffer: creating new buffer for {}x{} (content {}x{})",
                width,
                height,
                content_width,
                content_height
            );
            self.width = width;
            self.height = height;
            self.content_width = content_width;
            self.content_height = content_height;
            self.dirty = true;

            // Clean up old buffer
            if let Some(buffer) = self.buffer.take() {
                buffer.destroy();
            }
            if let Some(pool) = self.pool.take() {
                pool.destroy();
            }

            // Calculate buffer size (ARGB8888 = 4 bytes per pixel)
            let stride = width * 4;
            let size = stride * height;
            log::debug!("ensure_buffer: stride={}, size={}", stride, size);

            // Create memfd for shared memory
            let memfd = match memfd::MemfdOptions::default()
                .close_on_exec(true)
                .create("rwm-titlebar")
            {
                Ok(fd) => fd,
                Err(e) => {
                    log::error!("Failed to create memfd: {}", e);
                    return;
                }
            };

            // Set size
            if let Err(e) = memfd.as_file().set_len(size as u64) {
                log::error!("Failed to set memfd size: {}", e);
                return;
            }

            // Create mmap
            let mmap = match unsafe { memmap2::MmapMut::map_mut(memfd.as_file()) } {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to mmap: {}", e);
                    return;
                }
            };

            // Create wl_shm_pool
            let pool = shm.create_pool(memfd.as_file().as_fd(), size, qh, ());

            // Create buffer
            let buffer = pool.create_buffer(
                0,
                width,
                height,
                stride,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );

            log::debug!("ensure_buffer: buffer created successfully");

            // Store everything
            self.memfile = Some(memfd.into_file());
            self.mmap = Some(mmap);
            self.pool = Some(pool);
            self.buffer = Some(buffer);
        }
    }

    /// Render the titlebar with the given title and active state
    pub fn render(&mut self, title: Option<&str>, is_active: bool) {
        if let Some(ref mut mmap) = self.mmap {
            let width = self.width;
            let height = self.height;
            let content_width = self.content_width;
            let content_height = self.content_height;
            if width <= 0 || height <= 0 || content_width <= 0 || content_height <= 0 {
                return;
            }

            let pixels = mmap.as_mut();
            clear_buffer(pixels);

            let border_offset = 0;
            draw_border_layer(
                pixels,
                width,
                height,
                border_offset,
                BORDER_OUTER,
                BORDER_COLOR_OUTER,
            );
            draw_border_layer(
                pixels,
                width,
                height,
                border_offset + BORDER_OUTER,
                BORDER_MID,
                BORDER_COLOR_MID,
            );
            draw_border_layer(
                pixels,
                width,
                height,
                border_offset + BORDER_OUTER + BORDER_MID,
                BORDER_INNER,
                BORDER_COLOR_INNER,
            );

            // Choose background color based on active state
            let bg_color = if is_active {
                BG_COLOR_ACTIVE
            } else {
                BG_COLOR_INACTIVE
            };
            let bg_argb = rgba_to_argb(bg_color);

            let title_height = TITLEBAR_HEIGHT.min(content_height);
            if title_height > 0 {
                let title_x = BORDER_WIDTH;
                let title_y = BORDER_WIDTH;
                fill_rect(
                    pixels,
                    width,
                    height,
                    title_x,
                    title_y,
                    content_width,
                    title_height,
                    bg_argb,
                );

                // Render title text if we have a title and font
                if let Some(title_str) = title {
                    if !title_str.is_empty() {
                        render_title(
                            pixels,
                            width,
                            height,
                            title_str,
                            title_x,
                            title_y,
                            content_width,
                            title_height,
                        );
                    }
                }
            }

            self.dirty = false;
        }
    }

    /// Commit the titlebar surface
    pub fn commit(&self) {
        if let Some(ref buffer) = self.buffer {
            self.surface.attach(Some(buffer), 0, 0);
            self.surface.damage_buffer(0, 0, self.width, self.height);
            self.surface.commit();
        }
    }

    /// Set the offset position relative to window
    pub fn set_offset(&self, x: i32, y: i32) {
        self.decoration.set_offset(x, y);
    }

    /// Sync the next commit with render_finish
    pub fn sync_next_commit(&self) {
        self.decoration.sync_next_commit();
    }
}

/// Render the title text onto the titlebar pixels
fn render_title(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    title: &str,
    origin_x: i32,
    origin_y: i32,
    area_width: i32,
    area_height: i32,
) {
    let font = match get_font() {
        Some(f) => f,
        None => return,
    };

    let text_argb = rgba_to_argb(TEXT_COLOR);

    // Calculate metrics for vertical centering
    let metrics = font.metrics('A', FONT_SIZE);
    let baseline_y = origin_y as f32
        + (area_height as f32 + metrics.height as f32) / 2.0
        - metrics.height as f32
        + (metrics.ymin.abs() as f32);

    // Rasterize and draw each character
    let mut x_pos = (origin_x + TITLE_PADDING) as f32;
    let max_x = origin_x + area_width - TITLE_PADDING;

    for ch in title.chars() {
        let (metrics, bitmap) = font.rasterize(ch, FONT_SIZE);

        // Check if we have room for this character
        if (x_pos + metrics.advance_width) as i32 > max_x {
            break;
        }

        // Calculate position
        let glyph_x = x_pos as i32 + metrics.xmin;
        let glyph_y = baseline_y as i32 - metrics.ymin;

        // Draw the glyph
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let px = glyph_x + col as i32;
                let py = glyph_y + row as i32;

                if px >= origin_x
                    && px < origin_x + area_width
                    && py >= origin_y
                    && py < origin_y + area_height
                    && px >= 0
                    && px < buffer_width
                    && py >= 0
                    && py < buffer_height
                {
                    let alpha = bitmap[row * metrics.width + col];
                    if alpha > 0 {
                        let offset = ((py * buffer_width + px) * 4) as usize;
                        if offset + 4 <= pixels.len() {
                            // Alpha blend the text onto the background
                            blend_pixel(&mut pixels[offset..offset + 4], text_argb, alpha);
                        }
                    }
                }
            }
        }

        x_pos += metrics.advance_width;
    }
}

/// Convert RGBA (0xRRGGBBAA) to ARGB (0xAARRGGBB) for wl_shm format
fn rgba_to_argb(rgba: u32) -> u32 {
    let r = (rgba >> 24) & 0xff;
    let g = (rgba >> 16) & 0xff;
    let b = (rgba >> 8) & 0xff;
    let a = rgba & 0xff;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Alpha blend a text pixel onto a background pixel
fn blend_pixel(bg: &mut [u8], fg_argb: u32, alpha: u8) {
    let fg_a = ((fg_argb >> 24) & 0xff) as u16;
    let fg_r = ((fg_argb >> 16) & 0xff) as u16;
    let fg_g = ((fg_argb >> 8) & 0xff) as u16;
    let fg_b = (fg_argb & 0xff) as u16;

    // Scale alpha by foreground alpha
    let a = (alpha as u16 * fg_a) / 255;
    let inv_a = 255 - a;

    // Read background (native endian ARGB)
    let bg_val = u32::from_ne_bytes([bg[0], bg[1], bg[2], bg[3]]);
    let bg_r = ((bg_val >> 16) & 0xff) as u16;
    let bg_g = ((bg_val >> 8) & 0xff) as u16;
    let bg_b = (bg_val & 0xff) as u16;

    // Blend
    let out_r = ((fg_r * a + bg_r * inv_a) / 255) as u8;
    let out_g = ((fg_g * a + bg_g * inv_a) / 255) as u8;
    let out_b = ((fg_b * a + bg_b * inv_a) / 255) as u8;

    // Write back (keep full alpha)
    let out_argb = 0xFF000000 | ((out_r as u32) << 16) | ((out_g as u32) << 8) | (out_b as u32);
    bg.copy_from_slice(&out_argb.to_ne_bytes());
}

fn clear_buffer(pixels: &mut [u8]) {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[0, 0, 0, 0]);
    }
}

fn fill_rect(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color_argb: u32,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + width).min(buffer_width);
    let y1 = (y + height).min(buffer_height);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let color_bytes = color_argb.to_ne_bytes();
    for row in y0..y1 {
        for col in x0..x1 {
            let offset = ((row * buffer_width + col) * 4) as usize;
            if offset + 4 <= pixels.len() {
                pixels[offset..offset + 4].copy_from_slice(&color_bytes);
            }
        }
    }
}

fn draw_border_layer(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    offset: i32,
    thickness: i32,
    color: u32,
) {
    if thickness <= 0 {
        return;
    }

    let layer_width = buffer_width - offset * 2;
    let layer_height = buffer_height - offset * 2;
    if layer_width <= 0 || layer_height <= 0 {
        return;
    }

    let argb = rgba_to_argb(color);
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        offset,
        offset,
        layer_width,
        thickness,
        argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        offset,
        offset + layer_height - thickness,
        layer_width,
        thickness,
        argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        offset,
        offset + thickness,
        thickness,
        layer_height - thickness * 2,
        argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        offset + layer_width - thickness,
        offset + thickness,
        thickness,
        layer_height - thickness * 2,
        argb,
    );
}

impl Drop for Titlebar {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        // Decoration (role object) must be destroyed before the surface
        self.decoration.destroy();
        self.surface.destroy();
    }
}
