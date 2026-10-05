//! Painting pages: paper, page stacks, static pages and the page-turn animation.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Shape, Stroke, pos2, vec2};
use egui::epaint::{Mesh, TessellationOptions, Tessellator, WHITE_UV};

use crate::App;
use crate::notes::{NoteLook, mirror_note, note_color, note_rect};
use crate::theme::{INK, PAPER};

/// Which way, per sheet, the piles of pages ahead and behind spread out from under the page.
const AHEAD: egui::Vec2 = egui::vec2(1.0, 0.64);
const BEHIND: egui::Vec2 = egui::vec2(-1.0, 0.5);
/// How much further a curled corner reaches down the side of the page than along its top.
pub const CURL_TALL: f32 = 1.3;
/// Colour of the turned-over sheets in the pile behind the page.
const BEHIND_FILL: Color32 = Color32::from_rgb(226, 224, 218);

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

    /// The post-its of the pages in the piles beside the page at `rect`, deepest first: each one's
    /// id and page, and the part of it that sticks out from under the pages on top.
    pub fn pile_note_hits(&self, ctx: &egui::Context, rect: Rect, behind: usize, ahead: usize) -> Vec<(u64, usize, Rect)> {
        if self.doc.notes.is_empty() {
            return Vec::new();
        }
        let (gap, sc) = (self.stack_gap(ctx, rect), self.scale_of(rect));
        let places = self.note_places(ctx);
        let n = self.doc.pages();
        let mut hits = Vec::new();
        let mut add = |page: usize, visible: &dyn Fn(Rect) -> Rect, r: Rect| {
            for &(k, y) in places.get(&page).into_iter().flatten() {
                hits.push((self.doc.notes[k].id, page, visible(note_rect(r, sc, y))));
            }
        };
        for k in (1..=ahead).rev() {
            let above = rect.translate(AHEAD * (k - 1) as f32 * gap).right();
            let r = rect.translate(AHEAD * k as f32 * gap);
            add(n - ahead + k - 1, &|pr| Rect::from_min_max(pos2(pr.left().max(above), pr.top()), pr.max), r);
        }
        for k in (1..=behind).rev() {
            let above = rect.translate(BEHIND * (k - 1) as f32 * gap).left();
            let r = rect.translate(BEHIND * k as f32 * gap);
            add(k - 1, &|pr| {
                let pr = mirror_note(r, pr);
                Rect::from_min_max(pr.min, pos2(pr.right().min(above), pr.bottom()))
            }, r);
        }
        hits
    }

    /// Every how many sheets the piles beside a page at `rect` draw one, so they don't smear together.
    fn pile_step(&self, ctx: &egui::Context, rect: Rect) -> usize {
        let gap_px = self.stack_gap(ctx, rect) * ctx.pixels_per_point();
        (1.5 / gap_px).ceil().max(1.0) as usize
    }

    /// Stack hints: every sheet piled to the right (pages ahead) and left (pages behind). A turned
    /// page slides into the back of the left pile, so page `k - 1` lies `k` sheets out.
    pub fn stack(&self, painter: &egui::Painter, rect: Rect, behind: usize, ahead: usize) {
        let ctx = painter.ctx();
        let gap = self.stack_gap(ctx, rect);
        let gap_px = gap * ctx.pixels_per_point();
        // Sheets closer than ~1.5 px would smear into one dark edge: draw every `step`th one only,
        // and fade the edge lines as they crowd, leaving a soft blur for long documents.
        let step = self.pile_step(ctx, rect);
        let line = (gap_px / 3.0).clamp(0.25, 1.0);

        // Post-its of the pages in the pile peek out of it: to the right from pages ahead, and
        // (those pages being turned over) to the left from pages behind.
        let sc = self.scale_of(rect);
        let places = if self.doc.notes.is_empty() { Default::default() } else { self.note_places(ctx) };
        let n = self.doc.pages();

        let side = |count: usize, dir: egui::Vec2, fill: Color32, alpha: f32, page_at: &dyn Fn(usize) -> usize, turned: bool| {
            for k in (1..=count).rev() {
                let r = rect.translate(dir * k as f32 * gap);
                for &(note, y) in places.get(&page_at(k)).into_iter().flatten() {
                    let n = &self.doc.notes[note];
                    let pr = note_rect(r, sc, y);
                    if turned {
                        // Turned over, a post-it shows its blank back.
                        let pr = mirror_note(r, pr);
                        painter.rect_filled(pr, 1.0, note_color(n.color).gamma_multiply(0.8).to_opaque());
                        painter.rect_stroke(pr, 1.0, Stroke::new(0.6, Color32::from_black_alpha(40)), egui::StrokeKind::Inside);
                    } else {
                        // It keeps its writing: the pages on top hide the rest.
                        let look = NoteLook { shade: 0.96, ..NoteLook::FLAT };
                        painter.extend(self.note_shapes(ctx, n, pr, sc, self.pad(n.id).sheet as f32, &look));
                    }
                }
                if k == count || k % step == 0 {
                    painter.rect_filled(r, 1.0, fill);
                    let edge = Color32::from_black_alpha((alpha * line) as u8);
                    painter.rect_stroke(r, 1.0, Stroke::new(0.6, edge), egui::StrokeKind::Inside);
                }
            }
        };
        side(ahead, AHEAD, Color32::from_rgb(232, 230, 224), 60.0, &|k| n - ahead + k - 1, false);
        side(behind, BEHIND, BEHIND_FILL, 50.0, &|k| k - 1, true);
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
        ui.painter().extend(self.backdrop_shapes(ui.ctx(), rect, &|p| p, 1.0, 1.0));
        let sc = self.scale_of(rect);
        let layout = self.page_layout(ui.ctx(), i, sc);
        self.paint_layout(ui.painter(), &layout, rect.min + self.doc.setup.margin_origin() * sc);
        self.draw_footer(ui, rect, i);
        self.paint_notes(ui.ctx(), ui.painter(), rect, i, &NoteLook::FLAT);
    }

    /// The top corner of the page at `rect` curled back over the page by `size` (screen points
    /// along the top edge), on the right or the left, uncovering the paper of the page underneath.
    pub fn corner_curl(&self, painter: &egui::Painter, rect: Rect, size: f32, right: bool) {
        let a = size.min(rect.width() * 0.9);
        let b = (size * CURL_TALL).min(rect.height() * 0.9);
        // Worked out from the corner inwards, so the same lines make either corner.
        let x = |dx: f32| if right { rect.right() - dx } else { rect.left() + dx };
        let corner = pos2(x(0.0), rect.top());
        let (fa, fb) = (pos2(x(a), rect.top()), pos2(x(0.0), rect.top() + b));
        // The corner folded over the line from `fa` to `fb`.
        let along = (fb - fa).normalized();
        let v = corner - fa;
        let tip = fa + 2.0 * v.dot(along) * along - v;
        let shade = |k: f32| Color32::from_rgb(
            (f32::from(PAPER.r()) * k) as u8,
            (f32::from(PAPER.g()) * k) as u8,
            (f32::from(PAPER.b()) * k) as u8,
        );
        let triangle = |pts: [(Pos2, Color32); 3]| {
            let mut m = Mesh::default();
            for (p, c) in pts {
                m.colored_vertex(p, c);
            }
            m.add_triangle(0, 1, 2);
            Shape::mesh(m)
        };
        // The page underneath, in the flap's shadow along the fold.
        painter.add(triangle([(fa, shade(0.8)), (corner, shade(0.97)), (fb, shade(0.8))]));
        // The flap's own shadow on the page, a little beyond its tip.
        let out = (tip - fa.lerp(fb, 0.5)) * 0.12;
        let soft = Color32::from_black_alpha(0);
        painter.add(triangle([(fa, Color32::from_black_alpha(45)), (tip + out, soft), (fb, Color32::from_black_alpha(45))]));
        // The flap: the back of the page, catching the light where it bends.
        painter.add(triangle([(fa, shade(0.99)), (tip, shade(0.86)), (fb, shade(0.99))]));
        let edge = Stroke::new(0.6, Color32::from_black_alpha(35));
        painter.line_segment([fa, tip], edge);
        painter.line_segment([tip, fb], edge);
    }

    /// Pages `lo` up to `hi` turning over together to uncover `hi`, `t` going from 0 to 1: the
    /// bundle turns, fanned a little to show its thickness, then slides into the back of the pile
    /// behind. One page (`hi = lo + 1`) is an ordinary flip.
    pub fn bundle_flip(&self, ui: &mut egui::Ui, rect: Rect, lo: usize, hi: usize, t: f32) {
        use std::f32::consts::{FRAC_PI_2, PI};
        let ease = |s: f32| s * s * (3.0 - 2.0 * s);
        let ahead = self.last() - hi;
        if t < crate::TURN {
            // Turning, it is above everything, covering the pile and its post-its as it comes down.
            self.stack(ui.painter(), rect, lo, ahead);
            self.static_page(ui, rect, hi);
            const LAG: f32 = 0.06;
            let (count, k) = (hi - lo, (hi - lo).min(5));
            let s = t / crate::TURN;
            let mut sheets: Vec<(f32, usize)> = (0..k)
                .map(|j| {
                    let sj = ((s - j as f32 * LAG) / (1.0 - (k - 1) as f32 * LAG)).clamp(0.0, 1.0);
                    (ease(sj) * PI, lo + j * count / k)
                })
                .collect();
            // Down on the left, the first sheet to land lies underneath; up on the right, the one
            // turned furthest is on top.
            sheets.sort_by(|x, y| {
                let (xl, yl) = (x.0 > FRAC_PI_2, y.0 > FRAC_PI_2);
                yl.cmp(&xl).then(if xl { y.0.total_cmp(&x.0) } else { x.0.total_cmp(&y.0) })
            });
            for (theta, page) in sheets {
                self.flipping_page(ui, rect, page, theta);
            }
        } else {
            // Then it slides into the back of the pile, under it and the page uncovered. Leaving the
            // top of the pile, it lets the pile and its post-its show through bit by bit.
            let slide = (t - crate::TURN) / (1.0 - crate::TURN);
            let from = vec2(-rect.width(), 0.0);
            let step = self.pile_step(ui.ctx(), rect);
            for k in (lo + 1..=hi).rev().filter(|&k| k == hi || k % step == 0) {
                self.sheet_to_pile(ui, rect, k - 1, from, ease(slide));
            }
            self.stack(ui.painter(), rect, lo, ahead);
            let on_top = 1.0 - (slide / 0.5).min(1.0);
            if on_top > 0.0 {
                ui.scope(|ui| {
                    ui.multiply_opacity(on_top);
                    self.sheet_to_pile(ui, rect, hi - 1, from, ease(slide));
                });
            }
            self.static_page(ui, rect, hi);
        }
    }

    /// Draw page `i` turned over, sliding by `s` from `from` (an offset from `rect`) into its place
    /// at the back of the pile behind it. Drawn before the pile and the page at `rect`, so it passes
    /// under them.
    pub fn sheet_to_pile(&self, ui: &egui::Ui, rect: Rect, i: usize, from: egui::Vec2, s: f32) {
        let ctx = ui.ctx();
        let gap = self.stack_gap(ctx, rect);
        let sheet = rect.translate(from + (BEHIND * gap * (i + 1) as f32 - from) * s);
        // `flipping_page` lays a page turned all the way over to the left of its hinge.
        self.flipping_page(ui, sheet.translate(vec2(sheet.width(), 0.0)), i, std::f32::consts::PI);
        // The pile's edge line, coming in as the sheet arrives.
        let line = (gap * ctx.pixels_per_point() / 3.0).clamp(0.25, 1.0);
        let edge = Color32::from_black_alpha((50.0 * line * s) as u8);
        ui.painter().rect_stroke(sheet, 1.0, Stroke::new(0.6, edge), egui::StrokeKind::Inside);
    }

    /// Draw page `i` hinged on its left edge, turned by `theta` (0 = flat, PI = fully flipped).
    pub fn flipping_page(&self, ui: &egui::Ui, rect: Rect, i: usize, theta: f32) {
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
            let mut c = Color32::from_rgb(
                (f32::from(PAPER.r()) * shade) as u8,
                (f32::from(PAPER.g()) * shade) as u8,
                (f32::from(PAPER.b()) * shade) as u8,
            );
            if !front {
                // Lying down, the back takes on the colour of the pile it joins.
                c = c.lerp_to_gamma(BEHIND_FILL, -cos);
            }
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
            let a = (90.0 * sin) as u8;
            for (x, al) in [(edge, a), (edge + reach, 0)] {
                sh.colored_vertex(pos2(x, rect.top()), Color32::from_black_alpha(al));
                sh.colored_vertex(pos2(x, rect.bottom()), Color32::from_black_alpha(al));
            }
            sh.add_triangle(0, 1, 2);
            sh.add_triangle(1, 3, 2);
            painter.add(Shape::mesh(sh));
        }
        // Seen from behind, the post-its are under the paper: only what sticks out shows.
        let on_page = |p: Pos2| map(p.x - spine, p.y);
        let shade = 1.0 - 0.28 * sin * 0.5 - 0.10 * sin;
        if !front {
            let look = NoteLook { map: &on_page, shade, back: true, ..NoteLook::FLAT };
            self.paint_notes(ui.ctx(), painter, rect, i, &look);
        }
        painter.add(Shape::mesh(mesh));

        if front {
            painter.extend(self.backdrop_shapes(ui.ctx(), rect, &on_page, shade, 1.0));
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
                v.color = Color32::from_rgba_premultiplied(
                    (f32::from(c.r()) * shade) as u8,
                    (f32::from(c.g()) * shade) as u8,
                    (f32::from(c.b()) * shade) as u8,
                    c.a(),
                );
            }
            painter.add(Shape::mesh(text_mesh));

            // Pictures are textured quads that follow the paper the same way.
            for p in &page.paras {
                let (Some(img), Some(tex)) = (p.image, p.image.and_then(|i| self.textures.get(&i.id))) else { continue };
                let r = img.rect.translate(vec2(origin.x, origin.y + p.y));
                let tint = Color32::from_rgb(
                    (255.0 * shade) as u8,
                    (255.0 * shade) as u8,
                    (255.0 * shade) as u8,
                );
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

            let look = NoteLook { map: &on_page, shade, ..NoteLook::FLAT };
            self.paint_notes(ctx, painter, rect, i, &look);
        }
    }
}
