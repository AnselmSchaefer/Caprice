//! The two docks: formatting on top, pages at the bottom.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Id, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::App;
use crate::model::{Align, CM, ListKind, Orientation};
use crate::theme::{ACCENT, BTN, BTN_HOVER, DOCK, DOCK_EDGE, TEXT, TEXT_DIM};

pub const DOCK_H: f32 = 48.0;
pub const DOCK_GAP: f32 = 14.0;
/// Space at the top/bottom of the window taken by the two docks.
pub const RESERVED: f32 = DOCK_GAP + DOCK_H + 10.0;
/// Widest the docks get.
pub const DOCK_MAX_W: f32 = 1180.0;
/// A paragraph-format change picked in the toolbar.
#[derive(Clone, Copy)]
enum ParaChange {
    Align(Align),
    Spacing(f32),
    List(ListKind),
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

impl App {
    pub fn dock_rect(area: Rect, top: bool) -> Rect {
        let width = (area.width() - 48.0).min(DOCK_MAX_W);
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

    /// The formatting dock: one row of tools that slides sideways when the window is too narrow,
    /// with an arrow at each end.
    pub fn format_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let rect = Self::dock_rect(area, true);
        Self::paint_dock(ui, rect);
        let inner = rect.shrink2(vec2(14.0, 0.0));
        let overflow = self.toolbar_w > inner.width() + 0.5;
        let arrow_w = 28.0;
        let view = if overflow { inner.shrink2(vec2(arrow_w + 6.0, 0.0)) } else { inner };
        let max_scroll = (self.toolbar_w - view.width()).max(0.0);

        if overflow {
            // Wheel / two-finger scrolling over the bar slides it too.
            let over_bar = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p));
            if over_bar {
                let d = ui.input(|i| i.smooth_scroll_delta);
                let step = if d.x.abs() > d.y.abs() { d.x } else { d.y };
                self.toolbar_target = (self.toolbar_target - step).clamp(0.0, max_scroll);
            }
            for (left, ar) in [(true, Rect::from_min_size(inner.min, vec2(arrow_w, inner.height()))),
                               (false, Rect::from_min_size(pos2(inner.right() - arrow_w, inner.top()), vec2(arrow_w, inner.height())))] {
                let can = if left { self.toolbar_target > 0.5 } else { self.toolbar_target < max_scroll - 0.5 };
                let btn = Rect::from_center_size(ar.center(), vec2(arrow_w, 30.0));
                let resp = ui.interact(btn, Id::new(("toolbar_arrow", left)), if can { Sense::click() } else { Sense::hover() });
                let fill = if can && resp.hovered() { BTN_HOVER } else { BTN };
                let ink = if can { TEXT } else { TEXT_DIM.gamma_multiply(0.5) };
                let p = ui.painter();
                p.rect_filled(btn, 8.0, fill);
                let c = btn.center();
                let dx = if left { -1.0 } else { 1.0 };
                let s = Stroke::new(1.8, ink);
                p.line_segment([pos2(c.x - 2.5 * dx, c.y - 5.0), pos2(c.x + 2.5 * dx, c.y)], s);
                p.line_segment([pos2(c.x + 2.5 * dx, c.y), pos2(c.x - 2.5 * dx, c.y + 5.0)], s);
                if can && resp.clicked() {
                    let by = view.width() * 0.6 * if left { -1.0 } else { 1.0 };
                    self.toolbar_target = (self.toolbar_target + by).clamp(0.0, max_scroll);
                }
            }
        } else {
            self.toolbar_target = 0.0;
        }

        // Ease towards the target offset.
        let dt = ui.input(|i| i.stable_dt).min(0.05);
        let diff = self.toolbar_target - self.toolbar_scroll;
        if diff.abs() > 0.3 {
            self.toolbar_scroll += diff * (1.0 - (-dt * 16.0).exp());
            ui.ctx().request_repaint();
        } else {
            self.toolbar_scroll = self.toolbar_target;
        }
        self.toolbar_scroll = self.toolbar_scroll.clamp(0.0, max_scroll);

