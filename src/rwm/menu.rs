//! Window menu rendering and interaction.

use crate::config::UiConfig;
use fontdue::{Font, FontSettings};
use memmap2::MmapMut;
use std::fs::File;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::OnceLock;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_region, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::QueueHandle;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use super::{OutputId, WindowId};

/// Menu entry data.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub window_id: WindowId,
    pub title: String,
    pub hidden: bool,
    pub active: bool,
}

/// Window menu surface and state.
pub struct WindowMenu {
    pub surface: wl_surface::WlSurface,
    pub layer_surface: ZwlrLayerSurfaceV1,
    pub buffer: Option<wl_buffer::WlBuffer>,
    pub pool: Option<wl_shm_pool::WlShmPool>,
    pub memfile: Option<File>,
    pub mmap: Option<MmapMut>,
    pub width: i32,
    pub height: i32,
    pub buffer_width: i32,
    pub buffer_height: i32,
    pub scale: i32,
    pub configured: bool,
    pub items: Vec<MenuItem>,
    pub hovered: Option<usize>,
    pub output_id: OutputId,
    pub origin_x: i32,
    pub origin_y: i32,
    pub theme: MenuTheme,
}

const MENU_BORDER: i32 = 1;
const ITEM_PADDING_X: i32 = 8;
const ITEM_PADDING_Y: i32 = 4;
const ICON_SIZE: i32 = 10;
const ICON_GAP: i32 = 6;
const ACTIVE_DIAMOND_SIZE: i32 = 8;
const SHADOW_SIZE: i32 = 3;

const BORDER_COLOR: u32 = 0x000000FF;
const SHADOW_COLOR: u32 = 0x404040FF;

#[derive(Debug, Clone)]
pub struct MenuTheme {
    pub font_name: Option<String>,
    pub font_size: f32,
    pub bg: u32,
    pub text: u32,
    pub highlight_bg: u32,
    pub highlight_text: u32,
}

impl MenuTheme {
    pub fn from_ui(ui: &UiConfig) -> Self {
        Self {
            font_name: ui.font_name.clone(),
            font_size: ui.font_size,
            bg: ui.menu_bg,
            text: ui.menu_text,
            highlight_bg: ui.menu_highlight_bg,
            highlight_text: ui.menu_highlight_text,
        }
    }
}

impl WindowMenu {
    pub fn new(
        surface: wl_surface::WlSurface,
        layer_surface: ZwlrLayerSurfaceV1,
        output_id: OutputId,
        items: Vec<MenuItem>,
        origin_x: i32,
        origin_y: i32,
        theme: MenuTheme,
    ) -> Self {
        let (width, height) = measure_menu(&items, &theme);
        Self {
            surface,
            layer_surface,
            buffer: None,
            pool: None,
            memfile: None,
            mmap: None,
            width,
            height,
            buffer_width: width,
            buffer_height: height,
            scale: 1,
            configured: false,
            items,
            hovered: None,
            output_id,
            origin_x,
            origin_y,
            theme,
        }
    }

    pub fn item_at(&self, x: i32, y: i32) -> Option<usize> {
        let content_w = self.menu_width();
        let content_h = self.menu_height();
        let content_x = MENU_BORDER;
        let content_y = MENU_BORDER;
        let content_w = content_w - MENU_BORDER * 2;
        let content_h = content_h - MENU_BORDER * 2;
        if x < content_x
            || y < content_y
            || x >= content_x + content_w
            || y >= content_y + content_h
        {
            return None;
        }

        let item_h = item_height(&self.theme);
        let idx = ((y - content_y) / item_h) as usize;
        if idx < self.items.len() {
            Some(idx)
        } else {
            None
        }
    }

    pub fn update_hover(&mut self, x: i32, y: i32) -> bool {
        let next = self.item_at(x, y);
        if next != self.hovered {
            self.hovered = next;
            return true;
        }
        false
    }

    pub fn select_next(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let next = match self.hovered {
            Some(idx) => (idx + 1) % self.items.len(),
            None => 0,
        };
        if self.hovered != Some(next) {
            self.hovered = Some(next);
            return true;
        }
        false
    }

    pub fn select_window(&mut self, window_id: Option<WindowId>) -> bool {
        let next = window_id.and_then(|id| self.items.iter().position(|item| item.window_id == id));
        if self.hovered != next {
            self.hovered = next;
            return true;
        }
        false
    }

