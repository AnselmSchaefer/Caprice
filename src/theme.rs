use eframe::egui::{self, Color32, Stroke, vec2};

pub const INK: Color32 = Color32::from_rgb(28, 28, 32);
pub const DESK: Color32 = Color32::from_rgb(30, 32, 38);
pub const DOCK: Color32 = Color32::from_rgb(42, 45, 54);
pub const DOCK_EDGE: Color32 = Color32::from_rgb(62, 66, 78);
pub const BTN: Color32 = Color32::from_rgb(55, 59, 71);
pub const BTN_HOVER: Color32 = Color32::from_rgb(68, 73, 88);
pub const TEXT: Color32 = Color32::from_rgb(222, 225, 232);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 146, 160);
pub const ACCENT: Color32 = Color32::from_rgb(143, 162, 255);
pub const PAPER: Color32 = Color32::from_rgb(253, 252, 249);

pub fn apply_theme(ctx: &egui::Context) {
    // Caprice only has a dark theme; don't let the OS's light/dark setting pick between
    // this and an unstyled default (that mismatch is what made menus look so rough).
    ctx.set_theme(egui::ThemePreference::Dark);
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
