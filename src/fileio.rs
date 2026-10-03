//! The `.caprice` file format (JSON).
//!
//! Version 2 stores the document as one list of styled runs; page breaks are `\u{c}` characters
//! inside it. Version 1 files stored a list of pages, which are loaded as hard-separated pages.

use std::collections::BTreeSet;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::model::{Doc, Flow, ImageData, Note, PAGE_BREAK, PageSetup, ParaAttrs, Style};

#[derive(Serialize, Deserialize)]
struct Run {
    text: String,
    font: String,
    size: f32,
    bold: bool,
    underline: bool,
    /// Paragraph format; only present on runs holding paragraph marks.
    #[serde(default, skip_serializing_if = "is_default_para")]
    para: ParaAttrs,
    /// Picture id, on the placeholder character of a picture.
    #[serde(default, skip_serializing_if = "is_zero")]
    image: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn is_default_para(p: &ParaAttrs) -> bool {
    *p == ParaAttrs::default()
}

#[derive(Serialize, Deserialize)]
struct NoteFile {
    start: usize,
    end: usize,
    text: String,
    #[serde(default)]
    color: usize,
}

#[derive(Serialize, Deserialize)]
struct ImageFile {
    id: u32,
    format: String,
    /// The original file, base64 encoded.
    data: String,
    width_pt: f32,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    rotation: u8,
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

#[derive(Serialize, Deserialize)]
pub struct DocFile {
    version: u32,
    #[serde(default)]
    setup: PageSetup,
    #[serde(default)]
    content: Vec<Run>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<NoteFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<ImageFile>,
    /// Version 1 only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pages: Vec<Vec<Run>>,
}

fn to_runs(text: &str, styles: &[Style]) -> Vec<Run> {
    let mut runs: Vec<(Style, String)> = Vec::new();
    for (ch, st) in text.chars().zip(styles) {
        match runs.last_mut() {
            Some((s, t)) if s == st => t.push(ch),
            _ => runs.push((st.clone(), ch.to_string())),
        }
    }
    runs.into_iter()
        .map(|(s, text)| Run { text, font: s.font.to_string(), size: s.size, bold: s.bold, underline: s.underline, para: s.para, image: s.image })
        .collect()
}

fn from_runs(runs: Vec<Run>, text: &mut String, styles: &mut Vec<Style>) {
    for r in runs {
        let st = Style { font: r.font.into(), size: r.size, bold: r.bold, underline: r.underline, para: r.para, image: r.image };
        styles.extend(std::iter::repeat_n(st, r.text.chars().count()));
        text.push_str(&r.text);
    }
}

impl DocFile {
    pub fn from_doc(doc: &Doc) -> Self {
        let notes = doc
            .notes
            .iter()
            .map(|n| NoteFile { start: n.start, end: n.end, text: n.text.clone(), color: n.color })
            .collect();
        // Only pictures that are still in the text are kept.
        let used: BTreeSet<u32> = doc.flow.styles.iter().map(|s| s.image).filter(|&i| i != 0).collect();
        let images = doc
            .images
            .iter()
            .filter(|i| used.contains(&i.id))
            .map(|i| ImageFile { id: i.id, format: i.format.clone(), data: BASE64.encode(&i.bytes), width_pt: i.width_pt, rotation: i.rotation })
            .collect();
        Self {
            version: 2,
            setup: doc.setup.clone(),
            content: to_runs(&doc.flow.text, &doc.flow.styles),
            notes,
            images,
            pages: Vec::new(),
        }
    }

