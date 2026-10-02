use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Color32, FontFamily, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Shape, Stroke, TextEdit, UiBuilder, Vec2,
    pos2, text::LayoutJob, TextFormat, vec2,
};
use serde::{Deserialize, Serialize};
use egui::epaint::{Mesh, TessellationOptions, Tessellator, WHITE_UV};
use egui::text::{CCursor, CCursorRange};
use egui::widgets::text_edit::TextEditState;

// A4 in points, 12pt text, 1 inch margins.
const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 72.0;
const FONT_SIZE: f32 = 12.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;
const CONTENT_H: f32 = PAGE_H - 2.0 * MARGIN;

const DOCK_H: f32 = 48.0;
const DOCK_GAP: f32 = 14.0;
/// Space at the top/bottom of the window taken by the two docks.
const RESERVED: f32 = DOCK_GAP + DOCK_H + 10.0;
const MIN_ZOOM: f32 = 0.3;
const MAX_ZOOM: f32 = 4.0;

const INK: Color32 = Color32::from_rgb(28, 28, 32);
const DESK: Color32 = Color32::from_rgb(30, 32, 38);
const DOCK: Color32 = Color32::from_rgb(42, 45, 54);
const DOCK_EDGE: Color32 = Color32::from_rgb(62, 66, 78);
const BTN: Color32 = Color32::from_rgb(55, 59, 71);
const BTN_HOVER: Color32 = Color32::from_rgb(68, 73, 88);
const TEXT: Color32 = Color32::from_rgb(222, 225, 232);
const TEXT_DIM: Color32 = Color32::from_rgb(140, 146, 160);
const ACCENT: Color32 = Color32::from_rgb(143, 162, 255);
const PAPER: Color32 = Color32::from_rgb(253, 252, 249);

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = DESK;
    v.window_fill = DOCK;
    v.window_stroke = Stroke::new(1.0, DOCK_EDGE);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.extreme_bg_color = Color32::from_rgb(34, 36, 43);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(20, 22, 30));
    v.text_cursor.stroke = Stroke::new(1.5, Color32::from_rgb(70, 90, 220));
    let r = egui::CornerRadius::same(8);
    let w = &mut v.widgets;
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    w.noninteractive.bg_stroke = Stroke::new(1.0, DOCK_EDGE);
    for (state, fill) in [(&mut w.inactive, BTN), (&mut w.hovered, BTN_HOVER), (&mut w.active, BTN_HOVER), (&mut w.open, BTN)] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0, TEXT);
        state.corner_radius = r;
        state.expansion = 0.0;
    }
    w.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    ctx.set_visuals(v);
    ctx.global_style_mut(|st| {
        st.spacing.button_padding = vec2(12.0, 5.0);
        st.spacing.item_spacing = vec2(8.0, 8.0);
        st.spacing.interact_size.y = 30.0;
        st.spacing.combo_height = 340.0;
    });
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 900.0])
            .with_title("Caprice"),
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


// ------------------------------------------------------------------ model

/// Per-character formatting.
#[derive(Clone, PartialEq, Debug)]
struct Style {
    font: Arc<str>,
    size: f32,
    bold: bool,
    underline: bool,
}

impl Style {
    fn new(font: &str) -> Self {
        Self { font: font.into(), size: FONT_SIZE, bold: false, underline: false }
    }
}

/// Keeps `styles` aligned with the TextEdit buffer.
struct Rich {
    styles: Vec<Style>,
    /// The text `styles` currently corresponds to.
    synced: String,
}

impl Rich {
    /// Bring `styles` in line with `new`, giving freshly inserted chars the `fill` style.
    fn sync(&mut self, new: &str, fill: &Style) {
        if self.synced == new {
            return;
        }
        let old: Vec<char> = self.synced.chars().collect();
        let new_c: Vec<char> = new.chars().collect();
        self.styles.resize(old.len(), fill.clone());
        let max = old.len().min(new_c.len());
        let p = old.iter().zip(&new_c).take_while(|(a, b)| a == b).count();
        let s = old[p..].iter().rev().zip(new_c[p..].iter().rev()).take(max - p).take_while(|(a, b)| a == b).count();
        let inserted = new_c.len() - p - s;
        self.styles.splice(p..old.len() - s, std::iter::repeat_n(fill.clone(), inserted));
        self.synced = new.to_owned();
    }
}

struct Page {
    text: String,
    rich: RefCell<Rich>,
}

impl Page {
    fn new(text: String, styles: Vec<Style>) -> Self {
        let rich = RefCell::new(Rich { styles, synced: text.clone() });
        Self { text, rich }
    }

    fn chars(&self) -> usize {
        self.text.chars().count()
    }

    fn resync(&mut self) {
        self.rich.get_mut().synced = self.text.clone();
    }

    fn split_off(&mut self, byte: usize, chars: usize) -> Page {
        let tail_text = self.text.split_off(byte);
        let styles = &mut self.rich.get_mut().styles;
        let tail_styles = styles.split_off(chars.min(styles.len()));
        self.resync();
        Page::new(tail_text, tail_styles)
    }

