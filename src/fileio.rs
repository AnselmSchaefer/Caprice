//! The `.caprice` file format (JSON).
//!
//! Version 2 stores the document as one list of styled runs; page breaks are `\u{c}` characters
//! inside it. Version 1 files stored a list of pages, which are loaded as hard-separated pages.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

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
    /// The picture behind the pages and its description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<crate::backdrop::SceneFile>,
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
            scene: None,
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
        let removed = doc.take_out_contents_chars();
        for img in self.images {
            let Ok(bytes) = BASE64.decode(img.data.as_bytes()) else { continue };
            let Ok((format, px)) = crate::images::describe(&bytes) else { continue };
            doc.images.push(ImageData { id: img.id, format, bytes, px, width_pt: img.width_pt, rotation: img.rotation % 4 });
        }
        let total = doc.total_chars();
        for n in self.notes {
            let shift = |c: usize| c - removed.iter().filter(|&&r| r < c).count();
            let (start, end) = (shift(n.start).min(total), shift(n.end).min(total));
            let id = doc.next_note_id;
            doc.next_note_id += 1;
            doc.notes.push(Note { id, start: start.min(end), end, text: n.text, color: n.color });
        }
        doc
    }
}

/// What an open file dialog is choosing a path for.
pub enum DialogFor {
    Open,
    Save,
    ExportDocx,
    Picture,
    ScenePicture,
}

/// Something that would throw away unsaved changes, waiting for the user to say what to do.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Leaving {
    /// Opening another document.
    Open,
    /// Closing the window.
    Close,
}

/// A file dialog showing on its own thread.
pub struct PendingDialog {
    purpose: DialogFor,
    rx: Receiver<Option<PathBuf>>,
}

impl App {
    /// The document as saved, with the scene behind its pages.
    pub fn doc_file(&self) -> DocFile {
        DocFile { scene: self.backdrop.to_file(), ..DocFile::from_doc(&self.doc) }
    }

