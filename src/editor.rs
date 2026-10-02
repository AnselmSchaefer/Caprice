//! Editing the current page. The page's text is copied into `App::view` (what egui's `TextEdit`
//! edits); every change is written back into the document flow and the pages are recomputed.

use eframe::egui::{self, FontId, Id, Key, Modifiers, Rect, TextEdit, UiBuilder, Vec2};
use egui::text::{CCursor, CCursorRange};
use egui::widgets::text_edit::TextEditState;

use crate::App;
use crate::edit::{Edit, Piece};
use crate::layout::build_job;
use crate::model::{EditBuf, FONT_SIZE, PAGE_BREAK, PageSetup, Style};
use crate::theme::INK;

impl App {
    pub fn last(&self) -> usize {
        self.doc.pages() - 1
    }

    pub fn page_id(i: usize) -> Id {
        Id::new(("page", i))
    }

    /// (cursor, other end of the selection) in chars, relative to the page.
    pub fn cursor_state(ctx: &egui::Context, page: usize) -> Option<(usize, usize)> {
        let range = TextEditState::load(ctx, Self::page_id(page))?.cursor.char_range()?;
        Some((usize::from(range.primary.index), usize::from(range.secondary.index)))
    }

    pub fn cursor_of(ctx: &egui::Context, page: usize) -> Option<usize> {
        Self::cursor_state(ctx, page).map(|(p, _)| p)
    }

    /// Which page and which offset in it a flow position `g` belongs to. `prefer` wins when `g`
    /// sits exactly on a boundary, except right after a line break (the cursor is on the next page then).
    pub fn locate(&self, g: usize, prefer: usize) -> (usize, usize) {
        let sp = &self.doc.spans;
        let n = sp.len();
        let prefer = prefer.min(n - 1);
        let s = sp[prefer];
        if g >= s.start && g <= s.end {
            let on_soft_boundary = g == s.end && prefer + 1 < n && sp[prefer + 1].start == g;
            if on_soft_boundary && g > 0 && self.doc.char_at(g - 1) == Some('\n') {
                return (prefer + 1, 0);
            }
            return (prefer, g - s.start);
        }
        let p = sp.iter().position(|s| g <= s.end).unwrap_or(n - 1);
        (p, g.saturating_sub(sp[p].start).min(sp[p].chars()))
    }

    /// Recompute all pages and put the cursor back where it was in the text.
    pub fn repaginate_keeping_cursor(&mut self, ctx: &egui::Context) {
        let i = self.target.min(self.last());
        let g = self.doc.spans[i].start + Self::cursor_of(ctx, i).unwrap_or(0).min(self.doc.spans[i].chars());
        self.doc.full_paginate(ctx, &self.typing);
        let (p, local) = self.locate(g, i);
        self.view_for = None;
        self.target = p;
        self.cursor_req = Some((p, local));
        ctx.request_repaint();
    }

    /// Insert an empty page after page `i`.
    pub fn insert_page_after(&mut self, ctx: &egui::Context, i: usize) {
        let g = self.doc.spans[i].end;
        // One break is enough if a break (or the end) already follows; otherwise the following text
        // needs its own break too.
        let follows = self.doc.char_at(g);
        let breaks = if follows.is_none() || follows == Some(PAGE_BREAK) { 1 } else { 2 };
        let text: String = std::iter::repeat_n(PAGE_BREAK, breaks).collect();
        let now = ctx.input(|inp| inp.time);
        self.doc.apply(Edit::insert(g, &text, &self.typing), now);
        self.doc.full_paginate(ctx, &self.typing);
        self.view_for = None;
        self.target = (i + 1).min(self.last());
        self.cursor_req = Some((self.target, 0));
    }

