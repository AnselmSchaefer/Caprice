# Caprice — Architecture

How Caprice is put together, and the rules that hold it together. The rules (§4) are the part to
read before changing anything: each says what depends on what, what to check when touching it, and
which tests guard it. Keep this file true: a change that alters a rule updates it in the same commit.

*Last brought up to date: 2026-10-06 (commit `8604a5b`).*

---

## 1. What Caprice is

A desktop word processor in Rust on egui/eframe 0.36. It shows one page at a time on a desk, with
the pages before and after it as piles, and turns pages with an animation ("Book": hinged like a
book; "Paperstack": sliding into the pile). Post-its stick out of the page's edge. Chapters get
titles, drop caps and contents pages. It saves `.caprice` (JSON) and exports `.docx`. Claude Code
(the CLI) can review a passage drawn around with the pen, and paint a scene behind the pages.

---

## 2. The one idea: one stored flow, everything else derived

```
stored (.caprice)            derived, rebuilt from the stored state, never saved
─────────────────            ───────────────────────────────────────────────────
Flow: text + one Style       ──paginate (page size, scale 1)──▶ Doc::spans (the pages)
      per char                 │                                     │
PageSetup (paper, margins,     ├─ Doc::chapters() ─▶ contents pages  │
  page numbers, drop caps,     ├─ drop caps (which paragraphs, how   │
  contents)                    │   many chars beside the cap)        ▼
images (bytes, width,          └──────────────────────────▶ layout_page(i, zoom) ─▶ drawing
  rotation)                                                 (PageLayout: galleys,     (render, book,
notes (char ranges + text)                                   hit-testing, caret)       editor, notes)
                             export::to_docx(&Doc) ─▶ .docx (Word lays it out again itself)
```

- **`Flow`** (`model.rs`) is one `String` plus one `Style` per char (font, size, bold, underline,
  image id, paragraph attributes). Paragraph formatting sits on the paragraph's **terminator** char.
- **Pages are not stored.** `Doc::spans` is a cache: per page, a char/byte range of the flow, whether
  a hard break ends it, and, for a contents page, which part of the contents it shows.
