//! Sticky notes: anchored to text without being part of it. They show as a highlight on the
//! text and a small tab beside the page; clicking the tab opens the note.

use eframe::egui::{self, Color32, Id, Pos2, Rect, Sense, Stroke, TextEdit, pos2, vec2};

use crate::App;
use crate::layout::Mark;
use crate::model::{NOTE_COLORS, Note};

pub const SEARCH_FIELD: &str = "search_field";
pub const NOTE_FIELD: &str = "note_text";

pub fn note_color(k: usize) -> Color32 {
    let (r, g, b) = NOTE_COLORS[k % NOTE_COLORS.len()];
    Color32::from_rgb(r, g, b)
}

impl App {
    /// Is the user typing in the search box or a note (so the page must not grab the keyboard)?
    pub fn ui_field_focused(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.has_focus(Id::new(SEARCH_FIELD)) || m.has_focus(Id::new(NOTE_FIELD)))
    }

    /// Highlights for page `i`, relative to the page text: notes first, then search results on top.
    pub fn page_marks(&self, i: usize) -> Vec<Mark> {
        let sp = self.doc.spans[i];
        let local = |s: usize, e: usize| s.saturating_sub(sp.start)..e.min(sp.end).saturating_sub(sp.start);
        let mut marks = Vec::new();
        for n in &self.doc.notes {
            if n.end > n.start && n.end > sp.start && n.start < sp.end {
                marks.push(Mark { range: local(n.start, n.end), color: note_color(n.color).gamma_multiply(0.38) });
            }
        }
        if self.search.open {
            for (k, &(s, e)) in self.search.matches.iter().enumerate() {
                if e > sp.start && s < sp.end {
                    let color = if k == self.search.current {
                        Color32::from_rgba_unmultiplied(255, 140, 0, 190)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 220, 0, 130)
                    };
                    marks.push(Mark { range: local(s, e), color });
                }
            }
        }
        marks
    }

    /// Attach a new note to the selection, or to the line the cursor is on.
    pub fn add_note(&mut self, ctx: &egui::Context) {
        let (mut start, mut end) = self.selection();
        if start == end {
            // No selection: take the line the caret is on.
            let page = self.doc.page_of(self.caret);
            let layout = self.doc.layout_page(ctx, page, 1.0, &[]);
            let first = self.doc.spans[page].start;
            if let Some(row) = layout.row_of(self.caret - first, self.prefer_next) {
                (start, end) = (first + row.start, first + row.end);
            }
        }
        let id = self.doc.next_note_id;
        self.doc.next_note_id += 1;
        self.doc.notes.push(Note { id, start, end, text: String::new(), color: 0 });
        self.open_note = Some(id);
        self.note_focus = true;
        let r = self.last_page_rect;
        self.note_pos = pos2((r.right() + 36.0).min(ctx.content_rect().right() - 290.0), r.top() + 70.0);
        ctx.request_repaint();
    }

    /// The little tabs beside the page, one per note starting on it.
    pub fn draw_notes(&mut self, ui: &mut egui::Ui, rect: Rect, i: usize) {
        let sp = self.doc.spans[i];
        let sc = self.scale_of(rect);
        let page_layout = self.page_layout(ui.ctx(), i, sc);
        let origin = rect.min + self.doc.setup.margin_origin() * sc;

        let tabs: Vec<(u64, usize, usize, String)> = self
            .doc
            .notes
            .iter()
            .filter(|n| n.start >= sp.start && n.start <= sp.end)
            .map(|n| (n.id, n.start - sp.start, n.color, n.text.clone()))
            .collect();
        let mut placed: Vec<f32> = Vec::new();
        for (id, local, color, text) in tabs {
            let y = origin.y + page_layout.caret_rect(local, true).center().y;
            let stacked = placed.iter().filter(|&&py| (py - y).abs() < 4.0).count();
            placed.push(y);
            let tab = Rect::from_center_size(pos2(rect.right() + 16.0 + stacked as f32 * 22.0, y), vec2(16.0, 16.0));
            let resp = ui.interact(tab, Id::new(("note_tab", id)), Sense::click());
            let c = note_color(color);
            let p = ui.painter();
            p.rect_filled(tab.translate(vec2(1.0, 1.5)), 3.0, Color32::from_black_alpha(60));
            p.rect_filled(tab, 3.0, if resp.hovered() { c.gamma_multiply(1.1) } else { c });
            for dy in [-3.0, 0.0, 3.0] {
                p.line_segment(
                    [pos2(tab.left() + 3.5, tab.center().y + dy), pos2(tab.right() - 3.5, tab.center().y + dy)],
                    Stroke::new(1.0, Color32::from_black_alpha(90)),
                );
            }
            let preview = if text.is_empty() { "Empty note".to_owned() } else { text.chars().take(160).collect() };
            if resp.on_hover_text(preview).clicked() {
                self.open_note = Some(id);
                self.note_focus = true;
                self.note_pos = pos2(tab.right() + 24.0, tab.top() - 10.0);
            }
        }
    }

    pub fn note_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.open_note else { return };
        let Some(idx) = self.doc.notes.iter().position(|n| n.id == id) else {
            self.open_note = None;
            return;
        };
        let focus = std::mem::take(&mut self.note_focus);
        let mut open = true;
        let mut delete = false;
        let pos: Pos2 = self.note_pos;
        egui::Window::new("Note")
            .id(Id::new(("note_win", id)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(260.0)
            .default_pos(pos)
            .show(ctx, |ui| {
                let note = &mut self.doc.notes[idx];
                let field = TextEdit::multiline(&mut note.text)
                    .id(Id::new(NOTE_FIELD))
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text("Write a note…");
                let resp = ui.add(field);
                if focus {
                    resp.request_focus();
                }
                ui.horizontal(|ui| {
                    for k in 0..NOTE_COLORS.len() {
                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        ui.painter().rect_filled(r, 6.0, note_color(k));
                        if k == note.color {
                            ui.painter().rect_stroke(r.expand(2.0), 7.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
                        }
                        if resp.clicked() {
                            note.color = k;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        delete = ui.button("Delete").clicked();
                    });
                });
            });
        if delete {
            self.doc.notes.remove(idx);
            self.open_note = None;
        } else if !open {
            self.open_note = None;
        }
    }
}
