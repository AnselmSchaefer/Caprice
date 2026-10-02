//! The two docks: formatting on top, pages at the bottom.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Id, Rect, Sense, Stroke, UiBuilder, pos2, vec2};

use crate::App;
use crate::model::{CM, Orientation};
use crate::theme::{ACCENT, DOCK, DOCK_EDGE, TEXT_DIM};

pub const DOCK_H: f32 = 48.0;
pub const DOCK_GAP: f32 = 14.0;
/// Space at the top/bottom of the window taken by the two docks.
pub const RESERVED: f32 = DOCK_GAP + DOCK_H + 10.0;

impl App {
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

    pub fn format_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
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

        ui.add_space(4.0);
        let new_setup = self.page_menu(ui);

        ui.add_space(4.0);
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
        if let Some(setup) = new_setup {
            self.set_setup(&ctx, setup);
        }
        if save {
            self.save(false);
        }
        if open {
            self.open(&ctx);
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
            format!("Page {} of {}", self.pos.round() as usize + 1, n),
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
