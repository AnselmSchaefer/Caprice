mod edit;
mod editor;
mod fileio;
mod fonts;
mod layout;
mod model;
mod notes;
mod paginate;
mod render;
mod search;
mod theme;
mod ui;
mod view;

use std::path::PathBuf;

use eframe::egui::{self, Key, Modifiers, Pos2, Rect};

use fonts::FontBook;
use model::{Doc, EditBuf, Style};
use search::Search;
use theme::{DESK, apply_theme};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 900.0]).with_title("Caprice"),
        ..Default::default()
    };
    eframe::run_native(
        "Caprice",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            // We handle zoom ourselves so the docks keep their size.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            Ok(Box::new(App::new(&cc.egui_ctx)))
        }),
    )
}

pub struct App {
    pub doc: Doc,
    /// Text of the page being edited, and which (page, doc version) it was loaded from.
    pub view: EditBuf,
    pub view_for: Option<(usize, u64)>,
    /// Style given to newly typed text; follows the cursor.
    pub typing: Style,
    pub fonts: FontBook,
    /// Page scale (screen points per page point) and the page's top-left on screen.
    pub zoom: f32,
    pub origin: Pos2,
    /// Follow the window size until the user zooms by hand.
    pub fit: bool,
    pub ctrl_down: bool,
    pub path: Option<PathBuf>,
    pub status: String,
    pub last_cursor: Option<(usize, usize)>,
    /// Animated position in page units. 2.4 means page 2 is 40% flipped over.
    pub pos: f32,
    /// Page we are flipping towards (and the one being edited once we arrive).
    pub target: usize,
    pub scrubbing: bool,
    /// (page, offset in page) to put the cursor at once that page is shown and settled.
    pub cursor_req: Option<(usize, usize)>,
    /// Typing that arrived while a page was turning; replayed once the next page is editable.
    pub held_input: Vec<egui::Event>,
    pub search: Search,
    /// The note whose editor window is open, where to put it, and whether to focus its text field.
    pub open_note: Option<u64>,
    pub note_pos: Pos2,
    pub note_focus: bool,
    pub last_page_rect: Rect,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let mut fonts = FontBook::new();
        let default = fonts.preferred_default();
        fonts.ensure(ctx, &default, true);
        Self {
            doc: Doc::new(),
            view: EditBuf::new(String::new(), Vec::new()),
            view_for: None,
            typing: Style::new(&default),
            fonts,
            zoom: 1.0,
            origin: Pos2::ZERO,
            fit: true,
            ctrl_down: false,
            path: None,
            status: String::new(),
            last_cursor: None,
            pos: 0.0,
            target: 0,
            scrubbing: false,
            cursor_req: Some((0, 0)),
            held_input: Vec::new(),
            search: Search::default(),
            open_note: None,
            note_pos: Pos2::ZERO,
            note_focus: false,
            last_page_rect: Rect::NOTHING,
        }
    }

    /// Keep typed text and plain editing keys that arrive mid-flip instead of losing them.
    fn hold_typing(&mut self, ctx: &egui::Context) {
        if Self::ui_field_focused(ctx) {
            return; // that typing is meant for the search box or a note
        }
        let is_typing = |e: &egui::Event| match e {
            egui::Event::Text(_) => true,
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                !modifiers.command
                    && !modifiers.alt
                    && matches!(key, Key::Enter | Key::Backspace | Key::Delete | Key::Tab)
            }
            _ => false,
        };
        let taken = ctx.input_mut(|i| {
            let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut i.events).into_iter().partition(|e| is_typing(e));
            i.events = kept;
            taken
        });
        self.held_input.extend(taken);
    }

    fn animate(&mut self, ui: &egui::Ui) {
        if self.scrubbing {
            return;
        }
        let target = self.target as f32;
        let diff = target - self.pos;
        if diff.abs() < 0.002 {
            self.pos = target;
            return;
        }
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let speed = (diff.abs() * 5.0).clamp(2.2, 14.0); // pages per second
        let step = speed * dt;
        self.pos = if diff.abs() <= step { target } else { self.pos + step * diff.signum() };
        ui.ctx().request_repaint();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }
}

impl App {
    /// One frame of the whole app (separate from `eframe::App::ui` so tests can drive it).
    fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let area = ui.max_rect();
        ui.painter().rect_filled(area, 0.0, DESK);

