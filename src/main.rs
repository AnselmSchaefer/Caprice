mod edit;
mod editor;
mod export;
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
use model::{Doc, Style};
use search::Search;
use theme::{DESK, apply_theme};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 900.0]).with_title("Caprice"),
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
    /// Caret and the other end of the selection, as positions in the flow (chars).
    pub caret: usize,
    pub anchor: usize,
    /// Horizontal position kept while moving up and down through lines.
    pub want_x: Option<f32>,
    /// At a soft line wrap, does the caret sit at the start of the next row (rather than the end of this one)?
    pub prefer_next: bool,
    pub blink_epoch: f64,
    /// Style given to newly typed text; follows the caret.
    pub typing: Style,
    pub fonts: FontBook,
    /// Page scale (screen points per page point) and the page's top-left on screen.
    pub zoom: f32,
    pub origin: Pos2,
    /// Follow the window size until the user zooms by hand.
    pub fit: bool,
    pub ctrl_down: bool,
    /// True while Ctrl is known from its own key events (then only its key release ends it).
    pub ctrl_via_key: bool,
    /// Width the toolbar wants, and how far it is slid sideways (current and target).
    pub toolbar_w: f32,
    pub toolbar_scroll: f32,
    pub toolbar_target: f32,
    pub path: Option<PathBuf>,
    pub status: String,
    /// Animated position in page units. 2.4 means page 2 is 40% flipped over.
    pub pos: f32,
    /// Page we are flipping towards.
    pub target: usize,
    pub scrubbing: bool,
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
        let typing = Style::new(&default);
        let mut doc = Doc::new();
        doc.flow.styles = vec![typing.clone()];
        Self {
            doc,
            caret: 0,
            anchor: 0,
            want_x: None,
            prefer_next: true,
            blink_epoch: 0.0,
            typing,
            fonts,
            zoom: 1.0,
            origin: Pos2::ZERO,
            fit: true,
            ctrl_down: false,
            ctrl_via_key: false,
            toolbar_w: 0.0,
            toolbar_scroll: 0.0,
            toolbar_target: 0.0,
            path: None,
            status: String::new(),
            pos: 0.0,
            target: 0,
            scrubbing: false,
            search: Search::default(),
            open_note: None,
            note_pos: Pos2::ZERO,
            note_focus: false,
            last_page_rect: Rect::NOTHING,
        }
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

        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
            self.open_search();
        }

        self.process_input(&ctx);

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
            self.stack(ui.painter(), page_rect, i, self.last() - i);
            Self::paper(ui.painter(), page_rect);
            self.draw_footer(ui, page_rect, i);
            self.editor_surface(ui, page_rect, i, true);
            self.draw_notes(ui, page_rect, i);
        } else {
            // Mid-flip: page `base` turns over, revealing `base + 1`.
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
        assert_eq!(h.app.doc.visible_text(), "hello");

        // Add two empty pages, then remove them again with backspace.
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), 2);
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
        assert_eq!(h.app.doc.visible_text(), "hello", "backspacing a page must not eat text");
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
        assert_eq!(h.app.doc.visible_text(), "hello world");
        h.key(Key::Z, Modifiers::COMMAND);
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.visible_text(), "", "typing within a second undoes as one step");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.visible_text(), "hello world");
        // The caret is back at the end, so typing continues there.
        h.type_text("!");
        assert_eq!(h.app.doc.visible_text(), "hello world!");
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
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line");
        // ...and the page gets it back when it closes.
        h.app.open_note = None;
        h.frames(5, vec![], Modifiers::NONE);
        h.type_text("!"); // typed right after the note's end: not part of it
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line!");
        assert_eq!((h.app.doc.notes[0].start, h.app.doc.notes[0].end), (11, 22));
    }

    // ----------------------------------------------------------- the editor

    impl Harness {
        fn shift(&mut self, key: Key) {
            self.key(key, Modifiers::SHIFT);
        }

        fn text(&self) -> String {
            self.app.doc.visible_text().to_owned()
        }

        /// Click at a point given relative to the writing area of the current page.
        fn click_in_page(&mut self, rel: egui::Vec2) {
            let rect = self.app.last_page_rect;
            let sc = self.app.scale_of(rect);
            let pos = rect.min + self.app.doc.setup.margin_origin() * sc + rel * sc;
            self.frames(1, vec![egui::Event::PointerMoved(pos)], Modifiers::NONE);
            let button = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frames(1, vec![button(true)], Modifiers::NONE);
            self.frames(1, vec![button(false)], Modifiers::NONE);
            self.frames(2, vec![], Modifiers::NONE);
        }
    }

    #[test]
    fn arrows_selection_and_replacing_text() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello world");
        h.key(Key::Home, Modifiers::NONE);
        assert_eq!(h.app.caret, 0);
        h.key(Key::ArrowRight, Modifiers::COMMAND); // next word
        assert_eq!(h.app.caret, 6, "after 'hello '");
        for _ in 0..5 {
            h.shift(Key::ArrowRight);
        }
        assert_eq!(h.app.selection(), (6, 11));
        h.type_text("there");
        assert_eq!(h.text(), "hello there");
        h.key(Key::Backspace, Modifiers::COMMAND); // delete the word before the caret
        assert_eq!(h.text(), "hello ");
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::Delete, Modifiers::NONE);
        assert_eq!(h.text(), "hello");
        h.key(Key::A, Modifiers::COMMAND);
        assert_eq!(h.app.selection(), (0, 5));
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.text(), "");
    }

    #[test]
    fn enter_splits_paragraphs_and_keeps_their_format() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("title");
        h.key(Key::E, Modifiers::COMMAND); // center
        h.key(Key::Enter, Modifiers::NONE);
        h.type_text("next");
        assert_eq!(h.text(), "title\nnext");
        let attrs = |h: &Harness, c| h.app.doc.para_attrs_at(c);
        assert_eq!(attrs(&h, 0).align, model::Align::Center);
        assert_eq!(attrs(&h, 8).align, model::Align::Center, "the new paragraph inherits the format");
        h.key(Key::L, Modifiers::COMMAND);
        assert_eq!(attrs(&h, 8).align, model::Align::Left);
        assert_eq!(attrs(&h, 0).align, model::Align::Center, "only the paragraph with the caret changes");
        // Backspace at the start of the second paragraph joins them; the merged paragraph keeps the second's format.
        h.key(Key::Home, Modifiers::NONE);
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.text(), "titlenext");
        assert_eq!(attrs(&h, 0).align, model::Align::Left);
        // And all of it can be undone.
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.text(), "title\nnext");
    }

    #[test]
    fn lists_start_continue_and_end_with_enter() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.key(Key::L, Modifiers::COMMAND | Modifiers::SHIFT); // bullets on
        h.type_text("one");
        h.key(Key::Enter, Modifiers::NONE);
        h.type_text("two");
        assert_eq!(h.app.doc.para_attrs_at(0).list, model::ListKind::Bullet);
        assert_eq!(h.app.doc.para_attrs_at(5).list, model::ListKind::Bullet, "Enter continues the list");
        h.key(Key::Enter, Modifiers::NONE);
        h.key(Key::Enter, Modifiers::NONE); // Enter on an empty item ends the list
        assert_eq!(h.text(), "one\ntwo\n");
        let last = h.app.caret;
        assert_eq!(h.app.doc.para_attrs_at(last).list, model::ListKind::None);
        h.type_text("plain");
        assert_eq!(h.text(), "one\ntwo\nplain");
        // Backspace at the start of a list item first removes the bullet, then joins.
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.key(Key::Home, Modifiers::NONE);
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.text(), "one\ntwo\nplain");
        assert_eq!(h.app.doc.para_attrs_at(h.app.caret).list, model::ListKind::None);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("first line here\nxx\nthird line here");
        // Caret is at the end of the third line (column 15). Up goes to the short line, up again returns to column 15.
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.caret, 15 + 1 + 2, "end of the short second line");
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.caret, 15, "back at the original column on the first line");
        h.key(Key::ArrowDown, Modifiers::NONE);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.caret, h.app.doc.total_chars() - 1, "end of the last line again");
    }

    #[test]
    fn clipboard_events_copy_cut_and_paste() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("abc def");
        h.key(Key::A, Modifiers::COMMAND);
        h.frames(1, vec![egui::Event::Cut], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.text(), "");
        h.frames(1, vec![egui::Event::Paste("one\r\ntwo\u{c}x".into())], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.text(), "one\ntwox", "carriage returns and page breaks do not get into the text");
    }

    #[test]
    fn clicking_places_the_caret_where_the_text_is() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("first line\nsecond line\nthird line");
        h.frames(3, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        // Click in the middle of the word "second" (char 17) on the second line.
        let target = layout.caret_rect(14, true);
        h.click_in_page(egui::vec2(target.left() + 1.0, target.center().y));
        assert_eq!(h.app.caret, 14, "start of 'second line' is char 11; x of char 14 hits 14");
        // A click below all the text goes to the end of the last line.
        h.click_in_page(egui::vec2(100.0, 500.0));
        assert_eq!(h.app.caret, h.app.doc.total_chars() - 1);
        // Shift-click extends the selection.
        let start = layout.caret_rect(0, true);
        h.click_in_page(egui::vec2(start.left() + 0.5, start.center().y));
        assert_eq!(h.app.caret, 0);
    }

    #[test]
    fn centered_text_is_hit_where_it_is_drawn() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("centered");
        h.key(Key::E, Modifiers::COMMAND);
        h.frames(3, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        let r = layout.caret_rect(4, true);
        assert!(r.left() > 100.0, "the text is in the middle of the writing width, not at its left edge: {r:?}");
        h.click_in_page(egui::vec2(r.left() + 0.5, r.center().y));
        assert_eq!(h.app.caret, 4);
    }

    #[test]
    fn typing_in_a_new_empty_page_and_page_break_semantics() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("page one");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(60, vec![], Modifiers::NONE);
        h.type_text("page two");
        assert_eq!(h.app.doc.pages(), 2);
        assert_eq!(h.app.doc.page_text(1), "page two\n");
        assert_eq!(h.text(), "page one\u{c}page two");
        h.key(Key::PageUp, Modifiers::NONE);
        assert_eq!(h.app.caret, 0);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.pos.round() as usize, 0);
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(h.app.doc.page_of(h.app.caret), 1);
    }

    #[test]
    fn character_styles_apply_to_the_selection_and_undo() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("make this bold");
        for _ in 0..4 {
            h.shift(Key::ArrowLeft);
        }
        h.key(Key::B, Modifiers::COMMAND);
        let styles = &h.app.doc.flow.styles;
        assert!(styles[10..14].iter().all(|s| s.bold));
        assert!(styles[0..10].iter().all(|s| !s.bold));
        h.key(Key::Z, Modifiers::COMMAND);
        assert!(h.app.doc.flow.styles.iter().all(|s| !s.bold));
    }
}
