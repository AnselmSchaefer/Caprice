mod backdrop;
mod book;
mod cast;
mod claude;
mod contents;
mod dropcap;
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
mod scratchpad;
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
    /// A message to show briefly ("- saved"); the toast takes it from here when it shows it.
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
    /// The status message being shown (taken from `status`), when it appeared, and the window title last set.
    pub shown_status: String,
    /// Right-click menu of a picture (where it was opened), a picture being dragged, a resize in progress.
    pub pic_menu: Option<Pos2>,
    /// The menu opened this very frame (so the click that opened it is not a click outside).
    pub pic_menu_fresh: bool,
    pub pic_drag: Option<images::PicDrag>,
    /// A selection being dragged out with the mouse, which can carry on over other pages.
    pub sel_drag: Option<editor::SelDrag>,
    pub resize: Option<images::ResizeDrag>,
    pub status_at: f64,
    pub title: String,
    /// The pen is on: dragging on the page draws a loop around text to ask Claude about.
    pub pen: bool,
    /// How passages are picked with it on: circled with the pen, or selected with the cursor.
    pub picking: claude::Picking,
    pub lasso: Option<claude::Lasso>,
    /// Claude's answer being shown, if any.
    pub answer: Option<claude::Answer>,
    /// The story notes window is open.
    pub story_notes_open: bool,
    /// The scratchpad shows, on the right.
    pub scratchpad_open: bool,
    /// The scene Claude paints behind the pages, and the panel to describe it.
    pub backdrop: backdrop::Backdrop,
    /// The cast window, and the model sheet being drawn.
    pub cast_panel: cast::CastPanel,
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
            sel_drag: None,
            resize: None,
            status_at: f64::NEG_INFINITY,
            title: String::new(),
            pen: false,
            picking: claude::Picking::Circle,
            lasso: None,
            answer: None,
            story_notes_open: false,
            scratchpad_open: false,
            backdrop: backdrop::Backdrop::default(),
            cast_panel: cast::CastPanel::default(),
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
        // Taken out of `status`, so the same message set again ("saved" twice) shows again.
        if !self.status.is_empty() {
            self.shown_status = std::mem::take(&mut self.status);
            self.status_at = now;
        }
        let age = now - self.status_at;
        const SHOW_FOR: f64 = 4.0;
        if !self.shown_status.is_empty() && age < SHOW_FOR {
            let text = self.shown_status.trim_start_matches(['-', ' ']).to_owned();
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
            self.turn_to(ctx, to, false);
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
            self.batch_to(|app| app.turn_to(ctx, to, false));
        }
    }

    /// Put the caret at `c`, the pages between here and its page moving there together, on from
    /// where the scrollbar holds them, if it does.
    pub fn batch_to_caret(&mut self, ctx: &egui::Context, c: usize) {
        self.batch_to(|app| app.set_caret(ctx, c, false));
    }

    /// Flip pages by `go`, the pages between here and there moving together.
    fn batch_to(&mut self, go: impl FnOnce(&mut Self)) {
        let from = self.target;
        go(self);
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
        // In a book, as unhurried as a new page arriving, since a page turns back with it.
        let speed = if self.appearance == Appearance::Book { book::NEW_PAGE_PACE } else { ((1.0 - s) * 5.0).clamp(2.2, 14.0) };
        let s = s + speed * dt;
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
        if self.slides_in(base as usize) {
            // A new page is something to watch arrive; several added quickly still come in about as long.
            speed = book::NEW_PAGE_PACE * diff.abs().ceil();
        } else if self.pos - base >= book::TURN {
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
            } else if let (Some(s), Appearance::Book) = (self.slide_out, self.appearance) {
                // In a book the page before lies turned over in the pile, so it turns back over the
                // page that slides out, its post-its with it; it is editable once it lies flat.
                let to_right = area.right() + 40.0 - page_rect.left();
                self.book_slide_out(ui, page_rect, i, s, to_right);
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
                ui.painter().extend(self.backdrop_shapes(&ctx, i, page_rect, &|p| p, 1.0, 1.0));
                self.draw_footer(ui, page_rect, i);
                self.editor_surface(ui, page_rect, i, !self.circling());
                self.draw_notes(ui, page_rect, i);
                self.pen_surface(ui, page_rect, i);
                if let Some(s) = self.slide_out {
                    let to_right = area.right() + 40.0 - page_rect.left();
                    let r = page_rect.translate(egui::vec2(to_right * ease(s), 0.0));
                    Self::paper(ui.painter(), r);
                    ui.painter().extend(self.backdrop_shapes(&ctx, i, r, &|p| p, 1.0, 1.0));
                }
            }
        } else if self.slides_in(base) && self.appearance == Appearance::Book {
            // A new page comes in from beyond the right edge of the window while the old one turns
            // over onto the pile, taking its post-its with it.
            let from_right = area.right() + 40.0 - page_rect.left();
            self.book_slide_in(ui, page_rect, base, t, from_right);
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
        self.story_notes_window(&ctx);
        self.scratchpad(&ctx);
        self.scene_panel(&ctx);
        self.scene_list(&ctx);
        self.cast_window(&ctx);
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
        /// The widgets of the last frame as a screen reader sees them, once `click_widget` asked for them.
        access: Option<egui::accesskit::TreeUpdate>,
        /// What the last frame painted.
        painted: Vec<egui::epaint::ClippedShape>,
    }

    impl Harness {
        fn new() -> Self {
            let ctx = egui::Context::default();
            apply_theme(&ctx);
            let app = App::new(&ctx);
            Self { ctx, app, time: 0.0, mods: Modifiers::NONE, access: None, painted: Vec::new() }
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
                self.access = out.platform_output.accesskit_update.take().or(self.access.take());
                self.painted = std::mem::take(&mut out.shapes);
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
    fn enter_after_a_chapter_title_goes_on_in_ordinary_text() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("Chapter One");
        let ctx = h.ctx.clone();
        h.app.set_para(&ctx, |p| p.set_chapter_title(true));
        h.key(Key::Enter, Modifiers::NONE);
        h.type_text("It was dark.");
        let d = &h.app.doc;
        assert_eq!(d.flow.text, "Chapter One\nIt was dark.\n");
        assert!(d.para_attrs_at(0).is_chapter_title() && d.para_attrs_at(0).align == model::Align::Center);
        assert_eq!(d.para_attrs_at(12), model::ParaAttrs::default());
        // One undo takes the text back, the next the paragraph break, leaving the title as it was.
        h.key(Key::Z, Modifiers::COMMAND);
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.app.doc.flow.text, "Chapter One\n");
        assert!(h.app.doc.para_attrs_at(0).is_chapter_title());

        // Enter inside a title splits it into two titles, like any other paragraph format.
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::Enter, Modifiers::NONE);
        assert!(h.app.doc.para_attrs_at(0).is_chapter_title() && h.app.doc.para_attrs_at(12).is_chapter_title());
    }

    #[test]
    fn up_and_down_from_the_start_of_a_wrapped_line_move_one_line() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let ctx = h.ctx.clone();
        let rows = h.app.doc.layout_page(&ctx, 0, 1.0, &[]).rows();
        let starts: Vec<usize> = rows.iter().filter(|r| r.para == rows[0].para).map(|r| r.start).take(4).collect();
        assert_eq!(starts.len(), 4, "the first paragraph wraps");
        // The caret at the start of the second line, where the first one wraps.
        h.app.set_caret(&ctx, starts[1], false);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.caret, starts[2], "down to the third line, not stuck");
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.app.caret, starts[3]);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.caret, starts[2], "up one line, not two");
        h.key(Key::ArrowUp, Modifiers::NONE);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(h.app.caret, starts[0]);
        // End on the line the caret starts goes to that line's end.
        h.app.set_caret(&ctx, starts[1], false);
        h.key(Key::End, Modifiers::NONE);
        assert!(h.app.caret > starts[1] && h.app.caret <= starts[2], "{} in {:?}", h.app.caret, starts);
    }

    #[test]
    fn arrow_keys_treat_a_drop_cap_and_the_line_beside_it_as_one_line() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("Chapter One");
        let ctx = h.ctx.clone();
        h.app.set_para(&ctx, |p| p.set_chapter_title(true));
        h.key(Key::Enter, Modifiers::NONE);
        h.type_text(&"It was nearly midnight and the Prime Minister sat alone in his office. ".repeat(6));
        let setup = model::PageSetup { drop_cap_lines: 3, ..h.app.doc.setup.clone() };
        h.app.set_setup(&ctx, setup);
        let layout = h.app.doc.layout_page(&ctx, 0, 1.0, &[]);
        let beside = layout.paras.iter().find(|p| p.beside).expect("the chapter has a drop cap");
        let rows: Vec<(usize, usize)> = layout.rows().iter().filter(|r| r.para == 2).map(|r| (r.start, r.end)).collect();
        let line = |c: usize| rows.iter().position(|&(s, e)| (s..e).contains(&c));

        // From the middle of the second line beside the cap: up to the first, then to the title.
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, rows[1].0 + 3, false);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(line(h.app.caret), Some(0), "the first line beside the cap: {}", h.app.caret);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert!(h.app.caret < 12, "the title: {}", h.app.caret);
        // And down again, past the cap, line by line.
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert!(h.app.caret == 12 || line(h.app.caret) == Some(0), "the cap's line: {}", h.app.caret);
        h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(line(h.app.caret), Some(1), "{}", h.app.caret);
        assert!(beside.galley.rows.len() == 3);
    }

    #[test]
    fn the_contents_pages_go_before_the_story_and_hold_none_of_its_text() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("Chapter One");
        let ctx = h.ctx.clone();
        h.app.set_para(&ctx, |p| p.set_chapter_title(true));
        h.key(Key::Enter, Modifiers::NONE);
        h.type_text("It was dark.");
        h.app.set_caret(&ctx, 3, false);
        h.app.set_contents(&ctx, true);
        assert_eq!(h.app.doc.flow.text, "Chapter One\nIt was dark.\n", "the text is unchanged");
        assert_eq!((h.app.doc.pages(), h.app.target, h.app.caret), (2, 0, 3), "shown, the caret left where it was");
        let layout = h.app.doc.layout_page(&ctx, 0, 1.0, &[]);
        assert_eq!(layout.paras[0].contents.as_ref().unwrap().links[0].1, 1, "Chapter One is on page 2");

        // Typing while they are shown writes at the caret, and shows it.
        h.type_text("x");
        assert_eq!((h.app.doc.flow.text.as_str(), h.app.target), ("Chaxpter One\nIt was dark.\n", 1));
        // Backspace at the start of the story, or deleting all of it, leaves the contents alone.
        h.app.set_caret(&ctx, 0, false);
        h.key(Key::Backspace, Modifiers::NONE);
        h.key(Key::A, Modifiers::COMMAND);
        h.key(Key::Delete, Modifiers::NONE);
        assert_eq!((h.app.doc.flow.text.as_str(), h.app.doc.pages()), ("\n", 2));
        // Undo is for the text: it all comes back, the contents staying.
        while h.app.doc.flow.text != "Chapter One\nIt was dark.\n" {
            h.key(Key::Z, Modifiers::COMMAND);
        }
        assert!(h.app.doc.setup.contents);
        h.app.set_contents(&ctx, false);
        assert_eq!((h.app.doc.pages(), h.app.target), (1, 0));
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
        assert!(near(h.app.held, 0.0), "{}", h.app.held);
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
        assert_eq!(h.app.doc.page_of(h.app.caret), 0, "the caret stays where it was");
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

    impl Harness {
        /// A story of `chapters` one-line chapters after a contents page, the caret at the start
        /// and the first page shown. Returns how many pages the contents take up.
        fn long_contents(&mut self, chapters: usize) -> usize {
            self.frames(3, vec![], Modifiers::NONE);
            let st = self.app.typing.clone();
            let mut title = model::ParaAttrs::default();
            title.set_chapter_title(true);
            let mut flow = model::Flow { text: String::new(), styles: Vec::new() };
            for n in 1..=chapters {
                let line = format!("Chapter {n}");
                flow.styles.extend(std::iter::repeat_n(st.clone(), line.chars().count()));
                flow.styles.push(st.with_para(title));
                flow.text.push_str(&line);
                flow.text.push('\n');
            }
            self.app.doc.flow = flow;
            let ctx = self.ctx.clone();
            self.app.doc.full_paginate(&ctx, &st);
            self.app.set_caret(&ctx, 0, false);
            self.app.set_contents(&ctx, true);
            self.frames(90, vec![], Modifiers::NONE);
            assert_eq!(self.app.target, 0);
            self.app.doc.contents_pages(&ctx)
        }

        /// The page shown, once the pages have settled.
        fn shown(&mut self) -> usize {
            self.frames(90, vec![], Modifiers::NONE);
            let target = self.app.target;
            assert_eq!(self.app.pos, target as f32, "the pages settled on the page shown");
            target
        }

        /// Drag the scrollbar's thumb to page `to` of `of` and let go.
        fn scrub_to(&mut self, to: usize, of: usize) {
            let (y, x0, x1) = self.app.last_track;
            let at = |page: usize| egui::pos2(x0 + (x1 - x0) * page as f32 / (of - 1) as f32, y);
            let from = at(self.app.target);
            let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
            self.frames(1, vec![egui::Event::PointerMoved(from), button(from, true)], Modifiers::NONE);
            self.frames(1, vec![egui::Event::PointerMoved(at(to))], Modifiers::NONE);
            self.frames(30, vec![], Modifiers::NONE);
            self.frames(1, vec![button(at(to), false)], Modifiers::NONE);
        }
    }

    #[test]
    fn page_keys_go_through_every_page_of_long_contents() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        assert!(n >= 3, "{n}");
        // Page Down shows each of them in turn, then the story; Page Up goes back the same way.
        for k in 1..=n {
            h.key(Key::PageDown, Modifiers::NONE);
            assert_eq!(h.app.target, k);
        }
        assert_eq!(h.shown(), n, "the story, with the caret in it");
        for k in (0..n).rev() {
            h.key(Key::PageUp, Modifiers::NONE);
            assert_eq!(h.app.target, k);
        }
    }

    #[test]
    fn swiping_goes_through_every_page_of_long_contents() {
        for look in [Appearance::Book, Appearance::Paperstack] {
            let mut h = Harness::new();
            h.app.appearance = look;
            let n = h.long_contents(120);
            let swipe = |dx| egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(dx, 0.0), modifiers: Modifiers::NONE, phase: egui::TouchPhase::Move };
            h.frames(1, vec![egui::Event::PointerMoved(h.app.last_page_rect.center())], Modifiers::NONE);
            for k in (1..=n).chain((0..n).rev()) {
                h.frames(1, vec![swipe(if k > h.app.target { -60.0 } else { 60.0 })], Modifiers::NONE);
                h.frames(20, vec![], Modifiers::NONE);
                assert_eq!(h.app.target, k, "{look:?}: swiped to page {k}");
                h.frames(30, vec![], Modifiers::NONE); // a pause: the next swipe is a new one
            }
        }
    }

    #[test]
    fn the_scrollbar_reaches_every_page_of_long_contents() {
        for look in [Appearance::Book, Appearance::Paperstack] {
            let mut h = Harness::new();
            h.app.appearance = look;
            let n = h.long_contents(120);
            let pages = h.app.doc.pages();
            for k in [1, n - 1, n + 2, 1] {
                h.scrub_to(k, pages);
                assert_eq!(h.app.target, k, "{look:?}: let go at page {k}");
                h.frames(60, vec![], Modifiers::NONE);
                assert_eq!(h.app.pos, k as f32, "{look:?}: settled on page {k}");
            }
        }
    }

    #[test]
    fn clicks_on_a_later_page_of_the_contents_stay_there_or_follow_a_line() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 1);
        h.frames(90, vec![], Modifiers::NONE);
        let l = h.app.doc.layout_page(&ctx, 1, 1.0, &[]);
        let block = l.paras[0].contents.as_ref().unwrap();
        // Below the list, at either side: nothing to put the caret at on this page.
        for at in [egui::vec2(1.0, block.size.y + 4.0), egui::vec2(block.size.x - 1.0, block.size.y + 30.0)] {
            h.click_in_page(at);
            assert_eq!(h.app.target, 1, "still the second page of the contents");
        }
        let (r, page) = block.links[0];
        h.click_in_page(r.center().to_vec2());
        assert_eq!(h.shown(), page, "the line's chapter");
        assert!(page >= n, "a chapter, after the contents");
    }

    #[test]
    fn the_caret_stays_in_the_story_around_long_contents() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        assert_eq!((h.shown(), h.app.doc.page_of(h.app.caret)), (0, n), "the contents shown, the caret where the story starts");
        // Moving the caret shows its page; it never goes onto the contents.
        h.key(Key::ArrowRight, Modifiers::NONE);
        assert_eq!(h.shown(), n);
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::ArrowLeft, Modifiers::NONE);
        h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!((h.shown(), h.app.caret), (n, 0));
        // Ctrl+End and Ctrl+Home, from a page of the contents in between.
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 1);
        h.key(Key::End, Modifiers::COMMAND);
        assert_eq!(h.shown(), h.app.last());
        h.key(Key::Home, Modifiers::COMMAND);
        assert_eq!(h.shown(), n);
    }

    #[test]
    fn typing_on_a_later_page_of_the_contents_writes_at_the_start_of_the_story() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 1);
        h.frames(90, vec![], Modifiers::NONE);
        h.type_text("Prologue ");
        assert!(h.app.doc.flow.text.starts_with("Prologue Chapter 1\n"), "{:?}", &h.app.doc.flow.text[..30]);
        assert_eq!(h.shown(), n, "the story's first page, where the typing went");
    }

    #[test]
    fn the_contents_lose_pages_with_their_chapters_without_losing_the_page_shown() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, n - 1);
        h.frames(90, vec![], Modifiers::NONE);
        // Most chapters go: the contents shrink to one page, and the page shown is still one there is.
        let end = h.app.doc.total_chars() - 1;
        let from = h.app.doc.flow.text.find("Chapter 6\n").unwrap();
        let from = h.app.doc.flow.text[..from].chars().count();
        h.app.replace_range(&ctx, from, end, "");
        assert_eq!(h.app.doc.contents_pages(&ctx), 1);
        let shown = h.shown();
        assert!(shown < h.app.doc.pages());
        // And taking out the contents keeps the story's first page in view.
        h.app.set_contents(&ctx, false);
        assert!(!h.app.doc.setup.contents);
        assert_eq!(h.shown(), 0);
    }

    #[test]
    fn the_contents_setting_is_saved_opened_and_exported_through_the_app() {
        use std::io::Read;
        let docx_text = |path: &std::path::Path| {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
            let mut xml = String::new();
            zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
            xml
        };
        let dir = std::env::temp_dir().join(format!("caprice-contents-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut h = Harness::new();
        let n = h.long_contents(60);
        let ctx = h.ctx.clone();
        assert!(h.app.unsaved(), "turning the contents on is a change to save");
        let path = dir.join("story.caprice");
        h.app.save_to(path.clone());
        assert!(!h.app.unsaved());
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains(r#""contents": true"#) && !saved.contains('\u{e000}'), "a setting, not text");

        // Opened again: the same pages, the contents first.
        let mut other = Harness::new();
        other.frames(3, vec![], Modifiers::NONE);
        let octx = other.ctx.clone();
        other.app.open_path(&octx, path.clone());
        assert!(other.app.doc.setup.contents && !other.app.unsaved());
        assert_eq!(other.app.doc.contents_pages(&octx), n);
        assert_eq!(other.app.doc.spans, h.app.doc.spans);

        // Exported to Word from either: the contents field first, then the story on a new page.
        for (app, name) in [(&mut h.app, "a.docx"), (&mut other.app, "b.docx")] {
            app.export_docx_to(dir.join(name));
            let xml = docx_text(&dir.join(name));
            let toc = xml.find("TOC \\o").expect("a contents field");
            let first = xml.find("Chapter 1<").unwrap();
            assert!(xml.find("<w:t>Contents</w:t>").unwrap() < toc && toc < first);
            assert!(xml[..xml.find("Heading1").unwrap() + 60].contains("<w:pageBreakBefore/>"), "the story on a new page");
        }

        // Turned off and saved, it is gone from the file and from Word.
        h.app.set_contents(&ctx, false);
        h.app.save_to(path.clone());
        h.app.export_docx_to(dir.join("c.docx"));
        assert!(std::fs::read_to_string(&path).unwrap().contains(r#""contents": false"#));
        assert!(!docx_text(&dir.join("c.docx")).contains("TOC"));
        let _ = std::fs::remove_dir_all(&dir);
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

        /// Where a point of the writing area (in page points) is on screen.
        fn in_page(&self, rel: egui::Vec2) -> egui::Pos2 {
            let rect = self.app.last_page_rect;
            rect.min + (self.app.doc.setup.margin_origin() + rel) * self.app.scale_of(rect)
        }

        fn press_at(&mut self, pos: egui::Pos2, pressed: bool) {
            let e = egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
            self.frames(1, vec![egui::Event::PointerMoved(pos), e], Modifiers::NONE);
        }

        /// Click the button labelled `label`, or the text field with that hint, found as a screen
        /// reader finds it.
        fn click_widget(&mut self, label: &str) {
            self.ctx.enable_accesskit();
            self.frames(1, vec![], Modifiers::NONE);
            let nodes = self.access.as_ref().map_or(&[][..], |t| &t.nodes[..]);
            let r = find_widget(nodes, label).unwrap_or_else(|| panic!("no button {label:?}"));
            let at = egui::pos2(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32);
            self.press_at(at, true);
            self.press_at(at, false);
            self.frames(2, vec![], Modifiers::NONE);
        }
    }

    /// Where the button labelled `label`, or the field with that hint, is. A window titled the same
    /// is not it.
    fn find_widget(nodes: &[(egui::accesskit::NodeId, egui::accesskit::Node)], label: &str) -> Option<egui::accesskit::Rect> {
        use egui::accesskit::Role;
        let named = |n: &egui::accesskit::Node| n.label() == Some(label) || n.placeholder() == Some(label);
        let takes_input = |n: &egui::accesskit::Node| !matches!(n.role(), Role::Label | Role::Window | Role::GenericContainer);
        nodes.iter().find(|(_, n)| named(n) && takes_input(n)).and_then(|(_, n)| n.bounds())
    }

    /// Three pages and more of text, the caret on the first.
    fn pages_of_text(h: &mut Harness) {
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(&"It was nearly midnight and the Prime Minister sat alone in his office. ".repeat(220));
        assert!(h.app.doc.pages() >= 4, "pages: {}", h.app.doc.pages());
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, 0, false);
        h.frames(120, vec![], Modifiers::NONE);
    }

    #[test]
    fn dragging_a_selection_past_the_bottom_of_the_page_goes_on_to_the_next_pages() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        assert_eq!(h.shown(), 0);
        let height = h.app.doc.setup.content_size().y;
        h.press_at(h.in_page(egui::vec2(100.0, 40.0)), true);
        let anchor = h.app.caret;
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, height / 2.0)))], Modifiers::NONE);
        // Into the bottom margin, the selection goes to the page's end without turning it.
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, height + 30.0)))], Modifiers::NONE);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 0, "a little past the last line, the page stays");
        assert!(h.app.caret + 80 > h.app.doc.spans[1].start, "the selection reaches the last line");
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, height + 60.0)))], Modifiers::NONE);
        h.frames(40, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 1, "the next page comes up");
        assert_eq!(h.app.anchor, anchor, "the selection still starts where the drag began");
        assert!(h.app.caret >= h.app.doc.spans[1].start, "the selection reaches onto it");
        // Held there, the pages go on turning, one at a time.
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 2);
        // Back up into the page, the selection ends where the pointer is.
        let at = h.in_page(egui::vec2(100.0, height / 2.0));
        h.frames(1, vec![egui::Event::PointerMoved(at)], Modifiers::NONE);
        h.press_at(at, false);
        h.frames(120, vec![], Modifiers::NONE);
        assert_eq!(h.shown(), 2, "no more pages turn");
        assert_eq!(h.app.anchor, anchor);
        let (a, b) = h.app.selection();
        assert!(a == anchor && b > h.app.doc.spans[2].start && b < h.app.doc.spans[2].end, "{a}..{b}");
    }

    #[test]
    fn dragging_a_selection_past_the_top_of_the_story_does_not_turn_onto_the_contents() {
        let mut h = Harness::new();
        let n = h.long_contents(120);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, n);
        assert_eq!(h.shown(), n);
        h.press_at(h.in_page(egui::vec2(100.0, 60.0)), true);
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, -60.0)))], Modifiers::NONE);
        h.frames(100, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, n);
        h.press_at(h.in_page(egui::vec2(100.0, -60.0)), false);
        assert_eq!((h.shown(), h.app.selection()), (n, (0, h.app.anchor)), "selected back to the story's start");
    }

    #[test]
    fn dragging_a_selection_past_the_top_of_the_page_goes_back_to_the_pages_before() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        let ctx = h.ctx.clone();
        h.app.go_to_page(&ctx, 2);
        assert_eq!(h.shown(), 2);
        let height = h.app.doc.setup.content_size().y;
        h.press_at(h.in_page(egui::vec2(100.0, height / 2.0)), true);
        let anchor = h.app.caret;
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, 40.0)))], Modifiers::NONE);
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, -30.0)))], Modifiers::NONE);
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 2, "a little above the first line, the page stays");
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, -60.0)))], Modifiers::NONE);
        h.frames(40, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 1, "the page before comes up");
        assert!(h.app.caret < h.app.doc.spans[2].start, "the selection reaches back onto it");
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.target, 0);
        let at = h.in_page(egui::vec2(100.0, height / 2.0));
        h.frames(1, vec![egui::Event::PointerMoved(at)], Modifiers::NONE);
        h.press_at(at, false);
        assert_eq!(h.shown(), 0, "no more pages turn");
        let (a, b) = h.app.selection();
        assert!(b == anchor && a > 0 && a < h.app.doc.spans[0].end, "{a}..{b}");
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
    fn asking_claude_by_cursor_offers_the_commands_for_a_selection_over_two_pages() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        h.app.pick_by(claude::Picking::Cursor);
        let height = h.app.doc.setup.content_size().y;

        // A click alone selects nothing and asks nothing.
        h.press_at(h.in_page(egui::vec2(100.0, height - 40.0)), true);
        h.press_at(h.in_page(egui::vec2(100.0, height - 40.0)), false);
        h.frames(2, vec![], Modifiers::NONE);
        assert!(h.app.lasso.is_none() && !h.app.has_selection());

        // Selected from near the end of the first page into the second, as without Claude.
        h.press_at(h.in_page(egui::vec2(100.0, height - 40.0)), true);
        let from = h.app.caret;
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(100.0, height + 60.0)))], Modifiers::NONE);
        h.frames(40, vec![], Modifiers::NONE);
        let at = h.in_page(egui::vec2(100.0, 100.0));
        h.frames(1, vec![egui::Event::PointerMoved(at)], Modifiers::NONE);
        assert!(h.app.lasso.is_none(), "no commands while still selecting");
        h.press_at(at, false);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.shown(), 1);
        let (a, b) = h.app.selection();
        assert!(a == from && b > h.app.doc.spans[1].start, "{a}..{b}");
        let caught = h.app.lasso.as_ref().and_then(|l| l.caught).map(|(a, b, _)| (a, b));
        assert_eq!(caught, Some((a, b)), "the commands are offered for the whole selection");
    }

    /// The rows of `text` painted this frame, each with whether its height shows in full
    /// (`Some(true)`), not at all (`Some(false)`), or cut by the clip (`None`).
    fn rows_shown(h: &Harness, text: &str) -> Vec<(String, Option<bool>)> {
        let mut rows = Vec::new();
        for cs in &h.painted {
            let egui::Shape::Text(t) = &cs.shape else { continue };
            if t.galley.text() != text {
                continue;
            }
            for row in &t.galley.rows {
                // Rows are cut from below or above; a tenth of a point either way is rounding.
                let (r, clip) = (row.rect().translate(t.pos.to_vec2()).y_range(), cs.clip_rect.y_range());
                let overlap = r.max.min(clip.max) - r.min.max(clip.min);
                let shown = if overlap >= r.span() - 0.1 { Some(true) } else if overlap <= 0.1 { Some(false) } else { None };
                rows.push((row.text(), shown));
            }
        }
        rows
    }

    #[test]
    fn saving_again_says_saved_again() {
        let dir = std::env::temp_dir().join(format!("caprice-saved-again-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.app.path = Some(dir.join("story.caprice"));
        let says_saved = |h: &Harness| h.painted.iter().any(|cs| matches!(&cs.shape, egui::Shape::Text(t) if t.galley.text() == "saved"));

        h.type_text("Once.");
        h.key(Key::S, Modifiers::COMMAND);
        assert!(says_saved(&h));
        h.frames(300, vec![], Modifiers::NONE);
        assert!(!says_saved(&h), "the message goes after a few seconds");
        // The same message a second time shows like the first.
        h.type_text(" Twice.");
        h.key(Key::S, Modifiers::COMMAND);
        assert!(says_saved(&h));
        let _ = std::fs::remove_dir_all(&dir);
    }

    impl Harness {
        /// Click the "‹" (`back`) or "›" of the sheet counter of the first post-it on the page shown,
        /// where it is drawn.
        fn turn_post_it(&mut self, back: bool) {
            let (rect, ctx) = (self.app.last_page_rect, self.ctx.clone());
            let sc = self.app.scale_of(rect);
            let (k, y) = self.app.note_places(&ctx)[&self.app.target][0];
            let note = &self.app.doc.notes[k];
            let n = notes::sheet_count(&ctx, &note.text, sc);
            let (_, r) = notes::sheet_counter(&ctx, notes::note_rect(rect, sc, y), sc, self.app.pad(note.id).sheet, n);
            let at = if back { r.left_center() + egui::vec2(2.0, 0.0) } else { r.right_center() - egui::vec2(2.0, 0.0) };
            self.press_at(at, true);
            self.press_at(at, false);
            self.frames(30, vec![], Modifiers::NONE);
        }
    }

    /// The middle, across, of everything painted in a green post-it's colour (the only green on screen).
    fn green_post_it_x(h: &Harness) -> Option<f32> {
        let green = |c: egui::Color32| c.a() > 100 && c.g() as i32 > c.r() as i32 + 40 && c.g() as i32 > c.b() as i32 + 40;
        let mut xs = Vec::new();
        for cs in &h.painted {
            match &cs.shape {
                egui::Shape::Mesh(m) => xs.extend(m.vertices.iter().filter(|v| green(v.color)).map(|v| v.pos.x)),
                egui::Shape::Rect(r) if green(r.fill) => xs.extend([r.rect.left(), r.rect.right()]),
                _ => {}
            }
        }
        (!xs.is_empty()).then(|| (xs.iter().fold(f32::MAX, |a, &b| a.min(b)) + xs.iter().fold(f32::MIN, |a, &b| a.max(b))) / 2.0)
    }

    #[test]
    fn in_a_book_a_new_page_slides_in_as_the_old_one_turns_over_with_its_post_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        assert_eq!(h.app.appearance, Appearance::Book);
        h.type_text("A page with a post-it.");
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 6, text: "Green.".into(), color: 2 });
        h.frames(3, vec![], Modifiers::NONE);
        let page = h.app.last_page_rect;
        let start = green_post_it_x(&h).unwrap();
        assert!(start > page.right(), "it sticks out on the right");

        h.click_widget("+ New page");
        let (mut xs, mut new_page_lefts, mut moving) = (vec![start], Vec::new(), 0);
        for _ in 0..120 {
            h.frames(1, vec![], Modifiers::NONE);
            moving += usize::from(h.app.pos != h.app.target as f32);
            xs.extend(green_post_it_x(&h));
            // Flat paper right of the page's place: the new page, still on its way in.
            let flat = h.painted.iter().filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == theme::PAPER && r.rect.width() > page.width() * 0.9 => Some(r.rect.left()),
                _ => None,
            });
            new_page_lefts.extend(flat.filter(|&x| x > page.left() + 1.0));
        }
        let seconds = moving as f32 / 60.0;
        assert!((0.8..1.1).contains(&seconds), "slow enough to follow: {seconds} s");
        assert!(new_page_lefts.iter().any(|&x| x > page.center().x), "the new page slides in from the right: {new_page_lefts:?}");
        assert!(new_page_lefts.windows(2).all(|w| w[1] <= w[0]), "coming in steadily: {new_page_lefts:?}");
        assert_eq!((h.app.doc.pages(), h.app.target), (2, 1), "on the new page");
        let end = *xs.last().unwrap();
        assert!(end < page.left(), "its page lies turned over on the left, the post-it's back showing: {end}");
        let jump = xs.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(jump < page.width() / 3.0, "it goes over with its page, not in one jump of {jump}: {xs:?}");
    }

    /// What a backspace that takes a page away shows, frame by frame for two seconds: where the green
    /// post-it is, where the page going out is (flat paper right of the page's place), and how long
    /// the slide-out lasted.
    struct SlideOut {
        post_it_xs: Vec<f32>,
        leaving_lefts: Vec<f32>,
        seconds: f32,
    }

    fn backspace_and_watch(h: &mut Harness, during: impl Fn(&mut Harness, usize)) -> SlideOut {
        let page = h.app.last_page_rect;
        let e = egui::Event::Key { key: Key::Backspace, physical_key: Some(Key::Backspace), pressed: true, repeat: false, modifiers: Modifiers::NONE };
        h.frames(1, vec![e], Modifiers::NONE);
        assert!(h.app.slide_out.is_some(), "the page slides out");
        let mut out = SlideOut { post_it_xs: Vec::new(), leaving_lefts: Vec::new(), seconds: 0.0 };
        for k in 0..120 {
            during(h, k);
            h.frames(1, vec![], Modifiers::NONE);
            out.seconds += if h.app.slide_out.is_some() { 1.0 / 60.0 } else { 0.0 };
            out.post_it_xs.extend(green_post_it_x(h));
            let flat = h.painted.iter().filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r) if r.fill == theme::PAPER && r.rect.width() > page.width() * 0.9 => Some(r.rect.left()),
                _ => None,
            });
            out.leaving_lefts.extend(flat.filter(|&x| x > page.left() + 1.0));
        }
        assert_eq!(h.app.slide_out, None, "it is gone");
        out
    }

    /// The post-it goes from the pile on the left back onto its page, turning with it, while the page
    /// taken away goes out to the right as long as a new page takes to come in.
    fn assert_turns_back(h: &Harness, start: f32, seen: &SlideOut) {
        let page = h.app.last_page_rect;
        let lefts = &seen.leaving_lefts;
        assert!(lefts.iter().any(|&x| x > page.center().x), "the page slides out to the right: {lefts:?}");
        assert!(lefts.windows(2).all(|w| w[1] >= w[0]), "going out steadily: {lefts:?}");
        assert!((0.8..1.1).contains(&seen.seconds), "as unhurried as a new page coming in: {} s", seen.seconds);
        let xs: Vec<f32> = std::iter::once(start).chain(seen.post_it_xs.iter().copied()).collect();
        let end = *xs.last().unwrap();
        assert!(end > page.right(), "the page before lies flat again, its post-it on the right: {end}");
        let jump = xs.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max);
        assert!(jump < page.width() / 3.0, "it turns back with its page, not in one jump of {jump}: {xs:?}");
    }

    #[test]
    fn in_a_book_a_backspaced_page_slides_out_as_the_one_before_turns_back_with_its_post_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        assert_eq!(h.app.appearance, Appearance::Book);
        h.type_text("A page with a post-it.");
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 6, text: "Green.".into(), color: 2 });
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(120, vec![], Modifiers::NONE);
        let start = green_post_it_x(&h).unwrap();
        assert!(start < h.app.last_page_rect.left(), "its page lies turned over on the left: {start}");

        let seen = backspace_and_watch(&mut h, |_, _| {});
        assert_eq!((h.app.doc.pages(), h.app.target), (1, 0));
        assert_turns_back(&h, start, &seen);
    }

    #[test]
    fn in_a_book_a_page_backspaced_away_between_others_turns_the_one_before_back_too() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("One.");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.type_text("Two, with a post-it.");
        let two = h.app.doc.spans[1].start;
        h.app.doc.notes.push(model::Note { id: 50, start: two, end: two + 3, text: "Green.".into(), color: 2 });
        h.key(Key::Enter, Modifiers::COMMAND);
        h.type_text("Three.");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.type_text("Four.");
        // To the start of page three, with page four still ahead in the book.
        let ctx = h.ctx.clone();
        let three = h.app.doc.spans[2].start;
        h.app.set_caret(&ctx, three, false);
        h.frames(120, vec![], Modifiers::NONE);
        assert_eq!((h.app.doc.pages(), h.app.target), (4, 2));
        let start = green_post_it_x(&h).unwrap();
        assert!(start < h.app.last_page_rect.left(), "page two lies turned over on the left: {start}");

        let seen = backspace_and_watch(&mut h, |_, _| {});
        assert_eq!((h.app.doc.pages(), h.app.target), (3, 1));
        assert!(h.app.doc.flow.text.contains("Two, with a post-it.Three."), "{:?}", h.app.doc.flow.text);
        assert_turns_back(&h, start, &seen);
    }

    #[test]
    fn typing_while_a_backspaced_page_slides_out_goes_into_the_page_turning_back() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("Before");
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(120, vec![], Modifiers::NONE);
        // Typed while the page before is still turning back over the one going out.
        backspace_and_watch(&mut h, |h, k| {
            if k == 10 {
                h.frames(1, vec![egui::Event::Text(" and after".into())], Modifiers::NONE);
            }
        });
        assert_eq!((h.app.doc.pages(), h.app.target), (1, 0));
        assert_eq!(h.app.doc.flow.text.trim_end(), "Before and after");
    }

    #[test]
    fn on_a_paperstack_a_backspaced_page_slides_out_quickly_over_a_flat_page() {
        let mut h = Harness::new();
        h.app.appearance = Appearance::Paperstack;
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("A page with a post-it.");
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 6, text: "Green.".into(), color: 2 });
        h.key(Key::Enter, Modifiers::COMMAND);
        h.frames(120, vec![], Modifiers::NONE);

        let seen = backspace_and_watch(&mut h, |_, _| {});
        let page = h.app.last_page_rect;
        assert!(seen.seconds < 0.6, "a paperstack does not turn pages, so it is quick: {} s", seen.seconds);
        assert!(seen.leaving_lefts.iter().any(|&x| x > page.center().x), "the page slides out to the right");
        // Nothing turns over: the post-it stays on the right of its page as it comes up from the pile.
        let xs = &seen.post_it_xs;
        assert!(!xs.is_empty() && xs.iter().all(|&x| x > page.center().x), "{xs:?}");
    }

    #[test]
    fn a_post_its_sheets_turn_back_as_well_as_forward() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(TWO_SCENES);
        let text = "Idea one: the roofs. Edwin climbs through the hatch and finds something. ".repeat(6);
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 4, text, color: 0 });
        h.frames(3, vec![], Modifiers::NONE);
        h.turn_post_it(false);
        h.turn_post_it(false);
        assert_eq!(h.app.pad(50).sheet, 2, "on the third sheet");
        h.turn_post_it(true);
        assert_eq!(h.app.pad(50).sheet, 1, "back to the second");
        h.turn_post_it(true);
        assert_eq!(h.app.pad(50).sheet, 0, "and the first");

        // Also after writing on the third sheet.
        h.turn_post_it(false);
        h.turn_post_it(false);
        let ctx = h.ctx.clone();
        let y = h.app.note_places(&ctx)[&0][0].1;
        let r = notes::note_rect(h.app.last_page_rect, h.app.scale_of(h.app.last_page_rect), y);
        h.press_at(r.center(), true);
        h.press_at(r.center(), false);
        h.frames(5, vec![], Modifiers::NONE);
        assert_eq!(h.app.pad(50).sheet, 2);
        h.turn_post_it(true);
        assert_eq!(h.app.pad(50).sheet, 1, "back to the second after writing on the third");
    }

    #[test]
    fn a_post_it_shows_whole_rows_and_the_rest_on_its_next_sheet() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(TWO_SCENES);
        let text = "Idea one: the roofs. Edwin climbs through the hatch and finds something on the roof that the boy \
            left behind.\n\nIdea two: the church. They search all night and go to church in the morning as if \
            nothing had happened, and the grandfather is waiting.";
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 4, text: text.into(), color: 0 });
        h.frames(3, vec![], Modifiers::NONE);

        let rows = rows_shown(&h, text);
        assert!(rows.len() > 10, "the note is drawn as one text: {rows:?}");
        let cut: Vec<_> = rows.iter().filter(|(_, s)| s.is_none()).collect();
        assert!(cut.is_empty(), "rows cut in half: {cut:?}");
        let first: Vec<_> = rows.iter().filter(|(_, s)| *s == Some(true)).map(|(t, _)| t.clone()).collect();
        assert!(first[0].starts_with("Idea one") && first.len() < rows.len(), "{rows:?}");

        // The counter's right half turns to the next sheet, which goes on with the next row.
        let rect = h.app.last_page_rect;
        let sc = h.app.scale_of(rect);
        let ctx = h.ctx.clone();
        let y = h.app.note_places(&ctx)[&0][0].1;
        let r = notes::note_rect(rect, sc, y);
        h.press_at(r.right_bottom() - egui::vec2(14.0, 12.0) * sc, true);
        h.press_at(r.right_bottom() - egui::vec2(14.0, 12.0) * sc, false);
        h.frames(30, vec![], Modifiers::NONE);
        let rows = rows_shown(&h, text);
        assert!(rows.iter().all(|(_, s)| s.is_some()), "{rows:?}");
        let second: Vec<_> = rows.iter().filter(|(_, s)| *s == Some(true)).map(|(t, _)| t.clone()).collect();
        let at = rows.iter().position(|(t, _)| *t == second[0]).unwrap();
        assert_eq!(rows[at - 1].0, first[first.len() - 1], "no row is skipped between the sheets");
    }

    impl Harness {
        /// Where the widget labelled `label` (or the field with that hint) was last drawn, if it was.
        fn widget_rect(&mut self, label: &str) -> Option<egui::Rect> {
            self.ctx.enable_accesskit();
            self.frames(1, vec![], Modifiers::NONE);
            let nodes = self.access.as_ref().map_or(&[][..], |t| &t.nodes[..]);
            let r = find_widget(nodes, label)?;
            Some(egui::Rect::from_min_max(egui::pos2(r.x0 as f32, r.y0 as f32), egui::pos2(r.x1 as f32, r.y1 as f32)))
        }
    }

    const SCRATCHPAD_HINT: &str = "Notes, lists, lines not yet placed… Plain text, saved with the document.";

    #[test]
    fn the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        let story = h.app.doc.visible_text().to_owned();
        assert!(h.widget_rect(SCRATCHPAD_HINT).is_none(), "off until asked for");
        h.click_widget("Scratchpad");
        h.click_widget(SCRATCHPAD_HINT);
        h.type_text("Ask about the boy's name.\n- the hatch\n- the church");
        assert_eq!(h.app.doc.scratchpad, "Ask about the boy's name.\n- the hatch\n- the church");
        assert_eq!(h.app.doc.visible_text(), story, "nothing is typed into the story");
        assert!(h.app.unsaved());
        let at = h.widget_rect(SCRATCHPAD_HINT).expect("the scratchpad shows");
        assert!(at.left() > 1100.0 / 2.0, "on the right: {at:?}");

        // Turning pages leaves it where and as it was.
        h.click_in_page(egui::vec2(100.0, 100.0));
        for _ in 0..2 {
            h.key(Key::PageDown, Modifiers::NONE);
            h.frames(40, vec![], Modifiers::NONE);
        }
        assert_eq!(h.shown(), 2);
        assert_eq!(h.widget_rect(SCRATCHPAD_HINT), Some(at));
        assert_eq!(h.app.doc.scratchpad, "Ask about the boy's name.\n- the hatch\n- the church");

        let other = h.reopened("scratchpad");
        assert_eq!(other.app.doc.scratchpad, "Ask about the boy's name.\n- the hatch\n- the church");

        // The same button puts it away; the text stays.
        h.click_widget("Scratchpad");
        assert!(h.widget_rect(SCRATCHPAD_HINT).is_none());
        assert_eq!(h.app.doc.scratchpad, "Ask about the boy's name.\n- the hatch\n- the church");
    }

    #[test]
    fn scrolling_over_the_scratchpad_scrolls_it_and_not_the_page() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        h.app.doc.scratchpad = (1..=200).map(|n| format!("line {n}\n")).collect();
        h.click_widget("Scratchpad");
        // Zoomed in, so that scrolling over the page would move it.
        h.key(Key::Plus, Modifiers::COMMAND);
        h.frames(10, vec![], Modifiers::NONE);
        let page = h.app.last_page_rect;
        let over = h.widget_rect(SCRATCHPAD_HINT).unwrap().min + egui::vec2(60.0, 60.0); // the field runs on below the window
        let (unit, delta, phase) = (egui::MouseWheelUnit::Point, egui::vec2(0.0, -200.0), egui::TouchPhase::Move);
        let wheel = egui::Event::MouseWheel { unit, delta, modifiers: Modifiers::NONE, phase };
        h.frames(1, vec![egui::Event::PointerMoved(over)], Modifiers::NONE);
        h.frames(1, vec![wheel], Modifiers::NONE);
        h.frames(30, vec![], Modifiers::NONE);
        assert_eq!(h.app.last_page_rect, page, "the page stays put");
        let field = h.widget_rect(SCRATCHPAD_HINT).unwrap();
        assert!(field.top() < over.y - 60.0 - 150.0, "the scratchpad scrolled: {field:?}");
    }

    #[test]
    fn claude_is_sent_only_the_marked_text_whatever_it_is_asked() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(&format!("The zebra chapter.\n{TWO_SCENES}\nThe quokka chapter."));
        h.app.doc.notes.push(model::Note { id: 50, start: 0, end: 7, text: "A platypus post-it.".into(), color: 0 });
        let (a, b) = (19, 19 + TWO_SCENES.chars().count());
        let ctx = h.ctx.clone();
        let commands = [
            claude::Command::Review,
            claude::Command::Fix,
            claude::Command::Grammar,
            claude::Command::Summarize,
            claude::Command::Ask("Why?".into()),
            claude::Command::Bridge("They hide a boy.".into()),
        ];
        for command in commands {
            h.app.ask_claude(&ctx, a, b, command.clone(), egui::Pos2::ZERO);
            let (_, sent) = h.app.asked().unwrap();
            assert!(sent.contains(TWO_SCENES), "{command:?}: {sent}");
            for elsewhere in ["zebra", "quokka", "platypus"] {
                assert!(!sent.contains(elsewhere), "{command:?} sent text from outside the marking: {sent}");
            }
        }

        // Nor do replies and drafts add any.
        h.app.fake_text("Idea one.");
        h.frames(2, vec![], Modifiers::NONE);
        h.click_widget("Draft it");
        let (_, sent) = h.app.asked().unwrap();
        assert!(!sent.contains("zebra") && !sent.contains("quokka"), "{sent}");
    }

    #[test]
    fn connecting_two_scenes_sends_the_marked_scenes_with_the_writers_note() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(TWO_SCENES);
        h.app.pick_by(claude::Picking::Cursor);

        // Both scenes and the gap between them, selected with the cursor.
        let ctx = h.ctx.clone();
        let end = TWO_SCENES.chars().count();
        let layout = h.app.page_layout(&ctx, 0, 1.0);
        let (from, to) = (layout.caret_rect(0, true).center(), layout.caret_rect(end, true).center());
        h.drag_in_page(from.to_vec2(), to.to_vec2());
        assert_eq!(h.app.selection(), (0, end));
        h.click_widget("Connect the scenes…");
        h.type_text("They hide a boy from far away in the attic.");
        h.click_widget("Get ideas");

        let (command, prompt) = h.app.asked().expect("Claude was asked");
        assert_eq!(command, &claude::Command::Bridge("They hide a boy from far away in the attic.".into()));
        assert!(prompt.contains(&format!("<passage>\n{TWO_SCENES}\n</passage>")), "{prompt}");
        assert!(prompt.contains("<writers_note>\nThey hide a boy from far away in the attic.\n</writers_note>"), "{prompt}");
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES, "the note is not typed into the story");
    }

    /// Two scenes with a gap between them, marked, and Claude's ideas for connecting them showing.
    fn ideas_for_a_gap(h: &mut Harness) {
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(TWO_SCENES);
        let end = TWO_SCENES.chars().count();
        h.app.fake_answer(0, end, claude::Command::Bridge(String::new()), "Idea one: the roofs.\nIdea two: the church.");
        h.frames(2, vec![], Modifiers::NONE);
    }

    const TWO_SCENES: &str = "They found the hatch open.\n-----\nAfter church they sat on the wall.";

    #[test]
    fn replying_to_claude_sends_the_conversation_so_far() {
        let mut h = Harness::new();
        ideas_for_a_gap(&mut h);
        h.click_widget("Reply to Claude…");
        h.type_text("Take the church, but the grandfather knows nothing yet.");
        h.key(Key::Enter, Modifiers::NONE);

        let (_, sent) = h.app.asked().unwrap();
        let (first, conversation) = sent.split_once("<conversation_so_far>").expect("the conversation is sent");
        assert!(first.contains(&format!("<passage>\n{TWO_SCENES}\n</passage>")), "the first question goes again: {first}");
        let answered = "<your_answer>\nIdea one: the roofs.\nIdea two: the church.\n</your_answer>";
        assert!(conversation.contains(answered), "{conversation}");
        let replied = "<writers_reply>\nTake the church, but the grandfather knows nothing yet.\n</writers_reply>";
        assert!(conversation.contains(replied), "{conversation}");
        assert!(conversation.ends_with(claude::FOLLOW_UP));
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES, "the reply is not typed into the story");

        // The next answer goes on from there.
        h.app.fake_text("Then the grandfather only sees them.");
        h.frames(2, vec![], Modifiers::NONE);
        h.click_widget("Reply to Claude…");
        h.type_text("Good.");
        h.key(Key::Enter, Modifiers::NONE);
        let (_, sent) = h.app.asked().unwrap();
        assert_eq!(sent.matches("<your_answer>").count(), 2);
        assert!(sent.contains("<your_answer>\nThen the grandfather only sees them.\n</your_answer>\n<writers_reply>\nGood."));
    }

    #[test]
    fn a_draft_fills_the_gap_in_the_marked_scenes_in_one_undo_step() {
        let mut h = Harness::new();
        ideas_for_a_gap(&mut h);
        h.click_widget("Draft it");
        let (_, sent) = h.app.asked().unwrap();
        assert!(sent.contains(claude::DRAFT), "{sent}");
        let ends = "Begin your reply with \"They found the hatch open.\" and end it with \"they sat on the wall.\".";
        assert!(sent.ends_with(ends), "the scenes' ends are named: {sent}");

        // Only the new part would replace both scenes, so it cannot be applied.
        h.app.fake_text("They ran across the square to the church.");
        h.frames(2, vec![], Modifiers::NONE);
        h.click_widget("Apply");
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES);
        h.click_widget("Ask for changes to the draft…");
        h.type_text("Keep the scenes around it.");
        h.key(Key::Enter, Modifiers::NONE);

        // The marked scenes again, the gap filled.
        let drafted = "They found the hatch open.\nThey ran across the square to the church.\nAfter church they sat on the wall.";
        h.app.fake_text(drafted);
        h.frames(2, vec![], Modifiers::NONE);
        h.click_widget("Apply");
        assert_eq!(h.app.doc.visible_text(), drafted);
        h.key(Key::Z, Modifiers::COMMAND);
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES);
    }

    #[test]
    fn a_reply_to_a_draft_asks_for_it_revised() {
        let mut h = Harness::new();
        ideas_for_a_gap(&mut h);
        h.click_widget("Draft it");
        h.app.fake_text("They ran to the church.");
        h.frames(2, vec![], Modifiers::NONE);
        h.click_widget("Ask for changes to the draft…");
        h.type_text("Shorter.");
        h.key(Key::Enter, Modifiers::NONE);
        let (_, sent) = h.app.asked().unwrap();
        assert!(sent.contains("<writers_reply>\nShorter.\n</writers_reply>") && sent.contains(claude::DRAFT), "{sent}");
    }

    #[test]
    fn pinning_an_answer_puts_it_on_a_post_it_over_the_passage_once() {
        let mut h = Harness::new();
        ideas_for_a_gap(&mut h);
        h.click_widget("Pin as note");
        let pinned: Vec<_> = h.app.doc.notes.iter().map(|n| (n.start, n.end, n.text.as_str())).collect();
        assert_eq!(pinned, [(0, TWO_SCENES.chars().count(), "Idea one: the roofs.\nIdea two: the church.")]);
        h.click_widget("Pin as note");
        assert_eq!(h.app.doc.notes.len(), 1, "the same answer is pinned only once");
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES);
    }

    #[test]
    fn story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text(TWO_SCENES);
        h.click_widget("Claude");
        h.click_widget("Story notes…");
        h.click_widget("Who is who, what has happened so far, what the reader must not learn yet…");
        h.type_text("Edwin and Mia hide a boy in the attic.");
        assert_eq!(h.app.doc.story_notes, "Edwin and Mia hide a boy in the attic.");
        assert_eq!(h.app.doc.visible_text(), TWO_SCENES, "the notes are not typed into the story");
        assert!(h.app.unsaved());

        let ctx = h.ctx.clone();
        let notes = "<story_notes>\nEdwin and Mia hide a boy in the attic.\n</story_notes>";
        for (command, sent) in [
            (claude::Command::Bridge(String::new()), true),
            (claude::Command::Review, true),
            (claude::Command::Ask("Why?".into()), true),
            (claude::Command::Grammar, false),
            (claude::Command::Fix, false),
        ] {
            h.app.ask_claude(&ctx, 27, 32, command.clone(), egui::Pos2::ZERO);
            assert_eq!(h.app.asked().unwrap().1.contains(notes), sent, "{command:?}");
        }

        let other = h.reopened("story-notes");
        assert_eq!(other.app.doc.story_notes, "Edwin and Mia hide a boy in the attic.");
    }

    #[test]
    fn circling_with_the_pen_takes_drags_away_from_the_cursor() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        h.app.pick_by(claude::Picking::Circle);
        h.press_at(h.in_page(egui::vec2(100.0, 40.0)), true);
        h.frames(1, vec![egui::Event::PointerMoved(h.in_page(egui::vec2(300.0, 200.0)))], Modifiers::NONE);
        h.press_at(h.in_page(egui::vec2(300.0, 200.0)), false);
        assert!(!h.app.has_selection(), "a stroke that is no loop selects nothing");
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

    /// Run frames until Claude is asked about the sentences written, and what it is asked.
    fn wait_for_reading(h: &mut Harness) -> String {
        for _ in 0..300 {
            h.frames(1, vec![], Modifiers::NONE);
            if let Some(asked) = h.app.backdrop.reading() {
                return asked.to_owned();
            }
        }
        panic!("the sentences were never read");
    }

    #[test]
    fn following_the_writing_paints_each_scene_once_the_next_one_begins() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.app.set_scene_mode(backdrop::Mode::Writing);
        h.frames(2, vec![], Modifiers::NONE);
        h.type_text("The harbour was full of boats. Gulls cried over the masts.\n");
        assert_eq!(h.app.open_scene_at(), Some(0), "writing opens a scene");
        // After a pause, Claude reads what was written since the scene's first sentence.
        let asked = wait_for_reading(&mut h);
        assert!(asked.contains("<scene_so_far>\nThe harbour was full of boats.\n</scene_so_far>"), "{asked}");
        assert!(asked.contains("<new_sentences>\n1. Gulls cried over the masts.\n</new_sentences>"), "{asked}");
        h.app.fake_reading("0");
        assert!(h.app.queued().is_empty(), "the scene goes on, and is not painted yet");

        // Dialogue on lines of its own, then another place: only what they say counts.
        h.type_text("\"Where is Mara?\" asked the fisherman.\nShe was up at the old house. Inside, the fire was out. ");
        let asked = wait_for_reading(&mut h);
        assert!(asked.contains("1. \"Where is Mara?\" asked the fisherman.\n2. She was up at the old house.\n3. Inside, the fire was out."), "{asked}");
        // Written on meanwhile, before it all: the scenes still begin where they did.
        let ctx = h.ctx.clone();
        h.app.replace_range(&ctx, 0, 0, "At dawn. ");
        h.app.fake_reading("3");
        let inside = h.app.doc.visible_text().find("Inside").unwrap();
        let harbour = "At dawn. The harbour was full of boats. Gulls cried over the masts.\n\"Where is Mara?\" asked the fisherman.\nShe was up at the old house.";
        assert_eq!(h.app.queued(), vec![(0, harbour.to_owned())], "the scene that ended is painted, all of it");
        assert_eq!(h.app.open_scene_at(), Some(inside), "the new one is open, partway into the line");

        // Painted once, and not again: no edits, no more questions.
        h.frames(1, vec![], Modifiers::NONE);
        assert!(h.app.queued().is_empty());
        h.frames(200, vec![], Modifiers::NONE);
        assert!(h.app.backdrop.reading().is_none() && !h.app.backdrop.drawing());
        // Writing on, only the open scene is sent along.
        let end = h.app.doc.total_chars() - 1;
        h.app.set_caret(&ctx, end, false);
        h.type_text("She lit a candle. ");
        let asked = wait_for_reading(&mut h);
        assert!(asked.contains("<scene_so_far>\nInside, the fire was out.\n</scene_so_far>\n\n<new_sentences>\n1. She lit a candle."), "{asked}");

        // Back to describing: nothing is open, read or drawn any more.
        h.app.set_scene_mode(backdrop::Mode::Described);
        assert!(h.app.open_scene_at().is_none() && h.app.backdrop.reading().is_none() && !h.app.backdrop.drawing());
        assert!(h.app.doc.scenes.iter().all(|s| !s.versions.is_empty()), "no unpainted scenes are left");
    }

    #[test]
    fn following_the_writing_begins_with_the_sentence_written_in() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("The old harbour lay still. ");
        h.app.set_scene_mode(backdrop::Mode::Writing);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.app.open_scene_at(), None, "what is written already is no new scene");
        h.type_text("The town woke up.");
        assert_eq!(h.app.open_scene_at(), Some(27), "the new sentence, not the one before");
        // Written into the middle of a sentence, the scene begins with that sentence.
        let ctx = h.ctx.clone();
        h.app.set_scene_mode(backdrop::Mode::Described);
        h.app.set_scene_mode(backdrop::Mode::Writing);
        h.frames(2, vec![], Modifiers::NONE);
        h.app.set_caret(&ctx, 8, false);
        h.type_text("grey ");
        assert_eq!(h.app.open_scene_at(), Some(0));
    }

    #[test]
    fn a_scene_that_names_someone_in_the_cast_is_painted_with_their_look_and_model_sheet() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        let story = "Mara walked along the harbour wall.\n* * *\nThe boats knocked against the stones.\nGrandma sat by the fire.";
        h.type_text(story);
        h.click_widget("Scene");
        h.click_widget("Cast\u{2026}");
        h.click_widget("Add someone");
        h.type_text("Mara");
        h.click_widget("Also called (Grandma, the old woman)");
        h.type_text("Grandma");
        h.click_widget(cast::LOOK_HINT);
        h.type_text("A tall woman of sixty, grey braid, red scarf.");
        let mara = &h.app.doc.cast[0];
        assert_eq!((mara.name.as_str(), mara.aliases.as_str()), ("Mara", "Grandma"));
        assert_eq!(mara.look().words, "A tall woman of sixty, grey braid, red scarf.");
        assert_eq!(h.app.doc.visible_text(), story, "nothing is typed into the story");
        assert!(h.app.unsaved());

        // Her model sheet, drawn (here as if by Claude), shows in the cast window.
        let id = mara.id;
        h.app.fake_sheet(id, BLUE);
        h.sheet_shown(BLUE);

        // A scene that names her, by her name or another, is painted with her look and sheet.
        let ctx = h.ctx.clone();
        let look = "<cast>\nMara (also called Grandma): A tall woman of sixty, grey braid, red scarf.\n</cast>";
        let sheet = format!("<model_sheet name=\"Mara\">\n{BLUE}\n</model_sheet>");
        for (passage, named) in [("Mara walked", true), ("The boats", false), ("Grandma sat", true)] {
            let a = story[..story.find(passage).unwrap()].chars().count();
            let b = a + story[a..].find('\n').unwrap_or(story.len() - a);
            h.app.paint_passage(&ctx, a, b);
            let asked = h.app.backdrop.asked();
            assert!(asked.contains("<passage>"), "{asked}");
            assert_eq!(asked.contains(look) && asked.contains(&sheet), named, "{passage}: {asked}");
            assert_eq!(asked.contains("<cast>"), named);
        }

        let other = h.reopened("cast");
        let mara = &other.app.doc.cast[0];
        assert_eq!((mara.name.as_str(), mara.aliases.as_str(), mara.look().sheet.as_deref()), ("Mara", "Grandma", Some(BLUE)));
        assert_eq!(mara.look().words, "A tall woman of sixty, grey braid, red scarf.");
    }

    #[test]
    fn a_model_sheet_is_drawn_from_the_look_and_the_example_opens_from_the_cast() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.click_widget("Scene");
        h.click_widget("Cast\u{2026}");
        h.click_widget("Add someone");
        h.type_text("Old Tom");
        h.click_widget(cast::LOOK_HINT);
        h.type_text("A fisherman in yellow oilskins.");
        h.click_widget("Draw a model sheet");
        assert!(h.app.cast_panel.asked().starts_with("<character>\nOld Tom: A fisherman in yellow oilskins.\n</character>"));
        // Tests never call Claude, so the drawing fails; it is not kept, and the button comes back.
        h.frames(30, vec![], Modifiers::NONE);
        assert!(!h.app.drawing_sheet());
        assert_eq!(h.app.doc.cast[0].look().sheet, None);

        h.click_widget("Delete");
        assert!(h.app.doc.cast.is_empty());
        h.click_widget("See an example");
        assert!(h.app.cast_panel.example_shown(), "the example picture is read and shown");
    }

    impl Harness {
        /// Someone added to the cast.
        fn add_to_cast(&mut self, name: &str, look: &str) {
            let (id, look_id) = (self.app.doc.take_cast_id(), self.app.doc.take_cast_id());
            let mut c = model::Character::new(id, look_id, name);
            c.looks[0].words = look.into();
            self.app.doc.cast.push(c);
        }

        /// Frames until the model sheet `svg` shows in the cast window (rendered on a thread).
        fn sheet_shown(&mut self, svg: &str) {
            for _ in 0..200 {
                self.frames(1, vec![], Modifiers::NONE);
                if self.app.cast_panel.has_sheet_shown(svg) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("the model sheet never showed");
        }

        /// Who of the cast a painting of the paragraph starting at char `at` is sent with.
        fn painted_with(&mut self, at: usize) -> Vec<String> {
            let end = at + self.app.doc.visible_text().chars().skip(at).take_while(|&c| c != '\n').count();
            let ctx = self.ctx.clone();
            self.app.paint_passage(&ctx, at, end);
            let asked = self.app.backdrop.asked();
            self.app.doc.cast.iter().filter(|c| asked.contains(&format!("\n{}: ", c.name))).map(|c| c.name.clone()).collect()
        }
    }

    #[test]
    fn a_passage_that_names_no_one_is_painted_with_those_named_just_before_it() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.add_to_cast("Lion Boy", "a thin boy with a mane of red hair");
        h.add_to_cast("Mara", "a tall woman of sixty, grey braid");
        let story = ["Lion Boy climbed up to the attic.", "They had hidden him behind the trunks.", "Mara heard steps on the stairs.",
            "She held her breath.", "* * *", "He woke at dawn.", "The light came in.", "Lion Boy sat up.", "The roof creaked.",
            "Rain fell.", "Wind blew.", "Then all was still."];
        h.type_text(&story.join("\n"));
        let starts: Vec<usize> = story.iter().scan(0, |at, p| { let s = *at; *at += p.chars().count() + 1; Some(s) }).collect();
        let lion = vec!["Lion Boy".to_owned()];
        assert_eq!(h.painted_with(starts[0]), lion, "named");
        assert_eq!(h.painted_with(starts[1]), lion, "carried on from the paragraph before");
        assert_eq!(h.painted_with(starts[3]), vec!["Mara".to_owned()], "only those named nearest");
        assert!(h.painted_with(starts[5]).is_empty(), "not across a starred line");
        assert_eq!(h.painted_with(starts[11]), Vec::<String>::new(), "not more than a few paragraphs back");
        assert_eq!(h.painted_with(starts[10]), lion, "but a few");

        // A description is painted with whom it names, and no one else.
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, starts[1] + 3, false);
        h.app.backdrop.description = "The attic at night, moonlight on the trunks.".into();
        h.app.draw_backdrop(&ctx);
        assert!(!h.app.backdrop.asked().contains("<cast>"));

        // Nor across a chapter title.
        h.app.set_caret(&ctx, starts[2], false);
        h.click_widget("Chapter");
        assert!(h.painted_with(starts[3]).is_empty(), "{:?}", h.app.doc.para_attrs_at(starts[2]).kind);
    }

    #[test]
    fn the_list_of_scenes_says_who_each_was_painted_with_and_paints_again_with_those_chosen() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.add_to_cast("Lion Boy", "a thin boy with a mane of red hair");
        h.add_to_cast("Mara", "a tall woman of sixty, grey braid");
        h.type_text("Lion Boy climbed up to the attic.\nThe trunks were dusty.");
        h.app.fake_painting_of(0, 33, BLUE);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.scenes[0].people, vec!["Lion Boy".to_owned()], "the scene keeps who it was painted with");

        h.click_widget("Scene");
        h.click_widget("All scenes (1)\u{2026}");
        h.click_widget("People");
        h.click_widget("Mara");
        let scene = &h.app.doc.scenes[0];
        assert_eq!((scene.people.clone(), scene.chosen), (vec!["Lion Boy".to_owned(), "Mara".to_owned()], true));

        h.click_widget("Paint again");
        let asked = h.app.backdrop.asked();
        assert!(asked.contains("\nLion Boy: ") && asked.contains("\nMara: "), "{asked}");
        // Painting its paragraph again keeps the people chosen too.
        h.frames(30, vec![], Modifiers::NONE);
        assert_eq!(h.painted_with(0), vec!["Lion Boy".to_owned(), "Mara".to_owned()]);

        let other = h.reopened("scene-people");
        let scene = &other.app.doc.scenes[0];
        assert_eq!((scene.people.clone(), scene.chosen), (vec!["Lion Boy".to_owned(), "Mara".to_owned()], true));
    }

    #[test]
    fn someone_can_keep_several_looks_and_scenes_get_the_one_in_use() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.add_to_cast("Mara", "A girl of ten in a blue smock.");
        let id = h.app.doc.cast[0].id;
        h.app.fake_sheet(id, BLUE);
        h.type_text("Mara ran down to the harbour.");
        let painted = |h: &mut Harness| {
            let ctx = h.ctx.clone();
            h.app.paint_passage(&ctx, 0, 29);
            let asked = h.app.backdrop.asked().to_owned();
            let words = h.app.doc.cast[0].looks.iter().position(|l| asked.contains(&format!("Mara: {}", l.words)));
            (words, asked.contains(BLUE), asked.contains(RED))
        };
        assert_eq!(painted(&mut h), (Some(0), true, false));

        // Another look, starting from the words of the one in use, kept beside it but not in use.
        h.frames(30, vec![], Modifiers::NONE);
        h.click_widget("Scene");
        h.click_widget("Cast\u{2026}");
        h.sheet_shown(BLUE);
        h.click_widget("Add a look");
        h.type_text("old");
        let mara = &h.app.doc.cast[0];
        assert_eq!((mara.looks.len(), mara.active, mara.looks[1].label.as_str()), (2, 0, "old"));
        assert_eq!(mara.looks[1].words, "A girl of ten in a blue smock.");
        h.frames(10, vec![], Modifiers::NONE);
        let window = h.ctx.memory(|m| m.area_rect(egui::Id::new("cast"))).unwrap();
        assert!(window.width() < 420.0, "the window keeps its width: {window:?}");
        assert_eq!(painted(&mut h), (Some(0), true, false), "the look in use is still the girl");
        h.app.doc.cast[0].looks[1].words = "A woman of sixty, grey braid, red scarf.".into();

        // Put in use, scenes get its words, and no sheet until it has its own.
        h.click_widget("old");
        assert_eq!(h.app.doc.cast[0].active, 1);
        assert_eq!(painted(&mut h), (Some(1), false, false));
        h.app.fake_sheet(id, RED);
        h.sheet_shown(RED);
        assert_eq!(painted(&mut h), (Some(1), false, true));

        // Switching back finds the girl as she was, sheet and all.
        h.click_widget("Look 1");
        assert_eq!(painted(&mut h), (Some(0), true, false));
        let other = h.reopened("looks");
        let mara = &other.app.doc.cast[0];
        assert_eq!((mara.looks.len(), mara.active, mara.looks[1].label.as_str()), (2, 0, "old"));
        assert_eq!((mara.looks[0].sheet.as_deref(), mara.looks[1].sheet.as_deref()), (Some(BLUE), Some(RED)));

        // Deleting the look in use puts the next one in use.
        h.click_widget("Delete look");
        let mara = &h.app.doc.cast[0];
        assert_eq!((mara.looks.len(), mara.look().label.as_str()), (1, "old"));
    }

    const BLUE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 60 85\"><circle cx=\"30\" cy=\"40\" r=\"20\" fill=\"#36c\"/></svg>";
    const RED: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 60 85\"><circle cx=\"30\" cy=\"40\" r=\"20\" fill=\"#c33\"/></svg>";

    impl Harness {
        /// Frames until the page shown has its scene's drawing behind it (rendered on a thread).
        fn scene_shown(&mut self) -> Option<backdrop::DrawingKey> {
            for _ in 0..200 {
                self.frames(1, vec![], Modifiers::NONE);
                let want = self.app.scene_here().map(|s| (s.id, s.shown));
                if self.app.pos == self.app.target as f32 && self.app.shown_scene() == want {
                    return want;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("the scene of page {} never showed", self.app.target);
        }

        /// Save the document and open it again in a new window.
        fn reopened(&mut self, name: &str) -> Harness {
            let dir = std::env::temp_dir().join(format!("caprice-scene-test-{}-{name}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("story.caprice");
            std::fs::write(&path, serde_json::to_string(&self.app.doc_file()).unwrap()).unwrap();
            let mut other = Harness::new();
            other.frames(3, vec![], Modifiers::NONE);
            let ctx = other.ctx.clone();
            other.app.open_path(&ctx, path);
            std::fs::remove_dir_all(dir).unwrap();
            other
        }
    }

    /// Twelve paragraphs over several pages, the caret at the start.
    fn paragraphs_over_pages(h: &mut Harness) {
        h.frames(3, vec![], Modifiers::NONE);
        let para = "It was nearly midnight and the Prime Minister sat alone in his office. ".repeat(20);
        h.type_text(&format!("{para}\n").repeat(12));
        assert!(h.app.doc.pages() >= 4, "pages: {}", h.app.doc.pages());
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, 0, false);
        h.frames(120, vec![], Modifiers::NONE);
    }

    /// Where paragraph `k` (from 0) starts.
    fn para_at(h: &Harness, k: usize) -> usize {
        if k == 0 {
            return 0;
        }
        h.app.doc.flow.text.chars().enumerate().filter(|&(_, c)| c == '\n').nth(k - 1).unwrap().0 + 1
    }

    #[test]
    fn the_picture_follows_the_caret_from_one_part_of_the_story_to_the_next() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let ctx = h.ctx.clone();
        // A paragraph that begins partway down a page, so its scene starts there.
        let k = (2..12).find(|&k| {
            let p = para_at(&h, k);
            p > h.app.doc.spans[h.app.doc.page_of(p)].start + 200
        });
        let late = para_at(&h, k.unwrap());
        let page = h.app.doc.page_of(late);
        h.app.fake_painting(5, BLUE); // painted in the first paragraph
        h.frames(2, vec![], Modifiers::NONE);
        h.app.fake_painting(late, RED);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.scenes.len(), 2, "the first scene is kept, not replaced");
        let (blue, red) = (h.app.doc.scenes[0].id, h.app.doc.scenes[1].id);
        assert_eq!(h.app.doc.scenes[1].at, late, "pinned to its paragraph's start");

        h.app.set_caret(&ctx, 0, false);
        assert_eq!(h.scene_shown(), Some((blue, 0)));
        // On the same page, just before the red scene's paragraph, and then in it.
        h.app.set_caret(&ctx, late - 2, false);
        assert_eq!((h.scene_shown(), h.app.target), (Some((blue, 0)), page));
        h.key(Key::ArrowRight, Modifiers::NONE);
        h.key(Key::ArrowRight, Modifiers::NONE);
        assert_eq!((h.scene_shown(), h.app.target), (Some((red, 0)), page), "the caret moved into the red part");
        h.key(Key::ArrowLeft, Modifiers::NONE);
        assert_eq!(h.scene_shown(), Some((blue, 0)), "and back out of it");
        // Every page after shows red, until the end.
        h.app.go_to_page(&ctx, h.app.last());
        assert_eq!(h.scene_shown(), Some((red, 0)));
        // Hidden, the scene before shows in its place.
        h.app.doc.scenes[1].hidden = true;
        assert_eq!(h.scene_shown(), Some((blue, 0)));
    }

    #[test]
    fn turning_back_to_the_caret_finds_its_picture_as_it_was_left() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let ctx = h.ctx.clone();
        // The only scene begins partway down a page, so the page's top has none.
        let k = (2..12).find(|&k| {
            let p = para_at(&h, k);
            p > h.app.doc.spans[h.app.doc.page_of(p)].start + 200 && h.app.doc.page_of(p) + 2 <= h.app.last()
        });
        let late = para_at(&h, k.unwrap());
        let page = h.app.doc.page_of(late);
        h.app.fake_painting(late, RED);
        h.frames(2, vec![], Modifiers::NONE);
        let red = h.app.doc.scenes[0].id;
        h.app.set_caret(&ctx, late + 5, false);
        assert_eq!((h.scene_shown(), h.app.target), (Some((red, 0)), page));
        h.frames(200, vec![], Modifiers::NONE);

        h.key(Key::PageDown, Modifiers::NONE);
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(h.shown(), page + 2);
        h.key(Key::PageUp, Modifiers::NONE);
        h.key(Key::PageUp, Modifiers::NONE);
        assert_eq!(h.shown(), page);
        assert_eq!(h.app.shown_scene(), Some((red, 0)));
        assert!(!h.app.scene_fading(), "the picture is there at once, not fading in from blank paper");
    }

    #[test]
    fn painting_a_paragraph_again_adds_a_version_and_keeps_the_others() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let p = para_at(&h, 1);
        h.app.fake_painting(p + 3, BLUE);
        h.frames(2, vec![], Modifiers::NONE);
        h.app.fake_painting(p + 40, RED);
        h.frames(2, vec![], Modifiers::NONE);
        let s = &h.app.doc.scenes;
        assert_eq!(s.len(), 1, "one scene per paragraph");
        assert_eq!((s[0].at, s[0].versions.len(), s[0].shown), (p, 2, 1), "the new version shows");
        assert_eq!(s[0].versions[0], BLUE);
        // The list of scenes shows it.
        let id = s[0].id;
        h.app.backdrop.list = true;
        for _ in 0..100 {
            h.frames(1, vec![], Modifiers::NONE);
            if h.app.has_thumb_of((id, 1)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(h.app.has_thumb_of((id, 1)), "its small picture is in the list");
        h.app.doc.scenes[0].shown = 0;
        let other = h.reopened("versions");
        let s = &other.app.doc.scenes;
        assert_eq!((s.len(), s[0].at, s[0].versions.len(), s[0].shown), (1, p, 2, 0), "versions and the one shown are saved");
    }

    #[test]
    fn scenes_stay_with_their_paragraph_through_edits_and_saving() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let p = para_at(&h, 3);
        h.app.fake_painting(p, BLUE);
        h.frames(2, vec![], Modifiers::NONE);
        // Typed before it, and right at its start: it still starts the paragraph.
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, 0, false);
        h.type_text("Prologue. ");
        h.app.set_caret(&ctx, para_at(&h, 3), false);
        h.type_text("Then ");
        assert_eq!(h.app.doc.scenes[0].at, para_at(&h, 3));
        assert!(h.app.doc.flow.text[h.app.doc.char_to_byte(para_at(&h, 3))..].starts_with("Then "));
        let at = h.app.doc.scenes[0].at;
        let mut other = h.reopened("edits");
        assert_eq!(other.app.doc.scenes[0].at, at);
        assert_eq!(other.app.doc.scene_at(at).map(|s| s.at), Some(at), "it shows from its paragraph on");
        // Its paragraph deleted, it stays where the text was.
        let (a, b) = (para_at(&other, 2), para_at(&other, 4));
        let ctx = other.ctx.clone();
        other.app.replace_range(&ctx, a, b, "");
        assert_eq!(other.app.doc.scenes[0].at, a);
    }

    #[test]
    fn a_scene_still_being_painted_is_not_saved_and_goes_if_stopped() {
        let mut h = Harness::new();
        paragraphs_over_pages(&mut h);
        let _tx = h.app.fake_painting_under_way(10);
        h.frames(2, vec![], Modifiers::NONE);
        assert_eq!(h.app.doc.scenes.len(), 1);
        assert!(h.reopened("unpainted").app.doc.scenes.is_empty(), "nothing to save yet");
        drop(_tx); // stopped: the painting never comes
        h.frames(2, vec![], Modifiers::NONE);
        assert!(h.app.doc.scenes.is_empty());
    }

    #[test]
    fn an_older_files_one_scene_opens_pinned_to_the_start_of_the_story() {
        let mut h = Harness::new();
        h.frames(3, vec![], Modifiers::NONE);
        h.type_text("A storm at sea.");
        let old = backdrop::SceneFile { svg: Some(BLUE.into()), description: "A ship in a storm".into(), hidden: false };
        let mut json = serde_json::to_value(fileio::DocFile::from_doc(&h.app.doc)).unwrap();
        json["scene"] = serde_json::to_value(&old).unwrap();
        let dir = std::env::temp_dir().join(format!("caprice-scene-test-{}-old", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("story.caprice");
        std::fs::write(&path, json.to_string()).unwrap();

        let mut other = Harness::new();
        other.frames(3, vec![], Modifiers::NONE);
        let ctx = other.ctx.clone();
        other.app.open_path(&ctx, path.clone());
        let s = &other.app.doc.scenes;
        assert_eq!((s.len(), s[0].at, s[0].versions.clone()), (1, 0, vec![BLUE.to_owned()]));
        assert_eq!(other.app.backdrop.description, "A ship in a storm");
        assert!(other.scene_shown().is_some());
        assert!(!other.app.unsaved(), "opening an old file is no change");

        // Saved again, it is a scene like any other, and the description stays.
        let again = other.reopened("old-again");
        assert_eq!(again.app.doc.scenes.len(), 1);
        assert_eq!(again.app.backdrop.to_file().map(|f| (f.svg, f.description)), Some((None, "A ship in a storm".into())));
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
        assert_eq!(h.app.caret, 17, "turning pages leaves the caret");
        h.frames(60, vec![], Modifiers::NONE);
        assert_eq!(h.app.pos.round() as usize, 0);
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(h.app.target, 1);
    }

    #[test]
    fn turning_pages_leaves_the_caret_where_it_is_until_typing_or_a_click() {
        let mut h = Harness::new();
        pages_of_text(&mut h);
        let ctx = h.ctx.clone();
        h.app.set_caret(&ctx, 50, false);
        h.key(Key::PageDown, Modifiers::NONE);
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!((h.shown(), h.app.caret), (2, 50), "the pages turn, the caret stays");
        // Typing goes in at the caret, and its page comes back.
        h.type_text("Q");
        assert_eq!(h.app.doc.char_at(50), Some('Q'));
        assert_eq!(h.shown(), 0);
        // Turned away again, a click puts the caret on the page shown.
        h.app.go_to_page(&ctx, 2);
        assert_eq!(h.shown(), 2);
        h.click_in_page(egui::vec2(50.0, 50.0));
        assert_eq!((h.shown(), h.app.doc.page_of(h.app.caret)), (2, 2));
        // Shift+Page Down still selects on to the next page's start.
        let from = h.app.caret;
        h.key(Key::PageDown, Modifiers::SHIFT);
        assert_eq!((h.shown(), h.app.selection()), (3, (from, h.app.doc.spans[3].start)));
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
