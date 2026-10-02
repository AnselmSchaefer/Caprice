//! Pictures: loading them into the document, their textures, and resizing.

use std::path::Path;

use eframe::egui::{self, ColorImage, TextureOptions};

use crate::App;
use crate::edit::{Edit, Piece};
use crate::model::{IMAGE_CHAR, ImageData, ParaAttrs};

/// Largest side (in pixels) of the texture a picture is shown with.
const MAX_TEXTURE_SIDE: u32 = 2048;

/// Check that `bytes` is a picture we can show, and describe it. Other formats are not supported.
pub fn describe(bytes: &[u8]) -> Result<(String, (u32, u32)), String> {
    let format = match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::Jpeg) => "jpeg",
        _ => return Err("only PNG and JPEG pictures are supported".into()),
    };
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    Ok((format.to_owned(), (img.width(), img.height())))
}

fn texture_image(bytes: &[u8]) -> Option<ColorImage> {
    let mut img = image::load_from_memory(bytes).ok()?;
    if img.width().max(img.height()) > MAX_TEXTURE_SIDE {
        img = img.resize(MAX_TEXTURE_SIDE, MAX_TEXTURE_SIDE, image::imageops::FilterType::Triangle);
    }
    let rgba = img.to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw()))
}

impl App {
    /// Make sure every picture of the document has a texture.
    pub fn ensure_textures(&mut self, ctx: &egui::Context) {
        for img in &self.doc.images {
            if !self.textures.contains_key(&img.id) {
                if let Some(ci) = texture_image(&img.bytes) {
                    let tex = ctx.load_texture(format!("picture-{}", img.id), ci, TextureOptions::LINEAR);
                    self.textures.insert(img.id, tex);
                }
            }
        }
    }

    /// Put the picture in the file at `path` into the text at the caret, in a paragraph of its own.
    pub fn insert_image(&mut self, ctx: &egui::Context, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        self.insert_image_bytes(ctx, bytes)
    }

    pub fn insert_image_bytes(&mut self, ctx: &egui::Context, bytes: Vec<u8>) -> Result<(), String> {
        let (format, px) = describe(&bytes)?;
        let id = self.doc.images.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        // 96 dpi, but never wider than the writing area.
        let width_pt = (px.0 as f32 * 0.75).min(self.doc.setup.content_size().x);
        self.doc.images.push(ImageData { id, format, bytes, px, width_pt });
        self.ensure_textures(ctx);

        let (a, b) = self.selection();
        let (ps, _) = self.doc.para_start(a);
        let line = self.doc.para_attrs_at(a);
        let mark_attrs = ParaAttrs { list: crate::model::ListKind::None, ..line };
        let mut text = String::new();
        let mut styles = Vec::new();
        if ps != a {
            text.push('\n'); // the picture needs a paragraph of its own
            styles.push(self.typing.with_para(line));
        }
        text.push(IMAGE_CHAR);
        let mut pic = self.typing.with_para(ParaAttrs::default());
        pic.image = id;
        styles.push(pic);
        text.push('\n');
        styles.push(self.typing.with_para(mark_attrs));

        let max = self.doc.total_chars() - 1;
        let (a, b) = (a.min(max), b.min(max));
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        let old = Piece { text: self.doc.flow.text[ba..bb].to_owned(), styles: self.doc.flow.styles[a..b].to_vec() };
        self.apply_edit(ctx, Edit::Replace { at: a, old, new: Piece { text, styles } }, b);
        Ok(())
    }

    /// The picture the caret is on (the one in the caret's paragraph, if that paragraph is a picture).
    pub fn selected_image(&self) -> Option<u32> {
        let (ps, pb) = self.doc.para_start(self.caret);
        let (tc, _) = self.doc.term_from(ps, pb);
        (tc == ps + 1).then(|| self.doc.flow.styles[ps].image).filter(|&id| id != 0)
    }

    /// Show the selected picture at this share of the writing width.
    pub fn set_image_width_fraction(&mut self, ctx: &egui::Context, fraction: f32) {
        let Some(id) = self.selected_image() else { return };
        let full = self.doc.setup.content_size().x;
        if let Some(img) = self.doc.images.iter_mut().find(|i| i.id == id) {
            img.width_pt = full * fraction;
        }
        self.doc.version += 1;
        self.doc.full_paginate(ctx, &self.typing);
        let c = self.caret;
        self.set_caret(ctx, c, false);
    }
}

/// A small solid-colour PNG for tests.
#[cfg(test)]
pub fn test_png(w: u32, h: u32, rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb(rgb));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_png_and_rejects_other_formats() {
        let (format, px) = describe(&test_png(40, 20, [200, 30, 30])).unwrap();
        assert_eq!((format.as_str(), px), ("png", (40, 20)));
        assert!(describe(b"GIF89a....").is_err());
        assert!(describe(b"not an image").is_err());
    }
}
