//! Laying text out: one egui text layout per paragraph, so every paragraph can have its own
//! alignment, line spacing and list indent. A page is a stack of paragraph pieces.

use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Rect, Stroke, TextFormat, Vec2, pos2, text::LayoutJob, vec2};
use egui::text::CCursor;

use crate::fonts::family_for;
use crate::model::{Align, Doc, IMAGE_CHAR, ListKind, PAGE_BREAK, ParaAttrs, Style, is_terminator};
use crate::theme::INK;

/// Width reserved for the bullet or number of a list item, in page points.
pub const LIST_INDENT: f32 = 28.0;

/// A background highlight over a range of chars (notes, search results).
#[derive(Clone, Debug)]
pub struct Mark {
    pub range: Range<usize>,
    pub color: Color32,
}

fn run_format(ctx: &egui::Context, st: &Style, scale: f32, background: Color32, spacing: f32) -> TextFormat {
    let font_id = FontId::new(st.size * scale, family_for(&st.font, st.bold));
    let line_height = ((spacing - 1.0).abs() > 0.001).then(|| ctx.fonts_mut(|f| f.row_height(&font_id)) * spacing);
    TextFormat {
        background,
        line_height,
        font_id,
        color: INK,
        underline: if st.underline { Stroke::new(scale.max(1.0), INK) } else { Stroke::NONE },
        ..Default::default()
    }
}

/// Layout job for one paragraph (or the part of it that is on this page).
/// `styles[k]` is the style of the k-th char of `text`. Later `marks` paint over earlier ones.
pub fn paragraph_job(
    ctx: &egui::Context,
    text: &str,
    styles: &[Style],
    term: &Style,
    attrs: ParaAttrs,
    scale: f32,
    wrap_width: f32,
    marks: &[Mark],
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.halign = match attrs.align {
        Align::Left | Align::Justify => egui::Align::LEFT,
        Align::Center => egui::Align::Center,
        Align::Right => egui::Align::RIGHT,
    };
    job.justify = attrs.align == Align::Justify;
    if text.is_empty() {
        // An empty paragraph still has a height: that of the mark that ends it.
        job.append("", 0.0, run_format(ctx, term, scale, Color32::TRANSPARENT, attrs.spacing));
        return job;
    }
    let style_at = |k: usize| styles.get(k).or(styles.last()).unwrap_or(term);
    let mark_at = |k: usize| marks.iter().rev().find(|m| m.range.contains(&k)).map_or(Color32::TRANSPARENT, |m| m.color);
    let (mut start, mut run, mut run_mark) = (0, 0, mark_at(0));
    for (k, (b, _)) in text.char_indices().enumerate() {
        let mark = mark_at(k);
        if k > run && (!style_at(k).same_char(style_at(run)) || mark != run_mark) {
            job.append(&text[start..b], 0.0, run_format(ctx, style_at(run), scale, run_mark, attrs.spacing));
            start = b;
            run = k;
            run_mark = mark;
        }
    }
    job.append(&text[start..], 0.0, run_format(ctx, style_at(run), scale, run_mark, attrs.spacing));
    job
}

/// Where a picture is drawn, relative to the top-left of its paragraph.
#[derive(Clone, Copy, Debug)]
pub struct ImageBox {
    pub id: u32,
    pub rect: Rect,
    /// Clockwise quarter turns.
    pub rotation: u8,
}

/// A row of a paragraph: how many chars it holds and where it is, relative to the paragraph's top.
#[derive(Clone, Copy, Debug)]
pub struct RowGeom {
    pub chars: usize,
    pub y: f32,
    pub height: f32,
}

/// One paragraph (or the page's share of it), laid out.
pub struct ParaLayout {
    /// Page-local char range of the text; `end` is where the terminator sits (or the page ends).
    pub start: usize,
    pub end: usize,
    /// Top-left relative to the page's writing area.
    pub y: f32,
    pub x: f32,
    pub height: f32,
    pub galley: Arc<egui::Galley>,
    /// Bullet or number, with its position relative to the paragraph's own top-left (before `x`).
    pub marker: Option<(Arc<egui::Galley>, Vec2)>,
    pub attrs: ParaAttrs,
    /// Set when the paragraph is a picture.
    pub image: Option<ImageBox>,
    /// Does this piece begin its paragraph (rather than continue one from the previous page)?
    pub starts_paragraph: bool,
}

