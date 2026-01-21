//! Titlebar rendering for windows

use crate::protocol::RiverDecorationV1;
use fontdue::{Font, FontSettings};
use std::os::fd::AsFd;
use std::sync::OnceLock;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::QueueHandle;

/// Titlebar height in pixels
pub const TITLEBAR_HEIGHT: i32 = 24;

/// Font size for titlebar text
const FONT_SIZE: f32 = 14.0;

/// Background color for active window titlebar (yellow)
const BG_COLOR_ACTIVE: u32 = 0xFFDD00FF;

/// Background color for inactive window titlebar (dark gray)
const BG_COLOR_INACTIVE: u32 = 0x444444FF;

/// Text color for active window (black)
const TEXT_COLOR_ACTIVE: u32 = 0x000000FF;

/// Text color for inactive window (light gray)
const TEXT_COLOR_INACTIVE: u32 = 0xAAAAAAFF;

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
    /// Current width
    pub width: i32,
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
            dirty: true,
        }
    }

    /// Ensure buffer is allocated for the given width
    pub fn ensure_buffer<D: 'static>(
        &mut self,
        width: i32,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
    ) where
        D: wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
            + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>,
    {
        if width <= 0 {
            log::info!("ensure_buffer: width <= 0, skipping");
            return;
        }

        // Check if we need a new buffer
        if self.width != width || self.buffer.is_none() {
            log::info!("ensure_buffer: creating new buffer for width={}", width);
            self.width = width;
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
            let size = stride * TITLEBAR_HEIGHT;
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
                TITLEBAR_HEIGHT,
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
            let height = TITLEBAR_HEIGHT;

            // Choose background color based on active state
            let bg_color = if is_active {
                BG_COLOR_ACTIVE
            } else {
                BG_COLOR_INACTIVE
            };
            let bg_argb = rgba_to_argb(bg_color);

            // Fill the entire buffer with background
            let pixels = mmap.as_mut();
            for y in 0..height {
                for x in 0..width {
                    let offset = ((y * width + x) * 4) as usize;
                    if offset + 4 <= pixels.len() {
                        pixels[offset..offset + 4].copy_from_slice(&bg_argb.to_ne_bytes());
                    }
                }
            }

            // Render title text if we have a title and font
            if let Some(title_str) = title {
                if !title_str.is_empty() {
                    render_title(pixels, width, height, title_str, is_active);
                }
            }

            self.dirty = false;
        }
    }

    /// Commit the titlebar surface
    pub fn commit(&self) {
        if let Some(ref buffer) = self.buffer {
            self.surface.attach(Some(buffer), 0, 0);
            self.surface.damage_buffer(0, 0, self.width, TITLEBAR_HEIGHT);
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
fn render_title(pixels: &mut [u8], width: i32, height: i32, title: &str, is_active: bool) {
    let font = match get_font() {
        Some(f) => f,
        None => return,
    };

    let text_color = if is_active {
        TEXT_COLOR_ACTIVE
    } else {
        TEXT_COLOR_INACTIVE
    };
    let text_argb = rgba_to_argb(text_color);

    // Calculate metrics for vertical centering
    let metrics = font.metrics('A', FONT_SIZE);
    let baseline_y = (height as f32 + metrics.height as f32) / 2.0
        - metrics.height as f32
        + (metrics.ymin.abs() as f32);

    // Rasterize and draw each character
    let mut x_pos = TITLE_PADDING as f32;
    let max_x = width - TITLE_PADDING;

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

                if px >= 0 && px < width && py >= 0 && py < height {
                    let alpha = bitmap[row * metrics.width + col];
                    if alpha > 0 {
                        let offset = ((py * width + px) * 4) as usize;
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
