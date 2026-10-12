//! The two docks: formatting on top, pages at the bottom.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Id, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::{App, Appearance};
use crate::model::{Align, CM, ListKind, Orientation};
use crate::theme::{ACCENT, BTN, BTN_HOVER, DOCK, DOCK_EDGE, TEXT, TEXT_DIM};

pub const DOCK_H: f32 = 48.0;
pub const DOCK_GAP: f32 = 14.0;
/// Space at the top/bottom of the window taken by the two docks.
pub const RESERVED: f32 = DOCK_GAP + DOCK_H + 10.0;
/// The smallest the window gets: the docks then still hold File, "⋯" and a little of the name on
/// top, and Scratchpad, Fit, the zoom and "+" below.
pub const MIN_WINDOW: [f32; 2] = [340.0, 400.0];
/// Widest the docks get.
pub const DOCK_MAX_W: f32 = 1180.0;
/// The toolbar's groups of tools, in the order they show; the last ones go into "⋯" first.
/// File, font, letters, paragraph, page and image, note and find, Claude and scenes.
pub const GROUPS: usize = 7;
/// Room between two tools, or two groups.
const TOOL_GAP: f32 = 8.0;
/// The width of the font list.
const FONT_W: f32 = 128.0;
/// The "⋯" button's width.
const MORE_W: f32 = 30.0;
/// The height of a row in the second dock.
const MORE_LINE: f32 = 40.0;
/// Room between the tools and the document's name.
const NAME_GAP: f32 = 16.0;
/// How fast a name too long for its room runs past, in points a second.
const NAME_SPEED: f32 = 18.0;
/// How long a name too long for its room stands still between passes, in seconds.
const NAME_PAUSE: f32 = 3.0;
/// A paragraph-format change picked in the toolbar.
#[derive(Clone, Copy)]
enum ParaChange {
    Align(Align),
    Spacing(f32),
    List(ListKind),
    ChapterTitle(bool),
}

#[derive(Clone, Copy)]
enum Icon {
    Align(Align),
    Bullets,
    Numbers,
}

/// Draw an icon into a 30x30 square.
fn paint_icon(p: &egui::Painter, rect: Rect, icon: Icon, ink: Color32) {
    let stroke = Stroke::new(1.6, ink);
    let area = rect.shrink2(vec2(8.0, 8.0));
    match icon {
        Icon::Align(a) => {
            for (k, w) in [1.0, 0.62, 0.86, 0.5].into_iter().enumerate() {
                let w = if a == Align::Justify { 1.0 } else { w } * area.width();
                let x0 = match a {
                    Align::Left | Align::Justify => area.left(),
                    Align::Center => area.center().x - w / 2.0,
                    Align::Right => area.right() - w,
                };
                let y = area.top() + 1.0 + k as f32 * (area.height() - 2.0) / 3.0;
                p.line_segment([pos2(x0, y), pos2(x0 + w, y)], stroke);
            }
        }
        Icon::Bullets | Icon::Numbers => {
            for k in 0..3 {
                let y = area.top() + 2.0 + k as f32 * (area.height() - 4.0) / 2.0;
                if matches!(icon, Icon::Bullets) {
                    p.circle_filled(pos2(area.left() + 1.5, y), 1.7, ink);
                } else {
                    p.text(pos2(area.left() + 1.0, y), egui::Align2::LEFT_CENTER, (k + 1).to_string(), FontId::proportional(8.0), ink);
                }
                p.line_segment([pos2(area.left() + 6.5, y), pos2(area.right(), y)], stroke);
            }
        }
    }
}

/// A square toolbar button with a drawn icon.
fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, tip: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let fill = if selected { ACCENT } else if resp.hovered() { BTN_HOVER } else { BTN };
    let ink = if selected { Color32::from_rgb(20, 22, 30) } else { TEXT };
    ui.painter().rect_filled(rect, 8.0, fill);
    paint_icon(ui.painter(), rect, icon, ink);
    resp.on_hover_text(tip)
}

