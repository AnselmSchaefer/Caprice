//! Splitting the flow into pages. Pages end where the text no longer fits the writing area, or
//! at a hard page break. Nothing here changes the text.

use eframe::egui;

use crate::layout::{build_job, layout};
use crate::model::{Doc, PAGE_BREAK, Span, Style};

impl Doc {
    /// Recompute every page.
    pub fn full_paginate(&mut self, ctx: &egui::Context, fallback: &Style) {
        self.spans.clear();
        let mut start = Some((0, 0));
        while let Some((c, b)) = start {
            let (span, next) = self.next_span(ctx, fallback, c, b);
            self.spans.push(span);
            start = next;
        }
    }

    /// Recompute after an edit that happened *inside* page `page` and changed the flow's length by
    /// `dchars` chars / `dbytes` bytes. Stops as soon as a page starts where an old one did, since
    /// everything after that is unchanged.
    pub fn paginate_from(&mut self, ctx: &egui::Context, fallback: &Style, page: usize, dchars: isize, dbytes: isize) {
        if self.spans.is_empty() {
            return self.full_paginate(ctx, fallback);
        }
        let page = page.min(self.spans.len() - 1);
        let old = std::mem::take(&mut self.spans);
        let mut spans: Vec<Span> = old[..page].to_vec();
        let mut start = Some((old[page].start, old[page].bstart));
        let mut j = page + 1;
        while let Some((c, b)) = start {
            let (span, next) = self.next_span(ctx, fallback, c, b);
            spans.push(span);
            start = next;
            if let Some((nc, _)) = next {
                while j < old.len() && (old[j].start as isize + dchars) < nc as isize {
                    j += 1;
                }
                if j < old.len() && old[j].start as isize + dchars == nc as isize {
                    spans.extend(old[j..].iter().map(|s| s.shifted(dchars, dbytes)));
                    start = None;
                }
            }
        }
        self.spans = spans;
    }

    /// The page starting at char `c` / byte `b`, and where the next one starts (if there is one).
    fn next_span(&self, ctx: &egui::Context, fallback: &Style, c: usize, b: usize) -> (Span, Option<(usize, usize)>) {
        let text = &self.flow.text;
        let styles = &self.flow.styles;
        let content = self.setup.content_size();
        let seg_end_b = text[b..].find(PAGE_BREAK).map_or(text.len(), |p| b + p);
        let after_break = |chars: usize| (seg_end_b < text.len()).then(|| (c + chars + 1, seg_end_b + PAGE_BREAK.len_utf8()));

        let mut window = 4000usize;
        loop {
            // Lay out only a window of the remaining text; grow it until the page boundary shows up.
            let mut wchars = 0;
            let mut wend_b = seg_end_b;
            let mut complete = true;
            for (k, (off, _)) in text[b..seg_end_b].char_indices().enumerate() {
                if k == window {
                    wend_b = b + off;
                    complete = false;
                    break;
                }
                wchars = k + 1;
            }
            if wchars == 0 && complete {
                return (Span { start: c, end: c, bstart: b, bend: b }, after_break(0));
            }
            let job = build_job(&text[b..wend_b], &styles[c..c + wchars], fallback, 1.0, content.x, &[]);
            let galley = layout(ctx, job);
            let bad = galley.rows.iter().position(|r| r.pos.y + r.row.size.y > content.y + 0.5);
            match bad {
                None if !complete => window *= 2,
                None => {
                    let span = Span { start: c, end: c + wchars, bstart: b, bend: seg_end_b };
                    return (span, after_break(wchars));
                }
                Some(k) => {
                    // Everything before row `k` stays; always make progress by at least one char.
                    let cut: usize = galley.rows[..k.max(1)]
                        .iter()
                        .map(|r| usize::from(r.row.char_count_excluding_newline()) + usize::from(r.ends_with_newline))
                        .sum();
                    let cut = cut.clamp(1, wchars);
                    let cut_b = text[b..wend_b].char_indices().nth(cut).map_or(wend_b, |(o, _)| b + o);
                    let span = Span { start: c, end: c + cut, bstart: b, bend: cut_b };
                    return (span, Some((c + cut, cut_b)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Edit;

    fn with_ctx(f: impl FnOnce(&egui::Context)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                f(ui.ctx());
            }
        });
        out.textures_delta.clear(); // nothing renders these in a test
    }

    fn doc_with(text: &str, st: &Style) -> Doc {
        let mut d = Doc::new();
        d.flow.text = text.to_owned();
        d.flow.styles = vec![st.clone(); text.chars().count()];
        d
    }

    #[test]
    fn long_text_splits_into_contiguous_pages() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let text: String = (0..400).map(|n| format!("line {n}\n")).collect();
            let mut d = doc_with(&text, &st);
            d.full_paginate(ctx, &st);
            assert!(d.pages() > 3, "got {} pages", d.pages());
            for w in d.spans.windows(2) {
                assert_eq!(w[0].end, w[1].start);
                assert_eq!(w[0].bend, w[1].bstart);
            }
            assert_eq!(d.spans.last().unwrap().end, d.total_chars());
        });
    }

    #[test]
    fn hard_breaks_make_pages_including_a_trailing_empty_one() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let mut d = doc_with("a\u{c}b\u{c}", &st);
            d.full_paginate(ctx, &st);
            assert_eq!(d.pages(), 3);
            assert_eq!(d.page_text(0), "a");
            assert_eq!(d.page_text(1), "b");
            assert_eq!(d.page_text(2), "");
        });
    }

    #[test]
    fn incremental_matches_full() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let text: String = (0..300).map(|n| format!("line {n}\n")).collect();
            let mut d = doc_with(&text, &st);
            d.full_paginate(ctx, &st);
            // Type some text into the middle of page 1.
            let at = d.spans[1].start + 20;
            let added = "inserted words here\nand a new line\n";
            d.apply(Edit::insert(at, added, &st), 0.0);
            let mut inc = Doc::new();
            inc.flow = crate::model::Flow { text: d.flow.text.clone(), styles: d.flow.styles.clone() };
            inc.spans = d.spans.clone();
            inc.paginate_from(ctx, &st, 1, added.chars().count() as isize, added.len() as isize);
            d.full_paginate(ctx, &st);
            assert_eq!(inc.spans, d.spans);
        });
    }

    #[test]
    fn smaller_page_means_more_pages() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let text: String = (0..200).map(|n| format!("line {n}\n")).collect();
            let mut d = doc_with(&text, &st);
            d.full_paginate(ctx, &st);
            let before = d.pages();
            d.setup.margin_top = 200.0;
            d.setup.margin_bottom = 200.0;
            d.full_paginate(ctx, &st);
            assert!(d.pages() > before);
        });
    }
}