    pub fn handle_boundary_keys(&mut self, ctx: &egui::Context, i: usize) {
        let Some((p, s)) = Self::cursor_state(ctx, i) else { return };
        if p != s {
            return;
        }
        let len = self.doc.spans[i].chars();
        let consume = |k: Key| ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, k));
        if p == 0 && i > 0 {
            if ctx.input(|inp| inp.key_pressed(Key::Backspace)) && consume(Key::Backspace) {
                // Delete whatever sits before this page: a page break, or the last char of the page above.
                let g = self.doc.spans[i].start;
                let b = self.doc.char_to_byte(g - 1);
                let ch = self.doc.flow.text[b..].chars().next().unwrap_or(PAGE_BREAK);
                let old = Piece { text: ch.to_string(), styles: vec![self.doc.flow.styles[g - 1].clone()] };
                let now = ctx.input(|inp| inp.time);
                self.doc.apply(Edit::Replace { at: g - 1, old, new: Piece::default() }, now);
                self.doc.full_paginate(ctx, &self.typing);
                let (pg, local) = self.locate(g - 1, i - 1);
                self.view_for = None;
                self.pos = pg as f32;
                self.target = pg;
                self.cursor_req = Some((pg, local));
            } else if consume(Key::ArrowLeft) {
                self.target = i - 1;
                self.cursor_req = Some((i - 1, self.doc.spans[i - 1].chars()));
            }
        } else if p == len && i < self.last() && consume(Key::ArrowRight) {
            self.target = i + 1;
            self.cursor_req = Some((i + 1, 0));
        }
    }

    fn load_view(&mut self, i: usize) {
        let text = self.doc.page_text(i).to_owned();
        let styles = self.doc.page_styles(i).to_vec();
        self.view = EditBuf::new(text, styles);
        self.view_for = Some((i, self.doc.version));
    }

    pub fn edit_page(&mut self, ui: &mut egui::Ui, page_rect: Rect, i: usize) {
        let ctx = ui.ctx().clone();
        if self.view_for != Some((i, self.doc.version)) {
            self.load_view(i);
        }
        let id = Self::page_id(i);
        let sc = self.scale_of(page_rect);
        let setup = &self.doc.setup;
        let content = Rect::from_min_size(page_rect.min + setup.margin_origin() * sc, setup.content_size() * sc);

        if let Some((p, idx)) = self.cursor_req {
            if p == i && !self.scrubbing {
                let mut state = TextEditState::load(&ctx, id).unwrap_or_default();
                let idx = idx.min(self.view.text.chars().count());
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
                state.store(&ctx, id);
                ctx.memory_mut(|m| m.request_focus(id));
                self.cursor_req = None;
            }
        } else if !ctx.memory(|m| m.has_focus(id)) && !self.scrubbing && !Self::ui_field_focused(&ctx) {
            ctx.memory_mut(|m| m.request_focus(id));
        }

        let typing = self.typing.clone();
        let wrap_w = content.width();
        let marks = self.page_marks(i);
        let EditBuf { text, rich } = &mut self.view;
        let rich = &*rich;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            let mut r = rich.borrow_mut();
            r.sync(buf.as_str(), &typing);
            let job = build_job(buf.as_str(), &r.styles, &typing, sc, wrap_w, &marks);
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let edit = TextEdit::multiline(text)
            .id(id)
            .font(FontId::proportional(FONT_SIZE * sc))
            .text_color(INK)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(content.width())
            .min_size(Vec2::new(content.width(), content.height()))
            .lock_focus(true)
            .layouter(&mut layouter);
        if self.cursor_req.is_none() && !self.held_input.is_empty() {
            // Replay what was typed while the page was turning, ahead of this frame's own input.
            let mut events = std::mem::take(&mut self.held_input);
            ctx.input_mut(|i| {
                events.extend(std::mem::take(&mut i.events));
                i.events = events;
            });
        }
        ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
            ui.add(edit);
        });

        // Page breaks are structure, not text: drop any that arrive through paste.
        self.view.text.retain(|c| c != PAGE_BREAK);
        let text_now = self.view.text.clone();
        self.view.rich.get_mut().sync(&text_now, &typing);

        if self.view.text != self.doc.page_text(i) {
            self.commit_view(&ctx, i);
        }

        // The toolbox follows the character before the cursor.
        if let Some(c) = Self::cursor_of(&ctx, i) {
            if self.last_cursor != Some((i, c)) {
                self.last_cursor = Some((i, c));
                if let Some(st) = self.view.rich.get_mut().styles.get(c.saturating_sub(1)) {
                    self.typing = st.clone();
                }
            }
        }
    }

    /// Write the edited page back into the flow (as one edit) and recompute the pages.
    fn commit_view(&mut self, ctx: &egui::Context, i: usize) {
        let sp = self.doc.spans[i];
        let new_chars: Vec<char> = self.view.text.chars().collect();
        let mut new_styles = self.view.rich.get_mut().styles.clone();
        new_styles.resize(new_chars.len(), self.typing.clone());
        let cursor = Self::cursor_of(ctx, i).unwrap_or(new_chars.len()).min(new_chars.len());

        // Only the part that actually differs becomes the edit.
        let old_chars: Vec<char> = self.doc.page_text(i).chars().collect();
        let max = old_chars.len().min(new_chars.len());
        let p = old_chars.iter().zip(&new_chars).take_while(|(a, b)| a == b).count();
        let s = old_chars[p..].iter().rev().zip(new_chars[p..].iter().rev()).take(max - p).take_while(|(a, b)| a == b).count();
        let (old_end, new_end) = (old_chars.len() - s, new_chars.len() - s);
        let edit = Edit::Replace {
            at: sp.start + p,
            old: Piece {
                text: old_chars[p..old_end].iter().collect(),
                styles: self.doc.page_styles(i)[p..old_end].to_vec(),
            },
            new: Piece { text: new_chars[p..new_end].iter().collect(), styles: new_styles[p..new_end].to_vec() },
        };
        let (dchars, dbytes) = edit.delta();
        let now = ctx.input(|inp| inp.time);
        self.doc.apply(edit, now);
        self.doc.paginate_from(ctx, &self.typing, i, dchars, dbytes);
        self.view_for = None;

        // Text typed past the end of the page flows on to the next one, and the cursor with it.
        let (pg, local) = self.locate(sp.start + cursor, i);
        if pg != i {
            self.cursor_req = Some((pg, local));
            self.target = pg;
        }
        ctx.request_repaint();
    }

    /// Undo or redo one step and put the caret where the change happened.
    pub fn step_history(&mut self, ctx: &egui::Context, redo: bool) {
        let caret = if redo { self.doc.redo() } else { self.doc.undo() };
        let Some(caret) = caret else { return };
        self.doc.full_paginate(ctx, &self.typing);
        let (p, local) = self.locate(caret.min(self.doc.total_chars()), self.target.min(self.last()));
        self.view_for = None;
        self.target = p;
        self.cursor_req = Some((p, local));
        ctx.request_repaint();
    }

    /// Set a style property on the selection (if any) and on what gets typed next.
    pub fn apply_style(&mut self, ctx: &egui::Context, edit: impl Fn(&mut Style)) {
        edit(&mut self.typing);
        let i = self.target.min(self.last());
        let Some((a, b)) = Self::cursor_state(ctx, i) else { return };
        let (a, b) = (a.min(b), a.max(b));
        if a == b {
            return;
        }
        let at = self.doc.spans[i].start + a;
        let old = self.doc.flow.styles[at..at + (b - a)].to_vec();
        let mut new = old.clone();
        new.iter_mut().for_each(&edit);
        if new == old {
            return;
        }
        let now = ctx.input(|inp| inp.time);
        self.doc.apply(Edit::Restyle { at, old, new }, now);
        self.doc.paginate_from(ctx, &self.typing, i, 0, 0);
        self.view_for = None;
        ctx.request_repaint();
    }

    /// Swap in new page settings and reflow the whole document.
    pub fn set_setup(&mut self, ctx: &egui::Context, mut setup: PageSetup) {
        setup.clamp_margins();
        self.doc.setup = setup;
        self.doc.version += 1;
        self.repaginate_keeping_cursor(ctx);
    }
}