    fn prepend(&mut self, head: Page) {
        let Page { text, rich } = head;
        self.text.insert_str(0, &text);
        self.rich.get_mut().styles.splice(0..0, rich.into_inner().styles);
        self.resync();
    }

    fn append(&mut self, other: Page) {
        self.text.push_str(&other.text);
        self.rich.get_mut().styles.extend(other.rich.into_inner().styles);
        self.resync();
    }

    fn job(&self, fallback: &Style, scale: f32) -> LayoutJob {
        build_job(&self.text, &self.rich.borrow().styles, fallback, scale)
    }
}

static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn family_for(font: &str, bold: bool) -> FontFamily {
    let loaded = LOADED.lock().unwrap();
    if loaded.iter().any(|f| f == font) {
        let key = format!("{font}:bold");
        if bold && loaded.contains(&key) {
            return FontFamily::Name(key.into());
        }
        return FontFamily::Name(font.into());
    }
    FontFamily::Proportional
}

fn text_format(st: &Style, scale: f32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(st.size * scale, family_for(&st.font, st.bold)),
        color: INK,
        underline: if st.underline { Stroke::new(scale.max(1.0), INK) } else { Stroke::NONE },
        ..Default::default()
    }
}

fn build_job(text: &str, styles: &[Style], fallback: &Style, scale: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = CONTENT_W * scale;
    let style_at = |k: usize| styles.get(k).or(styles.last()).unwrap_or(fallback);
    let mut start = 0;
    let mut run = 0; // char index where the current run started
    for (k, (b, _)) in text.char_indices().enumerate() {
        if k > run && style_at(k) != style_at(run) {
            job.append(&text[start..b], 0.0, text_format(style_at(run), scale));
            start = b;
            run = k;
        }
    }
    job.append(&text[start..], 0.0, text_format(style_at(run), scale));
    job
}

// ------------------------------------------------------------ file format

#[derive(Serialize, Deserialize)]
struct Run {
    text: String,
    font: String,
    size: f32,
    bold: bool,
    underline: bool,
}

#[derive(Serialize, Deserialize)]
struct DocFile {
    version: u32,
    pages: Vec<Vec<Run>>,
}

impl DocFile {
    fn from_pages(pages: &[Page]) -> Self {
        let pages = pages
            .iter()
            .map(|p| {
                let rich = p.rich.borrow();
                let mut runs: Vec<(Style, String)> = Vec::new();
                for (ch, st) in p.text.chars().zip(&rich.styles) {
                    match runs.last_mut() {
                        Some((s, t)) if s == st => t.push(ch),
                        _ => runs.push((st.clone(), ch.to_string())),
                    }
                }
                runs.into_iter()
                    .map(|(s, text)| Run { text, font: s.font.to_string(), size: s.size, bold: s.bold, underline: s.underline })
                    .collect()
            })
            .collect();
        Self { version: 1, pages }
    }

    fn into_pages(self) -> Vec<Page> {
        let mut pages: Vec<Page> = self
            .pages
            .into_iter()
            .map(|runs| {
                let mut text = String::new();
                let mut styles = Vec::new();
                for r in runs {
                    let st = Style { font: r.font.into(), size: r.size, bold: r.bold, underline: r.underline };
                    styles.extend(std::iter::repeat_n(st, r.text.chars().count()));
                    text.push_str(&r.text);
                }
                Page::new(text, styles)
            })
            .collect();
        if pages.is_empty() {
            pages.push(Page::new(String::new(), Vec::new()));
        }
        pages
    }
}

// ------------------------------------------------------------------ fonts

/// Common document fonts, most popular first; the first ten that are installed get offered.
const COMMON_FONTS: [&str; 24] = [
    "Times New Roman", "Arial", "Calibri", "Helvetica", "Georgia", "Verdana", "Courier New", "Cambria", "Garamond",
    "Palatino Linotype", "Liberation Serif", "Liberation Sans", "Liberation Mono", "Noto Serif", "Noto Sans",
    "DejaVu Serif", "DejaVu Sans", "Nimbus Roman", "Nimbus Sans", "P052", "Nimbus Mono PS", "Tinos", "Arimo",
    "Cousine",
];

struct FontBook {
    db: fontdb::Database,
    /// (name shown in the UI and stored in files, installed family that renders it)
    families: Vec<(String, String)>,
    defs: egui::FontDefinitions,
    requested: BTreeSet<String>,
    pending: Vec<String>,
}

