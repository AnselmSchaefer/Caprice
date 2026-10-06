//! The text editor: caret, selection, typing, deleting, clipboard and keyboard/mouse handling.
//! It edits the document flow directly (through `Doc::apply`); the page is only a view of it.

use eframe::egui::{self, Event, Id, Key, Modifiers, Rect, Sense, Stroke, Vec2, pos2, vec2};
use egui::output::IMEOutput;

use crate::App;
use crate::edit::{Edit, Piece};
use crate::images::{Handle, PicDrag, ResizeDrag};
use crate::layout::{PageLayout, RowRef};
use crate::model::{Align, CONTENTS_CHAR, IMAGE_CHAR, ImageData, ListKind, PAGE_BREAK, PageSetup, ParaAttrs, Style, is_terminator};
use crate::theme::INK;

/// Roughly how a word processor groups characters when jumping by word.
fn char_class(c: char) -> u8 {
    if c.is_whitespace() && !is_terminator(c) {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else if is_terminator(c) {
        3
    } else {
        2
    }
}

/// What was last copied, with its formatting. The system clipboard only holds plain text, so
/// pasting that same text back in brings the formatting (and any pictures) along.
pub struct Clip {
    pub plain: String,
    pub piece: Piece,
    pub images: Vec<ImageData>,
}

impl App {
    pub fn last(&self) -> usize {
        self.doc.pages() - 1
    }

    /// The caret may sit anywhere in the text except after the final paragraph mark.
    fn max_caret(&self) -> usize {
        self.doc.total_chars() - 1
    }

    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        self.doc.flow.text[ba..bb].replace(PAGE_BREAK, "\n")
    }

    /// Flip to page `to`, the caret going to its start. On a page of the contents other than their
    /// first, that start is on another page, so the page is set rather than worked out from it.
    pub fn turn_to(&mut self, ctx: &egui::Context, to: usize, extend: bool) {
        self.set_caret(ctx, self.doc.spans[to].start, extend);
        self.target = to;
    }

    /// Move the caret (and the selection end, if `extend`), and flip to the page it is on.
    pub fn set_caret(&mut self, ctx: &egui::Context, c: usize, extend: bool) {
        let c = c.min(self.max_caret());
        let moved = c != self.caret;
        self.caret = c;
        if !extend {
            self.anchor = c;
        }
        self.prefer_next = true;
        self.target = self.doc.page_of(c);
        self.blink_epoch = ctx.input(|i| i.time);
        if moved && !extend {
            self.typing_follows_caret();
        }
        ctx.request_repaint();
    }

    /// Typing continues in the style of the character before the caret (or after it, at a paragraph start).
    pub fn typing_follows_caret(&mut self) {
        let c = self.caret;
        let before_is_char = c > 0 && !self.doc.char_at(c - 1).is_some_and(is_terminator);
        let k = if before_is_char { c - 1 } else { c };
        if let Some(st) = self.doc.flow.styles.get(k) {
            self.typing = st.with_para(ParaAttrs::default());
            self.typing.image = 0; // typing after a picture is text, not another picture
        }
    }

    // ------------------------------------------------------------------ editing

    /// Replace the chars `a..b` by `text`, put the caret after it, keep the pages up to date.
    pub fn replace_range(&mut self, ctx: &egui::Context, a: usize, b: usize, text: &str) {
        // Page breaks are structure, not text: typed or pasted ones are dropped.
        let text: String = text.chars().filter(|&c| c != '\r' && c != PAGE_BREAK).collect();
        self.replace_raw(ctx, a, b, &text);
    }

    fn replace_raw(&mut self, ctx: &egui::Context, a: usize, b: usize, text: &str) {
        let max = self.max_caret();
        let (mut a, mut b) = (a.min(max), b.min(max));
        let mut text: String = text.chars().filter(|&c| c != CONTENTS_CHAR).collect();
        // The contents page holds the contents and nothing else: what is typed on it goes to the
        // start of the story, and its page break goes only with the contents themselves.
        if self.doc.has_contents_page() && a < 2 {
            if a == 0 && b >= 1 {
                b = b.max(2);
            } else if b > 2 {
                a = 2;
            } else if text.is_empty() {
                return;
            } else {
                (a, b) = (2, 2);
            }
        }
        // The contents keep a paragraph of their own: text typed next to them goes on a line of its own.
        if !text.is_empty() {
            if a > 0 && self.doc.char_at(a - 1) == Some(CONTENTS_CHAR) && !text.starts_with(is_terminator) {
                text.insert(0, '\n');
            }
            if self.doc.char_at(b) == Some(CONTENTS_CHAR) && !text.ends_with(is_terminator) {
                text.push('\n');
            }
        }
        if a == b && text.is_empty() {
            return;
        }
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        let old = Piece { text: self.doc.flow.text[ba..bb].to_owned(), styles: self.doc.flow.styles[a..b].to_vec() };
        let line_format = self.doc.para_attrs_at(a);
        let styles = text
            .chars()
            .map(|c| if is_terminator(c) { self.typing.with_para(line_format) } else { self.typing.with_para(ParaAttrs::default()) })
            .collect();
        let edit = Edit::Replace { at: a, old, new: Piece { text, styles } };
        self.apply_edit(ctx, edit, b);
    }

    /// Apply an edit made around the caret and park the caret after it.
    pub fn apply_edit(&mut self, ctx: &egui::Context, edit: Edit, old_end: usize) {
        let (dchars, dbytes) = edit.delta();
        let at = match &edit {
            Edit::Replace { at, .. } | Edit::Restyle { at, .. } => *at,
        };
        let caret = edit.caret_after();
        let now = ctx.input(|i| i.time);
        self.doc.apply(edit, now);
        self.doc.paginate_after(ctx, &self.typing, at, old_end, dchars, dbytes);
        self.set_caret(ctx, caret, false);
    }

    pub fn insert_text(&mut self, ctx: &egui::Context, text: &str) {
        let (a, b) = self.selection();
        self.replace_range(ctx, a, b, text);
    }

    /// Put the selection on the clipboard, and keep its formatting for pasting it back in.
    fn copy_selection(&mut self, ctx: &egui::Context) {
        let (a, b) = self.selection();
        let plain = self.selected_text();
        let styles = self.doc.flow.styles[a..b].to_vec();
        let images = styles.iter().filter(|s| s.image != 0).filter_map(|s| self.doc.image(s.image).cloned()).collect();
        ctx.copy_text(plain.clone());
        self.clip = Some(Clip { piece: Piece { text: plain.clone(), styles }, plain, images });
    }

    /// Paste formatted if the clipboard still holds what was copied here, else as plain text.
    fn paste(&mut self, ctx: &egui::Context, t: &str) {
        let plain: String = t.chars().filter(|&c| c != '\r').collect();
        match self.clip.take() {
            Some(clip) if clip.plain == plain => {
                self.paste_clip(ctx, &clip);
                self.clip = Some(clip);
            }
            clip => {
                self.clip = clip;
                self.insert_text(ctx, t);
            }
        }
    }

    fn paste_clip(&mut self, ctx: &egui::Context, clip: &Clip) {
        // Pasted pictures are copies with ids of their own, so each can be resized or turned alone.
        let mut ids = std::collections::HashMap::new();
        let mut next = self.doc.images.iter().map(|i| i.id).max().unwrap_or(0) + 1;
        for img in &clip.images {
            if let std::collections::hash_map::Entry::Vacant(e) = ids.entry(img.id) {
                e.insert(next);
                self.doc.images.push(ImageData { id: next, ..img.clone() });
                next += 1;
            }
        }
        if !ids.is_empty() {
            self.ensure_textures(ctx);
        }

        let max = self.max_caret();
        let (a, b) = self.selection();
        let (a, b) = (a.min(max), b.min(max));
        let line = self.doc.para_attrs_at(a);
        let mark = self.typing.with_para(ParaAttrs { list: ListKind::None, ..line });
        let after = self.doc.char_at(b);
        let mut prev = if a == 0 { None } else { self.doc.char_at(a - 1) };
        let (mut text, mut styles) = (String::new(), Vec::new());
        let chars: Vec<char> = clip.piece.text.chars().collect();
        for (i, (&c, st)) in chars.iter().zip(&clip.piece.styles).enumerate() {
            let mut st = st.clone();
            let picture = c == IMAGE_CHAR && st.image != 0;
            if picture {
                // A picture needs a paragraph of its own.
                if prev.is_some_and(|p| !is_terminator(p)) {
                    text.push('\n');
                    styles.push(self.typing.with_para(line));
                }
                st.image = ids.get(&st.image).copied().unwrap_or(0);
            }
            text.push(c);
            styles.push(st);
            prev = Some(c);
            if picture && chars.get(i + 1).copied().or(after).is_some_and(|n| !is_terminator(n)) {
                text.push('\n');
                styles.push(mark.clone());
                prev = Some('\n');
            }
        }

        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        let old = Piece { text: self.doc.flow.text[ba..bb].to_owned(), styles: self.doc.flow.styles[a..b].to_vec() };
        self.apply_edit(ctx, Edit::Replace { at: a, old, new: Piece { text, styles } }, b);
    }

    pub fn press_enter(&mut self, ctx: &egui::Context) {
        let attrs = self.doc.para_attrs_at(self.caret);
        let (ps, _) = self.doc.para_start(self.caret);
        let empty_item = attrs.list != ListKind::None
            && !self.has_selection()
            && ps == self.caret
            && self.doc.char_at(self.caret).is_some_and(is_terminator);
        let end_of_title = attrs.is_chapter_title()
            && !self.has_selection()
            && self.doc.char_at(self.caret).is_some_and(is_terminator);
        if empty_item {
            // Enter on an empty list item ends the list.
            self.set_para(ctx, |p| p.list = ListKind::None);
        } else if end_of_title {
            // The story goes on below a chapter title in ordinary text.
            let at = self.caret;
            let mark = self.doc.flow.styles[at].clone();
            let new_line = Edit::insert(at, "\n", &self.typing.with_para(attrs));
            let body = Edit::Restyle { at: at + 1, old: vec![mark.clone()], new: vec![mark.with_para(ParaAttrs::default())] };
            self.doc.apply_group(vec![new_line, body]);
            self.doc.paginate_after(ctx, &self.typing, at, at + 1, 1, 1);
            self.set_caret(ctx, at + 1, false);
        } else {
            self.insert_text(ctx, "\n");
        }
    }

    /// The selection to delete: a selected picture goes together with its paragraph mark.
    fn deletion_range(&self) -> (usize, usize) {
        self.picture_paragraph().unwrap_or_else(|| self.selection())
    }

    pub fn backspace(&mut self, ctx: &egui::Context, by_word: bool) {
        if self.has_selection() {
            let (a, b) = self.deletion_range();
            return self.replace_range(ctx, a, b, "");
        }
        let (ps, _) = self.doc.para_start(self.caret);
        if ps == self.caret && self.doc.para_attrs_at(self.caret).list != ListKind::None {
            return self.set_para(ctx, |p| p.list = ListKind::None); // first remove the bullet
        }
        if self.caret == 0 {
            return;
        }
        let from = if by_word { self.word_start_before(self.caret) } else { self.caret - 1 };
        self.replace_range(ctx, from, self.caret, "");
    }

    pub fn delete_forward(&mut self, ctx: &egui::Context, by_word: bool) {
        if self.has_selection() {
            let (a, b) = self.deletion_range();
            return self.replace_range(ctx, a, b, "");
        }
        if self.caret >= self.max_caret() {
            return;
        }
        let to = if by_word { self.word_end_after(self.caret) } else { self.caret + 1 };
        self.replace_range(ctx, self.caret, to, "");
    }

    /// Insert a hard page break at the caret.
    pub fn insert_page_break(&mut self, ctx: &egui::Context) {
        let c = self.caret;
        let (ps, _) = self.doc.para_start(c);
        let at_end = self.doc.char_at(c).is_some_and(is_terminator);
        // A break is a paragraph of its own: at a paragraph start it goes right here, at the end of a
        // paragraph just after it, and in the middle the paragraph is split first. At the very end of
        // the document the break ends the last paragraph instead, and the (empty) final one moves to the new page.
        let (at, text) = if ps == c {
            (c, PAGE_BREAK.to_string())
        } else if at_end {
            (if c < self.max_caret() { c + 1 } else { c }, PAGE_BREAK.to_string())
        } else {
            (c, format!("\n{PAGE_BREAK}"))
        };
        self.replace_raw(ctx, at, at, &text);
    }

    /// Put a contents page first, unless there is one; the story goes on on the next page.
    pub fn insert_contents(&mut self, ctx: &egui::Context) {
        if self.doc.has_contents_page() {
            return;
        }
        let ps = 0;
        let text = crate::contents::contents_text();
        let styles = text.chars().map(|_| self.typing.with_para(ParaAttrs::default())).collect();
        let old = Piece::default();
        self.apply_edit(ctx, Edit::Replace { at: ps, old, new: Piece { text, styles } }, ps);
    }

    /// Take the contents page out, if there is one.
    pub fn remove_contents(&mut self, ctx: &egui::Context) {
        if self.doc.has_contents_page() {
            self.replace_range(ctx, 0, 1, "");
        }
    }

    /// Insert an empty page after page `i`.
    pub fn insert_page_after(&mut self, ctx: &egui::Context, i: usize) {
        let end = self.doc.spans[i].end;
        let hard = self.doc.hard_end(i);
        let at_doc_end = end >= self.max_caret();
        // Between pages `i` and `i+1` we need two breaks, unless a break (or the end) is already there.
        let text: String = if hard || at_doc_end { PAGE_BREAK.to_string() } else { format!("{PAGE_BREAK}{PAGE_BREAK}") };
        let at = end.min(self.max_caret());
        let old_end = at;
        self.apply_insert_at(ctx, at, &text, old_end);
        // The caret goes to the empty page.
        let new_page = (i + 1).min(self.last());
        let start = self.doc.spans[new_page].start;
        self.set_caret(ctx, start, false);
    }

    fn apply_insert_at(&mut self, ctx: &egui::Context, at: usize, text: &str, old_end: usize) {
        let styles = text.chars().map(|_| self.typing.with_para(ParaAttrs::default())).collect();
        let edit = Edit::Replace { at, old: Piece::default(), new: Piece { text: text.to_owned(), styles } };
        self.apply_edit(ctx, edit, old_end);
    }

    // ------------------------------------------------------------------ movement

    fn word_start_before(&self, c: usize) -> usize {
        let b = self.doc.char_to_byte(c);
        let mut chars = self.doc.flow.text[..b].chars().rev().peekable();
        let mut n = 0;
        while chars.peek().is_some_and(|&ch| char_class(ch) == 0) {
            chars.next();
            n += 1;
        }
        match chars.next() {
            None => return c - n,
            Some(first) => {
                n += 1;
                let class = char_class(first);
                if class != 3 {
                    while chars.peek().is_some_and(|&ch| char_class(ch) == class) {
                        chars.next();
                        n += 1;
                    }
                }
            }
        }
        c - n
    }

    fn word_end_after(&self, c: usize) -> usize {
        let b = self.doc.char_to_byte(c);
        let mut chars = self.doc.flow.text[b..].chars().peekable();
        let mut n = 0;
        if let Some(&first) = chars.peek() {
            let class = char_class(first);
            chars.next();
            n += 1;
            if class != 3 {
                while chars.peek().is_some_and(|&ch| char_class(ch) == class) {
                    chars.next();
                    n += 1;
                }
            }
        }
        while chars.peek().is_some_and(|&ch| char_class(ch) == 0) {
            chars.next();
            n += 1;
        }
        (c + n).min(self.max_caret())
    }

    pub fn word_range_at(&self, c: usize) -> (usize, usize) {
        let c = c.min(self.max_caret());
        let b = self.doc.char_to_byte(c);
        let class_at = |ch: Option<char>| ch.map_or(3, char_class);
        let here = class_at(self.doc.flow.text[b..].chars().next());
        if here == 3 {
            return (c, c);
        }
        let back = self.doc.flow.text[..b].chars().rev().take_while(|&ch| char_class(ch) == here).count();
        let fwd = self.doc.flow.text[b..].chars().take_while(|&ch| char_class(ch) == here).count();
        (c - back, c + fwd)
    }

    /// Horizontal move by one char or word; collapses a selection to the side being moved towards.
    fn move_horizontal(&mut self, ctx: &egui::Context, forward: bool, word: bool, extend: bool) {
        self.want_x = None;
        if self.has_selection() && !extend {
            let (a, b) = self.selection();
            return self.set_caret(ctx, if forward { b } else { a }, false);
        }
        let to = match (forward, word) {
            (true, false) => (self.caret + 1).min(self.max_caret()),
            (false, false) => self.caret.saturating_sub(1),
            (true, true) => self.word_end_after(self.caret),
            (false, true) => self.word_start_before(self.caret),
        };
        self.set_caret(ctx, to, extend);
    }

    fn layout_at_scale_one(&self, ctx: &egui::Context, page: usize) -> PageLayout {
        self.doc.layout_page(ctx, page, 1.0, &[])
    }

    /// The flow position one visual row above/below the caret, keeping the horizontal position.
    fn vertical_target(&mut self, ctx: &egui::Context, up: bool) -> Option<usize> {
        let page = self.doc.page_of(self.caret);
        let layout = self.layout_at_scale_one(ctx, page);
        let start = self.doc.spans[page].start;
        let local = self.caret - start;
        let here = layout.row_of(local, self.prefer_next)?;
        let x = match self.want_x {
            Some(x) => x,
            None => layout.caret_rect(local, self.prefer_next).left(),
        };
        self.want_x = Some(x);
        let rows = layout.rows();
        let idx = rows.iter().position(|r| r.para == here.para && r.start == here.start)?;
        // Rows side by side (a drop cap and the line beside it) are one line to move over.
        let level = |a: &RowRef, b: &RowRef| (a.top - b.top).abs() < 0.5;
        let next = if up {
            rows[..idx].iter().rposition(|r| r.top < here.top && !level(r, &here))
        } else {
            rows[idx + 1..].iter().position(|r| r.top > here.top && !level(r, &here)).map(|k| idx + 1 + k)
        };
        let (target_layout, target_start, rows, row) = if let Some(k) = next {
            let row = rows[k];
            (layout, start, rows, row)
        } else if up && page > 0 {
            let l = self.layout_at_scale_one(ctx, page - 1);
            let rows = l.rows();
            let r = *rows.last()?;
            (l, self.doc.spans[page - 1].start, rows, r)
        } else if !up && page < self.last() {
            let l = self.layout_at_scale_one(ctx, page + 1);
            let rows = l.rows();
            let r = *rows.first()?;
            (l, self.doc.spans[page + 1].start, rows, r)
        } else {
            return None;
        };
        let line: Vec<&RowRef> = rows.iter().filter(|r| level(r, &row)).collect();
        let mid = row.top + line.iter().map(|r| r.height).fold(row.height, f32::min) / 2.0;
        let hit = target_layout.hit(vec2(x, mid));
        let row = line.into_iter().find(|r| (r.start..=r.end).contains(&hit)).copied().unwrap_or(row);
        let p = &target_layout.paras[row.para];
        let mut local = hit.clamp(row.start, row.end);
        let last_row_of_piece = p.end == row.end;
        if local == row.end && !last_row_of_piece {
            local = local.saturating_sub(1).max(row.start);
        }
        Some(target_start + local)
    }

    fn move_vertical(&mut self, ctx: &egui::Context, up: bool, extend: bool) {
        let want = self.want_x;
        match self.vertical_target(ctx, up) {
            Some(to) => {
                self.set_caret(ctx, to, extend);
                self.want_x = want.or(self.want_x);
            }
            None => {
                let edge = if up { 0 } else { self.max_caret() };
                self.set_caret(ctx, edge, extend);
                self.want_x = None;
            }
        }
    }

    fn home_end(&mut self, ctx: &egui::Context, end: bool, whole_doc: bool, extend: bool) {
        self.want_x = None;
        if whole_doc {
            return self.set_caret(ctx, if end { self.max_caret() } else { 0 }, extend);
        }
        let page = self.doc.page_of(self.caret);
        let layout = self.layout_at_scale_one(ctx, page);
        let start = self.doc.spans[page].start;
        if let Some(row) = layout.row_of(self.caret - start, self.prefer_next) {
            self.set_caret(ctx, start + if end { row.end } else { row.start }, extend);
            self.prefer_next = !end;
        }
    }

    pub fn select_all(&mut self, ctx: &egui::Context) {
        self.anchor = 0;
        self.set_caret(ctx, self.max_caret(), true);
    }

    // ------------------------------------------------------------------ input

    /// Handle this frame's typing, keys and clipboard events.
    pub fn process_input(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return; // the search box, a note or a number field has the keyboard
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                Event::Text(t) => self.insert_text(ctx, &t),
                Event::Ime(egui::ImeEvent::Commit(t)) => self.insert_text(ctx, &t),
                Event::Paste(t) => self.paste(ctx, &t),
                Event::Copy => {
                    if self.has_selection() {
                        self.copy_selection(ctx);
                    }
                }
                Event::Cut => {
                    if self.has_selection() {
                        self.copy_selection(ctx);
                        let (a, b) = self.selection();
                        self.replace_range(ctx, a, b, "");
                    }
                }
                Event::Key { key, pressed: true, modifiers, .. } => self.handle_key(ctx, key, modifiers),
                _ => {}
            }
        }
    }

    /// Keys are handled in the order they arrived, together with typing, so that e.g. pressing a
    /// formatting shortcut right after Enter formats the new paragraph.
    fn handle_key(&mut self, ctx: &egui::Context, key: Key, m: Modifiers) {
        let (word, shift) = (m.command || m.ctrl, m.shift);
        if m.alt && !m.command && matches!(key, Key::ArrowUp | Key::ArrowDown) && self.picture_paragraph().is_some() {
            return self.nudge_picture(ctx, key == Key::ArrowUp);
        }
        if m.command {
            match key {
                Key::Z => return self.step_history(ctx, shift),
                Key::Y => return self.step_history(ctx, true),
                Key::Enter => return self.insert_page_break(ctx),
                Key::N if m.alt => return self.add_note(ctx),
                Key::P if shift => return self.toggle_pen(),
                Key::L if shift => {
                    let on = self.doc.para_attrs_at(self.caret).list == ListKind::Bullet;
                    return self.set_para(ctx, |p| p.list = if on { ListKind::None } else { ListKind::Bullet });
                }
                Key::L => return self.set_para(ctx, |p| p.align = Align::Left),
                Key::E => return self.set_para(ctx, |p| p.align = Align::Center),
                Key::R => return self.set_para(ctx, |p| p.align = Align::Right),
                Key::J => return self.set_para(ctx, |p| p.align = Align::Justify),
                Key::B => {
                    let on = !self.typing.bold;
                    return self.apply_style(ctx, |s| s.bold = on);
                }
                Key::U => {
                    let on = !self.typing.underline;
                    return self.apply_style(ctx, |s| s.underline = on);
                }
                _ => {}
            }
        }
        match key {
            Key::Escape if self.pen && self.lasso.is_none() => self.toggle_pen(),
            Key::PageDown | Key::PageUp => {
                // Jump to the start of the next / previous page.
                let page = self.target;
                let to = if key == Key::PageDown { (page + 1).min(self.last()) } else { page.saturating_sub(1) };
                self.turn_to(ctx, to, shift);
            }
            Key::ArrowLeft => self.move_horizontal(ctx, false, word, shift),
            Key::ArrowRight => self.move_horizontal(ctx, true, word, shift),
            Key::ArrowUp => self.move_vertical(ctx, true, shift),
            Key::ArrowDown => self.move_vertical(ctx, false, shift),
            Key::Home => self.home_end(ctx, false, word, shift),
            Key::End => self.home_end(ctx, true, word, shift),
            Key::Backspace => self.backspace(ctx, word),
            Key::Delete => self.delete_forward(ctx, word),
            Key::Enter if !m.command => self.press_enter(ctx),
            Key::Tab if !m.command => self.insert_text(ctx, "\t"),
            Key::A if m.command => self.select_all(ctx),
            _ => {}
        }
    }

    // ------------------------------------------------------------------ formatting

    /// Set a character style property on the selection (if any) and on what is typed next.
    pub fn apply_style(&mut self, ctx: &egui::Context, edit: impl Fn(&mut Style)) {
        edit(&mut self.typing);
        let (a, b) = self.selection();
        if a == b {
            return;
        }
        let old = self.doc.flow.styles[a..b].to_vec();
        let mut new = old.clone();
        for st in &mut new {
            let para = st.para;
            edit(st);
            st.para = para; // character edits never touch paragraph formatting
        }
        if new == old {
            return;
        }
        let now = ctx.input(|i| i.time);
        self.doc.apply(Edit::Restyle { at: a, old, new }, now);
        self.doc.paginate_after(ctx, &self.typing, a, b, 0, 0);
        ctx.request_repaint();
    }

    /// Change the format of every paragraph the selection touches.
    pub fn set_para(&mut self, ctx: &egui::Context, edit: impl Fn(&mut ParaAttrs)) {
        let (a, b) = self.selection();
        let (ps, _) = self.doc.para_start(a);
        let last_char = b.min(self.max_caret());
        let (end, _) = self.doc.term_from(last_char, self.doc.char_to_byte(last_char));
        let range = ps..end + 1;
        let old = self.doc.flow.styles[range.clone()].to_vec();
        let mut new = old.clone();
        for (k, st) in new.iter_mut().enumerate() {
            if self.doc.char_at(ps + k).is_some_and(is_terminator) {
                edit(&mut st.para);
            }
        }
        if new == old {
            return;
        }
        let now = ctx.input(|i| i.time);
        let len = range.len();
        self.doc.apply(Edit::Restyle { at: ps, old, new }, now);
        self.doc.paginate_after(ctx, &self.typing, ps, ps + len, 0, 0);
        ctx.request_repaint();
    }

    // ------------------------------------------------------------------ history, setup

    /// Undo or redo one step and put the caret where the change happened.
    pub fn step_history(&mut self, ctx: &egui::Context, redo: bool) {
        let caret = if redo { self.doc.redo() } else { self.doc.undo() };
        let Some(caret) = caret else { return };
        self.doc.full_paginate(ctx, &self.typing);
        self.set_caret(ctx, caret.min(self.max_caret()), false);
    }

    /// Swap in new page settings and reflow the whole document.
    pub fn set_setup(&mut self, ctx: &egui::Context, mut setup: PageSetup) {
        setup.clamp_margins();
        self.doc.setup = setup;
        self.doc.version += 1;
        self.doc.full_paginate(ctx, &self.typing);
        let c = self.caret;
        self.set_caret(ctx, c, false);
    }

    // ------------------------------------------------------------------ the page surface

    /// Draw the page that is being edited, and handle clicking and dragging on it.
    pub fn editor_surface(&mut self, ui: &mut egui::Ui, page_rect: Rect, i: usize, interactive: bool) {
        let ctx = ui.ctx().clone();
        let sc = self.scale_of(page_rect);
        let setup = &self.doc.setup;
        let content = Rect::from_min_size(page_rect.min + setup.margin_origin() * sc, setup.content_size() * sc);
        let layout = self.page_layout(&ctx, i, sc);
        let start = self.doc.spans[i].start;
        let page_len = self.doc.spans[i].chars() + usize::from(self.doc.hard_end(i));

        // Selection, then text, then caret.
        let (a, b) = self.selection();
        if a != b && b > start && a < start + page_len + 1 {
            let (la, lb) = (a.saturating_sub(start), (b - start).min(page_len + 1));
            for r in layout.selection_rects(la, lb) {
                let color = crate::theme::ACCENT.gamma_multiply(0.45);
                ui.painter().rect_filled(r.translate(content.min.to_vec2()), 1.0, color);
            }
        }
        self.paint_layout(ui.painter(), &layout, content.min);

        if a != b {
            // Pictures are drawn over the highlight, so tint them afterwards.
            let (la, lb) = (a.saturating_sub(start), (b.saturating_sub(start)).min(page_len + 1));
            for p in layout.paras.iter().filter(|p| p.image.is_some() && la < p.end && lb > p.start) {
                if let Some(img) = p.image {
                    let r = img.rect.translate(vec2(0.0, p.y) + content.min.to_vec2());
                    ui.painter().rect_filled(r, 1.0, crate::theme::ACCENT.gamma_multiply(0.35));
                }
            }
        }

        let caret_here = self.doc.page_of(self.caret) == i;
        if caret_here {
            let local = self.caret - start;
            let r = layout.caret_rect(local, self.prefer_next).translate(content.min.to_vec2());
            let now = ctx.input(|inp| inp.time);
            let phase = (now - self.blink_epoch) % 1.06;
            if phase < 0.53 && !ctx.egui_wants_keyboard_input() {
                let x = r.left();
                ui.painter().line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new((1.4 * sc).max(1.0), INK));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(if phase < 0.53 { (530.0 - phase * 1000.0) as u64 } else { (1060.0 - phase * 1000.0) as u64 } + 20));
            if !ctx.egui_wants_keyboard_input() {
                ui.output_mut(|o| {
                    o.ime = Some(IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: content,
                        cursor_rect: r,
                        should_interrupt_composition: false,
                    })
                });
            }
        }

        // Where a dragged picture would land.
        if let Some((_, y)) = self.pic_drag.as_ref().and_then(|d| d.drop) {
            let y = content.min.y + y;
            ui.painter().line_segment([pos2(content.left(), y), pos2(content.right(), y)], Stroke::new(2.5, crate::theme::ACCENT));
        }

        // Resize handles around a selected picture.
        let picture = self.selected_picture_box(&layout, content, start);
        if let Some((_, rect)) = picture {
            for h in Handle::ALL {
                let c = h.pos(rect);
                let sq = Rect::from_center_size(c, vec2(9.0, 9.0));
                ui.painter().rect_filled(sq, 2.0, egui::Color32::WHITE);
                ui.painter().rect_stroke(sq, 2.0, Stroke::new(1.5, crate::theme::ACCENT), egui::StrokeKind::Outside);
            }
        }

        if !interactive {
            return;
        }
        self.mouse(ui, &layout, content, i, picture);
    }

    /// The selected picture (if the selection is exactly one, on this page) and where it is on screen.
    fn selected_picture_box(&self, layout: &PageLayout, content: Rect, page_start: usize) -> Option<(u32, Rect)> {
        let (from, _) = self.picture_paragraph()?;
        let p = layout.paras.iter().find(|p| p.image.is_some() && page_start + p.start == from)?;
        let img = p.image?;
        Some((img.id, img.rect.translate(content.min.to_vec2() + vec2(0.0, p.y))))
    }

    fn mouse(&mut self, ui: &mut egui::Ui, layout: &PageLayout, content: Rect, i: usize, picture: Option<(u32, Rect)>) {
        let ctx = ui.ctx().clone();
        let sc = layout.scale;
        let resp = ui.interact(content.expand(6.0), Id::new("editor"), Sense::click_and_drag());

        // Resize handles sit on top of the editor.
        let mut over_handle = false;
        if let Some((id, rect)) = picture {
            for h in Handle::ALL {
                let hr = Rect::from_center_size(h.pos(rect), vec2(13.0, 13.0));
                let r = ui.interact(hr, Id::new(("pic_handle", h.kx, h.ky)), Sense::drag());
                if r.hovered() || r.dragged() {
                    over_handle = true;
                    ui.output_mut(|o| o.cursor_icon = h.cursor());
                }
                if r.drag_started() {
                    let start_width = self.doc.image(id).map_or(0.0, |im| im.width_pt);
                    self.resize = Some(ResizeDrag { id, handle: h, start_width, dragged: Vec2::ZERO });
                }
                if r.dragged() {
                    if let Some(rs) = &mut self.resize {
                        rs.dragged += r.drag_delta() / sc;
                        let aspect = self.doc.image(rs.id).map_or(1.0, |im| im.aspect());
                        let width = rs.handle.new_width(rs.start_width, aspect, rs.dragged);
                        let id = rs.id;
                        self.set_image_width(&ctx, id, width);
                    }
                }
                if r.drag_stopped() {
                    self.resize = None;
                }
            }
        }
        if resp.hovered() && !self.ctrl_down && !over_handle {
            let pointer = ctx.input(|inp| inp.pointer.latest_pos());
            let on_picture = pointer.is_some_and(|p| layout.image_at(p - content.min).is_some());
            let on_link = pointer.is_some_and(|p| layout.link_at(p - content.min).is_some());
            ui.output_mut(|o| {
                o.cursor_icon = match (on_picture, on_link) {
                    (true, _) => egui::CursorIcon::Grab,
                    (_, true) => egui::CursorIcon::PointingHand,
                    _ => egui::CursorIcon::Text,
                }
            });
        }
        if self.ctrl_down || over_handle || self.resize.is_some() {
            return; // Ctrl+drag moves the page; handles do their own thing
        }

        let start = self.doc.spans[i].start;
        // Where a point puts the caret, if on this page: the contents' later pages have no text
        // of their own, and their spots before and after the contents are on other pages.
        let at = |doc: &crate::model::Doc, p: egui::Pos2| Some(start + layout.hit(p - content.min)).filter(|&c| doc.page_of(c) == i);
        let (pressed, secondary, released, shift, pos) = ctx.input(|inp| {
            (
                inp.pointer.primary_pressed(),
                inp.pointer.secondary_pressed(),
                inp.pointer.primary_released(),
                inp.modifiers.shift,
                inp.pointer.latest_pos(),
            )
        });

        // Right click on a picture opens its menu.
        if secondary && resp.contains_pointer() {
            self.pic_menu = None;
            if let Some(p) = pos {
                if let Some(pic) = layout.image_at(p - content.min) {
                    self.anchor = start + pic.start;
                    self.set_caret(&ctx, start + pic.end, true);
                    self.pic_menu = Some(p);
                    self.pic_menu_fresh = true;
                }
            }
        }

        if pressed && resp.contains_pointer() {
            if let Some(p) = resp.interact_pointer_pos().or(pos) {
                self.pic_drag = None;
                if let Some(page) = layout.link_at(p - content.min).filter(|_| !shift) {
                    // A line of the contents leads to its chapter.
                    self.go_to_page(&ctx, page);
                } else if let Some(pic) = layout.image_at(p - content.min) {
                    // Clicking a picture selects it; dragging it moves it.
                    let (s, e) = (start + pic.start, start + pic.end);
                    self.anchor = s;
                    self.set_caret(&ctx, e, true);
                    self.pic_drag = Some(PicDrag { from: s, press: p, drop: None });
                } else if let Some(c) = at(&self.doc, p) {
                    self.set_caret(&ctx, c, shift);
                }
                self.want_x = None;
            }
        } else if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                if let Some(d) = &mut self.pic_drag {
                    if (p - d.press).length() > 5.0 {
                        d.drop = layout.drop_boundary(p.y - content.min.y).map(|(local, y)| (start + local, y));
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                    }
                } else if let Some(c) = at(&self.doc, p) {
                    self.set_caret(&ctx, c, true);
                }
            }
        }
        if released {
            if let Some(d) = self.pic_drag.take() {
                if let Some((boundary, _)) = d.drop {
                    self.move_picture(&ctx, d.from, boundary);
                }
            }
        }
        if resp.double_clicked() && self.pic_drag.is_none() && layout.image_at(pos.map_or(Vec2::ZERO.to_pos2(), |p| p) - content.min).is_none() {
            if let Some(c) = resp.interact_pointer_pos().and_then(|p| at(&self.doc, p)) {
                let (a, b) = self.word_range_at(c);
                self.anchor = a;
                self.set_caret(&ctx, b, true);
            }
        } else if resp.triple_clicked() && layout.image_at(pos.unwrap_or_default() - content.min).is_none() {
            if let Some(c) = resp.interact_pointer_pos().and_then(|p| at(&self.doc, p)) {
                let (ps, _) = self.doc.para_start(c);
                let (end, _) = self.doc.term_from(c, self.doc.char_to_byte(c));
                self.anchor = ps;
                self.set_caret(&ctx, (end + 1).min(self.max_caret()), true);
            }
        }
    }

    pub fn paint_layout(&self, painter: &egui::Painter, layout: &PageLayout, origin: egui::Pos2) {
        for p in &layout.paras {
            if let (Some(img), Some(tex)) = (p.image, p.image.and_then(|i| self.textures.get(&i.id))) {
                let rect = img.rect.translate(origin.to_vec2() + vec2(0.0, p.y));
                painter.add(egui::Shape::mesh(crate::images::picture_mesh(tex.id(), rect, img.rotation, egui::Color32::WHITE)));
            }
            if let Some((g, at)) = &p.marker {
                painter.galley(origin + vec2(at.x, p.y + at.y), g.clone(), INK);
            }
            if p.block().is_none() {
                painter.galley(origin + vec2(p.x, p.y), p.galley.clone(), INK);
            }
            for (g, at) in p.contents.iter().flat_map(|c| &c.texts) {
                painter.galley(origin + vec2(p.x + at.x, p.y + at.y), g.clone(), INK);
            }
        }
    }

    /// Layout of page `i` for drawing, with note and search highlights.
    pub fn page_layout(&self, ctx: &egui::Context, i: usize, scale: f32) -> PageLayout {
        self.doc.layout_page(ctx, i, scale, &self.page_marks(i))
    }
}

