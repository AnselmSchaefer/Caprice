//! Turning styled text into egui layout jobs.

use eframe::egui::{self, FontId, Stroke, TextFormat, text::LayoutJob};

use crate::fonts::family_for;
use crate::model::{Doc, Style};
use crate::theme::INK;

pub fn text_format(st: &Style, scale: f32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(st.size * scale, family_for(&st.font, st.bold)),
        color: INK,
        underline: if st.underline { Stroke::new(scale.max(1.0), INK) } else { Stroke::NONE },
        ..Default::default()
    }
}

/// `styles[k]` is the style of the k-th char of `text`; missing entries reuse the last one.
pub fn build_job(text: &str, styles: &[Style], fallback: &Style, scale: f32, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
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

pub fn layout(ctx: &egui::Context, job: LayoutJob) -> std::sync::Arc<egui::Galley> {
    ctx.fonts_mut(|f| f.layout_job(job))
}

impl Doc {
    /// Layout job for page `i` at the given on-screen scale.
    pub fn page_job(&self, i: usize, fallback: &Style, scale: f32) -> LayoutJob {
        build_job(self.page_text(i), self.page_styles(i), fallback, scale, self.setup.content_size().x * scale)
    }
}