impl FontBook {
    fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let installed: BTreeSet<String> = db
            .faces()
            .filter(|f| f.style == fontdb::Style::Normal)
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        let mut families: Vec<(String, String)> = COMMON_FONTS
            .iter()
            .filter(|f| installed.contains(**f))
            .take(10)
            .map(|f| ((*f).to_owned(), (*f).to_owned()))
            .collect();
        if families.is_empty() {
            families.push(("Default".into(), "Default".into()));
        }
        Self {
            db,
            families,
            defs: egui::FontDefinitions::default(),
            requested: BTreeSet::new(),
            pending: Vec::new(),
        }
    }

    fn preferred_default(&self) -> String {
        self.families[0].0.clone()
    }

    /// Register `font` with egui. It becomes usable on the next frame.
    fn ensure(&mut self, ctx: &egui::Context, font: &str, immediate: bool) {
        if font == "Default" || !self.requested.insert(font.to_owned()) {
            return;
        }
        let Some((_, actual)) = self.families.iter().find(|(n, _)| n == font).cloned() else { return };
        let families = [fontdb::Family::Name(&actual)];
        let query = |bold: bool| fontdb::Query {
            families: &families,
            weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            ..Default::default()
        };
        let mut keys = Vec::new();
        for bold in [false, true] {
            let Some(id) = self.db.query(&query(bold)) else { continue };
            let Some(info) = self.db.face(id) else { continue };
            if bold && info.weight.0 < 600 {
                continue;
            }
            let Some((bytes, index)) = self.db.with_face_data(id, |d, i| (d.to_vec(), i)) else { continue };
            let key = if bold { format!("{font}:bold") } else { font.to_owned() };
            let mut data = egui::FontData::from_owned(bytes);
            data.index = index;
            self.defs.font_data.insert(key.clone(), Arc::new(data));
            let mut chain = vec![key.clone()];
            chain.extend(self.defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
            self.defs.families.insert(FontFamily::Name(key.clone().into()), chain);
            keys.push(key);
        }
        if keys.is_empty() {
            return;
        }
        ctx.set_fonts(self.defs.clone());
        if immediate {
            LOADED.lock().unwrap().extend(keys);
        } else {
            self.pending.extend(keys);
        }
    }

    fn activate_pending(&mut self) {
        if !self.pending.is_empty() {
            LOADED.lock().unwrap().append(&mut self.pending);
        }
    }
}