impl ParaLayout {
    pub fn row_table(&self) -> Vec<RowGeom> {
        if self.image.is_some() {
            return vec![RowGeom { chars: self.end - self.start, y: 0.0, height: self.height }];
        }
        self.galley
            .rows
            .iter()
            .map(|r| RowGeom { chars: usize::from(r.row.char_count_excluding_newline()), y: r.pos.y, height: r.size.y })
            .collect()
    }
}

pub struct PageLayout {
    pub paras: Vec<ParaLayout>,
    pub height: f32,
    pub scale: f32,
}

/// What the layout of a piece is built from.
pub struct PieceSpec<'a> {
    pub text: &'a str,
    pub styles: &'a [Style],
    pub term: &'a Style,
    pub attrs: ParaAttrs,
    pub scale: f32,
    pub content_width: f32,
    pub marks: &'a [Mark],
    /// The text before a page break that has none: takes up no room.
    pub invisible: bool,
    /// Text of the bullet/number, if this piece starts a list item.
    pub marker: Option<String>,
    /// The picture this piece is: its id, its size in page points and its rotation.
    pub image: Option<(u32, Vec2, u8)>,
}

pub fn layout_piece(ctx: &egui::Context, spec: &PieceSpec, local_start: usize, local_end: usize, y: f32) -> ParaLayout {
    let indent = if spec.attrs.list == ListKind::None { 0.0 } else { LIST_INDENT * spec.scale };
    let wrap = (spec.content_width - indent).max(10.0);
    let job = paragraph_job(ctx, spec.text, spec.styles, spec.term, spec.attrs, spec.scale, wrap, spec.marks);
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    if let Some((id, size, rotation)) = spec.image {
        let size = size * spec.scale;
        let x = match spec.attrs.align {
            Align::Left | Align::Justify => 0.0,
            Align::Center => (spec.content_width - size.x) / 2.0,
            Align::Right => spec.content_width - size.x,
        };
        return ParaLayout {
            start: local_start,
            end: local_end,
            y,
            x: 0.0,
            height: size.y,
            galley,
            marker: None,
            attrs: spec.attrs,
            starts_paragraph: false,
            image: Some(ImageBox { id, rect: Rect::from_min_size(pos2(x, 0.0), size), rotation }),
        };
    }
    let galley = true_to_scale(ctx, spec, galley, wrap);
    let height = if spec.invisible { 0.0 } else { galley.size().y };
    let marker = spec.marker.as_ref().map(|text| {
        let st = spec.styles.first().unwrap_or(spec.term);
        let font = FontId::new(st.size * spec.scale, family_for(&st.font, false));
        let g = ctx.fonts_mut(|f| f.layout(text.clone(), font, INK, f32::INFINITY));
        let row = galley.rows.first().map_or((0.0, g.size().y), |r| (r.pos.y, r.size.y));
        let at = vec2(
            (indent - 6.0 * spec.scale - g.size().x).max(0.0),
            row.0 + (row.1 - g.size().y) / 2.0,
        );
        (g, at)
    });
    ParaLayout {
        start: local_start,
        end: local_end,
        y,
        // egui anchors centered and right-aligned text at its middle / right edge.
        x: indent + match spec.attrs.align {
            Align::Center => wrap / 2.0,
            Align::Right => wrap,
            Align::Left | Align::Justify => 0.0,
        },
        height,
        galley,
        marker,
        attrs: spec.attrs,
        starts_paragraph: false,
        image: None,
    }
}

/// egui rounds every line's height to whole pixels, so zoomed text comes out a little shorter or
/// taller than at page size, where the page breaks are worked out. Over a page that adds up to a
/// line or more. Give the paragraph its page-size height times the zoom instead, spreading its
/// lines to match (each placed on a whole pixel, so the text stays crisp).
fn true_to_scale(ctx: &egui::Context, spec: &PieceSpec, galley: Arc<egui::Galley>, wrap: f32) -> Arc<egui::Galley> {
    let sc = spec.scale;
    if (sc - 1.0).abs() < 1e-4 || galley.size().y <= 0.0 {
        return galley;
    }
    let job = paragraph_job(ctx, spec.text, spec.styles, spec.term, spec.attrs, 1.0, wrap / sc, &[]);
    let page = ctx.fonts_mut(|f| f.layout_job(job));
    let height = page.size().y * sc;
    let k = height / galley.size().y;
    if (k - 1.0).abs() < 1e-4 {
        return galley;
    }
    let ppp = ctx.pixels_per_point();
    let mut g = (*galley).clone();
    for row in &mut g.rows {
        row.pos.y = (row.pos.y * k * ppp).round() / ppp;
    }
    g.rect.max.y = g.rect.min.y + height;
    g.mesh_bounds.max.y += height - galley.size().y;
    Arc::new(g)
}

