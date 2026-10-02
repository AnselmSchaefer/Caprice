//! The edit layer. Every change to the document goes through here as an `Edit`, so one place
//! keeps notes anchored, remembers how to undo, and tells the rest of the app what changed.

use crate::model::{Doc, Note, Style};

/// A run of text together with its per-char styles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    pub text: String,
    pub styles: Vec<Style>,
}

impl Piece {
    pub fn chars(&self) -> usize {
        self.styles.len()
    }

    pub fn plain(text: &str, style: &Style) -> Self {
        Self { text: text.to_owned(), styles: vec![style.clone(); text.chars().count()] }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// Replace `old` at char `at` with `new` (insert: `old` empty, delete: `new` empty).
    Replace { at: usize, old: Piece, new: Piece },
    /// Change styles only; both vectors have the same length.
    Restyle { at: usize, old: Vec<Style>, new: Vec<Style> },
}

impl Edit {
    pub fn insert(at: usize, text: &str, style: &Style) -> Self {
        Edit::Replace { at, old: Piece::default(), new: Piece::plain(text, style) }
    }

    pub fn inverse(&self) -> Edit {
        match self {
            Edit::Replace { at, old, new } => Edit::Replace { at: *at, old: new.clone(), new: old.clone() },
            Edit::Restyle { at, old, new } => Edit::Restyle { at: *at, old: new.clone(), new: old.clone() },
        }
    }

    /// Where the caret belongs after this edit has been applied.
    pub fn caret_after(&self) -> usize {
        match self {
            Edit::Replace { at, new, .. } => at + new.chars(),
            Edit::Restyle { at, new, .. } => at + new.len(),
        }
    }

    /// Change in (chars, bytes) of the document length.
    pub fn delta(&self) -> (isize, isize) {
        match self {
            Edit::Replace { old, new, .. } => (
                new.chars() as isize - old.chars() as isize,
                new.text.len() as isize - old.text.len() as isize,
            ),
            Edit::Restyle { .. } => (0, 0),
        }
    }

    /// Would `self` followed by `next` read as one continuous act of typing or deleting?
    fn continues_with(&self, next: &Edit) -> bool {
        match (self, next) {
            (Edit::Replace { at: a1, old: o1, new: n1 }, Edit::Replace { at: a2, old: o2, new: n2 }) => {
                let typing = o1.chars() == 0 && o2.chars() == 0 && *a2 == a1 + n1.chars();
                let deleting = n1.chars() == 0 && n2.chars() == 0 && (*a2 + o2.chars() == *a1 || a2 == a1);
                typing || deleting
            }
            _ => false,
        }
    }
}

pub struct History {
    undo: Vec<Vec<Edit>>,
    redo: Vec<Vec<Edit>>,
    last_edit_time: f64,
}

impl Default for History {
    fn default() -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), last_edit_time: f64::NEG_INFINITY }
    }
}

impl History {
    const MAX: usize = 1000;
    /// Edits closer together than this, and contiguous, undo as one step.
    const GROUP_GAP: f64 = 1.0;

    fn record(&mut self, edit: Edit, now: f64) {
        self.redo.clear();
        let joins = now - self.last_edit_time < Self::GROUP_GAP;
        self.last_edit_time = now;
        if joins {
            if let Some(group) = self.undo.last_mut() {
                if group.last().is_some_and(|last| last.continues_with(&edit)) {
                    group.push(edit);
                    return;
                }
            }
        }
        self.undo.push(vec![edit]);
        if self.undo.len() > Self::MAX {
            self.undo.remove(0);
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_edit_time = f64::NEG_INFINITY;
    }
}

impl Doc {
    /// Apply an edit and remember it for undo. `now` is in seconds and drives grouping.
    pub fn apply(&mut self, edit: Edit, now: f64) {
        self.apply_raw(&edit);
        self.history.record(edit, now);
    }

    /// Undo the last step; returns where the caret should go.
    pub fn undo(&mut self) -> Option<usize> {
        let group = self.history.undo.pop()?;
        let mut caret = 0;
        for edit in group.iter().rev() {
            let inv = edit.inverse();
            self.apply_raw(&inv);
            caret = inv.caret_after();
        }
        self.history.redo.push(group);
        self.history.last_edit_time = f64::NEG_INFINITY;
        Some(caret)
    }

    pub fn redo(&mut self) -> Option<usize> {
        let group = self.history.redo.pop()?;
        let mut caret = 0;
        for edit in &group {
            self.apply_raw(edit);
            caret = edit.caret_after();
        }
        self.history.undo.push(group);
        self.history.last_edit_time = f64::NEG_INFINITY;
        Some(caret)
    }