    pub fn into_doc(self) -> Doc {
        let mut text = String::new();
        let mut styles = Vec::new();
        from_runs(self.content, &mut text, &mut styles);
        for (k, page) in self.pages.into_iter().enumerate() {
            if k > 0 {
                let st = styles.last().cloned().unwrap_or_else(|| Style::new("Default"));
                text.push(PAGE_BREAK);
                styles.push(st);
            }
            from_runs(page, &mut text, &mut styles);
        }
        let mut doc = Doc::new();
        doc.flow = Flow { text, styles };
        doc.ensure_final_mark();
        doc.setup = self.setup;
        doc.setup.clamp_margins();
        for img in self.images {
            let Ok(bytes) = BASE64.decode(img.data.as_bytes()) else { continue };
            let Ok((format, px)) = crate::images::describe(&bytes) else { continue };
            doc.images.push(ImageData { id: img.id, format, bytes, px, width_pt: img.width_pt, rotation: img.rotation % 4 });
        }
        let total = doc.total_chars();
        for n in self.notes {
            let (start, end) = (n.start.min(total), n.end.min(total));
            let id = doc.next_note_id;
            doc.next_note_id += 1;
            doc.notes.push(Note { id, start: start.min(end), end, text: n.text, color: n.color });
        }
        doc
    }
}

impl App {
    pub fn save(&mut self, save_as: bool) {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let dialog = rfd::FileDialog::new()
                    .add_filter("Caprice document", &["caprice"])
                    .set_file_name("Untitled.caprice");
                let Some(p) = dialog.save_file() else { return };
                p
            }
        };
        let json = serde_json::to_string_pretty(&DocFile::from_doc(&self.doc));
        let written = json.map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string()));
        self.status = match written {
            Ok(()) => {
                self.path = Some(path);
                "- saved".into()
            }
            Err(e) => format!("- save failed: {e}"),
        };
    }

    /// Write the document as a Word file (`.docx`) next to wherever the user chooses.
    pub fn export_docx(&mut self) {
        let stem = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or("Untitled".to_owned(), |s| s.to_string_lossy().into_owned());
        let dialog = rfd::FileDialog::new()
            .add_filter("Word document", &["docx"])
            .set_file_name(format!("{stem}.docx"));
        let Some(path) = dialog.save_file() else { return };
        let written = crate::export::to_docx(&self.doc).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
        self.status = match written {
            Ok(()) => format!("- exported {}", path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())),
            Err(e) => format!("- export failed: {e}"),
        };
    }

    pub fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("Caprice document", &["caprice"]).pick_file() else {
            return;
        };
        self.open_path(ctx, path);
    }

    pub fn open_path(&mut self, ctx: &egui::Context, path: std::path::PathBuf) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<DocFile>(&s).map_err(|e| e.to_string()));
        match loaded {
            Ok(file) => {
                let mut doc = file.into_doc();
                doc.version = self.doc.version + 1;
                self.pads.clear();
                self.search.matches.clear();
                let fonts: BTreeSet<String> = doc.flow.styles.iter().map(|s| s.font.to_string()).collect();
                for f in fonts {
                    self.fonts.ensure(ctx, &f, false);
                }
                if let Some(st) = doc.flow.styles.first() {
                    self.typing = st.clone();
                }
                self.doc = doc;
                self.textures.clear();
                self.ensure_textures(ctx);
                self.doc.full_paginate(ctx, &self.typing);
                self.pos = 0.0;
                self.target = 0;
                self.set_caret(ctx, 0, false);
                self.fit = true;
                self.path = Some(path);
                self.status = "- opened".into();
            }
            Err(e) => self.status = format!("- open failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_text_styles_breaks_and_setup() {
        let bold = Style { bold: true, ..Style::new("Foo") };
        let plain = Style::new("Foo");
        let mut doc = Doc::new();
        doc.flow.text = format!("hi{PAGE_BREAK}yo\n");
        doc.flow.styles = vec![bold.clone(), bold.clone(), plain.clone(), plain.clone(), plain.clone(), plain.clone()];
        doc.setup.margin_left = 40.0;
        doc.setup.page_numbers = true;
        doc.notes.push(Note { id: 7, start: 1, end: 4, text: "check this".into(), color: 2 });
        let json = serde_json::to_string(&DocFile::from_doc(&doc)).unwrap();
        let back = serde_json::from_str::<DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.flow.text, doc.flow.text);
        assert_eq!(back.flow.styles, doc.flow.styles);
        assert_eq!(back.setup, doc.setup);
        assert_eq!(back.notes.len(), 1);
        assert_eq!((back.notes[0].start, back.notes[0].end, back.notes[0].color), (1, 4, 2));
        assert_eq!(back.notes[0].text, "check this");
    }

    #[test]
    fn version_1_files_still_load_as_separate_pages() {
        let json = r#"{"version":1,"pages":[[{"text":"one","font":"F","size":12.0,"bold":false,"underline":false}],
                                             [{"text":"two","font":"F","size":12.0,"bold":true,"underline":false}]]}"#;
        let doc = serde_json::from_str::<DocFile>(json).unwrap().into_doc();
        assert_eq!(doc.flow.text, format!("one{PAGE_BREAK}two\n"));
        assert_eq!(doc.flow.styles.len(), 8);
    }
}