    pub fn ensure_buffer<D>(&mut self, shm: &wl_shm::WlShm, qh: &QueueHandle<D>, scale: i32)
    where
        D: 'static
            + wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>
            + wayland_client::Dispatch<wl_buffer::WlBuffer, ()>,
    {
        if self.width <= 0 || self.height <= 0 {
            return;
        }

        let scale = scale.max(1);
        let buffer_width = self.width * scale;
        let buffer_height = self.height * scale;
        if buffer_width <= 0 || buffer_height <= 0 {
            return;
        }

        if self.buffer.is_some()
            && self.buffer_width == buffer_width
            && self.buffer_height == buffer_height
            && self.scale == scale
        {
            return;
        }

        self.surface.set_buffer_scale(scale);

        let stride = buffer_width * 4;
        let size = stride * buffer_height;
        let memfd = match memfd::MemfdOptions::default()
            .close_on_exec(true)
            .create("rwm-menu")
        {
            Ok(fd) => fd,
            Err(e) => {
                log::error!("Failed to create menu memfd: {}", e);
                return;
            }
        };

        if let Err(e) = memfd.as_file().set_len(size as u64) {
            log::error!("Failed to size menu memfd: {}", e);
            return;
        }

        let mmap = match unsafe { memmap2::MmapMut::map_mut(memfd.as_file()) } {
            Ok(m) => m,
            Err(e) => {
                log::error!("Failed to mmap menu buffer: {}", e);
                return;
            }
        };

        let pool = shm.create_pool(memfd.as_file().as_fd(), size, qh, ());
        let buffer = pool.create_buffer(
            0,
            buffer_width,
            buffer_height,
            stride,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        self.memfile = Some(memfd.into_file());
        self.mmap = Some(mmap);
        self.pool = Some(pool);
        self.buffer = Some(buffer);
        self.buffer_width = buffer_width;
        self.buffer_height = buffer_height;
        self.scale = scale;
    }

    pub fn reset_buffer(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        self.memfile = None;
        self.mmap = None;
    }

    pub fn render(&mut self) {
        let menu_w = self.menu_width();
        let menu_h = self.menu_height();
        if menu_w <= 0 || menu_h <= 0 {
            return;
        }

        let Some(ref mut mmap) = self.mmap else {
            return;
        };

        let scale = self.scale.max(1);
        let menu_w_px = menu_w * scale;
        let menu_h_px = menu_h * scale;

        let pixels = mmap.as_mut();
        clear_buffer(pixels);

        draw_shadow(
            pixels,
            self.buffer_width,
            self.buffer_height,
            menu_w_px,
            menu_h_px,
            rgba_to_argb(SHADOW_COLOR),
        );

        fill_rect(
            pixels,
            self.buffer_width,
            self.buffer_height,
            0,
            0,
            menu_w_px,
            menu_h_px,
            rgba_to_argb(self.theme.bg),
        );

        draw_border_rect(
            pixels,
            self.buffer_width,
            self.buffer_height,
            0,
            0,
            menu_w_px,
            menu_h_px,
            rgba_to_argb(BORDER_COLOR),
        );

        let item_h = item_height(&self.theme);
        let start_x = MENU_BORDER + ITEM_PADDING_X;
        let mut y = MENU_BORDER;
        let text_start_x = start_x + ICON_SIZE + ICON_GAP;
        let text_area_w = menu_w - MENU_BORDER * 2 - text_start_x + MENU_BORDER;

        for (idx, item) in self.items.iter().enumerate() {
            let is_active = self.hovered == Some(idx);
            let bg = if is_active {
                rgba_to_argb(self.theme.highlight_bg)
            } else {
                rgba_to_argb(self.theme.bg)
            };
            let text_color = if is_active {
                rgba_to_argb(self.theme.highlight_text)
            } else {
                rgba_to_argb(self.theme.text)
            };
            let icon_color = if is_active {
                text_color
            } else {
                rgba_to_argb(self.theme.text)
            };

            fill_rect(
                pixels,
                self.buffer_width,
                self.buffer_height,
                MENU_BORDER * scale,
                y * scale,
                (menu_w - MENU_BORDER * 2) * scale,
                item_h * scale,
                bg,
            );

            if item.hidden {
                draw_dashed_rect(
                    pixels,
                    self.buffer_width,
                    self.buffer_height,
                    start_x * scale,
                    (y + (item_h - ICON_SIZE) / 2) * scale,
                    ICON_SIZE * scale,
                    ICON_SIZE * scale,
                    icon_color,
                );
            }
            if item.active {
                draw_diamond(
                    pixels,
                    self.buffer_width,
                    self.buffer_height,
                    (start_x + (ICON_SIZE - ACTIVE_DIAMOND_SIZE) / 2) * scale,
                    (y + (item_h - ACTIVE_DIAMOND_SIZE) / 2) * scale,
                    ACTIVE_DIAMOND_SIZE * scale,
                    text_color,
                );
            }

            render_text(
                pixels,
                self.buffer_width,
                self.buffer_height,
                &item.title,
                text_start_x * scale,
                y * scale,
                text_area_w * scale,
                item_h * scale,
                text_color,
                scale,
                &self.theme,
            );

            y += item_h;
        }
    }

    pub fn update_input_region<D>(
        &self,
        compositor: &wl_compositor::WlCompositor,
        qh: &QueueHandle<D>,
    ) where
        D: 'static + wayland_client::Dispatch<wl_region::WlRegion, ()>,
    {
        let menu_w = self.menu_width();
        let menu_h = self.menu_height();
        if menu_w <= 0 || menu_h <= 0 {
            return;
        }

        let region = compositor.create_region(qh, ());
        region.add(0, 0, menu_w, menu_h);
        self.surface.set_input_region(Some(&region));
        region.destroy();
    }

    pub fn commit(&self) {
        if let Some(ref buffer) = self.buffer {
            self.surface.attach(Some(buffer), 0, 0);
            self.surface
                .damage_buffer(0, 0, self.buffer_width, self.buffer_height);
            self.surface.commit();
        }
    }

    fn menu_width(&self) -> i32 {
        (self.width - SHADOW_SIZE).max(0)
    }

    fn menu_height(&self) -> i32 {
        (self.height - SHADOW_SIZE).max(0)
    }
}

impl Drop for WindowMenu {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }
        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }
        self.layer_surface.destroy();
        self.surface.destroy();
    }
}