    fn apply_raw(&mut self, edit: &Edit) {
        match edit {
            Edit::Replace { at, old, new } => {
                let b = self.char_to_byte(*at);
                debug_assert_eq!(&self.flow.text[b..b + old.text.len()], old.text);
                self.flow.text.replace_range(b..b + old.text.len(), &new.text);
                self.flow.styles.splice(*at..*at + old.chars(), new.styles.iter().cloned());
                rebase_notes(&mut self.notes, *at, old.chars(), new.chars());
            }
            Edit::Restyle { at, new, .. } => {
                self.flow.styles[*at..*at + new.len()].clone_from_slice(new);
            }
        }
        self.version += 1;
    }
}

/// Move note anchors to follow a replacement of `a` chars at `at` by `b` chars. Text typed right
/// at the edge of a note does not join it; a note whose text is deleted shrinks to a point.
fn rebase_notes(notes: &mut [Note], at: usize, a: usize, b: usize) {
    let start = |p: usize| if p < at { p } else if p >= at + a { p + b - a } else { at + b };
    let end = |p: usize| if p <= at { p } else if p >= at + a { p + b - a } else { at };
    for n in notes {
        let (s, e) = (start(n.start), end(n.end));
        (n.start, n.end) = if s > e { (e, e) } else { (s, e) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> (Doc, Style) {
        let st = Style::new("x");
        let mut d = Doc::new();
        d.flow.text = text.to_owned();
        d.flow.styles = vec![st.clone(); text.chars().count()];
        (d, st)
    }

    fn note(start: usize, end: usize) -> Note {
        Note { id: 1, start, end, text: String::new(), color: 0 }
    }

    #[test]
    fn undo_redo_restore_text_and_styles() {
        let (mut d, st) = doc("hello world");
        let bold = Style { bold: true, ..st.clone() };
        let old = Piece { text: "world".into(), styles: vec![st.clone(); 5] };
        d.apply(Edit::Replace { at: 6, old, new: Piece::plain("there", &bold) }, 0.0);
        assert_eq!(d.flow.text, "hello there");
        assert_eq!(d.flow.styles[6], bold);
        assert_eq!(d.undo(), Some(11)); // caret ends up after the restored text
        assert_eq!(d.flow.text, "hello world");
        assert_eq!(d.flow.styles[6], st);
        assert_eq!(d.redo(), Some(11));
        assert_eq!(d.flow.text, "hello there");
        assert!(d.redo().is_none());
    }

    #[test]
    fn typing_groups_into_one_undo_step_until_a_pause() {
        let (mut d, st) = doc("");
        for (k, c) in "abc".chars().enumerate() {
            d.apply(Edit::insert(k, &c.to_string(), &st), 0.1 * k as f64);
        }
        d.apply(Edit::insert(3, "d", &st), 5.0); // after a pause: its own step
        assert_eq!(d.flow.text, "abcd");
        d.undo();
        assert_eq!(d.flow.text, "abc");
        d.undo();
        assert_eq!(d.flow.text, "");
        assert!(d.undo().is_none());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let (mut d, st) = doc("");
        d.apply(Edit::insert(0, "a", &st), 0.0);
        d.undo();
        d.apply(Edit::insert(0, "b", &st), 9.0);
        assert!(d.redo().is_none());
    }

    #[test]
    fn notes_follow_edits() {
        let (mut d, st) = doc("one two three");
        d.notes.push(note(4, 7)); // "two"
        d.apply(Edit::insert(0, "zero ", &st), 0.0); // before it
        assert_eq!((d.notes[0].start, d.notes[0].end), (9, 12));
        d.apply(Edit::insert(10, "X", &st), 1.0); // inside it
        assert_eq!((d.notes[0].start, d.notes[0].end), (9, 13));
        d.apply(Edit::insert(13, "!", &st), 2.0); // right at its end: not included
        assert_eq!((d.notes[0].start, d.notes[0].end), (9, 13));
        d.apply(Edit::insert(9, "?", &st), 3.0); // right at its start: not included
        assert_eq!((d.notes[0].start, d.notes[0].end), (10, 14));
    }

    #[test]
    fn deleting_a_notes_text_leaves_a_point_note() {
        let (mut d, _) = doc("one two three");
        d.notes.push(note(4, 7));
        let old = Piece { text: "two".into(), styles: d.flow.styles[4..7].to_vec() };
        d.apply(Edit::Replace { at: 4, old, new: Piece::default() }, 0.0);
        assert_eq!((d.notes[0].start, d.notes[0].end), (4, 4));
    }
}
