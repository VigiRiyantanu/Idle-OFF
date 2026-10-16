//! Background image decoding, texture caching, and stretched background painter.
//!
//! Complies strictly with UI specifications:
//! - Decodes embedded PNG image once via `image` crate on the first frame.
//! - Downsamples if larger than 2x window dimensions to preserve memory.
//! - Immediately drops the CPU-side RGBA buffer after GPU texture upload.
//! - Paints stretched to the full window rectangle using `ui.painter().image(...)`.

use crate::logger;
use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions};

/// Embedded background artwork (PNG format).
const BG_PNG_BYTES: &[u8] = include_bytes!("../../assets/background.png");

/// Maximum image dimensions (2x window size ~380x460) to prevent RAM bloat.
const MAX_TEX_WIDTH: u32 = 760;
const MAX_TEX_HEIGHT: u32 = 920;

/// Manages background texture lifetime and rendering.
#[derive(Default)]
pub struct BackgroundLayer {
    texture: Option<TextureHandle>,
}

impl BackgroundLayer {
    /// Uploads the texture once on first frame and returns a reference to the handle.
    fn get_or_load_texture(&mut self, ctx: &egui::Context) -> &TextureHandle {
        self.texture.get_or_insert_with(|| {
            logger::log("Decoding embedded background image via image crate");
            // Decode PNG format from embedded binary bytes
            let decoded =
                match image::load_from_memory_with_format(BG_PNG_BYTES, image::ImageFormat::Png) {
                    Ok(img) => img,
                    Err(e) => {
                        logger::log(&format!("Failed to decode PNG background image: {e}"));
                        // Fallback to minimal 1x1 dark pixel if decode fails
                        let fallback = egui::ColorImage::from_rgba_unmultiplied(
                            [1, 1],
                            &[0x14, 0x13, 0x16, 0xFF],
                        );
                        return ctx.load_texture("bg_fallback", fallback, TextureOptions::NEAREST);
                    }
                };

            // Downsample if image exceeds 2x window dimensions
            let processed_img =
                if decoded.width() > MAX_TEX_WIDTH || decoded.height() > MAX_TEX_HEIGHT {
                    decoded.resize(
                        MAX_TEX_WIDTH,
                        MAX_TEX_HEIGHT,
                        image::imageops::FilterType::Triangle,
                    )
                } else {
                    decoded
                };

            // Convert to RGBA8 buffer
            let rgba = processed_img.into_rgba8();
            let (width, height) = rgba.dimensions();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [width as usize, height as usize],
                rgba.as_raw(),
            );

            // Note: `rgba` and `processed_img` are dropped right here, freeing CPU-side memory!
            logger::log(&format!(
                "Background texture uploaded to GPU ({width}x{height})"
            ));

            ctx.load_texture(
                "app_background_stretched",
                color_image,
                TextureOptions::LINEAR,
            )
        })
    }

    /// Paints the background image stretched across the complete window rectangle.
    pub fn paint(&mut self, ui: &egui::Ui) {
        let tex = self.get_or_load_texture(ui.ctx());
        let rect = ui.max_rect();

        // Stretched draw ignoring aspect ratio as explicitly specified
        ui.painter().image(
            tex.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}