        self.fonts.activate_pending();
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.save(false);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
            self.open(&ctx);
        }

        // Document-wide shortcuts. Undo is ours (one history for the whole document), so the
        // page editor never sees these keys.
        if !Self::ui_field_focused(&ctx) {
            let redo = ctx.input_mut(|i| {
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || i.consume_key(Modifiers::COMMAND, Key::Y)
            });
            let undo = !redo && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Z));
            if redo || undo {
                self.step_history(&ctx, redo);
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
            self.open_search();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::N)) {
            self.add_note(&ctx);
        }

        // Page navigation shortcuts.
        let (next, prev, new_page) = ctx.input(|i| {
            (
                i.key_pressed(Key::PageDown),
                i.key_pressed(Key::PageUp),
                i.modifiers.command && i.key_pressed(Key::Enter),
            )
        });
        if next {
            self.target = (self.target + 1).min(self.last());
            self.cursor_req = Some((self.target, 0));
        }
        if prev {
            self.target = self.target.saturating_sub(1);
            self.cursor_req = Some((self.target, 0));
        }
        if new_page {
            ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
            let cur = self.target;
            self.insert_page_after(&ctx, cur);
        }

        // Pages can disappear (backspacing away a break), so keep the position valid.
        self.target = self.target.min(self.last());
        self.animate(ui);
        self.pos = self.pos.clamp(0.0, self.last() as f32);

        let page_rect = self.page_rect(&ctx, area);
        self.last_page_rect = page_rect;
        let base = self.pos.floor() as usize;
        let t = self.pos - base as f32;
        let n = self.doc.pages();

        if t < 1e-3 || base >= self.last() {
            // Settled: this page is editable.
            let i = (self.pos.round() as usize).min(self.last());
            // May merge pages, so work out which page to show only afterwards.
            self.handle_boundary_keys(&ctx, i);
            let i = (self.pos.round() as usize).min(self.last());
            self.stack(ui.painter(), page_rect, i, self.last() - i);
            Self::paper(ui.painter(), page_rect);
            self.draw_footer(ui, page_rect, i);
            self.edit_page(ui, page_rect, i);
            self.draw_notes(ui, page_rect, i);
        } else {
            // Mid-flip: page `base` turns over, revealing `base + 1`.
            self.hold_typing(&ctx);
            self.stack(ui.painter(), page_rect, base, n - 1 - base - 1);
            self.static_page(ui, page_rect, base + 1);
            let eased = t * t * (3.0 - 2.0 * t);
            let fade = 1.0 - ((t - 0.78) / 0.22).clamp(0.0, 1.0);
            self.flipping_page(ui, page_rect, base, eased * std::f32::consts::PI, fade);
        }

        self.pan_with_ctrl(ui, area);
        self.format_bar(ui, area);
        self.page_bar(ui, area);
        self.search_bar(ui, area);
        self.note_window(&ctx);
        if self.cursor_req.is_some() {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Harness {
        ctx: egui::Context,
        app: App,
        time: f64,
        mods: Modifiers,
    }

    impl Harness {
        fn new() -> Self {
            let ctx = egui::Context::default();
            apply_theme(&ctx);
            let app = App::new(&ctx);
            Self { ctx, app, time: 0.0, mods: Modifiers::NONE }
        }

        fn frames(&mut self, n: usize, events: Vec<egui::Event>, modifiers: Modifiers) {
            for k in 0..n {
                self.time += 1.0 / 60.0;
                let mut input = egui::RawInput {
                    time: Some(self.time),
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 900.0))),
                    ..Default::default()
                };
                if modifiers != self.mods {
                    self.mods = modifiers;
                    input.events.push(egui::Event::ModifiersChanged(modifiers));
                }
                if k == 0 {
                    input.events.extend(events.clone());
                }
                let app = &mut self.app;
                let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
                out.textures_delta.clear();
            }
        }

        fn key(&mut self, key: Key, modifiers: Modifiers) {
            let e = egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers };
            self.frames(1, vec![e], modifiers);
            self.frames(3, vec![], Modifiers::NONE);
        }

        fn type_text(&mut self, text: &str) {
            self.frames(1, vec![egui::Event::Text(text.to_owned())], Modifiers::NONE);
            self.frames(2, vec![], Modifiers::NONE);
        }
    }

    #[test]
    fn typing_new_pages_and_backspacing_them_away() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello");
        assert_eq!(h.app.doc.flow.text, "hello");

        // Add two empty pages, then remove them again with backspace.
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), 3);
        assert_eq!(h.app.target, 2);

        h.key(Key::Backspace, Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), 2);
        h.key(Key::Backspace, Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), 1);
        assert_eq!(h.app.doc.flow.text, "hello", "backspacing a page must not eat text");
        assert_eq!(h.app.target, 0);
    }

    #[test]
    fn typing_past_the_page_end_flows_onto_a_new_page() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..80 {
            h.type_text("a line of text\n");
        }
        h.frames(90, vec![], Modifiers::NONE);
        assert!(h.app.doc.pages() >= 2, "pages: {}", h.app.doc.pages());
        assert_eq!(h.app.target, h.app.doc.pages() - 1, "cursor should follow the text");
        assert_eq!(h.app.doc.flow.text.matches("a line of text").count(), 80);
    }

    #[test]
    fn changing_margins_and_orientation_reflows() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..150 {
            h.type_text("a line of text\n");
        }
        h.frames(60, vec![], Modifiers::NONE);
        let before = h.app.doc.pages();
        let mut setup = h.app.doc.setup.clone();
        setup.margin_top = 160.0;
        setup.margin_bottom = 160.0;
        h.app.set_setup(&h.ctx.clone(), setup);
        h.frames(90, vec![], Modifiers::NONE);
        assert!(h.app.doc.pages() > before);
        let mut setup = h.app.doc.setup.clone();
        setup.margin_top = 72.0;
        setup.margin_bottom = 72.0;
        h.app.set_setup(&h.ctx.clone(), setup);
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), before, "text flows back when margins shrink again");
        assert_eq!(h.app.doc.flow.text.matches("a line of text").count(), 150);
    }

    #[test]
    fn undo_and_redo_work_on_typed_text() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello");
        h.type_text(" world");
        assert_eq!(h.app.doc.flow.text, "hello world");
        h.key(Key::Z, Modifiers::COMMAND);
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.flow.text, "", "typing within a second undoes as one step");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.flow.text, "hello world");
        // The caret is back at the end, so typing continues there.
        h.type_text("!");
        assert_eq!(h.app.doc.flow.text, "hello world!");
    }

    #[test]
    fn search_finds_matches_and_flips_to_their_page() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..90 {
            h.type_text("some filler text\n");
        }
        h.type_text("the NEEDLE is here");
        h.frames(90, vec![], Modifiers::NONE);
        assert!(h.app.doc.pages() >= 2);
        // Go back to the first page, then search.
        h.app.target = 0;
        h.frames(120, vec![], Modifiers::NONE);
        assert_eq!(h.app.pos.round() as usize, 0);
        h.app.open_search();
        h.app.search.query = "needle".into();
        h.frames(120, vec![], Modifiers::NONE);
        assert_eq!(h.app.search.matches.len(), 1);
        assert_eq!(h.app.target, h.app.doc.pages() - 1, "flipped to the page with the match");
        assert_eq!(h.app.pos.round() as usize, h.app.doc.pages() - 1);
    }

    #[test]
    fn a_note_on_a_line_stays_put_while_typing_next_to_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("first line\nsecond line");
        let ctx = h.ctx.clone();
        h.app.add_note(&ctx);
        h.frames(3, vec![], Modifiers::NONE);
        let note = h.app.doc.notes[0].clone();
        assert_eq!((note.start, note.end), (11,22), "the note covers the line the cursor was on");
        // The note editor grabs the keyboard when it opens...
        h.type_text("remember");
        assert_eq!(h.app.doc.notes[0].text, "remember");
        assert_eq!(h.app.doc.flow.text, "first line\nsecond line");
        // ...and the page gets it back when it closes.
        h.app.open_note = None;
        h.frames(5, vec![], Modifiers::NONE);
        h.type_text("!"); // typed right after the note's end: not part of it
        assert_eq!(h.app.doc.flow.text, "first line\nsecond line!");
        assert_eq!((h.app.doc.notes[0].start, h.app.doc.notes[0].end), (11, 22));
    }
}
