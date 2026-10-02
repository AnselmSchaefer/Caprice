//! Painting pages: paper, page stacks, static pages and the page-turn animation.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};
use egui::epaint::{Mesh, TessellationOptions, Tessellator, WHITE_UV};

use crate::App;
use crate::layout::layout;
use crate::theme::{INK, PAPER};

impl App {
    /// Screen points per page point for a page drawn into `rect`.
    pub fn scale_of(&self, rect: Rect) -> f32 {
        rect.width() / self.doc.setup.size().x
    }

    pub fn paper(painter: &egui::Painter, rect: Rect) {
        for s in 1..=6u8 {
            let r = rect.expand(f32::from(s) * 2.5);
            painter.rect_filled(r.translate(vec2(0.0, 6.0)), 2.0, Color32::from_black_alpha(14));
        }
        painter.rect_filled(rect, 1.0, PAPER);
    }

    /// Stack hints: sheets piled to the right (pages ahead) and left (pages behind).
    pub fn stack(&self, painter: &egui::Painter, rect: Rect, behind: usize, ahead: usize) {
        let sc = self.scale_of(rect);
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

    /// The page number, and where it goes relative to the page's top-left (None if turned off).
    fn footer(&self, ctx: &egui::Context, i: usize, sc: f32) -> Option<(Arc<egui::Galley>, Pos2)> {
        if !self.doc.setup.page_numbers {
            return None;
        }
        let galley = ctx.fonts_mut(|f| {
            f.layout((i + 1).to_string(), FontId::proportional(10.0 * sc), Color32::from_gray(110), f32::INFINITY)
        });
        let size = self.doc.setup.size() * sc;
        let x = (size.x - galley.size().x) / 2.0;
        let y = size.y - self.doc.setup.margin_bottom * sc / 2.0 - galley.size().y / 2.0;
        Some((galley, pos2(x, y)))
    }

    pub fn draw_footer(&self, ui: &egui::Ui, rect: Rect, i: usize) {
        if let Some((galley, at)) = self.footer(ui.ctx(), i, self.scale_of(rect)) {
            ui.painter().galley(rect.min + at.to_vec2(), galley, INK);
        }
    }

    pub fn static_page(&self, ui: &egui::Ui, rect: Rect, i: usize) {
        Self::paper(ui.painter(), rect);
        let sc = self.scale_of(rect);
        let galley = layout(ui.ctx(), self.doc.page_job(i, &self.typing, sc));
        ui.painter().galley(rect.min + self.doc.setup.margin_origin() * sc, galley, INK);
        self.draw_footer(ui, rect, i);
    }

    /// Draw page `i` hinged on its left edge, turned by `theta` (0 = flat, PI = fully flipped).
    pub fn flipping_page(&self, ui: &egui::Ui, rect: Rect, i: usize, theta: f32, fade: f32) {
        let painter = ui.painter();
        let (sin, cos) = theta.sin_cos();
        let pw = rect.width();
        let sc = self.scale_of(rect);
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
            // Text: tessellate the galleys, then squash/lift the vertices with the paper.
            let ctx = ui.ctx();
            let galley = layout(ctx, self.doc.page_job(i, &self.typing, sc));
            let font_tex = ctx.fonts(|f| f.font_image_size());
            let mut tess = Tessellator::new(ctx.pixels_per_point(), TessellationOptions::default(), font_tex, vec![]);
            let mut text_mesh = Mesh::default();
            let origin = self.doc.setup.margin_origin() * sc;
            tess.tessellate_shape(Shape::galley(pos2(origin.x, origin.y), galley, INK), &mut text_mesh);
            if let Some((g, at)) = self.footer(ctx, i, sc) {
                tess.tessellate_shape(Shape::galley(at, g, INK), &mut text_mesh);
            }
            let shade = 1.0 - 0.28 * sin * 0.5 - 0.10 * sin;
            for v in &mut text_mesh.vertices {
                v.pos = map(v.pos.x, rect.top() + v.pos.y);
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
}
