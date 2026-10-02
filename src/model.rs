//! The document: one continuous flow of styled text plus page setup. Pages are *derived*
//! from it (see `paginate`), they are not stored.

use std::sync::Arc;

use eframe::egui::{Vec2, vec2};
use serde::{Deserialize, Serialize};

pub const FONT_SIZE: f32 = 12.0;
/// A hard page break inside the flow (form feed). Never shown in an editor.
pub const PAGE_BREAK: char = '\u{c}';
/// Points per centimetre.
pub const CM: f32 = 72.0 / 2.54;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ListKind {
    #[default]
    None,
    Bullet,
    Numbered,
}

/// Paragraph formatting. It lives on the paragraph's terminating character (`\n` or `\f`), like
/// Word keeps it on the paragraph mark, so splitting, merging and undo handle it for free.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct ParaAttrs {
    pub align: Align,
    /// Line spacing as a multiple of the font's natural line height.
    pub spacing: f32,
    pub list: ListKind,
}

impl Default for ParaAttrs {
    fn default() -> Self {
        Self { align: Align::Left, spacing: 1.0, list: ListKind::None }
    }
}

/// Per-character formatting. `para` only means something on terminator characters.
#[derive(Clone, PartialEq, Debug)]
pub struct Style {
    pub font: Arc<str>,
    pub size: f32,
    pub bold: bool,
    pub underline: bool,
    pub para: ParaAttrs,
}

impl Style {
    pub fn new(font: &str) -> Self {
        Self { font: font.into(), size: FONT_SIZE, bold: false, underline: false, para: ParaAttrs::default() }
    }

    /// Same look of the glyph itself, ignoring paragraph formatting.
    pub fn same_char(&self, o: &Style) -> bool {
        self.font == o.font && self.size == o.size && self.bold == o.bold && self.underline == o.underline
    }

    pub fn with_para(&self, para: ParaAttrs) -> Style {
        Style { para, ..self.clone() }
    }
}

