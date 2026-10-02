//! Find: highlights every match, and the page flips to the current one.

use eframe::egui::{self, Color32, Id, Key, Modifiers, Rect, Stroke, TextEdit, UiBuilder, pos2, vec2};

use crate::App;
use crate::notes::SEARCH_FIELD;
use crate::theme::{DOCK, DOCK_EDGE, TEXT_DIM};
use crate::ui::{DOCK_GAP, DOCK_H};

#[derive(Default)]
pub struct Search {
    pub open: bool,
    pub query: String,
    /// Flow ranges (chars) of every match.
    pub matches: Vec<(usize, usize)>,
    pub current: usize,
    /// (document version, query) the matches were computed for.
    stamp: Option<(u64, String)>,
    want_focus: bool,
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// All non-overlapping, case-insensitive occurrences of `needle` in `text`, as char ranges.
pub fn find_all(text: &str, needle: &str) -> Vec<(usize, usize)> {
    let hay: Vec<char> = text.chars().map(fold).collect();
    let pat: Vec<char> = needle.chars().map(fold).collect();
    let mut out = Vec::new();
    if pat.is_empty() || pat.len() > hay.len() {
        return out;
    }
    let mut k = 0;
    while k + pat.len() <= hay.len() {
        if hay[k..k + pat.len()] == pat[..] {
            out.push((k, k + pat.len()));
            k += pat.len();
        } else {
            k += 1;
        }
    }
    out
}

impl App {
    pub fn open_search(&mut self) {
        self.search.open = true;
        self.search.want_focus = true;
    }

    fn close_search(&mut self, ctx: &egui::Context) {
        if let Some(&(s, _)) = self.search.matches.get(self.search.current) {
            self.set_caret(ctx, s, false);
        }
        self.search.open = false;
        self.search.matches.clear();
        self.search.stamp = None;
        ctx.request_repaint();
    }

    fn refresh_matches(&mut self) {
        let stamp = (self.doc.version, self.search.query.clone());
        if self.search.stamp.as_ref() == Some(&stamp) {
            return;
        }
        let query_changed = self.search.stamp.as_ref().is_none_or(|s| s.1 != stamp.1);
        self.search.matches = find_all(&self.doc.flow.text, &self.search.query);
        self.search.stamp = Some(stamp);
        let n = self.search.matches.len();
        if n == 0 {
            self.search.current = 0;
        } else if query_changed {
            let caret = self.caret;
            let first = self.search.matches.iter().position(|&(s, _)| s >= caret).unwrap_or(0);
            self.jump_to_match(first);
        } else {
            self.search.current = self.search.current.min(n - 1);
        }
    }

    /// Make match `k` the current one and flip to its page.
    fn jump_to_match(&mut self, k: usize) {
        let Some(&(s, _)) = self.search.matches.get(k) else { return };
        self.search.current = k;
        self.target = self.doc.page_of(s);
    }

    fn step_match(&mut self, forward: bool) {
        let n = self.search.matches.len();
        if n > 0 {
            let k = if forward { (self.search.current + 1) % n } else { (self.search.current + n - 1) % n };
            self.jump_to_match(k);
        }
    }

    pub fn search_bar(&mut self, ui: &mut egui::Ui, area: Rect) {
        if !self.search.open {
            return;
        }
        let ctx = ui.ctx().clone();
        let dock_w = (area.width() - 48.0).min(crate::ui::DOCK_MAX_W);
        let width = 430.0_f32.min(dock_w);
        let rect = Rect::from_min_size(
            pos2(area.center().x + dock_w / 2.0 - width, area.top() + DOCK_GAP + DOCK_H + 8.0),
            vec2(width, 42.0),
        );
        let p = ui.painter();
        for k in 1..=4 {
            p.rect_filled(rect.expand(k as f32 * 2.0).translate(vec2(0.0, 3.0)), 14.0 + k as f32 * 2.0, Color32::from_black_alpha(20));
        }
        p.rect_filled(rect, 14.0, DOCK);
        p.rect_stroke(rect, 14.0, Stroke::new(1.0, DOCK_EDGE), egui::StrokeKind::Inside);

        let (mut forward, mut backward, mut close) = (false, false, false);
        ui.scope_builder(
            UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let field = TextEdit::singleline(&mut self.search.query)
                    .id(Id::new(SEARCH_FIELD))
                    .hint_text("Find in document")
                    .desired_width(width - 190.0);
                let resp = ui.add(field);
                if std::mem::take(&mut self.search.want_focus) {
                    resp.request_focus();
                }
                if resp.changed() {
                    ctx.request_repaint();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if ui.input(|i| i.modifiers.shift) {
                        backward = true;
                    } else {
                        forward = true;
                    }
                    resp.request_focus();
                }
                if resp.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                    close = true;
                }
                let n = self.search.matches.len();
                let label = match (self.search.query.is_empty(), n) {
                    (true, _) => String::new(),
                    (false, 0) => "No results".to_owned(),
                    (false, _) => format!("{} / {}", self.search.current + 1, n),
                };
                ui.label(egui::RichText::new(label).size(12.5).color(TEXT_DIM));
                ui.add_enabled_ui(n > 0, |ui| {
                    backward |= ui.button("‹").on_hover_text("Previous (Shift+Enter)").clicked();
                    forward |= ui.button("›").on_hover_text("Next (Enter)").clicked();
                });
                close |= ui.button("✕").on_hover_text("Close (Esc)").clicked();
            },
        );
        self.refresh_matches();
        if forward {
            self.step_match(true);
        }
        if backward {
            self.step_match(false);
        }
        if close {
            self.close_search(&ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_case_insensitively_without_overlap() {
        assert_eq!(find_all("Abc abc ABC", "abc"), vec![(0, 3), (4, 7), (8, 11)]);
        assert_eq!(find_all("aaaa", "aa"), vec![(0, 2), (2, 4)]);
        assert!(find_all("abc", "").is_empty());
        assert!(find_all("abc", "abcd").is_empty());
    }

    #[test]
    fn positions_are_chars_not_bytes() {
        assert_eq!(find_all("ünï x", "x"), vec![(4, 5)]);
    }
}
