//! The document: one continuous flow of styled text plus page setup. Pages are *derived*
//! from it (see `paginate`), they are not stored.

use std::cell::RefCell;
use std::sync::Arc;

use eframe::egui::{Vec2, vec2};
use serde::{Deserialize, Serialize};

pub const FONT_SIZE: f32 = 12.0;
/// A hard page break inside the flow (form feed). Never shown in an editor.
pub const PAGE_BREAK: char = '\u{c}';
/// Points per centimetre.
pub const CM: f32 = 72.0 / 2.54;

/// Per-character formatting.
#[derive(Clone, PartialEq, Debug)]
pub struct Style {
    pub font: Arc<str>,
    pub size: f32,
    pub bold: bool,
    pub underline: bool,
}

impl Style {
    pub fn new(font: &str) -> Self {
        Self { font: font.into(), size: FONT_SIZE, bold: false, underline: false }
    }
}

/// Keeps `styles` aligned with a text buffer that is edited behind our back.
pub struct Rich {
    pub styles: Vec<Style>,
    /// The text `styles` currently corresponds to.
    pub synced: String,
}

impl Rich {
    /// Bring `styles` in line with `new`, giving freshly inserted chars the `fill` style.
    pub fn sync(&mut self, new: &str, fill: &Style) {
        if self.synced == new {
            return;
        }
        let old: Vec<char> = self.synced.chars().collect();
        let new_c: Vec<char> = new.chars().collect();
        self.styles.resize(old.len(), fill.clone());
        let max = old.len().min(new_c.len());
        let p = old.iter().zip(&new_c).take_while(|(a, b)| a == b).count();
        let s = old[p..].iter().rev().zip(new_c[p..].iter().rev()).take(max - p).take_while(|(a, b)| a == b).count();
        let inserted = new_c.len() - p - s;
        self.styles.splice(p..old.len() - s, std::iter::repeat_n(fill.clone(), inserted));
        self.synced = new.to_owned();
    }
}

/// The text of the page currently being edited, with the styles that go with it.
pub struct EditBuf {
    pub text: String,
    pub rich: RefCell<Rich>,
}

impl EditBuf {
    pub fn new(text: String, styles: Vec<Style>) -> Self {
        let rich = RefCell::new(Rich { styles, synced: text.clone() });
        Self { text, rich }
    }
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
}

impl Span {
    pub fn shifted(self, dchars: isize, dbytes: isize) -> Span {
        Span {
            start: self.start.wrapping_add_signed(dchars),
            end: self.end.wrapping_add_signed(dchars),
            bstart: self.bstart.wrapping_add_signed(dbytes),
            bend: self.bend.wrapping_add_signed(dbytes),
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
    pub fn new() -> Self {
        Self {
            flow: Flow { text: String::new(), styles: Vec::new() },
            setup: PageSetup::default(),
            spans: vec![Span::default()],
            version: 0,
            notes: Vec::new(),
            next_note_id: 1,
            history: Default::default(),
        }
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

    pub fn total_chars(&self) -> usize {
        self.flow.styles.len()
    }

    pub fn char_at(&self, c: usize) -> Option<char> {
        self.flow.text.chars().nth(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_tracks_insert_and_delete() {
        let plain = Style::new("x");
        let bold = Style { bold: true, ..plain.clone() };
        let mut r = Rich { styles: vec![plain.clone(), bold.clone(), plain.clone()], synced: "abc".into() };
        r.sync("abZc", &bold);
        assert_eq!(r.styles, vec![plain.clone(), bold.clone(), bold.clone(), plain.clone()]);
        r.sync("ac", &plain);
        assert_eq!(r.styles, vec![plain.clone(), plain.clone()]);
        r.sync("", &plain);
        assert!(r.styles.is_empty());
    }

    #[test]
    fn landscape_swaps_and_margins_are_kept_usable() {
        let mut s = PageSetup { orientation: Orientation::Landscape, margin_left: 900.0, ..Default::default() };
        assert_eq!(s.size(), vec2(842.0, 595.0));
        s.clamp_margins();
        assert!(s.content_size().x >= PageSetup::MIN_CONTENT - 0.01);
    }
}