    /// A fingerprint of what saving would write, to tell whether anything changed since.
    pub fn content_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&self.doc_file()).unwrap_or_default().hash(&mut h);
        h.finish()
    }

    /// Does the document hold anything not yet saved?
    pub fn unsaved(&self) -> bool {
        self.content_hash() != self.saved_hash
    }

    pub fn save(&mut self, ctx: &egui::Context, save_as: bool) {
        match (&self.path, save_as) {
            (Some(p), false) => self.save_to(p.clone()),
            _ => self.ask_path(ctx, DialogFor::Save, || {
                rfd::FileDialog::new()
                    .add_filter("Caprice document", &["caprice"])
                    .set_file_name("Untitled.caprice")
                    .save_file()
            }),
        }
    }

    pub fn save_to(&mut self, path: PathBuf) {
        let json = serde_json::to_string_pretty(&self.doc_file());
        let written = json.map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string()));
        self.status = match written {
            Ok(()) => {
                self.path = Some(path);
                self.saved_hash = self.content_hash();
                "- saved".into()
            }
            Err(e) => format!("- save failed: {e}"),
        };
    }

    /// Write the document as a Word file (`.docx`) next to wherever the user chooses.
    pub fn export_docx(&mut self, ctx: &egui::Context) {
        let stem = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or("Untitled".to_owned(), |s| s.to_string_lossy().into_owned());
        self.ask_path(ctx, DialogFor::ExportDocx, move || {
            rfd::FileDialog::new()
                .add_filter("Word document", &["docx"])
                .set_file_name(format!("{stem}.docx"))
                .save_file()
        });
    }

    pub(crate) fn export_docx_to(&mut self, path: PathBuf) {
        let written = crate::export::to_docx(&self.doc).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
        self.status = match written {
            Ok(()) => format!("- exported {}", path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())),
            Err(e) => format!("- export failed: {e}"),
        };
    }

    /// Choose another document to open, first asking about unsaved changes.
    pub fn open(&mut self, ctx: &egui::Context) {
        if self.unsaved() {
            self.leaving = Some(Leaving::Open);
        } else {
            self.choose_file_to_open(ctx);
        }
    }

    fn choose_file_to_open(&mut self, ctx: &egui::Context) {
        self.ask_path(ctx, DialogFor::Open, || {
            rfd::FileDialog::new().add_filter("Caprice document", &["caprice"]).pick_file()
        });
    }

    /// Show a file dialog without holding up the frames: the window keeps drawing (and answering
    /// the compositor) while it is open, and the chosen path is acted on in `poll_dialog`.
    pub fn ask_path(&mut self, ctx: &egui::Context, purpose: DialogFor, show: impl FnOnce() -> Option<PathBuf> + Send + 'static) {
        if self.dialog.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(show());
            ctx.request_repaint();
        });
        self.dialog = Some(PendingDialog { purpose, rx });
    }

    /// Act on the path from a file dialog once the user has chosen it.
    pub fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.dialog else { return };
        let path = match pending.rx.try_recv() {
            Ok(path) => path,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        let Some(PendingDialog { purpose, .. }) = self.dialog.take() else { return };
        let Some(path) = path else {
            self.after_save = None;
            return;
        };
        match purpose {
            DialogFor::Open => self.open_path(ctx, path),
            DialogFor::Save => {
                self.save_to(path);
                if let Some(next) = self.after_save.take()
                    && !self.unsaved()
                {
                    self.leave(ctx, next);
                }
            }
            DialogFor::ExportDocx => self.export_docx_to(path),
            DialogFor::Picture => {
                self.status = match self.insert_image(ctx, &path) {
                    Ok(()) => "- picture added".into(),
                    Err(e) => format!("- picture failed: {e}"),
                };
            }
            DialogFor::ScenePicture => self.save_scene_picture_to(path),
        }
    }

    /// Go on with what was waiting on the unsaved-changes question.
    fn leave(&mut self, ctx: &egui::Context, leaving: Leaving) {
        match leaving {
            Leaving::Open => self.choose_file_to_open(ctx),
            Leaving::Close => {
                self.may_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Hold the window open while it has unsaved changes, and ask instead.
    pub fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.may_close && self.unsaved() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.leaving = Some(Leaving::Close);
        }
    }

    /// The "Save changes?" question, while something waits on it.
    pub fn unsaved_prompt(&mut self, ctx: &egui::Context) {
        let Some(leaving) = self.leaving else { return };
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or("Untitled".to_owned(), |n| n.to_string_lossy().into_owned());
        let (mut save, mut discard, mut cancel) = (false, false, false);
        let modal = egui::Modal::new(egui::Id::new("unsaved_prompt")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading(format!("Save changes to \u{201c}{name}\u{201d}?"));
            ui.add_space(4.0);
            let then = match leaving {
                Leaving::Open => "before opening another document",
                Leaving::Close => "before closing",
            };
            ui.label(egui::RichText::new(format!("Your changes will be lost if you don\u{2019}t save them {then}.")).color(crate::theme::TEXT_DIM));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                save = ui.button("Save").clicked();
                let discard_btn = ui.button("Don\u{2019}t save");
                let cancel_btn = ui.button("Cancel");
                (discard, cancel) = (discard_btn.clicked(), cancel_btn.clicked());
                // Enter saves, unless the keyboard is on one of the other buttons: then it is theirs.
                let other = discard_btn.has_focus() || cancel_btn.has_focus();
                save |= !other && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            });
        });
        cancel |= modal.should_close();
        if save {
            self.leaving = None;
            match self.path.clone() {
                Some(path) => {
                    self.save_to(path);
                    if !self.unsaved() {
                        self.leave(ctx, leaving);
                    }
                }
                None => {
                    self.after_save = Some(leaving);
                    self.save(ctx, true);
                }
            }
        } else if discard {
            self.leaving = None;
            self.leave(ctx, leaving);
        } else if cancel {
            self.leaving = None;
        }
    }

    pub fn open_path(&mut self, ctx: &egui::Context, path: std::path::PathBuf) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<DocFile>(&s).map_err(|e| e.to_string()));
        match loaded {
            Ok(mut file) => {
                let scene = file.scene.take();
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
                self.status = match self.open_scene(ctx, scene) {
                    Ok(()) => "- opened".into(),
                    Err(e) => format!("- opened, but its scene could not be shown: {e}"),
                };
                self.saved_hash = self.content_hash();
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
    fn chapter_titles_are_saved_and_older_files_have_none() {
        let plain = Style::new("Foo");
        let mut title = ParaAttrs::default();
        title.set_chapter_title(true);
        let mut doc = Doc::new();
        doc.flow.text = "One\ntext\n".into();
        doc.flow.styles = vec![plain.clone(); 9];
        doc.flow.styles[3] = plain.with_para(title);
        let json = serde_json::to_string(&DocFile::from_doc(&doc)).unwrap();
        assert_eq!(json.matches("ChapterTitle").count(), 1, "ordinary paragraphs say nothing about it: {json}");
        let back = serde_json::from_str::<DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.flow.styles, doc.flow.styles);

        let old = json.replace(r#","kind":"ChapterTitle""#, "");
        let back = serde_json::from_str::<DocFile>(&old).unwrap().into_doc();
        assert!(!back.para_attrs_at(0).is_chapter_title());
    }

    #[test]
    fn files_with_the_old_contents_char_open_with_the_setting_and_post_its_in_place() {
        let st = Style::new("Foo");
        let mut doc = Doc::new();
        doc.flow.text = "\u{e000}\u{c}One\ntext\n".into();
        doc.flow.styles = vec![st; 11];
        doc.notes.push(Note { id: 1, start: 2, end: 5, text: "the title".into(), color: 0 });
        let json = serde_json::to_string(&DocFile::from_doc(&doc)).unwrap();
        let back = serde_json::from_str::<DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.flow.text, "One\ntext\n");
        assert!(back.setup.contents);
        assert_eq!((back.notes[0].start, back.notes[0].end), (0, 3), "still on \"One\"");
        // Saved again, the setting stays, and the char does not come back.
        let json = serde_json::to_string(&DocFile::from_doc(&back)).unwrap();
        let again = serde_json::from_str::<DocFile>(&json).unwrap().into_doc();
        assert_eq!((again.flow.text.as_str(), again.setup.contents), ("One\ntext\n", true));
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