/// Number of the list item that starts at (char `c`, byte `b`) among consecutive numbered paragraphs.
pub fn list_number(doc: &Doc, mut c: usize, mut b: usize) -> usize {
    let mut n = 1;
    while c > 0 {
        let prev_term = c - 1;
        if doc.flow.styles[prev_term].para.list != ListKind::Numbered {
            break;
        }
        n += 1;
        // Step back to the start of the previous paragraph.
        match doc.flow.text[..b - 1].rfind(is_terminator) {
            Some(t) => {
                c = doc.flow.text[..t].chars().count() + 1;
                b = t + 1;
            }
            None => {
                c = 0;
                b = 0;
            }
        }
    }
    n
}

impl Doc {
    /// If chars `c..ec` are exactly one picture placeholder, its id and size.
    pub fn picture_in(&self, c: usize, ec: usize) -> Option<(u32, Vec2, u8)> {
        if ec != c + 1 {
            return None;
        }
        let st = &self.flow.styles[c];
        let img = (st.image != 0).then(|| self.image(st.image)).flatten()?;
        (self.char_at(c) == Some(IMAGE_CHAR)).then(|| (img.id, self.image_size(img), img.rotation))
    }

    pub fn hard_end(&self, page: usize) -> bool {
        self.spans[page].hard
    }

    /// Lay out page `i`. `marks` are page-local highlights.
    pub fn layout_page(&self, ctx: &egui::Context, i: usize, scale: f32, marks: &[Mark]) -> PageLayout {
        let sp = self.spans[i];
        let hard_end = sp.hard;
        let content_width = self.setup.content_size().x * scale;
        let mut paras = Vec::new();
        let mut y = 0.0;
        let (mut c, mut b) = (sp.start, sp.bstart);
        let mut number: Option<usize> = None; // the number of the last numbered paragraph seen

        while c < sp.end || (c == sp.end && hard_end) {
            let (tc, tb) = self.term_from(c, b);
            let (pe_c, pe_b) = if tc <= sp.end { (tc, tb) } else { (sp.end, sp.bend) };
            let term = &self.flow.styles[tc];
            let attrs = term.para;
            let at_para_start = c == 0 || self.flow.text.as_bytes()[b - 1] == b'\n' || self.flow.text.as_bytes()[b - 1] == 0x0c;

            // Which number this item has (counting from the start of its list if it began on an earlier page).
            if attrs.list == ListKind::Numbered {
                number = Some(match number {
                    Some(n) if at_para_start => n + 1,
                    Some(n) => n,
                    None => {
                        let (pc, pb) = if at_para_start { (c, b) } else { self.para_start(c) };
                        list_number(self, pc, pb)
                    }
                });
            } else {
                number = None;
            }
            let is_picture = self.picture_in(c, pe_c).is_some();
            let marker = match (attrs.list, at_para_start && !is_picture) {
                (ListKind::Bullet, true) => Some("\u{2022}".to_owned()),
                (ListKind::Numbered, true) => Some(format!("{}.", number.unwrap_or(1))),
                _ => None,
            };

            let local = |x: usize| x - sp.start;
            let piece_marks: Vec<Mark> = marks
                .iter()
                .filter(|m| m.range.end > local(c) && m.range.start < local(pe_c))
                .map(|m| Mark { range: m.range.start.saturating_sub(local(c))..m.range.end.min(local(pe_c)) - local(c), color: m.color })
                .collect();
            let spec = PieceSpec {
                text: &self.flow.text[b..pe_b],
                styles: &self.flow.styles[c..pe_c],
                term,
                attrs,
                scale,
                content_width,
                marks: &piece_marks,
                invisible: pe_c == c && self.flow.text[tb..].starts_with(PAGE_BREAK) && tc == pe_c,
                marker,
                image: self.picture_in(c, pe_c),
            };
            let mut p = layout_piece(ctx, &spec, local(c), local(pe_c), y);
            p.starts_paragraph = at_para_start;
            y += p.height;
            paras.push(p);

            if tc >= sp.end {
                break;
            }
            c = tc + 1;
            b = tb + 1;
        }
        PageLayout { paras, height: y, scale }
    }
}