/// A toolbar button showing `icon` and a small arrow; clicking it opens a popup with more choices.
fn icon_dropdown(ui: &mut egui::Ui, icon: Icon, active: bool, tip: &str, popup: impl FnOnce(&mut egui::Ui)) {
    let (rect, resp) = ui.allocate_exact_size(vec2(46.0, 30.0), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let fill = if active { ACCENT } else if resp.hovered() || open { BTN_HOVER } else { BTN };
    let ink = if active { Color32::from_rgb(20, 22, 30) } else { TEXT };
    ui.painter().rect_filled(rect, 8.0, fill);
    paint_icon(ui.painter(), Rect::from_min_size(rect.min, vec2(30.0, 30.0)), icon, ink);
    let c = pos2(rect.right() - 9.0, rect.center().y);
    let s = Stroke::new(1.5, ink);
    ui.painter().line_segment([pos2(c.x - 3.5, c.y - 1.5), pos2(c.x, c.y + 2.0)], s);
    ui.painter().line_segment([pos2(c.x, c.y + 2.0), pos2(c.x + 3.5, c.y - 1.5)], s);
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(popup);
    if !open {
        resp.on_hover_text(tip);
    }
}

/// Text colours offered in the toolbar, named for their tooltips. Dark enough to read on paper.
/// Black is the usual ink (no colour of its own), to go back to it.
const TEXT_COLORS: [(&str, Option<[u8; 3]>); 11] = [
    ("Black", None),
    ("Dark red", Some([150, 28, 28])),
    ("Red", Some([205, 40, 40])),
    ("Orange", Some([215, 110, 15])),
    ("Gold", Some([180, 135, 0])),
    ("Green", Some([40, 125, 60])),
    ("Teal", Some([0, 120, 125])),
    ("Blue", Some([35, 90, 200])),
    ("Navy", Some([25, 40, 110])),
    ("Purple", Some([115, 50, 160])),
    ("Grey", Some([105, 105, 110])),
];

fn ink_of(color: Option<[u8; 3]>) -> Color32 {
    color.map_or(crate::theme::INK, |[r, g, b]| Color32::from_rgb(r, g, b))
}

/// The text colour button: an "A" over a bar in the colour of the text at the caret (light for
/// the usual ink, which would not show on the dock). Its popup sets a colour: `Some(None)` is the
/// usual ink again. Any other colour is picked in the popup itself: a picker opening its own popup
/// closed this one on the first click, as a click outside it.
fn color_dropdown(ui: &mut egui::Ui, color: Option<[u8; 3]>) -> Option<Option<[u8; 3]>> {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Text colour"));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    ui.painter().rect_filled(rect, 8.0, if resp.hovered() || open { BTN_HOVER } else { BTN });
    let font = egui::FontId::proportional(15.0);
    ui.painter().text(rect.center() - vec2(0.0, 3.0), egui::Align2::CENTER_CENTER, "A", font, TEXT);
    let bar = Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 7.0), vec2(16.0, 3.5));
    ui.painter().rect_filled(bar, 1.0, color.map_or(TEXT, |_| ink_of(color)));
    let mut pick = None;
    egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(6.0 * 26.0);
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (name, c) in TEXT_COLORS {
                let (r, swatch) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                swatch.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
                ui.painter().rect_filled(r, 4.0, ink_of(c));
                if color == c || swatch.hovered() {
                    ui.painter().rect_stroke(r.expand(1.5), 5.0, Stroke::new(1.5, TEXT), egui::StrokeKind::Outside);
                }
                if swatch.on_hover_text(name).clicked() {
                    pick = Some(c);
                    ui.close();
                }
            }
        });
        egui::CollapsingHeader::new("Other colour").show(ui, |ui| {
            let mut other = ink_of(color);
            if egui::color_picker::color_picker_color32(ui, &mut other, egui::color_picker::Alpha::Opaque) {
                pick = Some(Some([other.r(), other.g(), other.b()]));
            }
        });
    });
    if !open {
        resp.on_hover_text("Text colour");
    }
    pick
}

#[derive(Clone, Copy)]
enum ImageAction {
    Insert,
    Width(f32),
}

#[derive(Clone, Copy)]
enum FileAction {
    Open,
    Save,
    SaveAs,
    ExportDocx,
}

/// What was clicked in the tools this frame, done once they are all drawn.
#[derive(Default)]
struct Tools {
    file: Option<FileAction>,
    font: Option<String>,
    size: Option<f32>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<Option<[u8; 3]>>,
    para: Option<ParaChange>,
    setup: Option<crate::model::PageSetup>,
    image: Option<ImageAction>,
    add_note: bool,
    find: bool,
}