- **Notes** are char ranges kept beside the text, not in it.
- **Everything shown is recomputed** from these each frame (egui is immediate mode). Caches that do
  exist key on `Doc::version` (search matches, the scene's "read at").

---

## 3. Modules

| Module | Owns |
|---|---|
| `model.rs` | `Doc`, `Flow`, `Style`, `ParaAttrs`, `PageSetup`, `Span`, `Note`, `ImageData`; special chars; char↔byte, `page_of`, `para_start`, `term_from` |
| `edit.rs` | `Edit` (replace / restyle), `Doc::apply`, undo/redo `History`, moving notes with edits (`rebase_notes`) |
| `paginate.rs` | `full_paginate`, `paginate_after` (incremental), `next_span`: where pages break, at page size |
| `layout.rs` | One egui `LayoutJob` per paragraph piece; `layout_page`; `PageLayout` (hit, caret, rows, selection, links); `true_to_scale`, `tight_highlights` |
| `dropcap.rs` | Which paragraphs get a drop cap, and the three-piece layout (cap, lines beside it, rest) |
| `contents.rs` | `Doc::chapters`, splitting the contents over pages, laying out one page of them, migrating the old contents char |
| `editor.rs` | Caret, selection, typing, deleting, clipboard, keys, mouse, formatting commands, `set_setup`, `turn_to`, the editable page surface |
| `main.rs` | `App` state, `main()`, the frame loop, page position animation, `go_to_page`, the app-level tests and their `Harness` |
| `view.rs` | Camera: fit, zoom, panning, two-finger swipe through pages |
| `render.rs` | Paperstack look: paper, piles, static pages, sliding pages |
| `book.rs` | Book look: pages turning on their left edge |
| `notes.rs` | Post-its: placement, pads, drawing, editing |
| `images.rs` | Pictures: inserting, resizing, rotating, moving, textures |
| `search.rs` | Find bar and matches |
| `ui.rs` | Top toolbar (formatting, Page menu), bottom page bar and scrollbar |
| `fileio.rs` | `.caprice` format (`DocFile`), open/save/export commands, unsaved-changes detection |
| `export.rs` | Hand-written Office Open XML for `.docx` |
| `claude.rs` | Pen loop → passage → `claude -p` → streamed answer panel |
| `backdrop.rs` | Scenes Claude paints (SVG) faintly behind the pages; not part of the document |
| `fonts.rs`, `theme.rs` | Installed fonts loaded on demand; colours and egui visuals |

---

## 4. Rules

Each rule: what holds, why, what to check when you touch it, and the tests that guard it.

### R1 — Every change to the text goes through `Doc::apply`

`App::apply_edit` / `replace_range` build an `Edit`, `Doc::apply` applies it (moving notes, recording
undo, grouping typing, bumping `version`), then `paginate_after` repaginates and `set_caret` shows
the caret's page.

- **Exceptions, on purpose:** page settings (`set_setup`) and picture size/rotation (`images.rs`) are
  not text. They change the `Doc` directly, bump `version`, run `full_paginate`, and are **not
  undoable**. A new non-text setting should follow the same pattern.
- **If you write to `flow.text`/`flow.styles` anywhere else** (file migration is the one place), you
  must also move notes yourself and repaginate.
- Tests: `undo_redo_restore_text_and_styles`, `typing_groups_into_one_undo_step_until_a_pause`,
  `grouped_edits_undo_together_and_each_group_is_kept`, `notes_follow_edits`.

### R2 — A paragraph is formatted by its terminator, so an edit reaches back to its start

`\n` (or `\u{c}`, a hard page break) ends a paragraph and carries its `ParaAttrs` (alignment, line
spacing, list, chapter title). Changing or moving that terminator reformats the **whole** paragraph,
including the part on earlier pages.

- `paginate_after` therefore starts at the page where the edited paragraph **begins**, not where
  the edit is. Drop caps reach further still: whether a paragraph gets one depends on the
  paragraphs before it (`drop_cap_reach`).
- New paragraph-level attribute: put it in `ParaAttrs`, read it from the terminator's style.
- Tests: `a_new_chapter_far_on_reflows_like_from_scratch`,
  `editing_a_title_repaginates_the_chapter_after_it_like_from_scratch`.

### R3 — Incremental pagination must equal full pagination

`paginate_after` keeps the pages before the edit and stops once a new page starts where an old one
did, shifting the rest. Anything that can change a page's content from far away breaks that
shortcut unless handled: today the contents (rebuilt on every edit, R6) and drop caps (R2).

- **If you add something whose size depends on text elsewhere,** make `paginate_after` account for
  it, and add an "incremental equals full" test for it.
- Tests: `incremental_matches_full`, `incremental_reuses_later_pages_correctly`,
  `incremental_handles_edits_that_remove_page_breaks`,
  `chapters_added_or_taken_away_change_the_contents_pages_like_from_scratch`.

### R4 — Pages are worked out at page size; drawing at any zoom must agree

Pagination lays out at scale 1 (page points). Drawing lays out again at the zoom scale, and egui
rounds line heights to whole pixels and can **wrap a word differently**. So:

- `true_to_scale` gives zoomed lines their page-size height × zoom, **per line**, not per paragraph.
  (Per paragraph spread or squeezed lines whenever the zoomed text wrapped into a different number
  of lines.)
- Drop caps, the contents and pictures are measured at page size and scaled.
- **If you add anything drawn on a page,** measure it at page size and multiply by the scale; never
  let its height come from the zoomed layout.
- Known gap: when zoomed text wraps into one line more than at page size, its paragraph is a line
  taller on screen than pagination assumed.
- Tests: `text_fills_the_page_the_same_at_every_zoom`,
  `lines_keep_their_spacing_when_zoomed_text_wraps_differently`,
  `a_chapter_fills_the_page_the_same_at_every_zoom`, `the_contents_take_the_same_room_at_every_zoom`.

### R5 — A page index is not a caret position

`Doc::spans[i]` is page `i`. Most pages hold a stretch of the story, but the **contents pages hold
none of it** (empty span at the story's start, `Span::contents = Some(part)`). `page_of(c)` never
returns them, so the caret is always in the story.

- **To show a page, use `turn_to(i)`** (or `go_to_page`), never `set_caret(spans[i].start)`: the
  start of a contents page is on another page. Swipes, the scrollbar, Page Up/Down and contents
  links all use `turn_to`.
- **`set_caret` shows the caret's page.** The shown page (`target`) differs from the caret's page
  only while a contents page is shown.
- **A click only places the caret if the spot it hits is on the clicked page** (`editor_surface`).
- Anything per page that uses `spans[i].start` (selection, notes, the pen) must cope with a page
  that holds no text.
- Tests: `page_keys_go_through_every_page_of_long_contents`,
  `swiping_goes_through_every_page_of_long_contents`,
  `the_scrollbar_reaches_every_page_of_long_contents`,
  `clicks_on_a_later_page_of_the_contents_stay_there_or_follow_a_line`,
  `the_caret_stays_in_the_story_around_long_contents`,
  `an_empty_page_between_two_breaks_exists_and_can_hold_the_caret`.

### R6 — Derived things must not depend on their own results

The contents list page numbers, and their page count shifts every page after them. So **how many
pages the contents take depends only on the chapter titles**, never on page numbers; the numbers
are filled in when a page is drawn (`chapters(true)` in `layout_page`, `chapters(false)` while
paginating).

- Same for anything new that shows page numbers or other layout results: decide its size from
  inputs that layout does not change.
- Tests: `long_contents_go_on_over_as_many_pages_as_they_take`,
  `the_contents_list_each_chapter_and_lead_to_its_page`.

### R7 — Notes are char positions outside the text

- `rebase_notes` moves them with every `Edit`. A rewrite of the text that bypasses `Doc::apply`
  (file migration) must move them too (`take_out_contents_chars` returns what it removed, so
  `into_doc` can).
- Notes never change where pages break.
- Tests: `notes_follow_edits`, `replacing_across_a_notes_edges`,
  `deleting_a_notes_text_leaves_a_point_note`, `post_its_do_not_change_where_pages_break`,
  `files_with_the_old_contents_char_open_with_the_setting_and_post_its_in_place`.

### R8 — Special characters in the flow

| Char | Meaning | Rules |
|---|---|---|
| `\n` | paragraph end | carries `ParaAttrs` (R2) |
| `\u{c}` (`PAGE_BREAK`) | paragraph end + hard page break | dropped from typed/pasted text; `editor.rs` inserts it |
| `U+FFFC` (`IMAGE_CHAR`) | a picture, `Style::image` = its id | in a paragraph of its own |
| `U+E000` (`CONTENTS_CHAR`) | **legacy only**: older files' contents | taken out on open (sets `PageSetup::contents`), filtered from pasted text |

- The final char of the flow is always a paragraph mark (`ensure_final_mark`); the caret's last
  position is just before it (`max_caret`).
- Text leaving the app (Claude, the scene, plain clipboard, chapter titles) leaves out `IMAGE_CHAR`
  and turns page breaks into line breaks.

### R9 — Files: only stored state, old files keep opening

- `.caprice` is JSON, version 2 (`DocFile`). Version 1 still loads.
- **New fields get `#[serde(default)]`** (`PageSetup` has it on the whole struct), so older files
  load and older versions of Caprice ignore what they don't know.
- **Never save derived data** (pages, contents, drop caps).
- Converting old formats happens in `DocFile::into_doc`, and must move notes (R7).
- Unsaved changes = a hash of the serialized `DocFile`, so anything saved counts automatically.
- Tests: `roundtrip_keeps_text_styles_breaks_and_setup`, `version_1_files_still_load_as_separate_pages`,
  `chapter_titles_are_saved_and_older_files_have_none`,
  `the_contents_setting_is_saved_opened_and_exported_through_the_app`.

### R10 — Word export maps the model, not the pixels

Word lays the document out again by its own rules, so `export.rs` maps Caprice's model onto Word's
(table in its module comment): chapter titles → Heading 1, drop caps → Word drop caps, contents →
a `TOC` field (Caprice's page numbers are only its cached result, and Word updates them on opening).

- **A new formatting feature needs its Word mapping**, or a decision that it has none, in the same
  change. Check the output with LibreOffice (`CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`,
  then `soffice --headless --convert-to pdf out.docx`). Not yet checked in Microsoft Word itself.
- Tests: in `export.rs`, plus `notes_become_comments_over_the_same_text`.

### R11 — egui workarounds depend on egui internals

| Problem in egui 0.36 | Workaround | Where |
|---|---|---|
| Zoomed line heights rounded to pixels | Rescale rows to page-size height × zoom, per line | `layout.rs::true_to_scale` |
| Text background fills the whole line height | Trim background quads to the font height | `layout.rs::tight_highlights` |
| No text flowing around a box | Drop cap paragraph as three pieces | `dropcap.rs` |
| `TextEdit` caret height from its own font | Pass the post-it font | `notes.rs` |
| No bending of text | Tessellate galleys, move vertices | `render.rs`, `book.rs`, `notes.rs` |
| Modifiers unreliable on Hyprland | Track Ctrl from key events | `view.rs` |

Re-check every one after upgrading egui.

### R12 — Tests never call Claude

`claude::run_claude` refuses under `cfg(test)` unless a test calls `allow_live()`; those tests are
`#[ignore]` (`live_*`). Calls run `claude -p` with no tools, user settings only, no MCP, no saved
session, in the temp dir.

---

## 5. Checklists for common changes

**A new paragraph attribute** (like line spacing or chapter title): `ParaAttrs` → `run_format` /
`paragraph_job` → the toolbar (`ui.rs`) → file format default (R9) → Word mapping (R10) → whether
it changes drop caps (`has_drop_cap`) or the contents (`chapters`).

**A new page setting:** `PageSetup` field + default → Page menu (`ui.rs`, applied through
`set_setup`) → pagination/layout → Word section or settings (R10) → round-trip test.

**Something new drawn on a page:** size at page size (R4) → pagination (`next_span`) and layout
(`layout_page`) agree → incremental equals full (R3) → both looks (`render.rs` and `book.rs`) →
hit-testing and caret (`PageLayout`).

**Page navigation or a new way to move between pages:** use `turn_to` / `go_to_page` (R5); test it
on the contents pages with `Harness::long_contents`.

**The file format:** `serde(default)`, a migration in `into_doc` that moves notes, an old-file test.

---

## 6. Tests

- `cargo test` runs everything headlessly (about 130 tests, ~6 s). They drive the real `App`
  through an egui `Context`: `layout::with_ctx` for model/layout tests, `Harness` (in `main.rs`)
  for app-level ones (`frames`, `key`, `type_text`, `click_in_page`, `scrub_to`, `long_contents`,
  `shown`).
- Prefer tests of what the user sees (page shown, caret, text) over internals, so they survive
  redesigns. Name them as sentences of the behaviour.
- For anything incremental, compare with doing it from scratch.
- Before trusting a new test, check it fails without the fix.
- `cargo mutants` (config in `.cargo/mutants.toml`) finds untested code. `/tmp` is a small tmpfs
  here: run it with `TMPDIR` on disk.

---

## 7. Decisions

- **2026-10-06 — Contents are a page setting, not text.** First built as a char in the flow
  ("a real page in the story"). With contents running over several pages, all of them shared that
  one char and every page/caret mapping needed special cases. Now `PageSetup::contents` and empty
  `Span`s before the story (R5, R6).
- **No `.docx` import.** Export only.
- **Contents heading is "Contents"** (English), in the app and in Word.