/// A visual row of a page.
#[derive(Clone, Copy, Debug)]
pub struct RowRef {
    pub para: usize,
    /// Page-local range of chars on this row.
    pub start: usize,
    pub end: usize,
    /// Top and height relative to the writing area.
    pub top: f32,
    pub height: f32,
}

impl PageLayout {
    /// Index of the paragraph piece the page-local position belongs to.
    pub fn piece_of(&self, local: usize) -> Option<usize> {
        self.paras
            .iter()
            .position(|p| local >= p.start && local <= p.end)
            .or_else(|| self.paras.len().checked_sub(1))
    }

    /// The caret rectangle (a thin vertical bar) relative to the writing area.
    pub fn caret_rect(&self, local: usize, prefer_next_row: bool) -> Rect {
        let Some(k) = self.piece_of(local) else {
            return Rect::from_min_size(pos2(0.0, 0.0), vec2(1.0, 14.0 * self.scale));
        };
        let p = &self.paras[k];
        if let Some(img) = p.image {
            // Before the picture: its left edge; after it: its right edge.
            let x = if local <= p.start { img.rect.left() } else { img.rect.right() };
            return Rect::from_min_max(pos2(x, p.y), pos2(x + 1.0, p.y + p.height));
        }
        let idx = local.clamp(p.start, p.end) - p.start;
        let r = p.galley.pos_from_cursor(CCursor { index: idx.into(), prefer_next_row });
        r.translate(vec2(p.x, p.y))
    }

    /// Where a dragged picture would land for a pointer at height `y`: the paragraph start nearest
    /// to it, as (page-local position, height of the line to draw).
    pub fn drop_boundary(&self, y: f32) -> Option<(usize, f32)> {
        self.paras
            .iter()
            .filter(|p| p.starts_paragraph)
            .map(|p| (p.start, p.y))
            .min_by(|a, b| (a.1 - y).abs().total_cmp(&(b.1 - y).abs()))
    }

    /// The picture paragraph under a point relative to the writing area.
    pub fn image_at(&self, pos: Vec2) -> Option<&ParaLayout> {
        self.paras.iter().find(|p| {
            p.image.is_some_and(|img| img.rect.translate(vec2(0.0, p.y)).contains(pos.to_pos2()))
        })
    }

    /// Page-local position closest to a point relative to the writing area.
    pub fn hit(&self, pos: Vec2) -> usize {
        let Some(p) = self.paras.iter().find(|p| pos.y < p.y + p.height).or(self.paras.last()) else {
            return 0;
        };
        if let Some(img) = p.image {
            return if pos.x < img.rect.center().x { p.start } else { p.end };
        }
        let idx = usize::from(p.galley.cursor_from_pos(vec2(pos.x - p.x, pos.y - p.y)).index);
        p.start + idx.min(p.end - p.start)
    }

    pub fn rows(&self) -> Vec<RowRef> {
        let mut out = Vec::new();
        for (k, p) in self.paras.iter().enumerate() {
            let mut start = p.start;
            for r in p.row_table() {
                out.push(RowRef { para: k, start, end: start + r.chars, top: p.y + r.y, height: r.height });
                start += r.chars;
            }
        }
        out
    }

    /// The visual row containing the page-local position (the later one at a wrap point).
    pub fn row_of(&self, local: usize, prefer_next_row: bool) -> Option<RowRef> {
        let rows = self.rows();
        let k = self.piece_of(local)?;
        let in_piece: Vec<&RowRef> = rows.iter().filter(|r| r.para == k).collect();
        let last = in_piece.len().checked_sub(1)?;
        for (n, r) in in_piece.iter().enumerate() {
            let wraps_on = n < last && local == r.end;
            if (local >= r.start && local < r.end) || (wraps_on && prefer_next_row) {
                return Some(**r);
            }
            if local == r.end && (n == last || !prefer_next_row) {
                return Some(**r);
            }
        }
        in_piece.last().map(|r| **r)
    }

