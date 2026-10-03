//! Painting pages: paper, page stacks, static pages and the page-turn animation.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};
use egui::epaint::{Mesh, TessellationOptions, Tessellator, WHITE_UV};

use crate::App;
use crate::notes::{NoteLook, mirror_note, note_color, note_rect};
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

    /// Page-point offset between neighbouring sheets for a document of `n` pages. The whole pile
    /// is `max * n / (n + STACK_KNEE)` thick: a few pages get thick, clearly separate sheets, and
    /// the thickness then grows ever more slowly towards a tenth of the page width.
    fn sheet_gap(&self, n: usize) -> f32 {
        const STACK_KNEE: f32 = 18.0;
        let max = self.doc.setup.size().x / 10.0;
        max / (n as f32 + STACK_KNEE)
    }

    /// Screen points between neighbouring sheets of the piles beside a page at `rect`.
    fn stack_gap(&self, ctx: &egui::Context, rect: Rect) -> f32 {
        let target = self.sheet_gap(self.doc.pages());
        // Ease the spacing when pages come and go, so the pile visibly thins or thickens.
        ctx.animate_value_with_time(egui::Id::new("page-stack-gap"), target, 0.3) * self.scale_of(rect)
    }

    /// Where the page last turned over lies in the pile to the left of the page at `rect`.
    fn turned_rect(&self, ctx: &egui::Context, rect: Rect) -> Rect {
        rect.translate(vec2(-1.0, 0.5) * self.stack_gap(ctx, rect))
    }

    /// The post-its of page `i`, the one last turned over: they stick out to the left now, and lie
    /// on top of the page so what is written on them can still be read.
    pub fn paint_turned_notes(&self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, i: usize) {
        let r = self.turned_rect(ctx, rect);
        let look = NoteLook { shade: 0.96, ..NoteLook::FLAT };
        let sc = self.scale_of(r);
        for &(k, y) in self.note_places(ctx).get(&i).into_iter().flatten() {
            let note = &self.doc.notes[k];
            painter.extend(self.note_shapes(ctx, note, mirror_note(r, note_rect(r, sc, y)), sc, self.pad(note.id).sheet as f32, &look));
        }
    }

    /// Stack hints: every sheet piled to the right (pages ahead) and left (pages behind).
    pub fn stack(&self, painter: &egui::Painter, rect: Rect, behind: usize, ahead: usize) {
        let ctx = painter.ctx();
        let gap = self.stack_gap(ctx, rect);
        let gap_px = gap * ctx.pixels_per_point();
        // Sheets closer than ~1.5 px would smear into one dark edge: draw every `step`th one only,
        // and fade the edge lines as they crowd, leaving a soft blur for long documents.
        let step = (1.5 / gap_px).ceil().max(1.0) as usize;
        let line = (gap_px / 3.0).clamp(0.25, 1.0);

        // Post-its of the pages in the pile peek out of it: to the right from pages ahead, and
        // (those pages being turned over) to the left from pages behind. Those of the page just
        // ahead show their writing; those of the page just behind are drawn over the page instead
        // (`paint_turned_notes`).
        let sc = self.scale_of(rect);
        let places = if self.doc.notes.is_empty() { Default::default() } else { self.note_places(ctx) };
        let n = self.doc.pages();

        let side = |count: usize, dir: egui::Vec2, fill: Color32, alpha: f32, page_at: &dyn Fn(usize) -> usize, turned: bool| {
            for k in (1..=count).rev() {
                let r = rect.translate(dir * k as f32 * gap);
                let notes = if turned && k == 1 { None } else { places.get(&page_at(k)) };
                for &(note, y) in notes.into_iter().flatten() {
                    let mut pr = note_rect(r, sc, y);
                    if turned {
                        pr = mirror_note(r, pr);
                    }
                    let n = &self.doc.notes[note];
                    if !turned && k == 1 {
                        // The next page's post-its keep their writing: the pages on top hide the rest.
                        let look = NoteLook { shade: 0.96, ..NoteLook::FLAT };
                        painter.extend(self.note_shapes(ctx, n, pr, sc, self.pad(n.id).sheet as f32, &look));
                        continue;
                    }
                    painter.rect_filled(pr, 1.0, note_color(n.color).gamma_multiply(if turned { 0.8 } else { 0.92 }).to_opaque());
                    painter.rect_stroke(pr, 1.0, Stroke::new(0.6, Color32::from_black_alpha(40)), egui::StrokeKind::Inside);
                }
                if k == count || k % step == 0 {
                    painter.rect_filled(r, 1.0, fill);
                    let edge = Color32::from_black_alpha((alpha * line) as u8);
                    painter.rect_stroke(r, 1.0, Stroke::new(0.6, edge), egui::StrokeKind::Inside);
                }
            }
        };
        side(ahead, vec2(1.0, 0.64), Color32::from_rgb(232, 230, 224), 60.0, &|k| n - ahead + k - 1, false);
        side(behind, vec2(-1.0, 0.5), Color32::from_rgb(226, 224, 218), 50.0, &|k| behind - k, true);
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
        let layout = self.page_layout(ui.ctx(), i, sc);
        self.paint_layout(ui.painter(), &layout, rect.min + self.doc.setup.margin_origin() * sc);
        self.draw_footer(ui, rect, i);
        self.paint_notes(ui.ctx(), ui.painter(), rect, i, &NoteLook::FLAT);
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
        let on_page = |p: Pos2| map(p.x - spine, p.y);
        let shade = 1.0 - 0.28 * sin * 0.5 - 0.10 * sin;
        painter.add(Shape::mesh(mesh));
        if !front {
            // Seen from behind, the post-its still show what is written on them.
            let look = NoteLook { map: &on_page, shade, alpha: fade, mirrored: true, no_text: false };
            self.paint_notes(ui.ctx(), painter, rect, i, &look);
        }

        if front {
            // Text: tessellate the galleys, then squash/lift the vertices with the paper.
            let ctx = ui.ctx();
            let page = self.page_layout(ctx, i, sc);
            let font_tex = ctx.fonts(|f| f.font_image_size());
            let mut tess = Tessellator::new(ctx.pixels_per_point(), TessellationOptions::default(), font_tex, vec![]);
            let mut text_mesh = Mesh::default();
            let origin = self.doc.setup.margin_origin() * sc;
            for p in &page.paras {
                if let Some((g, at)) = &p.marker {
                    tess.tessellate_shape(Shape::galley(pos2(origin.x + at.x, origin.y + p.y + at.y), g.clone(), INK), &mut text_mesh);
                }
                if p.image.is_none() {
                    tess.tessellate_shape(Shape::galley(pos2(origin.x + p.x, origin.y + p.y), p.galley.clone(), INK), &mut text_mesh);
                }
            }
            if let Some((g, at)) = self.footer(ctx, i, sc) {
                tess.tessellate_shape(Shape::galley(at, g, INK), &mut text_mesh);
            }
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

            // Pictures are textured quads that follow the paper the same way.
            for p in &page.paras {
                let (Some(img), Some(tex)) = (p.image, p.image.and_then(|i| self.textures.get(&i.id))) else { continue };
                let r = img.rect.translate(vec2(origin.x, origin.y + p.y));
                let tint = alpha(Color32::from_rgb(
                    (255.0 * shade) as u8,
                    (255.0 * shade) as u8,
                    (255.0 * shade) as u8,
                ));
                let mut quad = Mesh::with_texture(tex.id());
                const N: usize = 8; // a few strips so it bends with the page
                for k in 0..=N {
                    let f = k as f32 / N as f32;
                    let x = r.left() + f * r.width();
                    let u = f;
                    for (y, v) in [(r.top(), 0.0), (r.bottom(), 1.0)] {
                        let pos = map(x, rect.top() + y);
                        quad.vertices.push(egui::epaint::Vertex { pos, uv: crate::images::rotated_uv(u, v, img.rotation), color: tint });
                    }
                    if k > 0 {
                        let b = (k * 2) as u32;
                        quad.add_triangle(b - 2, b - 1, b);
                        quad.add_triangle(b - 1, b + 1, b);
                    }
                }
                painter.add(Shape::mesh(quad));
            }

            let look = NoteLook { map: &on_page, shade, alpha: fade, mirrored: false, no_text: false };
            self.paint_notes(ctx, painter, rect, i, &look);
        }
    }
}
