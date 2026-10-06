mod backdrop;
mod book;
mod claude;
mod edit;
mod editor;
mod export;
mod fileio;
mod fonts;
mod images;
mod layout;
mod model;
mod notes;
mod paginate;
mod render;
mod search;
mod theme;
mod ui;
mod view;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui::{self, Key, Modifiers, Pos2, Rect};

use fonts::FontBook;
use model::{Doc, Style};
use search::Search;
use theme::{DESK, apply_theme};

/// The app icon, for window managers that take it from the window (the launcher uses the installed files).
fn window_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon-256.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            egui::IconData { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() }
        }
        Err(_) => egui::IconData::default(),
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 900.0])
            .with_title("Caprice")
            .with_app_id("Caprice")
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Caprice",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            // We handle zoom ourselves so the docks keep their size.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            let mut app = App::new(&cc.egui_ctx);
            // `caprice some.caprice` opens that document (once the first frame has fonts to lay it out).
            app.startup_file = std::env::args_os().nth(1).map(Into::into);
            Ok(Box::new(app))
        }),
    )
}

/// How the pages look and move as you go through them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Appearance {
    /// Pages turn over like a book's, hinged on their left edge.
    Book,
    /// Pages slide off to the left and tuck in under the pile behind.
    Paperstack,
}

