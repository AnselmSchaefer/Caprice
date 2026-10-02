//! Turning styled text into egui layout jobs.

use std::ops::Range;

use eframe::egui::{self, Color32, FontId, Stroke, TextFormat, text::LayoutJob};

use crate::fonts::family_for;
use crate::model::Style;
use crate::theme::INK;

/// A background highlight over a range of chars (notes, search results).
#[derive(Clone, Debug)]
pub struct Mark {
    pub range: Range<usize>,
    pub color: Color32,
}

pub fn text_format(st: &Style, scale: f32, background: Color32) -> TextFormat {
    TextFormat {
        background,
        font_id: FontId::new(st.size * scale, family_for(&st.font, st.bold)),
        color: INK,
        underline: if st.underline { Stroke::new(scale.max(1.0), INK) } else { Stroke::NONE },
        ..Default::default()
    }
}

/// `styles[k]` is the style of the k-th char of `text`; missing entries reuse the last one.
/// Later `marks` paint over earlier ones.
pub fn build_job(text: &str, styles: &[Style], fallback: &Style, scale: f32, wrap_width: f32, marks: &[Mark]) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let style_at = |k: usize| styles.get(k).or(styles.last()).unwrap_or(fallback);
    let mark_at = |k: usize| marks.iter().rev().find(|m| m.range.contains(&k)).map_or(Color32::TRANSPARENT, |m| m.color);
    let mut start = 0;
    let mut run = 0; // char index where the current run started
    let mut run_mark = mark_at(0);
    for (k, (b, _)) in text.char_indices().enumerate() {
        let mark = mark_at(k);
        if k > run && (style_at(k) != style_at(run) || mark != run_mark) {
            job.append(&text[start..b], 0.0, text_format(style_at(run), scale, run_mark));
            start = b;
            run = k;
            run_mark = mark;
        }
    }
    job.append(&text[start..], 0.0, text_format(style_at(run), scale, run_mark));
    job
}

pub fn layout(ctx: &egui::Context, job: LayoutJob) -> std::sync::Arc<egui::Galley> {
    ctx.fonts_mut(|f| f.layout_job(job))
}
