//! Titlebar rendering for windows

use crate::config::UiConfig;
use crate::protocol::RiverDecorationV1;
use fontdue::{Font, FontSettings};
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::OnceLock;
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_region, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::QueueHandle;

/// Titlebar height in pixels
pub fn titlebar_height(ui: &UiConfig) -> i32 {
    let base = (0.75 * ui.font_size).round() as i32;
    (base * 2 + 1).max(1)
}

/// Button background color (pressed left edge)
const BUTTON_BG_PRESSED_LEFT: u32 = 0xA0A0A0FF;
const BUTTON_LIGHT_EDGE: u32 = 0xFFFFFFFF;

const BORDER_OUTER: i32 = 1;
const BORDER_MID: i32 = 3;
const BORDER_INNER: i32 = 1;

/// Horizontal padding for title text
const TITLE_PADDING: i32 = 8;

const BUTTON_PADDING_X: i32 = 0;
const BUTTON_GAP: i32 = 1;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TitlebarButtons {
    pub close: Rect,
    pub hide: Rect,
    pub maximize: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TitlebarButton {
    Close,
    Hide,
    Maximize,
}

pub fn button_rects(content_width: i32, titlebar_height: i32) -> TitlebarButtons {
    let size = titlebar_height;
    let y = 0;
    let close = Rect {
        x: BUTTON_PADDING_X,
        y,
        width: size,
        height: size,
    };

    let right_outer_x = content_width - BUTTON_PADDING_X - size;
    let right_inner_x = right_outer_x - size - BUTTON_GAP;

    let maximize = Rect {
        x: right_outer_x.max(0),
        y,
        width: size,
        height: size,
    };
    let hide = Rect {
        x: right_inner_x.max(0),
        y,
        width: size,
        height: size,
    };

    TitlebarButtons {
        close,
        hide,
        maximize,
    }
}

pub fn button_at(
    content_width: i32,
    border_width: i32,
    local_x: i32,
    local_y: i32,
    titlebar_height: i32,
) -> Option<TitlebarButton> {
    if content_width <= 0 {
        return None;
    }

    let rel_x = local_x - border_width;
    let rel_y = local_y - border_width;
    if rel_x < 0 || rel_y < 0 || rel_x >= content_width || rel_y >= titlebar_height {
        return None;
    }

    let buttons = button_rects(content_width, titlebar_height);
    if buttons.close.contains(rel_x, rel_y) {
        return Some(TitlebarButton::Close);
    }
    if buttons.hide.contains(rel_x, rel_y) {
        return Some(TitlebarButton::Hide);
    }
    if buttons.maximize.contains(rel_x, rel_y) {
        return Some(TitlebarButton::Maximize);
    }

    None
}

/// Get or initialize the titlebar font
fn get_font(font_name: Option<&str>) -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    let requested = font_name.map(|name| name.to_string());
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

        if let Some(name) = requested.as_deref() {
            let path = Path::new(name);
            if path.is_file() {
                if let Ok(font_data) = std::fs::read(path) {
                    if let Ok(font) = Font::from_bytes(font_data, FontSettings::default()) {
                        log::info!("Loaded titlebar font from {}", path.display());
                        return Some(font);
                    }
                }
            } else {
                for candidate in font_paths {
                    if Path::new(candidate).file_name().and_then(|n| n.to_str()) == Some(name) {
                        if let Ok(font_data) = std::fs::read(candidate) {
                            if let Ok(font) = Font::from_bytes(font_data, FontSettings::default()) {
                                log::info!("Loaded titlebar font from {}", candidate);
                                return Some(font);
                            }
                        }
                    }
                }
            }
            log::warn!("Requested titlebar font {} not found, falling back", name);
        }

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
    /// Current buffer width in pixels
    pub buffer_width: i32,
    /// Current buffer height in pixels
    pub buffer_height: i32,
    /// Current content width
    pub content_width: i32,
    /// Current content height
    pub content_height: i32,
    /// Output scale factor
    pub scale: i32,
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
            buffer_width: 0,
            buffer_height: 0,
            content_width: 0,
            content_height: 0,
            scale: 1,
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
        scale: i32,
        ui: &UiConfig,
    ) where
        D: wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
            + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>,
    {
        if content_width <= 0 || content_height <= 0 {
            log::info!("ensure_buffer: invalid content size, skipping");
            return;
        }

        let scale = scale.max(1);
        let titlebar_height = titlebar_height(ui);
        let width = content_width + ui.border_width * 2;
        let height = content_height + titlebar_height + ui.border_width * 2;
        let buffer_width = width * scale;
        let buffer_height = height * scale;
        if buffer_width <= 0 || buffer_height <= 0 {
            log::info!("ensure_buffer: invalid buffer size, skipping");
            return;
        }

        // Check if we need a new buffer
        if self.width != width
            || self.height != height
            || self.buffer_width != buffer_width
            || self.buffer_height != buffer_height
            || self.scale != scale
            || self.buffer.is_none()
        {
            log::info!(
                "ensure_buffer: creating new buffer for {}x{} (content {}x{})",
                width,
                height,
                content_width,
                content_height
            );
            self.width = width;
            self.height = height;
            self.buffer_width = buffer_width;
            self.buffer_height = buffer_height;
            self.content_width = content_width;
            self.content_height = content_height;
            self.scale = scale;
            self.dirty = true;

            // Clean up old buffer
            if let Some(buffer) = self.buffer.take() {
                buffer.destroy();
            }
            if let Some(pool) = self.pool.take() {
                pool.destroy();
            }

            // Calculate buffer size (ARGB8888 = 4 bytes per pixel)
            let stride = buffer_width * 4;
            let size = stride * buffer_height;
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
                buffer_width,
                buffer_height,
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

        self.surface.set_buffer_scale(scale);
    }

    /// Render the titlebar with the given title and state
    pub fn render(
        &mut self,
        title: Option<&str>,
        is_active: bool,
        is_maximized: bool,
        hovered_button: Option<TitlebarButton>,
        left_down: bool,
        ui: &UiConfig,
    ) {
        if let Some(ref mut mmap) = self.mmap {
            let scale = self.scale.max(1);
            let width = self.width;
            let height = self.height;
            let buffer_width = self.buffer_width;
            let buffer_height = self.buffer_height;
            let content_width = self.content_width;
            let content_height = self.content_height;
            let titlebar_height = titlebar_height(ui);
            if width <= 0
                || height <= 0
                || buffer_width <= 0
                || buffer_height <= 0
                || content_width <= 0
                || content_height <= 0
            {
                return;
            }

            let pixels = mmap.as_mut();
            clear_buffer(pixels);

            let border_offset = 0;
            let border_colors = if is_active {
                ui.border_active
            } else {
                ui.border_inactive
            };
            draw_border_layer(
                pixels,
                buffer_width,
                buffer_height,
                border_offset,
                BORDER_OUTER * scale,
                border_colors.outer,
            );
            draw_border_layer(
                pixels,
                buffer_width,
                buffer_height,
                border_offset + BORDER_OUTER * scale,
                BORDER_MID * scale,
                border_colors.mid,
            );
            draw_border_layer(
                pixels,
                buffer_width,
                buffer_height,
                border_offset + (BORDER_OUTER + BORDER_MID) * scale,
                BORDER_INNER * scale,
                border_colors.inner,
            );

            // Choose background color based on active state
            let bg_color = if is_active {
                ui.titlebar_bg_active
            } else {
                ui.titlebar_bg_inactive
            };
            let bg_argb = rgba_to_argb(bg_color);

            let title_height = titlebar_height.min(height - ui.border_width * 2);
            if title_height > 0 {
                let title_x = ui.border_width;
                let title_y = ui.border_width;
                fill_rect(
                    pixels,
                    buffer_width,
                    buffer_height,
                    title_x * scale,
                    title_y * scale,
                    content_width * scale,
                    title_height * scale,
                    bg_argb,
                );

                let buttons = button_rects(content_width, titlebar_height);
                let button_bg = rgba_to_argb(ui.button_bg);
                let pressed_hover = if left_down { hovered_button } else { None };
                let close_bg = if pressed_hover == Some(TitlebarButton::Close) {
                    rgba_to_argb(BUTTON_BG_PRESSED_LEFT)
                } else {
                    button_bg
                };
                let button_border = rgba_to_argb(border_colors.outer);

                fill_rect(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.close.x) * scale,
                    (title_y + buttons.close.y) * scale,
                    buttons.close.width * scale,
                    title_height * scale,
                    close_bg,
                );
                draw_glyph_close(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.close.x) * scale,
                    (title_y + buttons.close.y) * scale,
                    buttons.close.width * scale,
                    button_border,
                    titlebar_height,
                );
                draw_left_border(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.close.x + buttons.close.width) * scale,
                    title_y * scale,
                    title_height * scale,
                    button_border,
                    titlebar_height,
                );

                draw_button_bevel(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.hide.x) * scale,
                    (title_y + buttons.hide.y) * scale,
                    buttons.hide.width * scale,
                    button_bg,
                    rgba_to_argb(border_colors.mid),
                    pressed_hover == Some(TitlebarButton::Hide),
                    titlebar_height,
                );
                draw_left_border(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.hide.x - 1) * scale,
                    title_y * scale,
                    title_height * scale,
                    button_border,
                    titlebar_height,
                );
                draw_glyph_caret(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.hide.x) * scale,
                    (title_y + buttons.hide.y) * scale,
                    buttons.hide.width * scale,
                    button_border,
                    true,
                    titlebar_height,
                );

                draw_button_bevel(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.maximize.x) * scale,
                    (title_y + buttons.maximize.y) * scale,
                    buttons.maximize.width * scale,
                    button_bg,
                    rgba_to_argb(border_colors.mid),
                    pressed_hover == Some(TitlebarButton::Maximize),
                    titlebar_height,
                );
                draw_left_border(
                    pixels,
                    buffer_width,
                    buffer_height,
                    (title_x + buttons.maximize.x - 1) * scale,
                    title_y * scale,
                    title_height * scale,
                    button_border,
                    titlebar_height,
                );
                if is_maximized {
                    draw_glyph_caret_pair(
                        pixels,
                        buffer_width,
                        buffer_height,
                        (title_x + buttons.maximize.x) * scale,
                        (title_y + buttons.maximize.y) * scale,
                        buttons.maximize.width * scale,
                        button_border,
                        titlebar_height,
                    );
                } else {
                    draw_glyph_caret(
                        pixels,
                        buffer_width,
                        buffer_height,
                        (title_x + buttons.maximize.x) * scale,
                        (title_y + buttons.maximize.y) * scale,
                        buttons.maximize.width * scale,
                        button_border,
                        false,
                        titlebar_height,
                    );
                }

                let separator_y = title_y + title_height;
                if separator_y >= 0 && separator_y < height - ui.border_width {
                    fill_rect(
                        pixels,
                        buffer_width,
                        buffer_height,
                        title_x * scale,
                        separator_y * scale,
                        content_width * scale,
                        scale,
                        rgba_to_argb(border_colors.outer),
                    );
                }

                // Render title text if we have a title and font
                if let Some(title_str) = title {
                    if !title_str.is_empty() {
                        let text_start = (buttons.close.x + buttons.close.width + BUTTON_GAP)
                            .max(0);
                        let text_end = (buttons.maximize.x - BUTTON_GAP).min(content_width);
                        let text_width = (text_end - text_start).max(0);
                        if text_width > 0 {
                            let text_color = if is_active {
                                ui.titlebar_text_active
                            } else {
                                ui.titlebar_text_inactive
                            };
                            render_title(
                                pixels,
                                buffer_width,
                                buffer_height,
                                title_str,
                                (title_x + text_start) * scale,
                                title_y * scale,
                                text_width * scale,
                                title_height * scale,
                                scale,
                                text_color,
                                ui.font_size,
                                ui.font_name.as_deref(),
                            );
                        }
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
            self.surface
                .damage_buffer(0, 0, self.buffer_width, self.buffer_height);
            self.surface.commit();
        }
    }

    /// Limit input to the frame (titlebar + borders), let content receive clicks.
    pub fn update_input_region<D: 'static>(
        &self,
        compositor: &wl_compositor::WlCompositor,
        qh: &QueueHandle<D>,
        ui: &UiConfig,
    ) where
        D: wayland_client::Dispatch<wl_region::WlRegion, ()>,
    {
        if self.width <= 0 || self.height <= 0 {
            return;
        }

        let region = compositor.create_region(qh, ());
        region.add(0, 0, self.width, self.height);
        if self.content_width > 0 && self.content_height > 0 {
            let titlebar_height = titlebar_height(ui);
            region.subtract(
                ui.border_width,
                ui.border_width + titlebar_height,
                self.content_width,
                self.content_height,
            );
        }
        self.surface.set_input_region(Some(&region));
        region.destroy();
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
    scale: i32,
    text_color: u32,
    font_size: f32,
    font_name: Option<&str>,
) {
    let font = match get_font(font_name) {
        Some(f) => f,
        None => return,
    };

    let origin_y = origin_y + 1;
    let text_argb = rgba_to_argb(text_color);
    let font_size = font_size * scale.max(1) as f32;

    // Calculate baseline for vertical centering across glyphs.
    let baseline_y = if let Some(line_metrics) = font.horizontal_line_metrics(font_size) {
        let line_height = line_metrics.ascent - line_metrics.descent;
        origin_y as f32 + (area_height as f32 - line_height) / 2.0 + line_metrics.ascent
    } else {
        let metrics = font.metrics('A', font_size);
        origin_y as f32
            + (area_height as f32 - metrics.height as f32) / 2.0
            + (metrics.ymin as f32 + metrics.height as f32)
    };

    // Rasterize and draw each character
    let mut x_pos = (origin_x + TITLE_PADDING * scale.max(1)) as f32;
    let max_x = origin_x + area_width - TITLE_PADDING * scale.max(1);

    for ch in title.chars() {
        let (metrics, bitmap) = font.rasterize(ch, font_size);

        // Check if we have room for this character
        if (x_pos + metrics.advance_width) as i32 > max_x {
            break;
        }

        // Calculate position
        let glyph_x = x_pos as i32 + metrics.xmin;
        let glyph_y = baseline_y as i32 - (metrics.ymin + metrics.height as i32);

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
                            // Faux-bold: blend an extra pixel to the right.
                            let px_bold = px + 1;
                            if px_bold < origin_x + area_width && px_bold < buffer_width {
                                let offset_bold = ((py * buffer_width + px_bold) * 4) as usize;
                                if offset_bold + 4 <= pixels.len() {
                                    blend_pixel(&mut pixels[offset_bold..offset_bold + 4], text_argb, alpha);
                                }
                            }
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

fn draw_button_bevel(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    size: i32,
    bg_argb: u32,
    shadow_argb: u32,
    pressed: bool,
    titlebar_height: i32,
) {
    let unit = (size / titlebar_height.max(1)).max(1);
    let light_argb = rgba_to_argb(BUTTON_LIGHT_EDGE);
    let (light_argb, shadow_argb) = if pressed {
        (shadow_argb, light_argb)
    } else {
        (light_argb, shadow_argb)
    };

    fill_rect(pixels, buffer_width, buffer_height, x, y, size, size, bg_argb);
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y,
        size,
        unit,
        light_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y,
        unit,
        size,
        light_argb,
    );

    if size >= 3 * unit {
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            x,
            y + size - 2 * unit,
            size - unit,
            unit,
            shadow_argb,
        );
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            x + size - 2 * unit,
            y + unit,
            unit,
            size - 2 * unit,
            shadow_argb,
        );
    }

    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y + size - unit,
        size - unit,
        unit,
        shadow_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x + size - unit,
        y + unit,
        unit,
        size - 2 * unit,
        shadow_argb,
    );
}