fn item_height(theme: &MenuTheme) -> i32 {
    let font_h = theme.font_size.ceil() as i32;
    (font_h + ITEM_PADDING_Y * 2).max(1)
}

fn measure_menu(items: &[MenuItem], theme: &MenuTheme) -> (i32, i32) {
    let font = match get_font(theme.font_name.as_deref()) {
        Some(f) => f,
        None => {
            let menu_w = 120;
            let menu_h = (items.len() as i32 * item_height(theme)).max(1) + MENU_BORDER * 2;
            return (menu_w + SHADOW_SIZE, menu_h + SHADOW_SIZE);
        }
    };

    let mut max_width = 0.0f32;
    for item in items {
        let w = measure_text(font, &item.title, theme.font_size);
        if w > max_width {
            max_width = w;
        }
    }

    let content_w = ITEM_PADDING_X * 2 + ICON_SIZE + ICON_GAP + max_width.ceil() as i32;
    let content_h = item_height(theme) * items.len() as i32;
    let menu_w = content_w + MENU_BORDER * 2;
    let menu_h = content_h + MENU_BORDER * 2;
    (menu_w + SHADOW_SIZE, menu_h + SHADOW_SIZE)
}

fn measure_text(font: &Font, text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| font.metrics(ch, font_size).advance_width)
        .sum()
}

fn get_font(font_name: Option<&str>) -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    let requested = font_name.map(|name| name.to_string());

    FONT.get_or_init(|| {
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
                        return Some(font);
                    }
                }
            } else {
                for candidate in font_paths {
                    if Path::new(candidate).file_name().and_then(|n| n.to_str()) == Some(name) {
                        if let Ok(font_data) = std::fs::read(candidate) {
                            if let Ok(font) = Font::from_bytes(font_data, FontSettings::default()) {
                                return Some(font);
                            }
                        }
                    }
                }
            }
        }

        for path in font_paths {
            if let Ok(font_data) = std::fs::read(path) {
                if let Ok(font) = Font::from_bytes(font_data, FontSettings::default()) {
                    return Some(font);
                }
            }
        }
        None
    })
    .as_ref()
}