/// The "⋯" button that opens the second row of tools.
fn more_button(ui: &mut egui::Ui, open: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(MORE_W, 30.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More tools"));
    let fill = if open { ACCENT } else if resp.hovered() { BTN_HOVER } else { BTN };
    let ink = if open { Color32::from_rgb(20, 22, 30) } else { TEXT };
    ui.painter().rect_filled(rect, 8.0, fill);
    for k in [-1.0, 0.0, 1.0] {
        ui.painter().circle_filled(rect.center() + vec2(k * 5.5, 0.0), 1.7, ink);
    }
    resp.on_hover_text("More tools")
}

impl App {
    pub fn dock_rect(area: Rect, top: bool) -> Rect {
        let width = (area.width() - 48.0).clamp(0.0, DOCK_MAX_W);
        let y = if top { area.top() + DOCK_GAP + DOCK_H / 2.0 } else { area.bottom() - DOCK_GAP - DOCK_H / 2.0 };
        Rect::from_center_size(pos2(area.center().x, y), vec2(width, DOCK_H))
    }

    fn paint_dock(ui: &egui::Ui, rect: Rect) {
        // Too narrow to hold anything, it would be only a blob.
        if rect.width() < 40.0 {
            return;
        }
        let p = ui.painter();
        for k in 1..=5 {
            let g = rect.expand(k as f32 * 2.0).translate(vec2(0.0, 4.0));
            p.rect_filled(g, 16.0 + k as f32 * 2.0, Color32::from_black_alpha(20));
        }
        p.rect_filled(rect, 16.0, DOCK);
        p.rect_stroke(rect, 16.0, Stroke::new(1.0, DOCK_EDGE), egui::StrokeKind::Inside);
    }

    /// The formatting dock: one row of tools in groups, the document's name at its right end. Groups
    /// that do not fit, the last first, go into a second row that "⋯" opens under it.
    pub fn format_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let ctx = ui.ctx().clone();
        let rect = Self::dock_rect(area, true);
        Self::paint_dock(ui, rect);
        let inner = rect.shrink2(vec2(14.0, 0.0));
        let name = self.doc_name();
        let galley = ui.painter().layout_no_wrap(name.clone(), FontId::proportional(13.0), TEXT_DIM);
        // The name is never hidden: when the tools need the room, it keeps about four characters.
        let name_w = galley.size().x;
        let four = (name_w / name.chars().count().max(1) as f32 * 4.0).min(name_w);

        // Which groups fit, by their widths as last drawn.
        let w = self.tool_w;
        let room = inner.width() - NAME_GAP - four;
        let all = w.iter().sum::<f32>() + TOOL_GAP * (GROUPS - 1) as f32;
        let shown = if all <= room + 0.5 {
            GROUPS
        } else {
            let mut used = MORE_W;
            w.iter().take_while(|&&g| { used += g + TOOL_GAP; used <= room + 0.5 }).count()
        };
        // With no room even for "⋯", the tools are put away until the window is wider.
        let more_fits = shown == GROUPS || MORE_W <= room + 0.5;
        if shown == GROUPS || !more_fits {
            self.more_tools = false;
        }

        let mut tools = Tools::default();
        let mut end = inner.left();
        let mut more = None;
        ui.scope_builder(
            UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing = vec2(TOOL_GAP, 8.0);
                for g in 0..shown {
                    self.tool_group(ui, g, g == 0, &mut tools);
                }
                if shown < GROUPS && more_fits {
                    more = Some(more_button(ui, self.more_tools));
                }
                end = ui.min_rect().right();
            },
        );
        let more_rect = more.as_ref().map(|r| r.rect);
        if more.is_some_and(|r| r.clicked()) {
            self.more_tools = !self.more_tools;
        }

        // The rest, in a second dock under the first, in as many rows as they need.
        let mut bottom = rect.bottom();
        if self.more_tools {
            let width = rect.width() - 28.0;
            let mut lines: Vec<Vec<usize>> = vec![vec![]];
            let mut x = 0.0;
            for (g, &gw) in w.iter().enumerate().skip(shown) {
                if x > 0.0 && x + TOOL_GAP + gw > width {
                    lines.push(vec![]);
                    x = 0.0;
                }
                x += if x > 0.0 { TOOL_GAP } else { 0.0 } + gw;
                lines.last_mut().unwrap().push(g);
            }
            let line_w = |l: &Vec<usize>| l.iter().map(|&g| w[g]).sum::<f32>() + TOOL_GAP * (l.len().max(1) - 1) as f32;
            let widest = lines.iter().map(line_w).fold(0.0, f32::max);
            let panel = Rect::from_min_size(
                // Under "⋯", as far as the toolbar reaches.
                pos2((more_rect.map_or(rect.left(), |r| r.left() - 14.0)).min(rect.right() - widest - 28.0).max(rect.left()), rect.bottom() + 8.0),
                vec2(widest + 28.0, lines.len() as f32 * MORE_LINE + 8.0),
            );
            // On a layer of its own, like the windows, so the page under it takes no clicks (R16).
            egui::Area::new(Id::new("more_tools")).order(egui::Order::Middle).fixed_pos(panel.min).show(&ctx, |ui| {
                Self::paint_dock(ui, panel);
                ui.allocate_rect(panel, Sense::click_and_drag());
                for (k, line) in lines.iter().enumerate() {
                    let row = Rect::from_min_size(pos2(panel.left() + 14.0, panel.top() + 4.0 + k as f32 * MORE_LINE), vec2(widest, MORE_LINE));
                    ui.scope_builder(UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                        ui.spacing_mut().item_spacing = vec2(TOOL_GAP, 8.0);
                        for (j, &g) in line.iter().enumerate() {
                            self.tool_group(ui, g, j == 0, &mut tools);
                        }
                    });
                }
            });
            bottom = panel.bottom();
            // A click on the pages closes it; one in a menu opened from it does not.
            let pressed = ui.input(|i| if i.pointer.primary_pressed() { i.pointer.interact_pos() } else { None });
            if let Some(at) = pressed {
                let elsewhere = !panel.contains(at) && !rect.contains(at);
                if elsewhere && ctx.layer_id_at(at).is_none_or(|l| l.order == egui::Order::Background) {
                    self.more_tools = false;
                }
            }
        }
        self.tools_bottom = bottom;
        self.apply_tools(&ctx, tools);

        // The name takes the room right of the tools, no more than it needs; if that is too little
        // for all of it, it runs past slowly, over and over.
        let slot_left = more_rect.map_or(end, |r| r.right()) + NAME_GAP;
        let slot = Rect::from_min_max(pos2(slot_left.max(inner.right() - name_w), inner.top()), inner.max);
        let y = rect.center().y - galley.size().y / 2.0;
        let p = ui.painter().with_clip_rect(ui.clip_rect().intersect(slot.expand2(vec2(0.0, 6.0))));
        if slot.width() <= 0.0 {
            // No room for any of it.
        } else if name_w <= slot.width() + 0.5 {
            p.galley(pos2(inner.right() - name_w, y), galley, TEXT_DIM);
        } else {
            // The gap between one pass and the next is narrower than the room, so the room is never empty.
            let lap = name_w + (NAME_GAP * 2.0).min(slot.width() * 0.6);
            // Each pass begins with the start of the name standing still for a while.
            let t = ui.input(|i| i.time) as f32 % (NAME_PAUSE + lap / NAME_SPEED);
            let x = slot.left() - (t - NAME_PAUSE).max(0.0) * NAME_SPEED;
            p.galley(pos2(x, y), galley.clone(), TEXT_DIM);
            p.galley(pos2(x + lap, y), galley, TEXT_DIM);
            let wait = if t < NAME_PAUSE { NAME_PAUSE - t } else { 0.033 };
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(wait));
        }
    }

    pub fn page_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let rect = Self::dock_rect(area, false);
        Self::paint_dock(ui, rect);
        self.scrollbar(ui, rect.shrink2(vec2(18.0, 0.0)));
    }

    /// Draw tool group `g` (see `GROUPS`), noting what was clicked in `out`, and measure it.
    fn tool_group(&mut self, ui: &mut egui::Ui, g: usize, first: bool, out: &mut Tools) {
        let r = ui.scope(|ui| {
            // The paragraph and page groups are set apart by a line, unless they start a row.
            if !first && (g == 3 || g == 4) {
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);
            }
            match g {
                0 => self.file_tools(ui, out),
                1 => self.font_tools(ui, out),
                2 => self.letter_tools(ui, out),
                3 => self.paragraph_tools(ui, out),
                4 => {
                    out.setup = self.page_menu(ui);
                    self.image_menu(ui, out);
                }
                5 => {
                    out.add_note |= ui.button("Note").on_hover_text("Attach a note to the selection or line (Ctrl+Alt+N)").clicked();
                    out.find |= ui.button("Find").on_hover_text("Search (Ctrl+F)").clicked();
                }
                _ => {
                    self.claude_menu(ui);
                    self.scene_menu(ui);
                }
            }
        });
        // Measured this frame, used the next; a changed width asks for one more frame.
        let width = r.response.rect.width();
        if (width - self.tool_w[g]).abs() > 0.5 {
            self.tool_w[g] = width;
            ui.ctx().request_repaint();
        }
    }

    fn file_tools(&mut self, ui: &mut egui::Ui, out: &mut Tools) {
        egui::containers::menu::MenuButton::new("File").ui(ui, |ui| {
            ui.set_min_width(210.0);
            let items = [
                ("Open…", "Ctrl+O", FileAction::Open),
                ("Save", "Ctrl+S", FileAction::Save),
                ("Save as…", "", FileAction::SaveAs),
                ("Export as Word (.docx)…", "", FileAction::ExportDocx),
            ];
            for (label, shortcut, action) in items {
                let button = egui::Button::new(label).shortcut_text(shortcut).frame(false);
                if ui.add(button).clicked() {
                    out.file = Some(action);
                }
            }
        });
    }

    fn font_tools(&mut self, ui: &mut egui::Ui, out: &mut Tools) {
        egui::ComboBox::from_id_salt("font")
            .selected_text(self.typing.font.to_string())
            .width(FONT_W)
            .show_ui(ui, |ui| {
                for (f, _) in &self.fonts.families {
                    if ui.selectable_label(*f == *self.typing.font, f).clicked() {
                        out.font = Some(f.clone());
                    }
                }
            });
        let mut size = self.typing.size;
        ui.add_sized(
            [58.0, 30.0],
            egui::DragValue::new(&mut size).range(6.0..=96.0).speed(0.15).max_decimals(1).suffix(" pt"),
        );
        if size != self.typing.size {
            out.size = Some(size);
        }
    }

    /// The look of the letters, close together as one group.
    fn letter_tools(&mut self, ui: &mut egui::Ui, out: &mut Tools) {
        let t = &self.typing;
        let small = vec2(28.0, 30.0);
        ui.spacing_mut().item_spacing.x = 3.0;
        let b = egui::Button::new(egui::RichText::new("B").strong()).selected(t.bold).min_size(small);
        out.bold ^= ui.add(b).on_hover_text("Bold (Ctrl+B)").clicked();
        let i = egui::Button::new(egui::RichText::new("I").italics()).selected(t.italic).min_size(small);
        out.italic ^= ui.add(i).on_hover_text("Italic (Ctrl+I)").clicked();
        let u = egui::Button::new(egui::RichText::new("U").underline()).selected(t.underline).min_size(small);
        out.underline ^= ui.add(u).on_hover_text("Underline (Ctrl+U)").clicked();
        out.color = color_dropdown(ui, t.color).or(out.color);
    }

    /// Paragraph format of the paragraph the caret is in. Alignment (with line spacing) and lists
    /// are one dropdown each, to save room.
    fn paragraph_tools(&mut self, ui: &mut egui::Ui, out: &mut Tools) {
        let attrs = self.doc.para_attrs_at(self.caret);
        let para_change = &mut out.para;
        icon_dropdown(ui, Icon::Align(attrs.align), false, "Alignment and line spacing", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let aligns = [
                    (Align::Left, "Align left (Ctrl+L)"),
                    (Align::Center, "Center (Ctrl+E)"),
                    (Align::Right, "Align right (Ctrl+R)"),
                    (Align::Justify, "Justify (Ctrl+J)"),
                ];
                for (a, tip) in aligns {
                    if icon_button(ui, Icon::Align(a), attrs.align == a, tip).clicked() {
                        *para_change = Some(ParaChange::Align(a));
                    }
                }
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new("Line spacing").size(12.0).color(TEXT_DIM));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for s in [1.0_f32, 1.15, 1.5, 2.0, 3.0] {
                    let label = format!("{s:.2}").trim_end_matches('0').trim_end_matches('.').to_owned();
                    if ui.add(egui::Button::new(label).selected((attrs.spacing - s).abs() < 0.01).min_size(vec2(34.0, 28.0))).clicked() {
                        *para_change = Some(ParaChange::Spacing(s));
                    }
                }
            });
        });
        let list_icon = if attrs.list == ListKind::Numbered { Icon::Numbers } else { Icon::Bullets };
        icon_dropdown(ui, list_icon, attrs.list != ListKind::None, "Lists", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icon_button(ui, Icon::Bullets, attrs.list == ListKind::Bullet, "Bulleted list (Ctrl+Shift+L)").clicked() {
                    *para_change = Some(ParaChange::List(ListKind::Bullet));
                }
                if icon_button(ui, Icon::Numbers, attrs.list == ListKind::Numbered, "Numbered list").clicked() {
                    *para_change = Some(ParaChange::List(ListKind::Numbered));
                }
            });
        });

        let chapter = egui::Button::new("Chapter").selected(attrs.is_chapter_title()).min_size(vec2(0.0, 30.0));
        if ui.add(chapter).on_hover_text("Chapter title: starts a new chapter").clicked() {
            *para_change = Some(ParaChange::ChapterTitle(!attrs.is_chapter_title()));
        }
    }

    fn image_menu(&mut self, ui: &mut egui::Ui, out: &mut Tools) {
        let selected_image = self.selected_image().is_some();
        egui::containers::menu::MenuButton::new("Image").ui(ui, |ui| {
            ui.set_min_width(190.0);
            if ui.add(egui::Button::new("Insert picture…").frame(false)).clicked() {
                out.image = Some(ImageAction::Insert);
            }
            ui.add_enabled_ui(selected_image, |ui| {
                ui.separator();
                for (label, fraction) in [("Small  (25% of width)", 0.25), ("Medium  (50%)", 0.5), ("Large  (75%)", 0.75), ("Full width", 1.0)] {
                    if ui.add(egui::Button::new(label).frame(false)).clicked() {
                        out.image = Some(ImageAction::Width(fraction));
                    }
                }
            });
        });
    }

    /// Do what was clicked in the tools, after they are all drawn.
    fn apply_tools(&mut self, ctx: &egui::Context, out: Tools) {
        if let Some(f) = out.font {
            self.fonts.ensure(ctx, &f, false);
            let f: Arc<str> = f.into();
            self.apply_style(ctx, |s| s.font = f.clone());
        }
        if let Some(size) = out.size {
            self.apply_style(ctx, |s| s.size = size);
        }
        if out.bold {
            let bold = !self.typing.bold;
            self.apply_style(ctx, |s| s.bold = bold);
        }
        if out.italic {
            let italic = !self.typing.italic;
            self.apply_style(ctx, |s| s.italic = italic);
        }
        if out.underline {
            let underline = !self.typing.underline;
            self.apply_style(ctx, |s| s.underline = underline);
        }
        if let Some(color) = out.color {
            self.apply_style(ctx, |s| s.color = color);
        }
        match out.para {
            Some(ParaChange::Align(a)) => self.set_para(ctx, |p| p.align = a),
            Some(ParaChange::Spacing(s)) => self.set_para(ctx, |p| p.spacing = s),
            Some(ParaChange::List(kind)) => {
                // Clicking the active kind turns the list off.
                let kind = if self.doc.para_attrs_at(self.caret).list == kind { ListKind::None } else { kind };
                self.set_para(ctx, |p| p.list = kind);
            }
            Some(ParaChange::ChapterTitle(on)) => self.set_para(ctx, |p| p.set_chapter_title(on)),
            None => {}
        }
        if let Some(setup) = out.setup {
            self.set_setup(ctx, setup);
        }
        if out.add_note {
            self.add_note(ctx);
        }
        if out.find {
            self.open_search();
        }
        match out.image {
            Some(ImageAction::Insert) => {
                self.ask_path(ctx, crate::fileio::DialogFor::Picture, || {
                    rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg"]).pick_file()
                });
            }
            Some(ImageAction::Width(f)) => self.set_image_width_fraction(ctx, f),
            None => {}
        }
        match out.file {
            Some(FileAction::Open) => self.open(ctx),
            Some(FileAction::Save) => self.save(ctx, false),
            Some(FileAction::SaveAs) => self.save(ctx, true),
            Some(FileAction::ExportDocx) => self.export_docx(ctx),
            None => {}
        }
    }

    /// The Page menu: new page settings, if they were changed.
    fn page_menu(&mut self, ui: &mut egui::Ui) -> Option<crate::model::PageSetup> {
        let mut setup = self.doc.setup.clone();
        let mut look = self.appearance;
        egui::containers::menu::MenuButton::new("Page")
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                ui.set_min_width(250.0);
                ui.horizontal(|ui| {
                    ui.label("View");
                    ui.selectable_value(&mut look, Appearance::Book, "Book").on_hover_text("Pages turn over like a book's");
                    ui.selectable_value(&mut look, Appearance::Paperstack, "Paperstack").on_hover_text("Pages slide onto a stack");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Orientation");
                    ui.selectable_value(&mut setup.orientation, Orientation::Portrait, "Portrait");
                    ui.selectable_value(&mut setup.orientation, Orientation::Landscape, "Landscape");
                });
                ui.horizontal(|ui| {
                    ui.label("Paper");
                    for (name, w, h) in [("A4", 595.0, 842.0), ("Letter", 612.0, 792.0)] {
                        let on = (setup.paper_w - w).abs() < 1.0 && (setup.paper_h - h).abs() < 1.0;
                        if ui.selectable_label(on, name).clicked() {
                            setup.paper_w = w;
                            setup.paper_h = h;
                        }
                    }
                });
                ui.separator();
                let cm = |ui: &mut egui::Ui, label: &str, v: &mut f32| {
                    ui.label(label);
                    let mut x = *v / CM;
                    let drag = egui::DragValue::new(&mut x).range(0.0..=12.0).speed(0.02).max_decimals(2).suffix(" cm");
                    if ui.add(drag).changed() {
                        *v = x * CM;
                    }
                };
                egui::Grid::new("margins").num_columns(4).show(ui, |ui| {
                    cm(ui, "Top", &mut setup.margin_top);
                    cm(ui, "Bottom", &mut setup.margin_bottom);
                    ui.end_row();
                    cm(ui, "Left", &mut setup.margin_left);
                    cm(ui, "Right", &mut setup.margin_right);
                    ui.end_row();
                });
                ui.separator();
                ui.checkbox(&mut setup.page_numbers, "Page numbers");
                ui.horizontal(|ui| {
                    ui.label("Chapter initials");
                    let tip = "The first letter of each chapter, enlarged to sink into its first lines";
                    ui.selectable_value(&mut setup.drop_cap_lines, 0, "Off").on_hover_text(tip);
                    ui.selectable_value(&mut setup.drop_cap_lines, 2, "2 lines").on_hover_text(tip);
                    ui.selectable_value(&mut setup.drop_cap_lines, 3, "3 lines").on_hover_text(tip);
                });
                ui.checkbox(&mut setup.contents, "Contents pages")
                    .on_hover_text("Pages before the story listing the chapter titles and their pages. Mark titles with the Chapter button.");
            });
        if look != self.appearance {
            self.appearance = look;
            // Whatever was moving finishes the new way.
            (self.held, self.batch) = (0.0, None);
        }
        (setup != self.doc.setup).then_some(setup)
    }

    pub fn scrollbar(&mut self, ui: &mut egui::Ui, r: Rect) {
        let n = self.doc.pages();
        let cy = r.center().y;

        // From the right: Scratchpad, Fit, the zoom, New page; the page count at the left, the track
        // between. When the bar is short, the track goes first, then New page shrinks to "+", then
        // the page count goes, then the zoom, Scratchpad, "+" and Fit, so nothing is drawn over
        // anything or past the bar however narrow the window is.
        let label_w = 92.0;
        let zoom_w = 68.0;
        let tools_w = |new_w: f32, zoom: bool| 100.0 + 8.0 + 48.0 + if zoom { zoom_w } else { 8.0 } + new_w;
        let new_w = if r.width() >= label_w + 20.0 + tools_w(104.0, true) { 104.0 } else { 30.0 };
        let show_label = r.width() >= label_w + 20.0 + tools_w(new_w, true);
        let show_zoom = r.width() >= tools_w(new_w, true);
        let show_scratch = r.width() >= tools_w(30.0, false);
        let show_new = r.width() >= 48.0 + 8.0 + 30.0;
        let show_fit = r.width() >= 48.0;
        if show_label {
            ui.painter().text(
                pos2(r.left(), cy),
                egui::Align2::LEFT_CENTER,
                format!("Page {} of {}", self.scrub_page().filter(|_| self.appearance == Appearance::Paperstack).unwrap_or(self.pos.round() as usize) + 1, n),
                FontId::proportional(13.0),
                TEXT_DIM,
            );
        }
        let scratch_btn = Rect::from_min_max(pos2(r.right() - 100.0, cy - 15.0), pos2(r.right(), cy + 15.0));
        let fit_right = if show_scratch { scratch_btn.left() - 8.0 } else { r.right() };
        let fit_btn = Rect::from_min_max(pos2(fit_right - 48.0, cy - 15.0), pos2(fit_right, cy + 15.0));
        let new_right = fit_btn.left() - if show_zoom { zoom_w } else { 8.0 };
        let btn = Rect::from_min_max(pos2(new_right - new_w, cy - 15.0), pos2(new_right, cy + 15.0));
        let new_page = if new_w > 30.0 { egui::Button::new("+ New page") } else { egui::Button::new("+") };
        if show_new && ui.put(btn, new_page).on_hover_text("New page").clicked() {
            let cur = self.target;
            let ctx = ui.ctx().clone();
            self.insert_page_after(&ctx, cur);
        }
        // With the view's other controls: it is beside the pages, not on them.
        let scratch_tip = "Plain notes on the right, beside the pages, saved with the document";
        let scratch = egui::Button::new("Scratchpad").selected(self.scratchpad_open);
        if show_scratch && ui.put(scratch_btn, scratch).on_hover_text(scratch_tip).clicked() {
            self.toggle_scratchpad();
        }

        let fit_tip = "Show the full page (Ctrl+0). Ctrl+drag moves the page.";
        if show_fit && ui.put(fit_btn, egui::Button::new("Fit").selected(self.fit)).on_hover_text(fit_tip).clicked() {
            self.fit = true;
            self.fit_settling = true;
        }
        if show_zoom {
            ui.painter().text(
                pos2(fit_btn.left() - 8.0, cy),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", self.zoom * 100.0),
                FontId::proportional(13.0),
                TEXT_DIM,
            );
        }

        let track = Rect::from_min_max(pos2(r.left() + label_w + 8.0, cy - 4.0), pos2(btn.left() - 20.0, cy + 4.0));
        if track.width() < 60.0 {
            return;
        }
        ui.painter().rect_filled(track, 4.0, Color32::from_rgb(30, 32, 38));
        // Pages sliding in count only as they arrive, and a page sliding out until it is gone, so
        // the thumb glides instead of jumping when pages come and go.
        let arriving = if self.slide_in.is_some() { (self.target as f32 - self.pos).max(0.0) } else { 0.0 };
        let going = self.slide_out.map_or(0.0, |s| 1.0 - s);
        let last = (n - 1) as f32 - arriving + going;
        let thumb_w = (track.width() / (last + 1.0)).clamp(40.0, track.width());
        let travel = track.width() - thumb_w;
        if last > 0.0 && n <= 80 {
            for k in 0..=last.floor() as usize {
                let x = track.left() + thumb_w / 2.0 + travel * k as f32 / last;
                ui.painter().circle_filled(pos2(x, cy), 1.3, Color32::from_white_alpha(45));
            }
        }

        // Dragging (or pressing) slides the pages half out one by one, following the thumb: right,
        // off the page; left, out of the pile behind. Letting go moves them all there together.
        // A book turns its pages along with the thumb instead.
        let resp = ui.interact(track.expand2(vec2(6.0, 12.0)), Id::new("scroll"), Sense::click_and_drag());
        self.last_track = (cy, track.left() + thumb_w / 2.0, track.right() - thumb_w / 2.0);
        let mut f = if last > 0.0 { (self.pos + going) / last } else { 0.0 };
        if n > 1 && (resp.dragged() || resp.drag_started() || resp.is_pointer_button_down_on()) {
            if let Some(p) = resp.interact_pointer_pos() {
                f = ((p.x - track.left() - thumb_w / 2.0) / travel).clamp(0.0, 1.0);
                self.scrub_to = Some(f * (n - 1) as f32);
                if self.appearance == Appearance::Book {
                    // In a book the pages turn along with the thumb.
                    self.pos = f * (n - 1) as f32;
                    let ctx = ui.ctx().clone();
                    self.turn_to(&ctx, (self.pos.round() as usize).min(self.last()), false);
                }
            }
        } else if let Some(to) = self.scrub_page() {
            self.scrub_to = None;
            if self.appearance == Appearance::Paperstack {
                let ctx = ui.ctx().clone();
                self.let_go_to(&ctx, to);
            }
        }

        let thumb = Rect::from_min_size(pos2(track.left() + travel * f, cy - 8.0), vec2(thumb_w, 16.0));
        let hot = resp.hovered() || self.scrub_to.is_some();
        ui.painter().rect_filled(thumb, 8.0, if hot { ACCENT } else { Color32::from_rgb(112, 124, 196) });
    }
}