pub struct App {
    pub doc: Doc,
    pub appearance: Appearance,
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
    /// Gliding into the fitted position (after zooming back to it), rather than already there.
    pub fit_settling: bool,
    pub ctrl_down: bool,
    /// True while Ctrl is known from its own key events (then only its key release ends it).
    pub ctrl_via_key: bool,
    /// Width the toolbar wants, and how far it is slid sideways (current and target).
    pub toolbar_w: f32,
    pub toolbar_scroll: f32,
    pub toolbar_target: f32,
    pub path: Option<PathBuf>,
    pub status: String,
    /// Animated position in page units. 2.4 means page 2 has slid 40% of the way into the pile behind.
    pub pos: f32,
    /// Page we are moving towards.
    pub target: usize,
    /// The first of the pages just added and turned to: they slide in from the right instead of
    /// slid out of the pile (several, when pages are added faster than they slide in).
    pub slide_in: Option<usize>,
    /// How far a page just taken away (backspaced) has slid out to the right, 0 to 1.
    pub slide_out: Option<f32>,
    /// Page count and target as of the last frame, to notice a page being added.
    pub seen_pages: usize,
    pub seen_target: usize,
    /// While the scrollbar is dragged: where it points, in pages (fractional: between pages).
    pub scrub_to: Option<f32>,
    /// While the scrollbar points ahead, the pages ahead slide out half a page to the left one
    /// after the other, following it: how many pages' worth (2.5: two are out, the third halfway).
    /// Pointing back (negative), the pages behind slide half out of the pile instead.
    pub held: f32,
    /// Pages let go of by the scrollbar, moving into or out of the pile together: from, to, how
    /// far along (0 to 1), and how far they were held out (see `held`).
    pub batch: Option<(usize, usize, f32, f32)>,
    /// Sideways two-finger swipe not yet turned into page moves, and when it last moved.
    pub swipe: f32,
    pub swipe_at: f64,
    /// The pages are moving because of a swipe: they go at half the pace.
    pub swiped: bool,
    pub search: Search,
    /// The note to put the keyboard on (a new one), and which sheet each note's pad shows.
    pub note_focus: Option<u64>,
    pub pads: HashMap<u64, notes::Pad>,
    pub last_page_rect: Rect,
    /// Where the scrollbar's track was drawn last, and the stretch its thumb's centre travels.
    pub last_track: (f32, f32, f32),
    /// Textures of the document's pictures, by picture id.
    pub textures: HashMap<u32, egui::TextureHandle>,
    pub startup_file: Option<PathBuf>,
    /// The last status message shown, when it appeared, and the window title last set.
    pub shown_status: String,
    /// Right-click menu of a picture (where it was opened), a picture being dragged, a resize in progress.
    pub pic_menu: Option<Pos2>,
    /// The menu opened this very frame (so the click that opened it is not a click outside).
    pub pic_menu_fresh: bool,
    pub pic_drag: Option<images::PicDrag>,
    pub resize: Option<images::ResizeDrag>,
    pub status_at: f64,
    pub title: String,
    /// The pen is on: dragging on the page draws a loop around text to ask Claude about.
    pub pen: bool,
    pub lasso: Option<claude::Lasso>,
    /// Claude's answer being shown, if any.
    pub answer: Option<claude::Answer>,
    /// The scene Claude paints behind the pages, and the panel to describe it.
    pub backdrop: backdrop::Backdrop,
    /// A file dialog waiting for the user to choose.
    pub dialog: Option<fileio::PendingDialog>,
    /// What saving last wrote (or opening read), to notice unsaved changes.
    pub saved_hash: u64,
    /// Opening or closing that waits on the "Save changes?" question, and what to do once saved.
    pub leaving: Option<fileio::Leaving>,
    pub after_save: Option<fileio::Leaving>,
    /// The user has said the window may close (changes saved or given up).
    pub may_close: bool,
    /// What was last copied or cut, with its formatting.
    pub clip: Option<editor::Clip>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let mut fonts = FontBook::new();
        let default = fonts.preferred_default();
        fonts.ensure(ctx, &default, true);
        let typing = Style::new(&default);
        let mut doc = Doc::new();
        doc.flow.styles = vec![typing.clone()];
        let mut app = Self {
            doc,
            appearance: Appearance::Book,
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
            fit_settling: false,
            ctrl_down: false,
            ctrl_via_key: false,
            toolbar_w: 0.0,
            toolbar_scroll: 0.0,
            toolbar_target: 0.0,
            path: None,
            status: String::new(),
            pos: 0.0,
            target: 0,
            slide_in: None,
            slide_out: None,
            seen_pages: 0,
            seen_target: 0,
            scrub_to: None,
            held: 0.0,
            batch: None,
            swipe: 0.0,
            swipe_at: 0.0,
            swiped: false,
            search: Search::default(),
            note_focus: None,
            pads: HashMap::new(),
            last_page_rect: Rect::NOTHING,
            clip: None,
            last_track: (0.0, 0.0, 0.0),
            textures: HashMap::new(),
            startup_file: None,
            shown_status: String::new(),
            pic_menu: None,
            pic_menu_fresh: false,
            pic_drag: None,
            resize: None,
            status_at: f64::NEG_INFINITY,
            title: String::new(),
            pen: false,
            lasso: None,
            answer: None,
            backdrop: backdrop::Backdrop::default(),
            dialog: None,
            saved_hash: 0,
            leaving: None,
            after_save: None,
            may_close: false,
        };
        app.saved_hash = app.content_hash();
        app
    }

    /// The document name goes in the window title; messages ("saved", errors) show briefly above the page bar.
    fn update_title_and_toast(&mut self, ui: &egui::Ui, area: Rect) {
        let ctx = ui.ctx();
        let name = self.path.as_ref().and_then(|p| p.file_name()).map_or("Untitled".to_owned(), |n| n.to_string_lossy().into_owned());
        let title = format!("{name} \u{2014} Caprice");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        let now = ctx.input(|i| i.time);
        if self.status != self.shown_status {
            self.shown_status = self.status.clone();
            self.status_at = now;
        }
        let age = now - self.status_at;
        const SHOW_FOR: f64 = 4.0;
        if !self.status.is_empty() && age < SHOW_FOR {
            let text = self.status.trim_start_matches(['-', ' ']).to_owned();
            let fade = (((SHOW_FOR - age) / 0.6) as f32).clamp(0.0, 1.0);
            let galley = ui.painter().layout_no_wrap(text, egui::FontId::proportional(13.0), theme::TEXT.gamma_multiply(fade));
            let size = galley.size() + egui::vec2(28.0, 14.0);
            let center = egui::pos2(area.center().x, area.bottom() - ui::DOCK_GAP - ui::DOCK_H - 26.0);
            let pill = Rect::from_center_size(center, size);
            ui.painter().rect_filled(pill, 12.0, theme::DOCK.gamma_multiply(fade));
            ui.painter().rect_stroke(pill, 12.0, egui::Stroke::new(1.0, theme::DOCK_EDGE.gamma_multiply(fade)), egui::StrokeKind::Inside);
            ui.painter().galley(pill.center() - galley.size() / 2.0, galley, theme::TEXT);
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Go to page `to`: the pages in between slide into, or out of, the pile behind one by one.
    pub fn go_to_page(&mut self, ctx: &egui::Context, to: usize) {
        let to = to.min(self.last());
        if to != self.target {
            self.set_caret(ctx, self.doc.spans[to].start, false);
        }
    }

    /// The page letting go of the scrollbar goes to: any page it has begun to slide out.
    pub fn scrub_page(&self) -> Option<usize> {
        let here = self.target as f32;
        let to = self.scrub_to.map(|at| {
            let pages = ((at - here).abs() - 0.15).ceil().max(0.0);
            if at > here { here + pages } else { here - pages }
        });
        to.map(|p| (p.max(0.0) as usize).min(self.last()))
    }

    /// Let go of the scrollbar pointing at page `to`: the pages held out move there together.
    pub fn let_go_to(&mut self, ctx: &egui::Context, to: usize) {
        if to != self.target {
            self.batch_to(ctx, self.doc.spans[to].start);
        }
    }

    /// Put the caret at `c`, the pages between here and its page moving there together, on from
    /// where the scrollbar holds them, if it does.
    pub fn batch_to(&mut self, ctx: &egui::Context, c: usize) {
        let from = self.target;
        self.set_caret(ctx, c, false);
        let to = self.target;
        if to == from {
            return;
        }
        let held = std::mem::take(&mut self.held);
        let held = if (to > from) == (held > 0.0) { held } else { 0.0 };
        self.batch = Some((from, to, 0.0, held));
        self.pos = to as f32;
    }

    /// Move the pages let go of along.
    fn animate_batch(&mut self, ui: &egui::Ui) {
        let Some((from, to, s, held)) = self.batch else { return };
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let s = s + dt / 0.6;
        // Done, or another way of moving took over.
        self.batch = (s < 1.0 && self.target == to).then_some((from, to, s, held));
        ui.ctx().request_repaint();
    }

    /// Slide the pages out one after the other while the dragged scrollbar points away from the
    /// page, following it; they go back when it points here again.
    fn animate_held(&mut self, ui: &egui::Ui) {
        let here = self.target as f32;
        let settled = self.batch.is_none() && self.pos == here && self.appearance == Appearance::Paperstack;
        let goal = match self.scrub_to {
            Some(at) if settled => (at - here).clamp(-here, self.last() as f32 - here),
            _ => 0.0,
        };
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        self.held += (goal - self.held) * (1.0 - (-dt / 0.06).exp());
        if (goal - self.held).abs() < 0.003 {
            self.held = goal;
        } else {
            ui.ctx().request_repaint();
        }
    }

    /// Move a page that was taken away further out to the right, at the pace of a flip.
    fn animate_slide_out(&mut self, ui: &egui::Ui) {
        let Some(s) = self.slide_out else { return };
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let s = s + ((1.0 - s) * 5.0).clamp(2.2, 14.0) * dt;
        self.slide_out = (s < 1.0).then_some(s);
        ui.ctx().request_repaint();
    }

    /// Whether page `base + 1` slides in over `base` (it was just added) rather than slid out of the pile.
    fn slides_in(&self, base: usize) -> bool {
        self.slide_in.is_some_and(|s| base + 1 >= s)
    }

    fn animate(&mut self, ui: &egui::Ui) {
        if self.appearance == Appearance::Book {
            return self.animate_book(ui);
        }
        let goal = self.target as f32;
        let diff = goal - self.pos;
        if diff.abs() < 0.002 {
            self.pos = goal;
            self.swiped = false;
            return;
        }
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        // Pages per second, by whole pages left: one page eases by itself, many go by quickly.
        let mut speed = (diff.abs().ceil() * 2.75).max(2.2);
        if self.swiped {
            speed /= 2.0;
        }
        let step = speed * dt;
        self.pos = if diff.abs() <= step { goal } else { self.pos + step * diff.signum() };
        ui.ctx().request_repaint();
    }

    /// In a book, the dragged scrollbar moves the pages itself; otherwise each turn eases by
    /// itself, as fast forwards as backwards (the slow slide into the pile comes at its other end).
    fn animate_book(&mut self, ui: &egui::Ui) {
        self.swiped = false;
        if self.scrub_to.is_some() {
            return;
        }
        let target = self.target as f32;
        let diff = target - self.pos;
        if diff.abs() < 0.002 {
            self.pos = target;
            return;
        }
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let mut speed = (diff.abs().ceil() * 2.75).clamp(2.2, 14.0);
        let base = self.pos.floor();
        if self.pos - base >= book::TURN && !self.slides_in(base as usize) {
            speed /= 2.0; // a turned page slides onto the pile at half the pace
        }
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
        if let Some(path) = self.startup_file.take() {
            self.open_path(&ctx, path);
        }
        self.poll_dialog(&ctx);
        self.guard_close(&ctx);
        self.ensure_textures(&ctx);
        if self.leaving.is_none() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
                self.save(&ctx, false);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O)) {
                self.open(&ctx);
            }
        }

        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::F)) {
            self.open_search();
        }

        if self.leaving.is_none() {
            self.process_input(&ctx);
            self.swipe_pages(&ctx, area);
        }

        // Pages can disappear (backspacing away a break), so keep the position valid.
        self.target = self.target.min(self.last());
        let pages = self.doc.pages();
        if pages > self.seen_pages && self.target == self.seen_target + 1 {
            self.slide_in = Some(self.slide_in.map_or(self.target, |s| s.min(self.target)));
        } else if pages < self.seen_pages && self.target + 1 == self.seen_target {
            // The page was taken away: it slides out to the right, uncovering the one before.
            self.slide_out = Some(0.0);
            self.slide_in = None;
            self.pos = self.target as f32;
        }
        (self.seen_pages, self.seen_target) = (pages, self.target);
        self.animate(ui);
        if self.pos == self.target as f32 {
            self.slide_in = None;
        }
        self.animate_slide_out(ui);
        self.animate_batch(ui);
        self.animate_pads(ui);
        self.update_backdrop(&ctx);
        self.pos = self.pos.clamp(0.0, self.last() as f32);

        let page_rect = self.page_rect(&ctx, area);
        self.last_page_rect = page_rect;
        let base = self.pos.floor() as usize;
        let t = self.pos - base as f32;
        let n = self.doc.pages();

        self.animate_held(ui);

        if let Some((from, to, s, held)) = self.batch {
            // Pages let go of by the scrollbar, moving together.
            self.batch_pages(ui, page_rect, from, to, s, held);
        } else if self.held != 0.0 && self.pos == self.target as f32 {
            // Held out by the scrollbar.
            self.held_pages(ui, page_rect, self.held);
        } else if t < 1e-3 || base >= self.last() {
            let i = (self.pos.round() as usize).min(self.last());
            if i != self.target {
                // Passing by: just the page.
                self.stack(ui.painter(), page_rect, i, self.last() - i);
                self.static_page(ui, page_rect, i);
            } else {
                // Settled: this page is editable.
                let ease = |s: f32| s * s * (3.0 - 2.0 * s);
                if let Some(s) = self.slide_out {
                    // While a page slides out, the one before comes up from the back of the pile.
                    self.sheet_to_pile(ui, page_rect, i, egui::Vec2::ZERO, 1.0 - ease(s));
                }
                self.stack(ui.painter(), page_rect, i, self.last() - i);
                // A post-it of a covered page takes clicks where it shows, under the page's own widgets.
                for (id, _, r) in self.pile_note_hits(&ctx, page_rect, i, self.last() - i) {
                    let resp = ui.interact(r, egui::Id::new(("pile_note", id)), egui::Sense::click());
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("Go to this note").clicked() {
                        self.go_to_note(&ctx, id);
                    }
                }
                Self::paper(ui.painter(), page_rect);
                ui.painter().extend(self.backdrop_shapes(&ctx, page_rect, &|p| p, 1.0, 1.0));
                self.draw_footer(ui, page_rect, i);
                self.editor_surface(ui, page_rect, i, !self.pen);
                self.draw_notes(ui, page_rect, i);
                self.pen_surface(ui, page_rect, i);
                if let Some(s) = self.slide_out {
                    let to_right = area.right() + 40.0 - page_rect.left();
                    let r = page_rect.translate(egui::vec2(to_right * ease(s), 0.0));
                    Self::paper(ui.painter(), r);
                    ui.painter().extend(self.backdrop_shapes(&ctx, r, &|p| p, 1.0, 1.0));
                }
            }
        } else if self.slides_in(base) {
            // A new page comes in from beyond the right edge of the window, over the old one,
            // while the pile behind grows by a sheet from the back.
            let ease = |s: f32| s * s * (3.0 - 2.0 * s);
            self.sheet_to_pile(ui, page_rect, base, egui::Vec2::ZERO, ease(t));
            self.stack(ui.painter(), page_rect, base, n - 1 - base - 1);
            self.static_page(ui, page_rect, base);
            let from_right = area.right() + 40.0 - page_rect.left();
            self.static_page(ui, page_rect.translate(egui::vec2(from_right * (1.0 - ease(t)), 0.0)), base + 1);
        } else if self.appearance == Appearance::Book {
            // Mid-turn: page `base` turns over, revealing `base + 1`, or back.
            self.book_turn(ui, page_rect, base, t);
        } else {
            // Between pages: page `base` slides off `base + 1` into the back of the pile behind, or out of it.
            let back = (self.target as f32) < self.pos;
            self.slide_page(ui, page_rect, base, t, back);
        }

        self.pan_with_ctrl(ui, area);
        self.update_title_and_toast(ui, area);
        self.format_bar(ui, area);
        self.page_bar(ui, area);
        self.search_bar(ui, area);
        self.picture_menu(&ctx);
        self.answer_panel(&ctx);
        self.scene_panel(&ctx);
        self.unsaved_prompt(&ctx);
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
    fn a_new_page_slides_in_but_going_back_to_it_slides_out_of_the_pile() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello");
        h.key(Key::Enter, Modifiers::COMMAND);
        assert_eq!(h.app.slide_in, Some(1), "the added page slides in");
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.slide_in, None);

        h.key(Key::PageUp, Modifiers::NONE);
        h.frames(90, vec![], Modifiers::NONE);
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(h.app.slide_in, None, "a page that was already there comes out of the pile");
    }

    #[test]
    fn the_scrollbar_holds_the_pages_half_out_then_moves_them_together() {
        let mut h = Harness::new();
        h.app.appearance = Appearance::Paperstack;
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..6 {
            h.type_text("page");
            h.key(Key::Enter, Modifiers::COMMAND);
        }
        h.frames(120, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 1);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!((h.app.target, h.app.pos), (1, 1.0));

        // Dragging the thumb right slides the pages half out one by one, following it. The page
        // and the caret stay put meanwhile.
        let (y, x0, x1) = h.app.last_track;
        let at = |page: f32| egui::pos2(x0 + (x1 - x0) * page / 6.0, y);
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let near = |a: f32, b: f32| (a - b).abs() < 0.01;
        h.frames(1, vec![egui::Event::PointerMoved(at(1.0)), button(at(1.0), true)], Modifiers::NONE);
        h.frames(1, vec![egui::Event::PointerMoved(at(3.5))], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert!(near(h.app.held, 2.5), "{}", h.app.held);
        assert_eq!((h.app.target, h.app.pos, h.app.scrub_page()), (1, 1.0, Some(4)));
        let (rect, ctx) = (h.app.last_page_rect, h.ctx.clone());
        let half = (-rect.width() / 2.0).round();
        assert_eq!(h.app.held_offset(&ctx, rect, 2, 1, 2.0).x.round(), half, "half out");
        assert!(h.app.held_offset(&ctx, rect, 1, 1, 2.0).x < h.app.held_offset(&ctx, rect, 2, 1, 2.0).x, "a sheet apart");
        // Pointing here again lays them back; pointing back slides the page behind half out of the pile.
        h.frames(1, vec![egui::Event::PointerMoved(at(1.0))], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert_eq!(h.app.held, 0.0);
        h.frames(1, vec![egui::Event::PointerMoved(at(0.0))], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert!(near(h.app.held, -1.0), "{}", h.app.held);
        assert_eq!(h.app.held_offset(&ctx, rect, 0, 1, -1.0).x.round(), half);

        // Letting go moves them all together, on from where they were held.
        h.frames(1, vec![egui::Event::PointerMoved(at(6.0))], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        h.frames(1, vec![button(at(6.0), false)], Modifiers::NONE);
        h.frames(1, vec![], Modifiers::NONE);
        let (from, to, _, held) = h.app.batch.unwrap();
        assert!((from, to) == (1, 6) && near(held, 5.0), "{:?}", h.app.batch);
        assert_eq!(h.app.doc.page_of(h.app.caret), 6);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!((h.app.batch, h.app.held, h.app.pos), (None, 0.0, 6.0));

        // Back the same way: half out of the pile while held, then all onto the page together.
        h.frames(1, vec![egui::Event::PointerMoved(at(6.0)), button(at(6.0), true)], Modifiers::NONE);
        h.frames(1, vec![egui::Event::PointerMoved(at(4.5))], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert!(near(h.app.held, -1.5), "{}", h.app.held);
        h.frames(1, vec![button(at(4.5), false)], Modifiers::NONE);
        h.frames(1, vec![], Modifiers::NONE);
        let (from, to, _, held) = h.app.batch.unwrap();
        assert!((from, to) == (6, 4) && near(held, -1.5), "{:?}", h.app.batch);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!((h.app.batch, h.app.held, h.app.pos), (None, 0.0, 4.0));
    }

    #[test]
    fn in_a_book_the_scrollbar_turns_the_pages_along_with_it() {
        let mut h = Harness::new();
        assert_eq!(h.app.appearance, Appearance::Book, "a book is the default");
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..6 {
            h.type_text("page");
            h.key(Key::Enter, Modifiers::COMMAND);
        }
        h.frames(120, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 1);
        h.frames(150, vec![], Modifiers::NONE);
        assert_eq!(h.app.pos, 1.0);

        // The pages turn with the thumb, nothing is held out, and the caret goes along.
        let (y, x0, x1) = h.app.last_track;
        let at = |page: f32| egui::pos2(x0 + (x1 - x0) * page / 6.0, y);
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        h.frames(1, vec![egui::Event::PointerMoved(at(1.0)), button(at(1.0), true)], Modifiers::NONE);
        h.frames(1, vec![egui::Event::PointerMoved(at(3.4))], Modifiers::NONE);
        h.frames(10, vec![], Modifiers::NONE);
        assert!((h.app.pos - 3.4).abs() < 0.01, "{}", h.app.pos);
        assert_eq!((h.app.target, h.app.held), (3, 0.0));
        // Let go, the page turning finishes by itself, without moving pages together.
        h.frames(1, vec![button(at(3.4), false)], Modifiers::NONE);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!((h.app.pos, h.app.target, h.app.batch), (3.0, 3, None));

        // Going to a post-it turns page by page too.
        let start = h.app.doc.spans[0].start;
        h.app.doc.notes.push(model::Note { id: 7, start, end: start + 2, text: String::new(), color: 0 });
        h.app.go_to_note(&ctx, 7);
        h.frames(1, vec![], Modifiers::NONE);
        assert_eq!((h.app.target, h.app.batch), (0, None));
        assert!(h.app.pos > 0.0 && h.app.pos < 3.0, "{}", h.app.pos);
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.pos, 0.0);
    }

    #[test]
    fn a_backspaced_page_slides_out() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        h.key(Key::Backspace, Modifiers::NONE);
        assert_eq!(h.app.doc.pages(), 1);
        assert!(h.app.slide_out.is_some(), "the page slides out");
        assert_eq!(h.app.pos, 0.0, "without going back through the page before");
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.slide_out, None);
    }

    #[test]
    fn pages_added_quickly_all_slide_in() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("hello");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.key(Key::Enter, Modifiers::COMMAND);
        h.key(Key::Enter, Modifiers::COMMAND);
        assert_eq!(h.app.target, 3);
        assert_eq!(h.app.slide_in, Some(1), "it keeps sliding, and so do the ones after it");
        h.frames(120, vec![], Modifiers::NONE);
        assert_eq!(h.app.slide_in, None);
    }

    #[test]
    fn swiping_sideways_with_two_fingers_moves_through_pages() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("one");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.type_text("two");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        h.key(Key::Home, Modifiers::COMMAND);
        h.frames(90, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 0);

        let over_page = egui::Event::PointerMoved(h.app.last_page_rect.center());
        let swipe = |dx: f32| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(dx, 0.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        h.frames(1, vec![over_page], Modifiers::NONE);
        // Fingers to the left: forward, one page per swipe length.
        for _ in 0..2 {
            h.frames(1, vec![swipe(-40.0)], Modifiers::NONE);
        }
        h.frames(20, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 1);
        assert_eq!(h.app.doc.page_of(h.app.caret), 1, "the caret goes along to the page");
        // A pause, then fingers to the right: back again.
        h.frames(30, vec![], Modifiers::NONE);
        for _ in 0..2 {
            h.frames(1, vec![swipe(40.0)], Modifiers::NONE);
        }
        h.frames(20, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 0);
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
        assert_eq!(h.app.slide_in, None, "the new page has slid in and settled");

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
        assert_eq!(h.app.target, h.app.doc.pages() - 1, "moved to the page with the match");
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
        // ...and the page gets it back after Escape.
        h.key(Key::Escape, Modifiers::NONE);
        h.type_text("!"); // typed right after the note's end: not part of it
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line!");
        assert_eq!((h.app.doc.notes[0].start, h.app.doc.notes[0].end), (11, 22));
    }

    #[test]
    fn going_to_a_post_it_moves_the_pages_in_between_together() {
        let mut h = Harness::new();
        h.app.appearance = Appearance::Paperstack;
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..4 {
            h.type_text("page");
            h.key(Key::Enter, Modifiers::COMMAND);
        }
        h.frames(120, vec![], Modifiers::NONE);
        let start = h.app.doc.spans[1].start;
        h.app.doc.notes.push(model::Note { id: 7, start, end: start + 2, text: String::new(), color: 0 });
        let ctx = h.ctx.clone();
        h.app.go_to_note(&ctx, 7);
        assert_eq!(h.app.batch.map(|b| (b.0, b.1)), Some((4, 1)));
        assert_eq!((h.app.target, h.app.caret), (1, start));
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!((h.app.batch, h.app.pos), (None, 1.0));
    }

    #[test]
    fn post_its_follow_their_line_without_overlapping_or_leaving_the_page() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..60 {
            h.type_text("a line of text\n");
        }
        // Notes on five lines close together near the bottom of page 1, and one far down the text.
        let ctx = h.ctx.clone();
        let line = |k: usize| k * "a line of text\n".chars().count();
        for k in 30..35 {
            h.app.doc.notes.push(model::Note { id: k as u64, start: line(k), end: line(k) + 4, text: String::new(), color: 0 });
        }
        h.app.doc.notes.push(model::Note { id: 99, start: line(59), end: line(59) + 4, text: String::new(), color: 1 });
        let places = h.app.note_places(&ctx);
        let page_h = h.app.doc.setup.size().y;
        for (page, list) in &places {
            for &(k, y) in list {
                assert_eq!(*page, h.app.doc.page_of(h.app.doc.notes[k].start), "a post-it is on its line's page");
                assert!(y >= 0.0 && y + notes::NOTE_SIZE <= page_h + 0.01, "inside the page: {y}");
            }
            for w in list.windows(2) {
                assert!(w[1].1 >= w[0].1 + notes::NOTE_SIZE, "no overlap: {:?}", w);
            }
        }
        assert_eq!(places.values().map(Vec::len).sum::<usize>(), 6);
    }

    #[test]
    fn post_its_do_not_change_where_pages_break() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        for _ in 0..120 {
            h.type_text("a line of text\n");
        }
        h.frames(30, vec![], Modifiers::NONE);
        let before = h.app.doc.spans.clone();
        // Post-its near the bottom of the first page, one of them long enough for several sheets.
        let line = "a line of text\n".chars().count();
        let last_on_first = h.app.doc.spans[0].end / line - 1;
        for (id, k) in [(1, last_on_first - 1), (2, last_on_first)] {
            let text = if id == 1 { "x\n".repeat(30) } else { "short".into() };
            h.app.doc.notes.push(model::Note { id, start: k * line, end: k * line + 4, text, color: 0 });
        }
        h.app.target = 0;
        h.frames(120, vec![], Modifiers::NONE);
        h.app.set_setup(&h.ctx.clone(), h.app.doc.setup.clone()); // a full repagination
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.spans, before, "pages break at the same places with post-its on them");
        let ctx = h.ctx.clone();
        let layout = h.app.doc.layout_page(&ctx, 0, 1.0, &[]);
        let room = h.app.doc.setup.content_size().y;
        assert!(room - layout.height < 20.0, "the first page is filled down to its bottom margin: {} of {room}", layout.height);
    }

    #[test]
    fn a_long_note_continues_on_the_next_sheet_of_its_pad() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("a line");
        let ctx = h.ctx.clone();
        h.app.add_note(&ctx);
        h.frames(3, vec![], Modifiers::NONE);
        let id = h.app.doc.notes[0].id;
        for k in 0..20 {
            h.type_text(&format!("thought {k}\n"));
        }
        h.frames(30, vec![], Modifiers::NONE);
        let pad = h.app.pads[&id];
        assert!(pad.sheet >= 1, "the pad flipped on as the text grew: {}", pad.sheet);
        assert_eq!(pad.pos, pad.sheet as f32, "and the flip animation finished");
        assert_eq!(h.app.doc.visible_text(), "a line", "the page text is untouched");
    }

    #[test]
    fn zooming_back_to_the_fitting_size_leaves_the_page_where_it_is() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        let (fit_origin, fit_zoom) = (h.app.origin, h.app.zoom);
        // Zoom in and move the page off-centre...
        h.frames(1, vec![egui::Event::Zoom(1.5)], Modifiers::NONE);
        h.app.origin += egui::vec2(-150.0, 60.0);
        h.frames(2, vec![], Modifiers::NONE);
        // ...then zoom back out to the fitting size: the page stays off-centre.
        h.frames(1, vec![egui::Event::Zoom(1.0 / 1.5)], Modifiers::NONE);
        let there = h.app.origin;
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.zoom, fit_zoom, "the zoom stops at the fitting size");
        assert_eq!(h.app.origin, there, "nothing moves the page afterwards");
        assert!((there - fit_origin).length() > 50.0 && !h.app.fit);

        // Ctrl+0 (like the Fit button) centres it, gliding there rather than jumping.
        h.frames(1, vec![egui::Event::Key { key: Key::Num0, physical_key: Some(Key::Num0), pressed: true, repeat: false, modifiers: Modifiers::COMMAND }], Modifiers::COMMAND);
        let first = (h.app.origin - there).length();
        assert!(first > 0.0 && first < (fit_origin - there).length() * 0.5, "one frame moves only part of the way ({first})");
        h.frames(60, vec![], Modifiers::NONE);
        assert!((h.app.origin - fit_origin).length() < 0.01 && h.app.fit && !h.app.fit_settling);
    }

    #[test]
    fn the_cross_on_a_post_it_deletes_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("first line\nsecond line");
        let ctx = h.ctx.clone();
        h.app.add_note(&ctx);
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("keep me");
        h.app.add_note(&ctx);
        h.frames(3, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.notes.len(), 2);
        let doomed = h.app.doc.notes[1].id;

        let page = h.app.last_page_rect;
        let sc = h.app.scale_of(page);
        let (k, y) = h.app.note_places(&ctx)[&0].iter().copied().find(|&(k, _)| h.app.doc.notes[k].id == doomed).unwrap();
        assert_eq!(h.app.doc.notes[k].id, doomed);
        let r = notes::note_rect(page, sc, y);
        let pos = egui::pos2(r.right() - 7.0 * sc, r.top() + 7.0 * sc);
        h.frames(2, vec![egui::Event::PointerMoved(pos)], Modifiers::NONE);
        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        h.frames(1, vec![button(true)], Modifiers::NONE);
        h.frames(2, vec![button(false)], Modifiers::NONE);

        assert_eq!(h.app.doc.notes.len(), 1, "the clicked post-it is gone");
        assert_eq!(h.app.doc.notes[0].text, "keep me", "the other one stays");
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line");
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
    fn a_loop_drawn_with_the_pen_selects_the_text_inside_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("first line\nsecond line\nthird line");
        h.key(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
        assert!(h.app.pen);

        // A loop around the second line (chars 11..22), drawn in screen points.
        let ctx = h.ctx.clone();
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        let (l, r) = (layout.caret_rect(11, true), layout.caret_rect(22, true));
        let (x0, x1, y0, y1) = (l.left() - 4.0, r.right() + 4.0, l.top() + 1.0, l.bottom() - 1.0);
        let rect = h.app.last_page_rect;
        let sc = h.app.scale_of(rect);
        let at = |x: f32, y: f32| rect.min + h.app.doc.setup.margin_origin() * sc + egui::vec2(x, y) * sc;
        let corners = [at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1), at(x0, y0)];
        let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        h.frames(1, vec![egui::Event::PointerMoved(corners[0])], Modifiers::NONE);
        h.frames(1, vec![button(corners[0], true)], Modifiers::NONE);
        for w in corners.windows(2) {
            for k in 1..=8 {
                h.frames(1, vec![egui::Event::PointerMoved(w[0] + (w[1] - w[0]) * k as f32 / 8.0)], Modifiers::NONE);
            }
        }
        h.frames(1, vec![button(corners[4], false)], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.app.selection(), (11, 22), "exactly 'second line'");
        assert!(h.app.lasso.as_ref().is_some_and(|l| l.caught.is_some()), "the command menu is open");

        // A corrected version replaces the passage in one undo step.
        h.app.lasso = None;
        h.app.fake_answer(11, 22, claude::Command::Grammar, "second line, fixed");
        h.frames(2, vec![], Modifiers::NONE);
        assert!(h.app.apply_answer(&ctx));
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line, fixed\nthird line");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.app.doc.visible_text(), "first line\nsecond line\nthird line");

        // Once the passage has been edited, the answer no longer replaces it.
        h.app.fake_answer(11, 22, claude::Command::Grammar, "other");
        h.frames(2, vec![], Modifiers::NONE);
        h.app.toggle_pen();
        h.click_in_page(egui::vec2(l.left() + 0.5, l.center().y));
        h.type_text("my ");
        assert!(!h.app.apply_answer(&ctx));
    }

    #[test]
    fn writing_never_starts_a_scene_by_itself() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.app.backdrop.panel = true;
        h.app.backdrop.description = "A storm at sea".into();
        h.type_text("The ship sailed on. The storm grew. ");
        h.frames(120, vec![], Modifiers::NONE);
        assert!(!h.app.backdrop.drawing(), "only the Draw button starts a picture");
    }

    #[test]
    fn following_the_writing_queues_finished_sentences_only() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.app.set_scene_mode(backdrop::Mode::Writing);
        h.type_text("The valley lies between the mountains and the sea");
        assert_eq!(h.app.backdrop.pending(), None, "a sentence still being written is not drawn");
        h.type_text(". Above it");
        assert_eq!(h.app.backdrop.pending(), Some("The valley lies between the mountains and the sea."));
        // Moving the caret is no edit, so the queued scene stays as it was.
        h.key(Key::Home, Modifiers::COMMAND);
        assert_eq!(h.app.backdrop.pending(), Some("The valley lies between the mountains and the sea."));
        // Back to describing: nothing is queued or drawn any more.
        h.app.set_scene_mode(backdrop::Mode::Described);
        assert_eq!(h.app.backdrop.pending(), None);
        assert!(!h.app.backdrop.drawing());
    }

    #[test]
    fn the_scene_is_saved_with_the_document_and_comes_back() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("A storm at sea.");
        let ctx = h.ctx.clone();
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 60 85\"><circle cx=\"30\" cy=\"40\" r=\"20\" fill=\"#36c\"/></svg>";
        let scene = backdrop::SceneFile { svg: Some(svg.into()), description: "A ship in a storm".into(), hidden: false };
        h.app.open_scene(&ctx, Some(scene.clone())).unwrap();

        let dir = std::env::temp_dir().join(format!("caprice-scene-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("story.caprice");
        std::fs::write(&path, serde_json::to_string(&h.app.doc_file()).unwrap()).unwrap();

        let mut other = Harness::new();
        other.frames(3, vec![], Modifiers::NONE);
        let octx = other.ctx.clone();
        other.app.open_path(&octx, path.clone());
        assert_eq!(other.app.doc.visible_text(), "A storm at sea.");
        assert_eq!(other.app.backdrop.to_file(), Some(scene), "picture and description are back");

        // A document without a scene clears the one shown before.
        std::fs::write(&path, serde_json::to_string(&fileio::DocFile::from_doc(&other.app.doc)).unwrap()).unwrap();
        other.app.open_path(&octx, path);
        assert_eq!(other.app.backdrop.to_file(), None);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn opening_with_unsaved_changes_asks_first() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        assert!(!h.app.unsaved(), "a fresh document has nothing to save");
        h.type_text("Draft");
        assert!(h.app.unsaved());

        h.key(Key::O, Modifiers::COMMAND);
        assert_eq!(h.app.leaving, Some(fileio::Leaving::Open), "asks instead of opening");
        assert!(h.app.dialog.is_none(), "no file dialog yet");
        h.type_text("x");
        assert_eq!(h.text(), "Draft", "the page takes no typing while asking");
        h.key(Key::Escape, Modifiers::NONE);
        assert_eq!(h.app.leaving, None, "Escape cancels");

        // With the keyboard on "Don't save", Enter doesn't save: it closes without saving.
        h.app.leaving = Some(fileio::Leaving::Close);
        h.frames(2, vec![], Modifiers::NONE);
        h.key(Key::Tab, Modifiers::NONE);
        h.key(Key::Tab, Modifiers::NONE);
        h.key(Key::Enter, Modifiers::NONE);
        assert_eq!(h.app.leaving, None);
        assert!(h.app.may_close && h.app.path.is_none() && h.app.dialog.is_none(), "closed without saving");
        h.app.may_close = false;

        let dir = std::env::temp_dir().join(format!("caprice-unsaved-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        h.app.save_to(dir.join("draft.caprice"));
        assert!(!h.app.unsaved(), "saved");
        h.frames(90, vec![], Modifiers::NONE); // a pause, so the next edit undoes on its own
        h.type_text("!");
        assert!(h.app.unsaved(), "changed again");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.text(), "Draft");
        assert!(!h.app.unsaved(), "undone back to what is saved");
        std::fs::remove_dir_all(dir).unwrap();
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
    fn copy_and_paste_keeps_the_formatting() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("plain ");
        h.key(Key::B, Modifiers::COMMAND);
        h.type_text("bold");
        h.key(Key::B, Modifiers::COMMAND);
        h.key(Key::E, Modifiers::COMMAND);
        h.key(Key::A, Modifiers::COMMAND);
        h.frames(1, vec![egui::Event::Copy], Modifiers::NONE);
        h.key(Key::End, Modifiers::COMMAND);
        h.type_text("\n");
        h.frames(1, vec![egui::Event::Paste("plain bold".into())], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.text(), "plain bold\nplain bold");
        let bold: String = h.text().chars().zip(&h.app.doc.flow.styles).filter(|(_, s)| s.bold).map(|(c, _)| c).collect();
        assert_eq!(bold, "boldbold", "the pasted copy is bold where the original was");
        assert_eq!(h.app.doc.para_attrs_at(12).align, model::Align::Center);
        // Text copied elsewhere is pasted plain, in the style typing would have.
        h.frames(1, vec![egui::Event::Paste(" other".into())], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        assert!(h.app.doc.flow.styles[h.text().len() - 5..h.text().len()].iter().all(|s| s.bold), "continues the bold before the caret");
    }

    #[test]
    fn a_copied_picture_is_pasted_as_a_picture_of_its_own() {
        let mut h = Harness::new();
        let ctx = h.ctx.clone();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("ab");
        h.app.insert_image_bytes(&ctx, images::test_png(20, 10, [1, 2, 3])).unwrap();
        h.frames(2, vec![], Modifiers::NONE);
        let pic = h.text().chars().position(|c| c == model::IMAGE_CHAR).unwrap();
        h.app.anchor = pic;
        h.app.set_caret(&ctx, pic + 1, true);
        h.frames(1, vec![egui::Event::Copy], Modifiers::NONE);
        h.app.set_caret(&ctx, 1, false);
        let pasted = h.app.clip.as_ref().unwrap().plain.clone();
        h.frames(1, vec![egui::Event::Paste(pasted)], Modifiers::NONE);
        h.frames(2, vec![], Modifiers::NONE);
        let p = model::IMAGE_CHAR;
        assert_eq!(h.text(), format!("a\n{p}\nb\n{p}\n"));
        assert_eq!(h.app.doc.images.len(), 2);
        let ids: Vec<u32> = h.app.doc.flow.styles.iter().map(|s| s.image).filter(|&i| i != 0).collect();
        assert_eq!(ids, vec![2, 1], "the pasted picture is a copy with its own id");
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

    // -------------------------------------------------------------- pictures

    #[test]
    fn inserting_a_picture_gives_it_a_paragraph_of_its_own() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("before after");
        for _ in 0..5 {
            h.key(Key::ArrowLeft, Modifiers::NONE);
        }
        let ctx = h.ctx.clone();
        h.app.insert_image_bytes(&ctx, images::test_png(200, 100, [10, 120, 200])).unwrap();
        h.frames(3, vec![], Modifiers::NONE);
        assert_eq!(h.text(), format!("before \n{}\nafter", model::IMAGE_CHAR));
        assert_eq!(h.app.doc.images.len(), 1);
        // The picture's paragraph is as tall as the picture; the caret lands after it, in the rest of the text.
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        assert_eq!(layout.paras.len(), 3);
        let pic = layout.paras[1].image.expect("the middle paragraph is the picture");
        assert!((pic.rect.width() - 150.0).abs() < 0.1, "200px at 96dpi is 150pt: {:?}", pic.rect);
        assert!((layout.paras[1].height - 75.0).abs() < 0.1);
        assert_eq!(h.app.caret, h.text().chars().count() - 5, "caret is at the start of 'after'");
        // Typing after a picture is text, not another picture. (Wait, so undo treats it as a separate step.)
        h.frames(90, vec![], Modifiers::NONE);
        h.type_text("X");
        h.frames(90, vec![], Modifiers::NONE);
        assert!(h.app.doc.flow.styles.iter().filter(|s| s.image != 0).count() == 1);
        // One Backspace chain removes the picture, and undo brings it back.
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::Backspace, Modifiers::NONE); // joins 'after' to the picture's paragraph
        h.key(Key::Backspace, Modifiers::NONE); // deletes the picture
        assert!(!h.text().contains(model::IMAGE_CHAR));
        h.key(Key::Z, Modifiers::COMMAND);
        h.key(Key::Z, Modifiers::COMMAND);
        assert!(h.text().contains(model::IMAGE_CHAR), "undo restores the picture: {:?}", h.text());
    }

    #[test]
    fn clicking_a_picture_selects_it_and_it_can_be_resized() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.app.insert_image_bytes(&ctx, images::test_png(100, 100, [0, 0, 0])).unwrap();
        h.frames(3, vec![], Modifiers::NONE);
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        let pic = layout.paras[0].image.unwrap().rect;
        h.click_in_page(pic.center().to_vec2());
        assert_eq!(h.app.selection(), (0, 1), "the picture's placeholder char is selected");
        assert!(h.app.selected_image().is_some());
        h.app.set_image_width_fraction(&ctx, 0.5);
        let half = h.app.doc.setup.content_size().x * 0.5;
        assert!((h.app.doc.images[0].width_pt - half).abs() < 0.1);
    }

    #[test]
    fn a_picture_that_does_not_fit_moves_to_the_next_page_as_a_whole() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        for _ in 0..40 {
            h.type_text("line\n");
        }
        h.app.insert_image_bytes(&ctx, images::test_png(300, 300, [0, 0, 0])).unwrap(); // 225pt tall
        h.frames(30, vec![], Modifiers::NONE);
        assert!(h.app.doc.pages() >= 2);
        let pic_page = h.app.doc.page_of(h.text().chars().position(|c| c == model::IMAGE_CHAR).unwrap());
        let layout = h.app.page_layout(&ctx, pic_page, 1.0);
        let pic = layout.paras.iter().find(|p| p.image.is_some()).expect("the picture is whole on its page");
        assert!(pic.y + pic.height <= h.app.doc.setup.content_size().y + 0.6);
    }

    #[test]
    fn pictures_survive_saving_and_loading() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        let ctx = h.ctx.clone();
        let png = images::test_png(64, 32, [5, 6, 7]);
        h.app.insert_image_bytes(&ctx, png.clone()).unwrap();
        let json = serde_json::to_string(&fileio::DocFile::from_doc(&h.app.doc)).unwrap();
        let back = serde_json::from_str::<fileio::DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.images.len(), 1);
        assert_eq!(back.images[0].bytes, png);
        assert_eq!(back.images[0].px, (64, 32));
        assert_eq!(back.flow.text, h.app.doc.flow.text);
        assert_eq!(back.flow.styles, h.app.doc.flow.styles);
    }

    /// Writes a sample document with a picture, for looking at in the real app:
    /// `CAPRICE_SAMPLE_OUT=/some/file.caprice cargo test sample_caprice`
    #[test]
    fn sample_caprice() {
        let Some(out) = std::env::var_os("CAPRICE_SAMPLE_OUT") else { return };
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("A document with a picture");
        h.key(Key::E, Modifiers::COMMAND);
        h.key(Key::Enter, Modifiers::NONE);
        h.key(Key::L, Modifiers::COMMAND);
        h.type_text("Text before the picture, and then the picture itself, centered below:");
        h.key(Key::Enter, Modifiers::NONE);
        let ctx = h.ctx.clone();
        // A gradient so that scaling and orientation are visible.
        let img = image::RgbImage::from_fn(240, 120, |x, y| image::Rgb([(x as f32 / 240.0 * 255.0) as u8, (y as f32 / 120.0 * 255.0) as u8, 150]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        h.app.insert_image_bytes(&ctx, png.into_inner()).unwrap();
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.key(Key::E, Modifiers::COMMAND);
        h.key(Key::ArrowDown, Modifiers::NONE);
        h.type_text("Text after the picture, which continues on the same page.");
        let docx = std::path::PathBuf::from(&out).with_extension("docx");
        std::fs::write(docx, export::to_docx(&h.app.doc).unwrap()).unwrap();
        std::fs::write(out, serde_json::to_string_pretty(&fileio::DocFile::from_doc(&h.app.doc)).unwrap()).unwrap();
    }

    // ------------------------------------------- picture handles, menu, moving

    impl Harness {
        /// Press at one point of the writing area, drag to another, release.
        fn drag_in_page(&mut self, from: egui::Vec2, to: egui::Vec2) {
            let rect = self.app.last_page_rect;
            let sc = self.app.scale_of(rect);
            let abs = |h: &Harness, v: egui::Vec2| rect.min + h.app.doc.setup.margin_origin() * sc + v * sc;
            let (a, b) = (abs(self, from), abs(self, to));
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frames(1, vec![egui::Event::PointerMoved(a)], Modifiers::NONE);
            self.frames(1, vec![button(a, true)], Modifiers::NONE);
            for k in 1..=6 {
                let p = a + (b - a) * (k as f32 / 6.0);
                self.frames(1, vec![egui::Event::PointerMoved(p)], Modifiers::NONE);
            }
            self.frames(1, vec![button(b, false)], Modifiers::NONE);
            self.frames(3, vec![], Modifiers::NONE);
        }

        fn picture_rect(&self) -> egui::Rect {
            let ctx = self.ctx.clone();
            let layout = self.app.page_layout(&ctx, 0, 1.0);
            let p = layout.paras.iter().find(|p| p.image.is_some()).expect("a picture on page 1");
            p.image.unwrap().rect.translate(egui::vec2(0.0, p.y))
        }

        fn with_picture_between_text(&mut self) {
            self.frames(3, vec![], Modifiers::NONE);
            self.type_text("top text");
            self.key(Key::Enter, Modifiers::NONE);
            let ctx = self.ctx.clone();
            self.app.insert_image_bytes(&ctx, images::test_png(200, 100, [9, 99, 199])).unwrap();
            self.type_text("bottom text");
            self.frames(3, vec![], Modifiers::NONE);
        }
    }

    #[test]
    fn rotating_swaps_the_picture_sides_and_exports_turned_pixels() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        let ctx = h.ctx.clone();
        let before = h.picture_rect();
        let (w0, h0) = (before.width(), before.height());
        h.click_in_page(before.center().to_vec2());
        h.app.do_picture_action(&ctx, images::PicAction::RotateRight);
        let after = h.picture_rect();
        assert!((after.width() - w0).abs() < 0.1, "the width setting stays");
        assert!((after.height() - w0 * (w0 / h0)).abs() < 0.5 || after.height() > h0, "taller than wide now: {after:?}");
        assert_eq!(h.app.doc.images[0].rotation, 1);
        // Four right turns come full circle.
        for _ in 0..3 {
            h.app.do_picture_action(&ctx, images::PicAction::RotateRight);
        }
        assert_eq!(h.app.doc.images[0].rotation, 0);
        h.app.do_picture_action(&ctx, images::PicAction::RotateLeft);
        assert_eq!(h.app.doc.images[0].rotation, 3);
        // The file keeps the rotation and the original pixels.
        let json = serde_json::to_string(&fileio::DocFile::from_doc(&h.app.doc)).unwrap();
        let back = serde_json::from_str::<fileio::DocFile>(&json).unwrap().into_doc();
        assert_eq!(back.images[0].rotation, 3);
        assert_eq!(back.images[0].bytes, h.app.doc.images[0].bytes);
        // The Word export turns the pixels: 200x100 becomes 100x200.
        let (bytes, ext) = images::export_media(&h.app.doc.images[0]);
        assert_eq!(ext, "png");
        assert_eq!(images::describe(&bytes).unwrap().1, (100, 200));
    }

    #[test]
    fn rotated_uvs_put_the_texture_corners_where_they_belong() {
        let c = images::rotated_uv;
        assert_eq!((c(0.0, 0.0, 0), c(1.0, 0.0, 0)), (egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0)));
        // Turned right once, the top-left of the screen shows the texture's bottom-left.
        assert_eq!(c(0.0, 0.0, 1), egui::pos2(0.0, 1.0));
        assert_eq!(c(1.0, 0.0, 1), egui::pos2(0.0, 0.0));
        assert_eq!(c(0.0, 0.0, 2), egui::pos2(1.0, 1.0));
        assert_eq!(c(0.0, 0.0, 3), egui::pos2(1.0, 0.0));
    }

    #[test]
    fn handles_resize_keeping_the_proportions() {
        use images::Handle;
        let se = Handle { kx: 1, ky: 1 };
        assert_eq!(se.new_width(100.0, 0.5, egui::vec2(20.0, 3.0)), 120.0, "dragging right grows it");
        assert_eq!(se.new_width(100.0, 0.5, egui::vec2(1.0, 20.0)), 140.0, "dragging down counts via the aspect ratio");
        let nw = Handle { kx: -1, ky: -1 };
        assert_eq!(nw.new_width(100.0, 0.5, egui::vec2(-20.0, 0.0)), 120.0, "the left handle grows it when dragged left");
        let n = Handle { kx: 0, ky: -1 };
        assert_eq!(n.new_width(100.0, 0.5, egui::vec2(50.0, -10.0)), 120.0, "side handles ignore the other axis");
    }

    #[test]
    fn dragging_a_corner_handle_resizes_the_selected_picture() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        let pic = h.picture_rect();
        h.click_in_page(pic.center().to_vec2()); // select it
        assert!(h.app.picture_paragraph().is_some());
        let width_before = h.app.doc.images[0].width_pt;
        let corner = pic.right_bottom();
        h.drag_in_page(corner.to_vec2(), corner.to_vec2() + egui::vec2(-60.0, -30.0));
        let after = h.app.doc.images[0].width_pt;
        assert!(after < width_before - 40.0, "dragging the bottom-right corner inwards shrinks it: {width_before} -> {after}");
        let rect = h.picture_rect();
        assert!((rect.height() / rect.width() - 0.5).abs() < 0.01, "proportions are kept");
        assert_eq!(h.app.doc.visible_text().matches(model::IMAGE_CHAR).count(), 1, "resizing did not turn into a move or a selection change");
    }

    #[test]
    fn right_click_opens_the_picture_menu_only_on_a_picture() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        let pic = h.picture_rect();
        let rect = h.app.last_page_rect;
        let sc = h.app.scale_of(rect);
        let origin = rect.min + h.app.doc.setup.margin_origin() * sc;
        let abs = |v: egui::Vec2| origin + v * sc;
        let secondary = |pos| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        let on_text = abs(egui::vec2(10.0, 5.0));
        h.frames(1, vec![egui::Event::PointerMoved(on_text)], Modifiers::NONE);
        h.frames(1, vec![secondary(on_text)], Modifiers::NONE);
        assert!(h.app.pic_menu.is_none());
        let on_pic = abs(pic.center().to_vec2());
        h.frames(1, vec![egui::Event::PointerMoved(on_pic)], Modifiers::NONE);
        h.frames(1, vec![secondary(on_pic)], Modifiers::NONE);
        assert!(h.app.pic_menu.is_some());
        assert!(h.app.picture_paragraph().is_some(), "right-clicking selects the picture");
    }

    #[test]
    fn dragging_a_picture_moves_it_between_paragraphs_as_one_undo_step() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        assert_eq!(h.text(), format!("top text\n{}\nbottom text", model::IMAGE_CHAR));
        let pic = h.picture_rect();
        // Drag it up above the first line.
        h.drag_in_page(pic.center().to_vec2(), egui::vec2(60.0, 0.5));
        assert_eq!(h.text(), format!("{}\ntop text\nbottom text", model::IMAGE_CHAR), "the picture is now first");
        assert!(h.app.picture_paragraph().is_some(), "and still selected");
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.text(), format!("top text\n{}\nbottom text", model::IMAGE_CHAR), "one undo puts it back");
        h.key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT);
        assert!(h.text().starts_with(model::IMAGE_CHAR), "and redo moves it again");
    }

    #[test]
    fn alt_arrows_nudge_the_selected_picture_by_a_paragraph() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        let pic = h.picture_rect();
        h.click_in_page(pic.center().to_vec2());
        h.key(Key::ArrowUp, Modifiers::ALT);
        assert!(h.text().starts_with(model::IMAGE_CHAR), "moved above 'top text': {:?}", h.text());
        h.key(Key::ArrowDown, Modifiers::ALT);
        assert_eq!(h.text(), format!("top text\n{}\nbottom text", model::IMAGE_CHAR));
    }

    #[test]
    fn deleting_a_selected_picture_removes_its_paragraph_too() {
        let mut h = Harness::new();
        h.with_picture_between_text();
        let pic = h.picture_rect();
        h.click_in_page(pic.center().to_vec2());
        h.key(Key::Delete, Modifiers::NONE);
        assert_eq!(h.text(), "top text\nbottom text");
    }
}
