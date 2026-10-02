//! The `.caprice` file format (JSON).
//!
//! Version 2 stores the document as one list of styled runs; page breaks are `\u{c}` characters
//! inside it. Version 1 files stored a list of pages, which are loaded as hard-separated pages.

use std::collections::BTreeSet;

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::App;
use crate::model::{Doc, Flow, PAGE_BREAK, PageSetup, Style};

#[derive(Serialize, Deserialize)]
struct Run {
    text: String,
    font: String,
    size: f32,
    bold: bool,
    underline: bool,
}

#[derive(Serialize, Deserialize)]
pub struct DocFile {
    version: u32,
    #[serde(default)]
    setup: PageSetup,
    #[serde(default)]
    content: Vec<Run>,
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
        .map(|(s, text)| Run { text, font: s.font.to_string(), size: s.size, bold: s.bold, underline: s.underline })
        .collect()
}

fn from_runs(runs: Vec<Run>, text: &mut String, styles: &mut Vec<Style>) {
    for r in runs {
        let st = Style { font: r.font.into(), size: r.size, bold: r.bold, underline: r.underline };
        styles.extend(std::iter::repeat_n(st, r.text.chars().count()));
        text.push_str(&r.text);
    }
}

impl DocFile {
    pub fn from_doc(doc: &Doc) -> Self {
        Self { version: 2, setup: doc.setup.clone(), content: to_runs(&doc.flow.text, &doc.flow.styles), pages: Vec::new() }
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
        doc.setup = self.setup;
        doc.setup.clamp_margins();
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

    pub fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("Caprice document", &["caprice"]).pick_file() else {
            return;
        };
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<DocFile>(&s).map_err(|e| e.to_string()));
        match loaded {
            Ok(file) => {
                let mut doc = file.into_doc();
                doc.version = self.doc.version + 1;
                let fonts: BTreeSet<String> = doc.flow.styles.iter().map(|s| s.font.to_string()).collect();
                for f in fonts {
                    self.fonts.ensure(ctx, &f, false);
                }
                if let Some(st) = doc.flow.styles.first() {
                    self.typing = st.clone();
                }
                self.doc = doc;
                self.doc.full_paginate(ctx, &self.typing);
                self.view_for = None;
                self.pos = 0.0;
                self.target = 0;
                self.last_cursor = None;
                self.cursor_req = Some((0, 0));
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
        doc.flow.text = format!("hi{PAGE_BREAK}yo");
        doc.flow.styles = vec![bold.clone(), bold.clone(), plain.clone(), plain.clone(), plain.clone()];
        doc.setup.margin_left = 40.0;
        doc.setup.page_numbers = true;
        let json = serde_json::to_string(&DocFile::from_doc(&doc)).unwrap();
        let back = serde_json::from_str::<DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.flow.text, doc.flow.text);
        assert_eq!(back.flow.styles, doc.flow.styles);
        assert_eq!(back.setup, doc.setup);
    }

    #[test]
    fn version_1_files_still_load_as_separate_pages() {
        let json = r#"{"version":1,"pages":[[{"text":"one","font":"F","size":12.0,"bold":false,"underline":false}],
                                             [{"text":"two","font":"F","size":12.0,"bold":true,"underline":false}]]}"#;
        let doc = serde_json::from_str::<DocFile>(json).unwrap().into_doc();
        assert_eq!(doc.flow.text, format!("one{PAGE_BREAK}two"));
        assert_eq!(doc.flow.styles.len(), 7);
    }
}