fn draw_left_border(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    height: i32,
    color_argb: u32,
    titlebar_height: i32,
) {
    if x < 0 {
        return;
    }
    let unit = (height / titlebar_height.max(1)).max(1);
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y,
        unit,
        height,
        color_argb,
    );
}

fn draw_glyph_close(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    size: i32,
    color_argb: u32,
    titlebar_height: i32,
) {
    let unit = (size / titlebar_height.max(1)).max(1);
    let inner_size = (size - 2 * unit).max(1);
    let line_y = y + unit + (inner_size - unit) / 2;
    let line_x = x + 6 * unit;
    let line_w = size - 12 * unit;
    if line_w > 0 {
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            line_x,
            line_y,
            line_w,
            unit,
            color_argb,
        );
    }
}

fn draw_glyph_caret(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    size: i32,
    color_argb: u32,
    down: bool,
    titlebar_height: i32,
) {
    let unit = (size / titlebar_height.max(1)).max(1);
    let span = (titlebar_height / 4).max(2);
    let glyph_height = span * 2 + 1;
    let glyph_height_px = glyph_height * unit;
    let inner_size = (size - 2 * unit).max(1);
    let top_y = y + unit + (inner_size - glyph_height_px) / 2 + 4 * unit;
    let mid_x = x + size / 2;

    for i in 0..=span {
        let row = top_y + i * unit;
        let (start, width) = if down {
            let w = unit + (span - i) * 2 * unit;
            (mid_x - (span - i) * unit, w)
        } else {
            let w = unit + i * 2 * unit;
            (mid_x - i * unit, w)
        };
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            start,
            row,
            width,
            unit,
            color_argb,
        );
    }
}

fn draw_glyph_caret_pair(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    size: i32,
    color_argb: u32,
    titlebar_height: i32,
) {
    let unit = (size / titlebar_height.max(1)).max(1);
    let span = (titlebar_height / 4).max(2);
    let glyph_height = span * 2 + 1;
    let glyph_height_px = glyph_height * unit;
    let inner_size = (size - 2 * unit).max(1);
    let gap = unit;
    let total_height = glyph_height_px * 2 + gap;
    let top_y = y + unit + (inner_size - total_height) / 2 + 5 * unit;
    let mid_x = x + size / 2;

    for i in 0..=span {
        let row = top_y + i * unit;
        let width = unit + i * 2 * unit;
        let start = mid_x - i * unit;
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            start,
            row,
            width,
            unit,
            color_argb,
        );
    }

    let down_top = top_y + glyph_height_px + gap - 5 * unit;
    for i in 0..=span {
        let row = down_top + i * unit;
        let width = unit + (span - i) * 2 * unit;
        let start = mid_x - (span - i) * unit;
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            start,
            row,
            width,
            unit,
            color_argb,
        );
    }
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
