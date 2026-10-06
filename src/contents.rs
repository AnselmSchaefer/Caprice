//! The contents page: one `CONTENTS_CHAR` in a paragraph of its own stands in the flow for a list
//! of the chapter titles and the pages they start on. The list is never stored; it is made from
//! the titles every time the page is laid out, so it is always up to date.
//!
//! Its height depends only on the titles, not on their page numbers, so pagination never goes in
//! circles: the pages are worked out first, and the numbers filled in when the page is drawn.

use std::sync::Arc;

use eframe::egui::{self, Color32, Pos2, Rect, Vec2, pos2, vec2};

use crate::layout::run_format;
use crate::model::{CHAPTER_TITLE_SCALE, CONTENTS_CHAR, Doc, IMAGE_CHAR, PAGE_BREAK, ParaAttrs, Style, is_terminator};

/// One line of the contents.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub title: String,
    /// Where the title starts in the flow.
    pub at: usize,
    /// The page it is on (counting from 0), once the pages are known.
    pub page: Option<usize>,
}

/// The contents laid out, relative to the top-left of its paragraph.
pub struct ContentsBlock {
    pub size: Vec2,
    pub texts: Vec<(Arc<egui::Galley>, Pos2)>,
    /// Each entry's area, and the page it leads to.
    pub links: Vec<(Rect, usize)>,
}

impl Doc {
    /// The chapter titles, in order. Their pages are only filled in if `with_pages`.
    pub fn chapters(&self, with_pages: bool) -> Vec<Entry> {
        let mut out = Vec::new();
        let (mut pc, mut pb) = (0, 0);
        for (c, (b, ch)) in self.flow.text.char_indices().enumerate() {
            if !is_terminator(ch) {
                continue;
            }
            if self.flow.styles[c].para.is_chapter_title() {
                let title: String = self.flow.text[pb..b].chars().filter(|&k| k != IMAGE_CHAR && k != CONTENTS_CHAR).collect();
                let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
                if !title.is_empty() {
                    let page = (with_pages && !self.spans.is_empty()).then(|| self.page_of(pc));
                    out.push(Entry { title, at: pc, page });
                }
            }
            (pc, pb) = (c + 1, b + ch.len_utf8());
        }
        out
    }

    /// Are chars `c..ec`, starting at byte `b`, exactly a contents block?
    pub fn contents_in(&self, c: usize, b: usize, ec: usize) -> bool {
        ec == c + 1 && self.flow.text[b..].starts_with(CONTENTS_CHAR)
    }

    /// The char where the first contents block is, if the story has one.
    pub fn contents_at(&self) -> Option<usize> {
        let b = self.flow.text.find(CONTENTS_CHAR)?;
        Some(self.flow.text[..b].chars().count())
    }
}