struct App {
    pages: Vec<Page>,
    typing: Style,
    fonts: FontBook,
    /// Page scale (screen points per page point) and the page's top-left on screen.
    zoom: f32,
    origin: Pos2,
    /// Follow the window size until the user zooms by hand.
    fit: bool,
    ctrl_down: bool,
    path: Option<PathBuf>,
    status: String,
    last_cursor: Option<(usize, usize)>,
    /// Animated position in page units. 2.4 means page 2 is 40% flipped over.
    pos: f32,
    /// Page we are flipping towards (and the one being edited once we arrive).
    target: usize,
    scrubbing: bool,
    /// Cursor placement to apply once `page` is shown and settled.
    cursor_req: Option<(usize, usize)>,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        let mut fonts = FontBook::new();
        let default = fonts.preferred_default();
        fonts.ensure(ctx, &default, true);
        Self {
            pages: vec![Page::new(String::new(), Vec::new())],
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
        }
    }

    fn last(&self) -> usize {
        self.pages.len() - 1
    }

    fn page_id(i: usize) -> Id {
        Id::new(("page", i))
    }

    fn cursor_of(ctx: &egui::Context, page: usize) -> Option<usize> {
        let state = TextEditState::load(ctx, Self::page_id(page))?;
        let range = state.cursor.char_range()?;
        (range.primary == range.secondary).then_some(usize::from(range.primary.index))
    }

    fn insert_page_after(&mut self, i: usize) {
        self.pages.insert(i + 1, Page::new(String::new(), Vec::new()));
        self.target = i + 1;
        self.cursor_req = Some((i + 1, 0));
    }

    // ---------------------------------------------------------------- layout

    fn layout(&self, ctx: &egui::Context, page: &Page, scale: f32) -> Arc<egui::Galley> {
        let job = page.job(&self.typing, scale);
        ctx.fonts_mut(|f| f.layout_job(job))
    }

    /// If the page doesn't fit, the byte and char offset where the overflow starts.
    fn overflow_split(&self, ctx: &egui::Context, page: &Page) -> Option<(usize, usize)> {
        let galley = self.layout(ctx, page, 1.0);
        let first_bad = galley.rows.iter().position(|r| r.pos.y + r.row.size.y > CONTENT_H + 0.5)?;
        if first_bad == 0 {
            return None;
        }
        let chars: usize = galley.rows[..first_bad]
            .iter()
            .map(|r| usize::from(r.row.char_count_excluding_newline()) + usize::from(r.ends_with_newline))
            .sum();
        let byte = page.text.char_indices().nth(chars).map_or(page.text.len(), |(b, _)| b);
        (byte < page.text.len()).then_some((byte, chars))
    }

    /// Reflow overflowing text onto following pages (creating them as needed).
    fn normalize(&mut self, ctx: &egui::Context) {
        let editing = (self.pos - self.target as f32).abs() < 1e-3 || self.cursor_req.is_some();
        let cursor = editing.then(|| Self::cursor_of(ctx, self.target)).flatten();
        let mut i = 0;
        while i < self.pages.len() {
            if let Some((byte, chars)) = self.overflow_split(ctx, &self.pages[i]) {
                let tail = self.pages[i].split_off(byte, chars);
                if i + 1 == self.pages.len() {
                    self.pages.push(Page::new(String::new(), Vec::new()));
                }
                self.pages[i + 1].prepend(tail);
                if let Some(c) = cursor {
                    if i == self.target && c >= chars {
                        self.cursor_req = Some((i + 1, c - chars));
                        self.target = i + 1;
                    }
                }
                ctx.request_repaint();
            }
            i += 1;
        }
    }

    // ------------------------------------------------------------- navigation

    fn handle_boundary_keys(&mut self, ctx: &egui::Context, i: usize) {
        let Some(c) = Self::cursor_of(ctx, i) else { return };
        let len = self.pages[i].chars();
        let consume = |k: Key| ctx.input_mut(|inp| inp.consume_key(Modifiers::NONE, k));
        if c == 0 && i > 0 {
            if ctx.input(|inp| inp.key_pressed(Key::Backspace)) && consume(Key::Backspace) {
                // Remove the page break: pull this page's text up onto the previous page.
                let cur = self.pages.remove(i);
                let prev = &mut self.pages[i - 1];
                let join = prev.chars();
                prev.append(cur);
                self.pos = (i - 1) as f32;
                self.target = i - 1;
                self.cursor_req = Some((i - 1, join));
            } else if consume(Key::ArrowLeft) {
                self.target = i - 1;
                self.cursor_req = Some((i - 1, self.pages[i - 1].chars()));
            }
        } else if c == len && i < self.last() && consume(Key::ArrowRight) {
            self.target = i + 1;
            self.cursor_req = Some((i + 1, 0));
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

    // --------------------------------------------------------------- drawing

    fn paper(painter: &egui::Painter, rect: Rect) {
        for s in 1..=6u8 {
            let r = rect.expand(f32::from(s) * 2.5);
            painter.rect_filled(r.translate(vec2(0.0, 6.0)), 2.0, Color32::from_black_alpha(14));
        }
        painter.rect_filled(rect, 1.0, PAPER);
    }

    /// Stack hints: sheets piled to the right (pages ahead) and left (pages behind).
    fn stack(painter: &egui::Painter, rect: Rect, behind: usize, ahead: usize) {
        let sc = rect.width() / PAGE_W;
        for k in (1..=ahead.min(5)).rev() {
            let r = rect.translate(vec2(k as f32 * 2.2, k as f32 * 1.4) * sc);
            painter.rect_filled(r, 1.0, Color32::from_rgb(232, 230, 224));
            painter.rect_stroke(r, 1.0, Stroke::new(0.6, Color32::from_black_alpha(60)), egui::StrokeKind::Inside);
        }
        for k in (1..=behind.min(4)).rev() {
            let r = rect.translate(vec2(-(k as f32) * 2.0, k as f32 * 1.0) * sc);
            painter.rect_filled(r, 1.0, Color32::from_rgb(226, 224, 218));
            painter.rect_stroke(r, 1.0, Stroke::new(0.6, Color32::from_black_alpha(50)), egui::StrokeKind::Inside);
        }
    }

    fn static_page(&self, ui: &egui::Ui, rect: Rect, page: &Page) {
        Self::paper(ui.painter(), rect);
        let sc = rect.width() / PAGE_W;
        let galley = self.layout(ui.ctx(), page, sc);
        ui.painter().galley(rect.min + vec2(MARGIN, MARGIN) * sc, galley, INK);
    }

    /// Draw a page hinged on its left edge, turned by `theta` (0 = flat, PI = fully flipped).
    fn flipping_page(&self, ui: &egui::Ui, rect: Rect, page: &Page, theta: f32, fade: f32) {
        let painter = ui.painter();
        let (sin, cos) = theta.sin_cos();
        let pw = rect.width();
        let sc = pw / PAGE_W;
        let spine = rect.left();
        let cy = rect.center().y;
        let lift = 0.07 * sin; // free edge swells towards the viewer
        let map = |u: f32, y: f32| -> Pos2 {
            let s = 1.0 + lift * (u / pw);
            pos2(spine + u * cos, cy + (y - cy) * s)
        };
        let front = cos >= 0.0;
        let alpha = |c: Color32| c.gamma_multiply(fade);

        // Paper, as vertical strips so shading can vary across the curl.
        const STRIPS: usize = 28;
        let mut mesh = Mesh::default();
        for k in 0..=STRIPS {
            let f = k as f32 / STRIPS as f32;
            let u = f * pw;
            // Darker towards the free edge and as the page turns edge-on; back side a touch dimmer.
            let mut shade = 1.0 - 0.28 * sin * f - 0.10 * sin;
            if !front {
                shade -= 0.06;
            }
            let c = alpha(Color32::from_rgb(
                (f32::from(PAPER.r()) * shade) as u8,
                (f32::from(PAPER.g()) * shade) as u8,
                (f32::from(PAPER.b()) * shade) as u8,
            ));
            mesh.colored_vertex(map(u, rect.top()), c);
            mesh.colored_vertex(map(u, rect.bottom()), c);
            if k > 0 {
                let b = (k * 2) as u32;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b + 1, b);
            }
        }
        debug_assert_eq!(mesh.vertices[0].uv, WHITE_UV);

        // Soft shadow on the page underneath, just beyond the moving edge.
        if front {
            let edge = spine + pw * cos;
            let reach = 70.0 * sc * sin;
            let mut sh = Mesh::default();
            let a = (90.0 * sin * fade) as u8;
            for (x, al) in [(edge, a), (edge + reach, 0)] {
                sh.colored_vertex(pos2(x, rect.top()), Color32::from_black_alpha(al));
                sh.colored_vertex(pos2(x, rect.bottom()), Color32::from_black_alpha(al));
            }
            sh.add_triangle(0, 1, 2);
            sh.add_triangle(1, 3, 2);
            painter.add(Shape::mesh(sh));
        }
        painter.add(Shape::mesh(mesh));

        if front {
            // Text: tessellate the galley, then squash/lift the vertices with the paper.
            let ctx = ui.ctx();
            let galley = self.layout(ctx, page, sc);
            let font_tex = ctx.fonts(|f| f.font_image_size());
            let mut tess = Tessellator::new(ctx.pixels_per_point(), TessellationOptions::default(), font_tex, vec![]);
            let mut text_mesh = Mesh::default();
            tess.tessellate_shape(Shape::galley(pos2(MARGIN * sc, MARGIN * sc), galley, INK), &mut text_mesh);
            let shade = 1.0 - 0.28 * sin * 0.5 - 0.10 * sin;
            for v in &mut text_mesh.vertices {
                let p = map(v.pos.x, rect.top() + v.pos.y);
                v.pos = p;
                let c = v.color;
                v.color = alpha(Color32::from_rgba_premultiplied(
                    (f32::from(c.r()) * shade) as u8,
                    (f32::from(c.g()) * shade) as u8,
                    (f32::from(c.b()) * shade) as u8,
                    c.a(),
                ));
            }
            painter.add(Shape::mesh(text_mesh));
        }
    }

    fn dock_rect(area: Rect, top: bool) -> Rect {
        let width = (area.width() - 48.0).min(940.0);
        let y = if top { area.top() + DOCK_GAP + DOCK_H / 2.0 } else { area.bottom() - DOCK_GAP - DOCK_H / 2.0 };
        Rect::from_center_size(pos2(area.center().x, y), vec2(width, DOCK_H))
    }

    fn paint_dock(ui: &egui::Ui, rect: Rect) {
        let p = ui.painter();
        for k in 1..=5 {
            let g = rect.expand(k as f32 * 2.0).translate(vec2(0.0, 4.0));
            p.rect_filled(g, 16.0 + k as f32 * 2.0, Color32::from_black_alpha(20));
        }
        p.rect_filled(rect, 16.0, DOCK);
        p.rect_stroke(rect, 16.0, Stroke::new(1.0, DOCK_EDGE), egui::StrokeKind::Inside);
    }

    fn format_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let rect = Self::dock_rect(area, true);
        Self::paint_dock(ui, rect);
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(rect.shrink2(vec2(14.0, 0.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                self.toolbox(ui);
            },
        );
    }

    fn page_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let rect = Self::dock_rect(area, false);
        Self::paint_dock(ui, rect);
        self.scrollbar(ui, rect.shrink2(vec2(18.0, 0.0)));
    }

    /// Hold Ctrl and drag to move the page around.
    fn pan_with_ctrl(&mut self, ui: &mut egui::Ui, area: Rect) {
        // The reported modifier state sometimes says "Ctrl released" while it is still held
        // (seen on Hyprland), but the key press/release events are reliable. Track those instead.
        let now = ui.input(|i| i.time);
        let debug = std::env::var_os("CAPRICE_DEBUG").is_some();
        let was = self.ctrl_down;
        ui.input(|i| {
            for e in &i.events {
                match e {
                    egui::Event::Key { key: Key::ControlLeft | Key::ControlRight, pressed, .. } => self.ctrl_down = *pressed,
                    egui::Event::ModifiersChanged(m) if m.ctrl => self.ctrl_down = true,
                    egui::Event::WindowFocused(false) => self.ctrl_down = false,
                    _ => {}
                }
            }
        });
        if debug && was != self.ctrl_down {
            eprintln!("[{now:.3}] ctrl {}", if self.ctrl_down { "down" } else { "up" });
        }
        if !self.ctrl_down {
            return;
        }
        let view = Rect::from_min_max(pos2(area.left(), area.top() + RESERVED), pos2(area.right(), area.bottom() - RESERVED));
        // Registered after the text editor, so it wins the drag.
        let resp = ui.interact(view, Id::new("pan"), Sense::drag());
        // Don't rely on `resp.hovered()`: it can be false for a frame right after a drag ends,
        // which made the hand cursor flicker back to the default one.
        let over = ui.input(|i| i.pointer.latest_pos()).is_some_and(|p| view.contains(p));
        if over || resp.dragged() {
            ui.output_mut(|o| {
                o.cursor_icon = if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab }
            });
        }
        if resp.dragged() {
            self.origin += resp.drag_delta();
            self.fit = false;
            ui.ctx().request_repaint();
        }
    }

    /// Pinch / Ctrl+scroll to zoom around the pointer, two-finger scroll to pan. Returns the page rectangle.
    fn page_rect(&mut self, ctx: &egui::Context, area: Rect) -> Rect {
        let view = Rect::from_min_max(
            pos2(area.left(), area.top() + RESERVED),
            pos2(area.right(), area.bottom() - RESERVED),
        );
        let fit = ((view.height() - 16.0) / PAGE_H).min((view.width() - 48.0) / PAGE_W).clamp(MIN_ZOOM, 3.0);
        let (pinch, scroll, hover) =
            ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
        let keys = ctx.input_mut(|i| {
            let mut z = 1.0;
            if i.consume_key(Modifiers::COMMAND, Key::Equals) || i.consume_key(Modifiers::COMMAND, Key::Plus) {
                z *= 1.1;
            }
            if i.consume_key(Modifiers::COMMAND, Key::Minus) {
                z /= 1.1;
            }
            z
        });
        let reset = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Num0));

        if reset {
            self.fit = true;
        }
        if self.fit {
            self.zoom = fit;
            self.origin = pos2(view.center().x - PAGE_W * fit / 2.0, view.center().y - PAGE_H * fit / 2.0);
        }
        let z = pinch * keys;
        if (z - 1.0).abs() > 1e-4 {
            let anchor = hover.filter(|p| view.contains(*p)).unwrap_or(view.center());
            let new = (self.zoom * z).clamp(MIN_ZOOM, MAX_ZOOM);
            self.origin = anchor + (self.origin - anchor) * (new / self.zoom);
            self.zoom = new;
            self.fit = (new - fit).abs() < 0.02;
        }
        if !self.fit {
            self.origin += scroll;
        }

        // The page can be moved anywhere as long as a good part of it stays in view.
        let (w, h) = (PAGE_W * self.zoom, PAGE_H * self.zoom);
        let keep = 80.0;
        self.origin = pos2(
            self.origin.x.clamp(view.left() + keep - w, view.right() - keep),
            self.origin.y.clamp(view.top() + keep - h, view.bottom() - keep),
        );
        Rect::from_min_size(self.origin, vec2(w, h))
    }

    fn scrollbar(&mut self, ui: &mut egui::Ui, r: Rect) {
        let n = self.pages.len();
        let cy = r.center().y;

        let label_w = 92.0;
        ui.painter().text(
            pos2(r.left(), cy),
            egui::Align2::LEFT_CENTER,
            format!("Page {} of {}", self.pos.round() as usize + 1, n),
            FontId::proportional(13.0),
            TEXT_DIM,
        );
        let fit_btn = Rect::from_center_size(pos2(r.right() - 24.0, cy), vec2(48.0, 30.0));
        let btn = Rect::from_center_size(pos2(fit_btn.left() - 56.0 - 12.0 - 52.0, cy), vec2(104.0, 30.0));
        if ui.put(btn, egui::Button::new("+ New page")).clicked() {
            let cur = self.target;
            self.insert_page_after(cur);
        }

        if ui.put(fit_btn, egui::Button::new("Fit").selected(self.fit)).on_hover_text("Show the full page (Ctrl+0). Ctrl+drag moves the page.").clicked() {
            self.fit = true;
        }
        ui.painter().text(
            pos2(fit_btn.left() - 8.0, cy),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}%", self.zoom * 100.0),
            FontId::proportional(13.0),
            TEXT_DIM,
        );

        let track = Rect::from_min_max(pos2(r.left() + label_w + 8.0, cy - 4.0), pos2(btn.left() - 20.0, cy + 4.0));
        if track.width() < 60.0 {
            return;
        }
        ui.painter().rect_filled(track, 4.0, Color32::from_rgb(30, 32, 38));
        let thumb_w = (track.width() / n as f32).clamp(40.0, track.width());
        let travel = track.width() - thumb_w;
        if n > 1 && n <= 80 {
            for k in 0..n {
                let x = track.left() + thumb_w / 2.0 + travel * k as f32 / (n - 1) as f32;
                ui.painter().circle_filled(pos2(x, cy), 1.3, Color32::from_white_alpha(45));
            }
        }

        let resp = ui.interact(track.expand2(vec2(6.0, 12.0)), Id::new("scroll"), Sense::click_and_drag());
        if n > 1 && (resp.dragged() || resp.drag_started() || resp.is_pointer_button_down_on()) {
            if let Some(p) = resp.interact_pointer_pos() {
                let f = ((p.x - track.left() - thumb_w / 2.0) / travel).clamp(0.0, 1.0);
                self.pos = f * (n - 1) as f32;
                self.target = self.pos.round() as usize;
                self.scrubbing = true;
                self.cursor_req = Some((self.target, 0));
            }
        } else {
            self.scrubbing = false;
        }

        let f = if n > 1 { self.pos / (n - 1) as f32 } else { 0.0 };
        let thumb = Rect::from_min_size(pos2(track.left() + travel * f, cy - 8.0), vec2(thumb_w, 16.0));
        let hot = resp.hovered() || self.scrubbing;
        ui.painter().rect_filled(thumb, 8.0, if hot { ACCENT } else { Color32::from_rgb(112, 124, 196) });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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

        // Global shortcuts.
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
            self.insert_page_after(cur);
        }

        self.animate(ui);
        self.pos = self.pos.clamp(0.0, self.last() as f32);

        let page_rect = self.page_rect(&ctx, area);

        let base = self.pos.floor() as usize;
        let t = self.pos - base as f32;
        let n = self.pages.len();

        if t < 1e-3 || base >= self.last() {
            // Settled: this page is editable.
            let i = (self.pos.round() as usize).min(self.last());
            // May merge/remove pages, so work out which page to show only afterwards.
            self.handle_boundary_keys(&ctx, i);
            let i = (self.pos.round() as usize).min(self.last());
            Self::stack(ui.painter(), page_rect, i, self.last() - i);
            Self::paper(ui.painter(), page_rect);
            self.edit_page(ui, page_rect, i);
        } else {
            // Mid-flip: page `base` turns over, revealing `base + 1`.
            Self::stack(ui.painter(), page_rect, base, n - 1 - base - 1);
            self.static_page(ui, page_rect, &self.pages[base + 1]);
            let eased = t * t * (3.0 - 2.0 * t);
            let fade = 1.0 - ((t - 0.78) / 0.22).clamp(0.0, 1.0);
            self.flipping_page(ui, page_rect, &self.pages[base], eased * std::f32::consts::PI, fade);
        }

        self.pan_with_ctrl(ui, area);
        self.format_bar(ui, area);
        self.page_bar(ui, area);
        self.normalize(&ctx);
        if self.cursor_req.is_some() {
            ctx.request_repaint();
        }
    }
}