    /// Rectangles (relative to the writing area) covering page-local `a..b`, one per row piece.
    pub fn selection_rects(&self, a: usize, b: usize) -> Vec<Rect> {
        let mut out = Vec::new();
        for r in self.rows() {
            let p = &self.paras[r.para];
            if let Some(img) = p.image {
                if a < p.end && b > p.start {
                    out.push(img.rect.translate(vec2(0.0, p.y)));
                }
                continue;
            }
            let (s, e) = (a.max(r.start), b.min(r.end));
            let includes_end = b > p.end && r.end == p.end && p.galley.rows.last().is_some();
            if s >= e && !(includes_end && a <= r.end) {
                continue;
            }
            let (s, e) = (s.min(e), e.max(s));
            let x0 = p.galley.pos_from_cursor(CCursor { index: (s - p.start).into(), prefer_next_row: true }).left();
            let x1 = p.galley.pos_from_cursor(CCursor { index: (e - p.start).into(), prefer_next_row: false }).left();
            let tail = if includes_end { 5.0 * self.scale } else { 0.0 };
            out.push(Rect::from_min_max(
                pos2(p.x + x0, r.top),
                pos2(p.x + x1.max(x0) + tail, r.top + r.height),
            ));
        }
        out
    }
}

/// Run `f` inside an egui frame, so fonts exist (tests only).
#[cfg(test)]
pub fn with_ctx(f: impl FnOnce(&egui::Context)) {
    let ctx = egui::Context::default();
    let mut f = Some(f);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        if let Some(f) = f.take() {
            f(ui.ctx());
        }
    });
    out.textures_delta.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc_with(text: &str, attrs: ParaAttrs) -> Doc {
        let mut d = Doc::new();
        let st = Style::new("x");
        d.flow.text = format!("{text}\n");
        d.flow.styles = vec![st.clone(); text.chars().count()];
        d.flow.styles.push(st.with_para(attrs));
        d
    }

    #[test]
    fn alignment_moves_the_text_within_the_writing_width() {
        with_ctx(|ctx| {
            let width = 300.0;
            let x_of = |align| {
                let mut d = doc_with("hello", ParaAttrs { align, ..Default::default() });
                d.setup.margin_left = 0.0;
                d.setup.margin_right = d.setup.size().x - width;
                d.full_paginate(ctx, &Style::new("x"));
                let l = d.layout_page(ctx, 0, 1.0, &[]);
                (l.caret_rect(0, true).left(), l.caret_rect(5, true).left())
            };
            let left = x_of(Align::Left);
            let center = x_of(Align::Center);
            let right = x_of(Align::Right);
            assert!(left.0 < 1.0, "left aligned starts at 0: {left:?}");
            let text_w = left.1 - left.0;
            assert!((center.0 - (width - text_w) / 2.0).abs() < 2.0, "centered: {center:?} text_w {text_w}");
            assert!((right.1 - width).abs() < 2.0, "right aligned ends at the edge: {right:?}");
        });
    }

    #[test]
    fn text_fills_the_page_the_same_at_every_zoom() {
        with_ctx(|ctx| {
            let long = "a paragraph long enough to wrap onto a few rows of the page. ".repeat(4);
            let text: String = (0..60).map(|k| if k % 5 == 0 { format!("{long}\n") } else { "qwe\n".into() }).collect();
            let st = Style::new("x");
            let mut d = Doc::new();
            d.flow.text = text.clone();
            d.flow.styles = vec![st.clone(); text.chars().count()];
            d.full_paginate(ctx, &st);
            let at_page_size = d.layout_page(ctx, 0, 1.0, &[]).height;
            for sc in [0.8f32, 0.9, 0.94, 1.2, 1.37] {
                let h = d.layout_page(ctx, 0, sc, &[]).height / sc;
                assert!((h - at_page_size).abs() < 0.01, "at zoom {sc} the text is {h}pt tall instead of {at_page_size}pt");
            }
        });
    }

    #[test]
    fn line_spacing_makes_lines_taller_and_lists_indent() {
        with_ctx(|ctx| {
            let long = "word ".repeat(60);
            let height = |spacing: f32, list| {
                let mut d = doc_with(&long, ParaAttrs { spacing, list, ..Default::default() });
                d.full_paginate(ctx, &Style::new("x"));
                let l = d.layout_page(ctx, 0, 1.0, &[]);
                (l.height, l.paras[0].x, l.paras[0].marker.is_some())
            };
            let single = height(1.0, ListKind::None);
            let double = height(2.0, ListKind::None);
            assert!(double.0 > single.0 * 1.8 && double.0 < single.0 * 2.2, "{single:?} {double:?}");
            let bullet = height(1.0, ListKind::Bullet);
            assert!(bullet.1 > 20.0 && bullet.2, "bullet items are indented and have a marker: {bullet:?}");
            assert!(bullet.0 >= single.0, "narrower text wraps into at least as many rows");
        });
    }
}