fn text(ctx: &egui::Context, s: &str, st: &Style, scale: f32, wrap: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    job.append(s, 0.0, run_format(ctx, st, scale, Color32::TRANSPARENT, ParaAttrs::default()));
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// Lay the contents out in the style `st` (that of its placeholder char), `width` wide (page
/// points) and at most `max_height` tall: entries that do not fit are left out. Heights and
/// positions come from page size, so the block is equally tall, relative to the page, at every zoom.
pub fn layout_contents(ctx: &egui::Context, entries: &[Entry], st: &Style, width: f32, max_height: f32, scale: f32) -> ContentsBlock {
    let heading_st = Style { size: st.size * CHAPTER_TITLE_SCALE, bold: true, ..st.clone() };
    let number_w = text(ctx, "0000", st, 1.0, f32::INFINITY).size().x;
    let title_w = (width - number_w - st.size).max(width * 0.5);
    let line = text(ctx, " ", st, 1.0, f32::INFINITY).size().y;

    let mut texts = Vec::new();
    let mut links = Vec::new();
    let heading = text(ctx, "Contents", &heading_st, scale, f32::INFINITY);
    let mut bottom = text(ctx, "Contents", &heading_st, 1.0, f32::INFINITY).size().y;
    texts.push((heading.clone(), pos2((width * scale - heading.size().x) / 2.0, 0.0)));
    let mut y = bottom + line;

    for e in entries {
        let h = text(ctx, &e.title, st, 1.0, title_w).size().y;
        if y + h > max_height {
            break;
        }
        bottom = y + h;
        let title = text(ctx, &e.title, st, scale, title_w * scale);
        let top = y * scale;
        if let Some(page) = e.page {
            // The page number at the right, on the title's last line, with dots leading to it.
            let last = title.rows.last().map_or(Rect::ZERO, |r| r.rect());
            let number = text(ctx, &(page + 1).to_string(), st, scale, f32::INFINITY);
            let nx = width * scale - number.size().x;
            let dot = text(ctx, " .", st, scale, f32::INFINITY).size().x.max(1.0);
            let room = nx - last.right() - st.size * scale * 0.6;
            let dots = text(ctx, &" .".repeat((room / dot).max(0.0) as usize), st, scale, f32::INFINITY);
            texts.push((dots.clone(), pos2(nx - dots.size().x - st.size * scale * 0.3, top + last.top())));
            texts.push((number, pos2(nx, top + last.top())));
            links.push((Rect::from_min_size(pos2(0.0, top), vec2(width * scale, h * scale)), page));
        }
        texts.push((title, pos2(0.0, top)));
        y += h + line * 0.4;
    }
    ContentsBlock { size: vec2(width, bottom) * scale, texts, links }
}

/// The text to insert for a contents page: the block, then a page break so the story goes on
/// on the next page.
pub fn contents_text() -> String {
    format!("{CONTENTS_CHAR}{PAGE_BREAK}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::with_ctx;

    /// A document of paragraphs; `true` marks a chapter title. "@" stands for the contents.
    fn doc(paras: &[(&str, bool)]) -> Doc {
        let mut d = Doc::new();
        let st = Style::new("x");
        let mut title = ParaAttrs::default();
        title.set_chapter_title(true);
        d.flow.text.clear();
        d.flow.styles.clear();
        for &(text, is_title) in paras {
            let text = text.replace('@', &CONTENTS_CHAR.to_string());
            d.flow.text.push_str(&text);
            d.flow.styles.extend(std::iter::repeat_n(st.clone(), text.chars().count()));
            let end = if text.ends_with(PAGE_BREAK) { d.flow.text.pop(); d.flow.styles.pop(); PAGE_BREAK } else { '\n' };
            d.flow.text.push(end);
            d.flow.styles.push(st.with_para(if is_title { title } else { ParaAttrs::default() }));
        }
        d
    }

    fn story(chapters: usize, paras_each: usize) -> Vec<(String, bool)> {
        let mut out = vec![("@\u{c}".to_owned(), false)];
        for n in 1..=chapters {
            out.push((format!("Chapter {n}"), true));
            out.extend(std::iter::repeat_n(("It was nearly midnight and the Prime Minister sat alone in his office. ".repeat(8), false), paras_each));
        }
        out
    }

    fn as_refs(v: &[(String, bool)]) -> Vec<(&str, bool)> {
        v.iter().map(|(s, t)| (s.as_str(), *t)).collect()
    }

    #[test]
    fn the_chapters_are_the_titles_with_text_and_their_pages() {
        with_ctx(|ctx| {
            let mut d = doc(&[("Intro", false), ("  The   Other\tMinister ", true), ("", true), ("Text", false), ("Next\u{c}", false), ("Hagrid?", true)]);
            d.full_paginate(ctx, &Style::new("x"));
            let ch = d.chapters(true);
            let titles: Vec<&str> = ch.iter().map(|e| e.title.as_str()).collect();
            assert_eq!(titles, ["The Other Minister", "Hagrid?"], "empty titles are left out, spaces tidied");
            assert_eq!((ch[0].at, ch[0].page, ch[1].page), (6, Some(0), Some(1)));
            assert!(d.chapters(false).iter().all(|e| e.page.is_none()));
        });
    }

    #[test]
    fn the_contents_list_each_chapter_and_lead_to_its_page() {
        with_ctx(|ctx| {
            let paras = story(4, 6);
            let mut d = doc(&as_refs(&paras));
            d.full_paginate(ctx, &Style::new("x"));
            assert!(d.pages() >= 3);
            assert!(d.chapters(true).windows(2).any(|w| w[0].page != w[1].page), "chapters on different pages");
            let l = d.layout_page(ctx, 0, 1.0, &[]);
            assert_eq!(d.spans[0], crate::model::Span { start: 0, end: 1, bstart: 0, bend: 3, hard: true }, "a page of its own");
            let block = l.paras[0].contents.as_ref().expect("the first page is the contents");
            assert_eq!(block.links.len(), 4);
            let shown: Vec<String> = block.texts.iter().map(|(g, _)| g.text().to_owned()).collect();
            for (n, e) in d.chapters(true).iter().enumerate() {
                assert!(shown.contains(&e.title));
                assert!(shown.contains(&(e.page.unwrap() + 1).to_string()), "page of {}: {shown:?}", e.title);
                let (r, page) = block.links[n];
                assert_eq!(l.link_at(r.center().to_vec2()), Some(page));
                assert_eq!(page, e.page.unwrap());
            }
            assert_eq!(l.link_at(vec2(1.0, 1.0)), None, "the heading leads nowhere");
            // The caret goes before or after the block, like a picture.
            assert_eq!(l.hit(vec2(1.0, 30.0)), 0);
            assert_eq!(l.hit(vec2(l.paras[0].block().unwrap().right() - 1.0, 30.0)), 1);
        });
    }

    #[test]
    fn the_contents_take_the_same_room_at_every_zoom_and_never_more_than_a_page() {
        with_ctx(|ctx| {
            let paras = story(3, 1);
            let mut d = doc(&as_refs(&paras));
            d.full_paginate(ctx, &Style::new("x"));
            let h = d.layout_page(ctx, 0, 1.0, &[]).height;
            for sc in [0.8f32, 1.37] {
                assert!((d.layout_page(ctx, 0, sc, &[]).height / sc - h).abs() < 0.01);
            }
            let paras = story(120, 0);
            let mut d = doc(&as_refs(&paras));
            d.full_paginate(ctx, &Style::new("x"));
            let l = d.layout_page(ctx, 0, 1.0, &[]);
            assert!(l.height <= d.setup.content_size().y + 0.5, "{} of {}", l.height, d.setup.content_size().y);
            assert!(l.paras[0].contents.as_ref().unwrap().links.len() < 120, "the chapters that fit");
        });
    }

    #[test]
    fn a_new_chapter_far_on_reflows_the_contents_page_like_from_scratch() {
        with_ctx(|ctx| {
            let st = Style::new("x");
            // Contents with text right after them on the same page, so their height moves it.
            let mut paras = story(3, 5);
            paras[0].0 = "@".into();
            paras.insert(1, ("Foreword. ".repeat(300), false));
            let mut d = doc(&as_refs(&paras));
            d.full_paginate(ctx, &st);
            let at = d.flow.text.rfind("It was").unwrap();
            let at = d.flow.text[..at].chars().count();
            let mut title = ParaAttrs::default();
            title.set_chapter_title(true);
            let new = crate::edit::Piece { text: "A new chapter\n".into(), styles: [vec![st.clone(); 13], vec![st.with_para(title)]].concat() };
            d.apply(crate::edit::Edit::Replace { at, old: crate::edit::Piece::default(), new }, 0.0);
            d.paginate_after(ctx, &st, at, at, 14, 14);
            let incremental = d.spans.clone();
            d.full_paginate(ctx, &st);
            assert_eq!(incremental, d.spans);
        });
    }
}