impl App {
    fn edit_page(&mut self, ui: &mut egui::Ui, page_rect: Rect, i: usize) {
        let ctx = ui.ctx().clone();
        let id = Self::page_id(i);
        let sc = page_rect.width() / PAGE_W;
        let content = Rect::from_min_size(page_rect.min + vec2(MARGIN, MARGIN) * sc, vec2(CONTENT_W, CONTENT_H) * sc);

        if let Some((p, idx)) = self.cursor_req {
            if p == i && !self.scrubbing {
                let mut state = TextEditState::load(&ctx, id).unwrap_or_default();
                let idx = idx.min(self.pages[i].chars());
                state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(idx))));
                state.store(&ctx, id);
                ctx.memory_mut(|m| m.request_focus(id));
                self.cursor_req = None;
            }
        } else if !ctx.memory(|m| m.has_focus(id)) && !self.scrubbing {
            ctx.memory_mut(|m| m.request_focus(id));
        }

        let typing = self.typing.clone();
        let Page { text, rich } = &mut self.pages[i];
        let rich = &*rich;
        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            let mut r = rich.borrow_mut();
            r.sync(buf.as_str(), &typing);
            let job = build_job(buf.as_str(), &r.styles, &typing, sc);
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let edit = TextEdit::multiline(text)
            .id(id)
            .font(FontId::proportional(FONT_SIZE * sc))
            .text_color(INK)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(CONTENT_W * sc)
            .min_size(Vec2::new(CONTENT_W, CONTENT_H) * sc)
            .lock_focus(true)
            .layouter(&mut layouter);
        ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
            ui.add(edit);
        });
        let text_now = self.pages[i].text.clone();
        self.pages[i].rich.get_mut().sync(&text_now, &typing);

        // The toolbox follows the character before the cursor.
        if let Some(c) = Self::cursor_of(&ctx, i) {
            if self.last_cursor != Some((i, c)) {
                self.last_cursor = Some((i, c));
                let rich = self.pages[i].rich.get_mut();
                if let Some(st) = rich.styles.get(c.saturating_sub(1)) {
                    self.typing = st.clone();
                }
            }
        }
    }

    // ---------------------------------------------------------------- toolbox

    /// Set a style property on the selection (if any) and on what gets typed next.
    fn apply(&mut self, ctx: &egui::Context, edit: impl Fn(&mut Style)) {
        edit(&mut self.typing);
        let i = self.target.min(self.last());
        let Some(range) = TextEditState::load(ctx, Self::page_id(i)).and_then(|s| s.cursor.char_range()) else {
            return;
        };
        let (a, b) = (usize::from(range.primary.index), usize::from(range.secondary.index));
        let (a, b) = (a.min(b), a.max(b));
        for st in self.pages[i].rich.get_mut().styles.iter_mut().skip(a).take(b - a) {
            edit(st);
        }
        ctx.request_repaint();
    }

    fn toolbox(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut font_pick = None;
        let (mut bold, mut underline, mut size) = (self.typing.bold, self.typing.underline, self.typing.size);
        let small = vec2(32.0, 30.0);

        egui::ComboBox::from_id_salt("font")
            .selected_text(self.typing.font.to_string())
            .width(170.0)
            .show_ui(ui, |ui| {
                for (f, _) in &self.fonts.families {
                    if ui.selectable_label(*f == *self.typing.font, f).clicked() {
                        font_pick = Some(f.clone());
                    }
                }
            });
        ui.add_sized(
            [64.0, 30.0],
            egui::DragValue::new(&mut size).range(6.0..=96.0).speed(0.15).max_decimals(1).suffix(" pt"),
        );
        let b = egui::Button::new(egui::RichText::new("B").strong()).selected(bold).min_size(small);
        bold ^= ui.add(b).clicked();
        let u = egui::Button::new(egui::RichText::new("U").underline()).selected(underline).min_size(small);
        underline ^= ui.add(u).clicked();

        ui.add_space(8.0);
        let save = ui.button("Save").clicked();
        let open = ui.button("Open").clicked();

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let name = self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or("Untitled".to_owned(), |n| n.to_string_lossy().into_owned());
            ui.label(egui::RichText::new(format!("{name} {}", self.status)).size(12.5).color(TEXT_DIM));
        });

        if let Some(f) = font_pick {
            self.fonts.ensure(&ctx, &f, false);
            let f: Arc<str> = f.into();
            self.apply(&ctx, |s| s.font = f.clone());
        }
        if size != self.typing.size {
            self.apply(&ctx, |s| s.size = size);
        }
        if bold != self.typing.bold {
            self.apply(&ctx, |s| s.bold = bold);
        }
        if underline != self.typing.underline {
            self.apply(&ctx, |s| s.underline = underline);
        }
        if save {
            self.save(false);
        }
        if open {
            self.open(&ctx);
        }
    }

    // --------------------------------------------------------------- file i/o

    fn save(&mut self, save_as: bool) {
        let path = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let Some(p) = rfd::FileDialog::new().add_filter("Caprice document", &["caprice"]).set_file_name("Untitled.caprice").save_file() else {
                    return;
                };
                p
            }
        };
        let json = serde_json::to_string_pretty(&DocFile::from_pages(&self.pages));
        self.status = match json.map_err(|e| e.to_string()).and_then(|j| std::fs::write(&path, j).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.path = Some(path);
                "- saved".into()
            }
            Err(e) => format!("- save failed: {e}"),
        };
    }

    fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("Caprice document", &["caprice"]).pick_file() else {
            return;
        };
        let doc = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<DocFile>(&s).map_err(|e| e.to_string()));
        match doc {
            Ok(doc) => {
                self.pages = doc.into_pages();
                let fonts: BTreeSet<String> = self
                    .pages
                    .iter()
                    .flat_map(|p| p.rich.borrow().styles.iter().map(|s| s.font.to_string()).collect::<Vec<_>>())
                    .collect();
                for f in fonts {
                    self.fonts.ensure(ctx, &f, false);
                }
                if let Some(st) = self.pages[0].rich.get_mut().styles.first() {
                    self.typing = st.clone();
                }
                self.pos = 0.0;
                self.target = 0;
                self.last_cursor = None;
                self.cursor_req = Some((0, 0));
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
    fn sync_tracks_insert_and_delete() {
        let plain = Style::new("x");
        let bold = Style { bold: true, ..plain.clone() };
        let mut r = Rich { styles: vec![plain.clone(), bold.clone(), plain.clone()], synced: "abc".into() };
        r.sync("abZc", &bold);
        assert_eq!(r.styles, vec![plain.clone(), bold.clone(), bold.clone(), plain.clone()]);
        r.sync("ac", &plain);
        assert_eq!(r.styles, vec![plain.clone(), plain.clone()]);
        r.sync("", &plain);
        assert!(r.styles.is_empty());
    }

    #[test]
    fn file_roundtrip() {
        let st = Style { bold: true, ..Style::new("Foo") };
        let pages = vec![Page::new("hi\nyo".into(), vec![st.clone(); 5])];
        let json = serde_json::to_string(&DocFile::from_pages(&pages)).unwrap();
        let back = serde_json::from_str::<DocFile>(&json).unwrap().into_pages();
        assert_eq!(back[0].text, "hi\nyo");
        assert_eq!(back[0].rich.borrow().styles, vec![st; 5]);
    }
}