        // The tools themselves, laid out on a very wide strip and clipped to the visible window.
        let strip = Rect::from_min_size(pos2(view.left() - self.toolbar_scroll, view.top()), vec2(4000.0, view.height()));
        let mut used = 0.0;
        ui.scope_builder(
            UiBuilder::new().max_rect(strip).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(view.expand2(vec2(0.0, 6.0))));
                ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
                self.toolbox(ui);
                used = ui.min_rect().width();
            },
        );
        // The document name sits at the right end, but only in room the tools leave over, so it can
        // never be the reason for the arrows (it is shortened, and hidden if there is hardly any room).
        if !overflow {
            let spare = inner.right() - (view.left() + self.toolbar_w) - 24.0;
            if spare > 70.0 {
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map_or("Untitled".to_owned(), |n| n.to_string_lossy().into_owned());
                let mut job = egui::text::LayoutJob::simple_singleline(name, FontId::proportional(13.0), TEXT_DIM);
                job.wrap.max_width = spare;
                job.wrap.max_rows = 1;
                job.wrap.break_anywhere = true;
                job.wrap.overflow_character = Some('\u{2026}');
                job.halign = egui::Align::RIGHT;
                let galley = ui.painter().layout_job(job);
                let at = pos2(inner.right(), rect.center().y - galley.size().y / 2.0);
                ui.painter().galley(at, galley, TEXT_DIM);
            }
        }

        // Measured this frame, used the next; a changed width asks for one more frame.
        if (used - self.toolbar_w).abs() > 0.5 {
            self.toolbar_w = used;
            ui.ctx().request_repaint();
        }
    }

    pub fn page_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        let rect = Self::dock_rect(area, false);
        Self::paint_dock(ui, rect);
        self.scrollbar(ui, rect.shrink2(vec2(18.0, 0.0)));
    }

    fn toolbox(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut font_pick = None;
        let (mut bold, mut underline, mut size) = (self.typing.bold, self.typing.underline, self.typing.size);
        let small = vec2(32.0, 30.0);

        let mut file_action = None;
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
                    file_action = Some(action);
                }
            }
        });

        ui.add_space(2.0);

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

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Paragraph format of the paragraph the caret is in. Alignment (with line spacing) and
        // lists are one dropdown each, to save room.
        let attrs = self.doc.para_attrs_at(self.caret);
        let mut para_change = None;
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
                        para_change = Some(ParaChange::Align(a));
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
                        para_change = Some(ParaChange::Spacing(s));
                    }
                }
            });
        });
        let list_icon = if attrs.list == ListKind::Numbered { Icon::Numbers } else { Icon::Bullets };
        icon_dropdown(ui, list_icon, attrs.list != ListKind::None, "Lists", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icon_button(ui, Icon::Bullets, attrs.list == ListKind::Bullet, "Bulleted list (Ctrl+Shift+L)").clicked() {
                    para_change = Some(ParaChange::List(ListKind::Bullet));
                }
                if icon_button(ui, Icon::Numbers, attrs.list == ListKind::Numbered, "Numbered list").clicked() {
                    para_change = Some(ParaChange::List(ListKind::Numbered));
                }
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);
        let new_setup = self.page_menu(ui);

        let mut image_action = None;
        let selected_image = self.selected_image().is_some();
        egui::containers::menu::MenuButton::new("Image").ui(ui, |ui| {
            ui.set_min_width(190.0);
            if ui.add(egui::Button::new("Insert picture…").frame(false)).clicked() {
                image_action = Some(ImageAction::Insert);
            }
            ui.add_enabled_ui(selected_image, |ui| {
                ui.separator();
                for (label, fraction) in [("Small  (25% of width)", 0.25), ("Medium  (50%)", 0.5), ("Large  (75%)", 0.75), ("Full width", 1.0)] {
                    if ui.add(egui::Button::new(label).frame(false)).clicked() {
                        image_action = Some(ImageAction::Width(fraction));
                    }
                }
            });
        });
        let add_note = ui.button("Note").on_hover_text("Attach a note to the selection or line (Ctrl+Alt+N)").clicked();
        let find = ui.button("Find").on_hover_text("Search (Ctrl+F)").clicked();
        let pen_tip = "Draw a loop around text to ask Claude about it (Ctrl+Shift+P, Esc to stop)";
        let pen = ui.add(egui::Button::new("Claude").selected(self.pen)).on_hover_text(pen_tip).clicked();
        self.scene_menu(ui);

        if let Some(f) = font_pick {
            self.fonts.ensure(&ctx, &f, false);
            let f: Arc<str> = f.into();
            self.apply_style(&ctx, |s| s.font = f.clone());
        }
        if size != self.typing.size {
            self.apply_style(&ctx, |s| s.size = size);
        }
        if bold != self.typing.bold {
            self.apply_style(&ctx, |s| s.bold = bold);
        }
        if underline != self.typing.underline {
            self.apply_style(&ctx, |s| s.underline = underline);
        }
        match para_change {
            Some(ParaChange::Align(a)) => self.set_para(&ctx, |p| p.align = a),
            Some(ParaChange::Spacing(s)) => self.set_para(&ctx, |p| p.spacing = s),
            Some(ParaChange::List(kind)) => {
                // Clicking the active kind turns the list off.
                let kind = if attrs.list == kind { ListKind::None } else { kind };
                self.set_para(&ctx, |p| p.list = kind);
            }
            None => {}
        }
        if let Some(setup) = new_setup {
            self.set_setup(&ctx, setup);
        }
        if add_note {
            self.add_note(&ctx);
        }
        if find {
            self.open_search();
        }
        if pen {
            self.toggle_pen();
        }
        match image_action {
            Some(ImageAction::Insert) => {
                self.ask_path(&ctx, crate::fileio::DialogFor::Picture, || {
                    rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg"]).pick_file()
                });
            }
            Some(ImageAction::Width(f)) => self.set_image_width_fraction(&ctx, f),
            None => {}
        }
        match file_action {
            Some(FileAction::Open) => self.open(&ctx),
            Some(FileAction::Save) => self.save(&ctx, false),
            Some(FileAction::SaveAs) => self.save(&ctx, true),
            Some(FileAction::ExportDocx) => self.export_docx(&ctx),
            None => {}
        }
    }

    /// Orientation, paper, margins and page numbers. Returns the new setup if something changed.
    fn page_menu(&mut self, ui: &mut egui::Ui) -> Option<crate::model::PageSetup> {
        let mut setup = self.doc.setup.clone();
        egui::containers::menu::MenuButton::new("Page")
            .config(
                egui::containers::menu::MenuConfig::new()
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
            )
            .ui(ui, |ui| {
                ui.set_min_width(250.0);
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
            });
        (setup != self.doc.setup).then_some(setup)
    }

    pub fn scrollbar(&mut self, ui: &mut egui::Ui, r: Rect) {
        let n = self.doc.pages();
        let cy = r.center().y;

        let label_w = 92.0;
        ui.painter().text(
            pos2(r.left(), cy),
            egui::Align2::LEFT_CENTER,
            format!("Page {} of {}", self.scrub_to.unwrap_or(self.pos.round() as usize) + 1, n),
            FontId::proportional(13.0),
            TEXT_DIM,
        );
        let fit_btn = Rect::from_center_size(pos2(r.right() - 24.0, cy), vec2(48.0, 30.0));
        let btn = Rect::from_center_size(pos2(fit_btn.left() - 56.0 - 12.0 - 52.0, cy), vec2(104.0, 30.0));
        if ui.put(btn, egui::Button::new("+ New page")).clicked() {
            let cur = self.target;
            let ctx = ui.ctx().clone();
            self.insert_page_after(&ctx, cur);
        }

        let fit_tip = "Show the full page (Ctrl+0). Ctrl+drag moves the page.";
        if ui.put(fit_btn, egui::Button::new("Fit").selected(self.fit)).on_hover_text(fit_tip).clicked() {
            self.fit = true;
            self.fit_settling = true;
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

        // Dragging (or pressing) only picks the page: the pages stay put, their corner curls, and
        // letting go turns there in one go.
        let resp = ui.interact(track.expand2(vec2(6.0, 12.0)), Id::new("scroll"), Sense::click_and_drag());
        self.last_track = (cy, track.left() + thumb_w / 2.0, track.right() - thumb_w / 2.0);
        let mut f = if last > 0.0 { (self.pos + going) / last } else { 0.0 };
        if n > 1 && (resp.dragged() || resp.drag_started() || resp.is_pointer_button_down_on()) {
            if let Some(p) = resp.interact_pointer_pos() {
                f = ((p.x - track.left() - thumb_w / 2.0) / travel).clamp(0.0, 1.0);
                self.scrub_to = Some((f * (n - 1) as f32).round() as usize);
            }
        } else if let Some(to) = self.scrub_to.take() {
            let ctx = ui.ctx().clone();
            self.flip_bundle_to(&ctx, to.min(self.last()));
        }

        let thumb = Rect::from_min_size(pos2(track.left() + travel * f, cy - 8.0), vec2(thumb_w, 16.0));
        let hot = resp.hovered() || self.scrub_to.is_some();
        ui.painter().rect_filled(thumb, 8.0, if hot { ACCENT } else { Color32::from_rgb(112, 124, 196) });
    }
}