pub fn is_terminator(c: char) -> bool {
    c == '\n' || c == PAGE_BREAK
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Paper, orientation, margins. All lengths in points.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSetup {
    pub orientation: Orientation,
    /// Portrait paper size.
    pub paper_w: f32,
    pub paper_h: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    pub page_numbers: bool,
}

impl Default for PageSetup {
    fn default() -> Self {
        Self {
            orientation: Orientation::Portrait,
            paper_w: 595.0,
            paper_h: 842.0,
            margin_top: 72.0,
            margin_bottom: 72.0,
            margin_left: 72.0,
            margin_right: 72.0,
            page_numbers: false,
        }
    }
}

impl PageSetup {
    pub const MIN_CONTENT: f32 = 120.0;

    pub fn size(&self) -> Vec2 {
        match self.orientation {
            Orientation::Portrait => vec2(self.paper_w, self.paper_h),
            Orientation::Landscape => vec2(self.paper_h, self.paper_w),
        }
    }

    pub fn margin_origin(&self) -> Vec2 {
        vec2(self.margin_left, self.margin_top)
    }

    pub fn content_size(&self) -> Vec2 {
        self.size() - vec2(self.margin_left + self.margin_right, self.margin_top + self.margin_bottom)
    }

    /// Keep at least `MIN_CONTENT` of writing area on every page.
    pub fn clamp_margins(&mut self) {
        let s = self.size();
        let max_x = (s.x - Self::MIN_CONTENT).max(0.0);
        let max_y = (s.y - Self::MIN_CONTENT).max(0.0);
        self.margin_left = self.margin_left.clamp(0.0, max_x);
        self.margin_right = self.margin_right.clamp(0.0, max_x - self.margin_left);
        self.margin_top = self.margin_top.clamp(0.0, max_y);
        self.margin_bottom = self.margin_bottom.clamp(0.0, max_y - self.margin_top);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// Char and byte range of the page text inside the flow. A hard break after `end` is not part of it.
    pub start: usize,
    pub end: usize,
    pub bstart: usize,
    pub bend: usize,
    /// Does a hard page break follow (rather than the page ending because it is full)?
    pub hard: bool,
}

impl Span {
    pub fn shifted(self, dchars: isize, dbytes: isize) -> Span {
        Span {
            start: self.start.wrapping_add_signed(dchars),
            end: self.end.wrapping_add_signed(dchars),
            bstart: self.bstart.wrapping_add_signed(dbytes),
            bend: self.bend.wrapping_add_signed(dbytes),
            hard: self.hard,
        }
    }

    pub fn chars(&self) -> usize {
        self.end - self.start
    }
}

/// A sticky note attached to a stretch of text without being part of it. `start..end` are flow
/// positions (chars); `start == end` is a point note. The edit layer keeps them in place as the text changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u64,
    pub start: usize,
    pub end: usize,
    pub text: String,
    /// Index into `NOTE_COLORS`.
    pub color: usize,
}

pub const NOTE_COLORS: [(u8, u8, u8); 5] =
    [(255, 214, 90), (255, 150, 185), (130, 215, 140), (120, 195, 255), (255, 175, 100)];

pub struct Flow {
    pub text: String,
    /// One style per char of `text`.
    pub styles: Vec<Style>,
}

pub struct Doc {
    pub flow: Flow,
    pub setup: PageSetup,
    pub spans: Vec<Span>,
    /// Bumped on every change, so views can tell when they are stale.
    pub version: u64,
    pub notes: Vec<Note>,
    pub next_note_id: u64,
    pub history: crate::edit::History,
}

impl Doc {
    /// An empty document: just the final paragraph mark every document ends with.
    pub fn new() -> Self {
        Self {
            flow: Flow { text: "\n".into(), styles: vec![Style::new("Default")] },
            setup: PageSetup::default(),
            spans: vec![Span { start: 0, end: 1, bstart: 0, bend: 1, hard: false }],
            version: 0,
            notes: Vec::new(),
            next_note_id: 1,
            history: Default::default(),
        }
    }

    /// Make sure the text ends with a paragraph mark (it is the carrier of the last paragraph's format).
    pub fn ensure_final_mark(&mut self) {
        if !self.flow.text.ends_with('\n') {
            let st = self.flow.styles.last().cloned().unwrap_or_else(|| Style::new("Default"));
            self.flow.text.push('\n');
            self.flow.styles.push(st.with_para(ParaAttrs::default()));
        }
    }

    /// The text as the user sees it, without the final paragraph mark.
    pub fn visible_text(&self) -> &str {
        &self.flow.text[..self.flow.text.len().saturating_sub(1)]
    }

    pub fn pages(&self) -> usize {
        self.spans.len()
    }

    pub fn page_text(&self, i: usize) -> &str {
        let s = self.spans[i];
        &self.flow.text[s.bstart..s.bend]
    }

    pub fn page_styles(&self, i: usize) -> &[Style] {
        let s = self.spans[i];
        &self.flow.styles[s.start..s.end]
    }

    pub fn char_to_byte(&self, c: usize) -> usize {
        self.flow.text.char_indices().nth(c).map_or(self.flow.text.len(), |(b, _)| b)
    }

    /// Chars in the flow, including the final paragraph mark. The caret can sit at `0..total_chars()`.
    pub fn total_chars(&self) -> usize {
        self.flow.styles.len()
    }

    pub fn char_at(&self, c: usize) -> Option<char> {
        self.flow.text.chars().nth(c)
    }

    /// The first paragraph terminator at or after (char `c`, byte `b`).
    pub fn term_from(&self, c: usize, b: usize) -> (usize, usize) {
        let rest = &self.flow.text[b..];
        let rel = rest.find(is_terminator).unwrap_or(rest.len().saturating_sub(1));
        (c + rest[..rel].chars().count(), b + rel)
    }

    /// Start (char, byte) of the paragraph containing char `c`.
    pub fn para_start(&self, c: usize) -> (usize, usize) {
        let b = self.char_to_byte(c);
        match self.flow.text[..b].rfind(is_terminator) {
            Some(t) => (self.flow.text[..t].chars().count() + 1, t + 1),
            None => (0, 0),
        }
    }

    /// Format of the paragraph containing char `c`.
    pub fn para_attrs_at(&self, c: usize) -> ParaAttrs {
        let b = self.char_to_byte(c.min(self.total_chars().saturating_sub(1)));
        let (tc, _) = self.term_from(c.min(self.total_chars().saturating_sub(1)), b);
        self.flow.styles[tc.min(self.flow.styles.len() - 1)].para
    }

    /// Which page the caret at flow position `c` is on. A position on a soft page boundary belongs
    /// to the page that starts there; the spot just before a hard break to the page it ends.
    pub fn page_of(&self, c: usize) -> usize {
        for (i, s) in self.spans.iter().enumerate() {
            if c < s.end || (s.hard && c == s.end) {
                return i;
            }
        }
        self.spans.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_swaps_and_margins_are_kept_usable() {
        let mut s = PageSetup { orientation: Orientation::Landscape, margin_left: 900.0, ..Default::default() };
        assert_eq!(s.size(), vec2(842.0, 595.0));
        s.clamp_margins();
        assert!(s.content_size().x >= PageSetup::MIN_CONTENT - 0.01);
    }
}
