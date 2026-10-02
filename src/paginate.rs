//! Splitting the flow into pages. A page ends when the next line no longer fits the writing
//! area, or at a hard page break. Nothing here changes the text.

use eframe::egui;

use crate::layout::{PieceSpec, layout_piece};
use crate::model::{Doc, PAGE_BREAK, Span, Style};

/// Chars of one paragraph laid out at a time (more are laid out only if they still fit).
const WINDOW: usize = 4000;

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

    /// Recompute after an edit that replaced the flow chars `at..old_end` (old coordinates) and
    /// changed the flow's length by `dchars` chars / `dbytes` bytes. Pages before the edit are kept,
    /// and so are the ones after it, as soon as a page starts where an old one did.
    pub fn paginate_after(&mut self, ctx: &egui::Context, fallback: &Style, at: usize, old_end: usize, dchars: isize, dbytes: isize) {
        if self.spans.is_empty() {
            return self.full_paginate(ctx, fallback);
        }
        let first = self.page_of(at.min(self.total_chars().saturating_sub(1)));
        let first = first.min(self.spans.len() - 1);
        let old = std::mem::take(&mut self.spans);
        let mut spans: Vec<Span> = old[..first].to_vec();
        let mut start = Some((old[first].start, old[first].bstart));
        let mut j = first + 1;
        while let Some((c, b)) = start {
            let (span, next) = self.next_span(ctx, fallback, c, b);
            spans.push(span);
            start = next;
            if let Some((nc, _)) = next {
                // Old pages that begin after the edited stretch are unchanged, merely shifted.
                while j < old.len() && ((old[j].start as isize + dchars) < nc as isize || old[j].start < old_end) {
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

    /// The page starting at char `c0` / byte `b0`, and where the next one starts (if there is one).
    fn next_span(&self, ctx: &egui::Context, fallback: &Style, c0: usize, b0: usize) -> (Span, Option<(usize, usize)>) {
        let _ = fallback;
        let content = self.setup.content_size();
        let total = self.total_chars();
        let limit = content.y + 0.5;
        let (mut c, mut b) = (c0, b0);
        let mut y = 0.0f32;
        loop {
            let (tc, tb) = self.term_from(c, b);
            let is_break = self.flow.text[tb..].starts_with(PAGE_BREAK);
            let term = &self.flow.styles[tc];

            // Lay out the paragraph (a window of it, if it is huge) and see how much of it fits.
            let mut window = WINDOW;
            let (ec, eb, layout, height) = loop {
                let (ec, eb) = if tc - c <= window {
                    (tc, tb)
                } else {
                    (c + window, b + self.flow.text[b..].char_indices().nth(window).map_or(tb - b, |(o, _)| o))
                };
                let spec = PieceSpec {
                    text: &self.flow.text[b..eb],
                    styles: &self.flow.styles[c..ec],
                    term,
                    attrs: term.para,
                    scale: 1.0,
                    content_width: content.x,
                    marks: &[],
                    invisible: ec == c && is_break && ec == tc,
                    marker: None,
                };
                let p = layout_piece(ctx, &spec, 0, ec - c, 0.0);
                if ec < tc && y + p.height <= limit {
                    window *= 2; // fits so far, but more of the paragraph follows
                    continue;
                }
                break (ec, eb, p.galley.clone(), p.height);
            };
            let truncated = ec < tc;

            let whole = !truncated && y + height <= limit;
            let rows_fit = layout.rows.iter().take_while(|r| y + r.pos.y + r.size.y <= limit).count();
            let nothing_fits_on_empty_page = rows_fit == 0 && (c, b) == (c0, b0);
            let empty_para = ec == c;
            if whole || (nothing_fits_on_empty_page && empty_para) {
                y += height;
                if is_break {
                    return (Span { start: c0, end: tc, bstart: b0, bend: tb, hard: true }, Some((tc + 1, tb + 1)));
                }
                c = tc + 1;
                b = tb + 1;
                if c >= total {
                    return (Span { start: c0, end: total, bstart: b0, bend: self.flow.text.len(), hard: false }, None);
                }
                continue;
            }
            if rows_fit == 0 && (c, b) != (c0, b0) {
                // This paragraph starts the next page.
                return (Span { start: c0, end: c, bstart: b0, bend: b, hard: false }, Some((c, b)));
            }
            // Split the paragraph after the rows that fit (at least one, to always make progress).
            let rows = rows_fit.max(1);
            let chars: usize = layout.rows[..rows].iter().map(|r| usize::from(r.row.char_count_excluding_newline())).sum();
            let cut = c + chars.max(1).min(ec - c);
            let cut_b = b + self.flow.text[b..eb].char_indices().nth(cut - c).map_or(eb - b, |(o, _)| o);
            return (Span { start: c0, end: cut, bstart: b0, bend: cut_b, hard: false }, Some((cut, cut_b)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::Edit;
    use crate::layout::with_ctx;
    use crate::model::{ListKind, ParaAttrs};

    /// A document with exactly this text (which must end with the final paragraph mark).
    fn doc_with(text: &str, st: &Style) -> Doc {
        assert!(text.ends_with('\n'));
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
            let mut d = doc_with("a\u{c}b\u{c}\n", &st);
            d.full_paginate(ctx, &st);
            assert_eq!(d.pages(), 3);
            assert_eq!(d.page_text(0), "a");
            assert_eq!(d.page_text(1), "b");
            assert_eq!(d.page_text(2), "\n");
        });
    }

    #[test]
    fn an_empty_page_between_two_breaks_exists_and_can_hold_the_caret() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let mut d = doc_with("a\u{c}\u{c}b\n", &st);
            d.full_paginate(ctx, &st);
            assert_eq!(d.pages(), 3);
            assert_eq!(d.page_text(1), "");
            assert_eq!(d.page_of(2), 1, "the spot between the breaks is on the empty page");
            assert_eq!(d.page_of(1), 0, "the spot just before a break is on the page it ends");
            assert_eq!(d.page_of(3), 2);
        });
    }

    #[test]
    fn one_long_paragraph_continues_across_pages() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let text = format!("{}\n", "word ".repeat(2500));
            let mut d = doc_with(&text, &st);
            d.full_paginate(ctx, &st);
            assert!(d.pages() >= 3, "{} pages", d.pages());
            assert_eq!(d.spans[0].end, d.spans[1].start, "the split happens inside the paragraph");
            assert!(!d.page_text(0).contains('\n'));
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
            inc.paginate_after(ctx, &st, at, at, added.chars().count() as isize, added.len() as isize);
            d.full_paginate(ctx, &st);
            assert_eq!(inc.spans, d.spans);
        });
    }

    #[test]
    fn incremental_handles_edits_that_remove_page_breaks() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let text = format!("{}\u{c}{}\u{c}tail\n", "a\n".repeat(5), "b\n".repeat(5));
            let mut d = doc_with(&text, &st);
            d.full_paginate(ctx, &st);
            assert_eq!(d.pages(), 3);
            let at = d.spans[0].end; // the first break
            let old = crate::edit::Piece { text: "\u{c}".into(), styles: vec![st.clone()] };
            d.apply(Edit::Replace { at, old, new: Default::default() }, 0.0);
            let mut inc = Doc::new();
            inc.flow = crate::model::Flow { text: d.flow.text.clone(), styles: d.flow.styles.clone() };
            inc.spans = d.spans.clone();
            inc.paginate_after(ctx, &st, at, at + 1, -1, -1);
            d.full_paginate(ctx, &st);
            assert_eq!(d.pages(), 2);
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

    #[test]
    fn double_line_spacing_needs_more_pages() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let mut d = doc_with(&"line\n".repeat(120), &st);
            d.full_paginate(ctx, &st);
            let single = d.pages();
            for s in d.flow.styles.iter_mut() {
                s.para = ParaAttrs { spacing: 2.0, ..Default::default() };
            }
            d.full_paginate(ctx, &st);
            assert!(d.pages() > single, "{} vs {single}", d.pages());
        });
    }

    #[test]
    fn numbered_list_items_count_up_and_restart() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            let mut d = doc_with("one\ntwo\nplain\nthree\n", &st);
            let numbered = ParaAttrs { list: ListKind::Numbered, ..Default::default() };
            let lines_end = [3usize, 7, 13, 19]; // index of each paragraph's '\n'
            for (k, &e) in lines_end.iter().enumerate() {
                if k != 2 {
                    d.flow.styles[e].para = numbered;
                }
            }
            d.full_paginate(ctx, &st);
            let layout = d.layout_page(ctx, 0, 1.0, &[]);
            assert_eq!(layout.paras.len(), 4);
            let marks: Vec<Option<String>> = layout
                .paras
                .iter()
                .map(|p| p.marker.as_ref().map(|_| String::new()))
                .collect();
            assert!(marks[0].is_some() && marks[1].is_some() && marks[2].is_none() && marks[3].is_some());
            assert_eq!(crate::layout::list_number(&d, 0, 0), 1);
            assert_eq!(crate::layout::list_number(&d, 4, 4), 2);
            assert_eq!(crate::layout::list_number(&d, 14, 14), 1, "an unnumbered paragraph in between restarts the count");
        });
    }
}
