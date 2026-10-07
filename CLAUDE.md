# Caprice

A desktop word processor in Rust on egui/eframe 0.36. **Read `ARCHITECTURE.md` before changing
behaviour**: its rules (§4) say what depends on what, and which tests guard each rule.

## Commands

- `cargo build`, `cargo test` (headless, about 6 s), `cargo clippy --all-targets`
- `./install.sh`: release build, installed to `~/.local/bin/caprice` with a launcher entry
- `cargo mutants` for untested code. `/tmp` is a small tmpfs here: set `TMPDIR` to a dir on disk.
- Word output check: `CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`, then
  `soffice --headless --convert-to pdf out.docx`

## Done means

- `cargo test` passes, and clippy has no warnings beyond the ones already there (15 today).
- Behaviour you can see in the app has an app-level test (`Harness` in `main.rs`). A bug fix has a
  test that fails without the fix: check that it does.
- If a change alters a rule in `ARCHITECTURE.md`, or adds one, update the file in the same commit.

## Keeping the rules growing

`ARCHITECTURE.md` §4 should cover every part of the code, and it grows as the code does. Without
being asked, whenever you:

- find a bug whose cause was an unwritten dependency between parts of the code,
- add a feature that other code must now keep in step with (a cache, a derived value, a new kind
  of page or character, a new file field, a new Word mapping),
- or notice that a rule there is wrong, too narrow, or names code or tests that no longer exist,

then add or adjust the rule in the same change: what holds, why, what to check when touching it,
and the tests that guard it (write the test if there is none). Add a line to §5 if it is a common
kind of change, and to §7 if it records a design decision. Keep the "Rules that bite most often"
list below in step when a rule is important enough. Say in your summary which rules you added or
changed.

## Code style

- **Do not run `cargo fmt`.** The code is not in rustfmt's default style; it would rewrite
  hundreds of places. Match the surrounding code by hand: lines up to about 130 characters,
  short ones preferred.
- Comments are plain English prose that say why, in the voice of the existing ones ("The caret
  goes before or after the block, like a picture."). Module comments (`//!`) explain the idea.
- Test names are sentences of the behaviour (`the_caret_stays_in_the_story_around_long_contents`).
  Test what the user sees (page shown, caret, text) rather than internals.
- Commit messages: `Area: what changed`, then a blank line and bullets or a short paragraph.

## Rules that bite most often

Full list with reasons and tests in `ARCHITECTURE.md` §4.

1. **Text changes go through `Doc::apply`** (via `apply_edit` / `replace_range`). Page settings
   and picture size/rotation are the deliberate exceptions: `full_paginate`, not undoable.
2. **Paragraph formatting is on the terminator**, so an edit can reformat its paragraph from the
   start. Reflow starts at the page where the edited paragraph begins.
3. **Incremental pagination must equal full pagination.** Add a test that compares them for
   anything new whose size depends on text elsewhere.
4. **Pages are measured at page size.** Anything drawn zoomed takes its height from the page-size
   layout × zoom.
5. **A page index is not a caret position.** Show a page with `turn_to` / `go_to_page`, never with
   `set_caret(spans[i].start)`. Contents pages hold no text.
6. **Derived things must not depend on their own results** (the contents' page count comes from
   the titles only).
7. **Notes and scenes are char positions outside the text.** Anything that rewrites text without
   `Doc::apply` must move them.
8. **Files hold only stored state.** New fields get `#[serde(default)]`; old formats are converted
   in `DocFile::into_doc`.
9. **Every formatting feature needs a Word mapping** in `export.rs`, or a decision that it has none.
10. **Tests never call Claude** unless marked `#[ignore]` and calling `claude::allow_live()`.

## Files

- Tracked: `README.md`, `ARCHITECTURE.md`, `CLAUDE.md`.
- Local, untracked planning notes: `TECHNICAL_DESCRIPTION.md` (older, superseded by
  `ARCHITECTURE.md`), `FORK_REQUIREMENTS.md` (when to fork egui), `CHAPTERS_PLAN.md`.