#[allow(clippy::too_many_arguments)]
fn render_text(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    text: &str,
    origin_x: i32,
    origin_y: i32,
    area_width: i32,
    area_height: i32,
    text_argb: u32,
    scale: i32,
    theme: &MenuTheme,
) {
    let font = match get_font(theme.font_name.as_deref()) {
        Some(f) => f,
        None => return,
    };

    let font_size = theme.font_size * scale.max(1) as f32;
    let baseline_y = if let Some(line_metrics) = font.horizontal_line_metrics(font_size) {
        let line_height = line_metrics.ascent - line_metrics.descent;
        origin_y as f32 + (area_height as f32 - line_height) / 2.0 + line_metrics.ascent
    } else {
        let metrics = font.metrics('A', font_size);
        origin_y as f32
            + (area_height as f32 - metrics.height as f32) / 2.0
            + (metrics.ymin as f32 + metrics.height as f32)
    };

    let mut x_pos = origin_x as f32;
    let max_x = origin_x + area_width;

    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        if (x_pos + metrics.advance_width) as i32 > max_x {
            break;
        }

        let glyph_x = x_pos as i32 + metrics.xmin;
        let glyph_y = baseline_y as i32 - (metrics.ymin + metrics.height as i32);

        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let px = glyph_x + col as i32;
                let py = glyph_y + row as i32;
                if px < origin_x
                    || px >= origin_x + area_width
                    || py < origin_y
                    || py >= origin_y + area_height
                    || px < 0
                    || py < 0
                    || px >= buffer_width
                    || py >= buffer_height
                {
                    continue;
                }
                let alpha = bitmap[row * metrics.width + col];
                if alpha > 0 {
                    let offset = ((py * buffer_width + px) * 4) as usize;
                    if offset + 4 <= pixels.len() {
                        blend_pixel(&mut pixels[offset..offset + 4], text_argb, alpha);
                    }
                }
            }
        }

        x_pos += metrics.advance_width;
    }
}

fn rgba_to_argb(rgba: u32) -> u32 {
    let r = (rgba >> 24) & 0xff;
    let g = (rgba >> 16) & 0xff;
    let b = (rgba >> 8) & 0xff;
    let a = rgba & 0xff;
    (a << 24) | (r << 16) | (g << 8) | b
}

fn blend_pixel(bg: &mut [u8], fg_argb: u32, alpha: u8) {
    let fg_a = ((fg_argb >> 24) & 0xff) as u16;
    let fg_r = ((fg_argb >> 16) & 0xff) as u16;
    let fg_g = ((fg_argb >> 8) & 0xff) as u16;
    let fg_b = (fg_argb & 0xff) as u16;

    let a = (alpha as u16 * fg_a) / 255;
    let inv_a = 255 - a;

    let bg_val = u32::from_ne_bytes([bg[0], bg[1], bg[2], bg[3]]);
    let bg_r = ((bg_val >> 16) & 0xff) as u16;
    let bg_g = ((bg_val >> 8) & 0xff) as u16;
    let bg_b = (bg_val & 0xff) as u16;

    let out_r = ((fg_r * a + bg_r * inv_a) / 255) as u8;
    let out_g = ((fg_g * a + bg_g * inv_a) / 255) as u8;
    let out_b = ((fg_b * a + bg_b * inv_a) / 255) as u8;

    let out_argb = 0xFF000000 | ((out_r as u32) << 16) | ((out_g as u32) << 8) | (out_b as u32);
    bg.copy_from_slice(&out_argb.to_ne_bytes());
}

fn clear_buffer(pixels: &mut [u8]) {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk.copy_from_slice(&[0, 0, 0, 0]);
    }
}

#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
fn draw_border_rect(
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

    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y,
        width,
        1,
        color_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y + height - 1,
        width,
        1,
        color_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x,
        y,
        1,
        height,
        color_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        x + width - 1,
        y,
        1,
        height,
        color_argb,
    );
}

fn draw_shadow(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    menu_width: i32,
    menu_height: i32,
    color_argb: u32,
) {
    if menu_width <= 0 || menu_height <= 0 {
        return;
    }

    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        SHADOW_SIZE,
        menu_height,
        menu_width,
        SHADOW_SIZE,
        color_argb,
    );
    fill_rect(
        pixels,
        buffer_width,
        buffer_height,
        menu_width,
        SHADOW_SIZE,
        SHADOW_SIZE,
        menu_height,
        color_argb,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_dashed_rect(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color_argb: u32,
) {
    let dash = 2;
    let gap = 2;
    let mut px = x;
    while px < x + width {
        let segment = (x + width - px).min(dash);
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            px,
            y,
            segment,
            1,
            color_argb,
        );
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            px,
            y + height - 1,
            segment,
            1,
            color_argb,
        );
        px += dash + gap;
    }

    let mut py = y;
    while py < y + height {
        let segment = (y + height - py).min(dash);
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            x,
            py,
            1,
            segment,
            color_argb,
        );
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            x + width - 1,
            py,
            1,
            segment,
            color_argb,
        );
        py += dash + gap;
    }
}

fn draw_diamond(
    pixels: &mut [u8],
    buffer_width: i32,
    buffer_height: i32,
    x: i32,
    y: i32,
    size: i32,
    color_argb: u32,
) {
    let half = size / 2;
    for row in 0..size {
        let dist = (half - row).abs();
        let span = size - dist * 2;
        let draw_x = x + dist;
        fill_rect(
            pixels,
            buffer_width,
            buffer_height,
            draw_x,
            y + row,
            span.max(1),
            1,
            color_argb,
        );
    }
}
