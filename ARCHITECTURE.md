# Caprice — Architecture

How Caprice is put together, and the rules that hold it together. The rules (§4) are the part to
read before changing anything: each says what depends on what, what to check when touching it, and
which tests guard it. Keep this file true: a change that alters a rule updates it in the same commit.

*Last brought up to date: 2026-10-22 (the cast peeking out from behind the page).*

---

## 1. What Caprice is

A desktop word processor in Rust on egui/eframe 0.36. It shows one page at a time on a desk, with
the pages before and after it as piles, and turns pages with an animation ("Book": hinged like a
book; "Paperstack": sliding into the pile). Post-its stick out of the page's edge. Chapters get
titles, drop caps and contents pages; headers and footers in a few looks, page numbers that can
start again or be left off some pages, and text boxes placed anywhere on a page. It saves `.caprice` (JSON), exports `.docx`, and prints, to a
printer or a PDF file, the pages as it shows them. Claude Code
(the CLI) can review a passage drawn around with the pen or selected with the cursor, and paint scenes that show faintly behind the pages, each
for its part of the story, drawing the people of the story's cast the same in every scene. On the
left, Claude can review a chapter (mistakes, style, too thin, too wordy) and propose what could
happen next.

---

## 2. The one idea: one stored flow, everything else derived

```
stored (.caprice)            derived, rebuilt from the stored state, never saved
─────────────────            ───────────────────────────────────────────────────
Flow: text + one Style       ──paginate (page size, scale 1)──▶ Doc::spans (the pages)
      per char                 │                                     │
PageSetup (paper, margins,     ├─ Doc::chapters() ─▶ contents pages  │
  header, footer, drop caps,   ├─ drop caps (which paragraphs, how   │
  contents)                    │   many chars beside the cap)        ▼
images (bytes, width,          └──────────────────────────▶ layout_page(i, zoom) ─▶ drawing
  rotation)                                                 (PageLayout: galleys,     (render, book,
notes (char ranges + text)                                   hit-testing, caret)       editor, notes)
numbering (a char + change)  ──page_numbers()──▶ what {page} and the contents show on each page
text boxes (a char + place)  ──boxes_on(i)──▶ drawn over page i (App::furniture, with the header and footer),
                                               or under its text (App::underlay)
scenes (a char + drawings)   ──scene_at(caret) / page_scene(i)──▶ the drawing behind a page
cast (names, looks, sheets)  ──named_in(what is painted, or just before)──▶ the people sent with a scene
                             export::to_docx(&Doc) ─▶ .docx (Word lays it out again itself)
                             layout_page(i, 1) ─▶ underlay + PageLayout::ink + furniture ─▶ PDF ─▶ a file, or lp (CUPS)
```

- **`Flow`** (`model.rs`) is one `String` plus one `Style` per char (font, size, bold, italic,
  underline, colour, image id, paragraph attributes). Paragraph formatting sits on the paragraph's **terminator** char.
- **Pages are not stored.** `Doc::spans` is a cache: per page, a char/byte range of the flow, whether
  a hard break ends it, and, for a contents page, which part of the contents it shows.
- **Notes** are char ranges kept beside the text, not in it; **scenes** and **text boxes** are chars likewise.
- **Everything shown is recomputed** from these each frame (egui is immediate mode). Caches that do
  exist key on `Doc::version` (search matches, the scene's "read at").

---

## 3. Modules

| Module | Owns |
|---|---|
| `model.rs` | `Doc`, `Flow`, `Style`, `ParaAttrs`, `PageSetup`, `Span`, `Note`, `ImageData`, `Scene`, `Character`, `Look`; special chars; char↔byte, `page_of`, `para_start`, `term_from` |
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
| `search.rs` | Find and replace bar, matches (case, whole words) |
| `words.rs` | The Document info window: dates, file, counts (words, chapter, selection…), today's words and the daily goal, kept in the data folder |
| `ui.rs` | Top toolbar (formatting, Page and Insert menus, the text box gallery), bottom page bar and scrollbar |
| `bands.rs` | Headers and footers: their looks drawn as `Ink`, fields filled in per page, the gallery tiles, accent colours |
| `numbers.rs` | Page numbering: changes pinned to a char (start again, count on, none; figures or roman), the number of each page, setting one from a page |
| `textbox.rs` | Text boxes: drawn as `Ink`, inserted, written in, moved, resized, pinned to a char, in front of the text or behind it; their selection |
| `fileio.rs` | `.caprice` format (`DocFile`), open/save/export commands, unsaved-changes detection |
| `export.rs` | Hand-written Office Open XML for `.docx` |
| `emblem.rs` | The emblem beside the name on the welcome page: the app icon's tile and page, a pencil drawing its loop over and over |
| `leave.rs` | The question before leaving a story with changes (save or not; keep or drop): its first page, what is at stake, that a version is kept; the name suggested for a story never saved |
| `versions.rs` | Autosaving a story with a file; versions of every story kept in the data folder; the Versions window |
| `print.rs` | The Print window; pages drawn into a PDF (krilla) from `PageLayout::ink`; printers and jobs through CUPS (`Spooler`) |
| `claude.rs` | Pen loop or cursor selection → passage → `claude -p` → streamed answer panel, a conversation; story notes |
| `backdrop.rs` | Scenes Claude paints (SVG), pinned to the story, faintly behind the pages; the Scene menu and list |
| `cast.rs` | The cast: people found by name in what is painted, their looks and model sheets in the scene's prompt; the Cast window and its example |
| `start.rs` | The welcome page when Caprice opens without a file: kinds of new story (`Kind`), the stories last opened with what they hold, the sheet flying to the desk; the list of stories last opened, kept in the data folder |
| `peek.rs` | The cast peeking out from behind the page: the puppet asked of Claude, cut into parts, who peeks when, where they look, the peek drawn |
| `scratchpad.rs` | The scratchpad: plain text saved with the document, in a window on the right |
| `review.rs` | The Review window (upper left): a chapter or the selection read for mistakes, style, detail, wordiness; findings found again by their quote |
| `ideas.rs` | The What next window (lower left): ways the story could go on from the caret, kept in the scratchpad |
| `fonts.rs`, `theme.rs` | Installed fonts loaded on demand, a missing one drawn with a twin of the same widths; colours and egui visuals |

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
- **Enter puts the caret before the old paragraph's terminator**, which keeps the look it had.
  Typing must not take its style from there: after text is typed (`replace_raw`) the typing style
  stays as it was, so a colour, size or bold chosen to type with goes on to the next line. Only
  moving the caret or deleting takes the style from the text (`typing_follows_caret`).
- Tests: `a_new_chapter_far_on_reflows_like_from_scratch`,
  `editing_a_title_repaginates_the_chapter_after_it_like_from_scratch`,
  `a_colour_chosen_for_typing_goes_on_to_the_next_line`.

### R3 — Incremental pagination must equal full pagination

`paginate_after` keeps the pages before the edit and stops once a new page starts where an old one
did, shifting the rest. Anything that can change a page's content from far away breaks that
shortcut unless handled: today the contents (rebuilt on every edit, R6) and drop caps (R2; the
paragraphs beside a cap are on its page, so the reflow from there covers them, R4).

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
- **A drop cap fits the lines set, top to tail** (`Doc::drop_cap`): its top is where the first
  line's letters' tops are and its baseline the last line's; a letter that hangs below its
  baseline (j, p, q, y) is made smaller instead, so its tail ends with the last line. It is
  measured at a large size (`REF`), since at text size its ink is a few whole pixels. Should a tail
  still reach into the next line, that line goes beside it too; the rest of the paragraph never
  starts under the tail with a line left empty beside it (it did). Pagination and drawing both
  take the block from `DropCap`. With nothing after the letter yet, its lines are measured with
  a stand-in letter in its style (an empty line has no baseline: the cap sat lower, then jumped up
  as the first letter was typed). Tests: `the_cap_sinks_as_many_lines_deep_as_set`,
  `a_cap_hanging_below_its_line_is_made_smaller_to_keep_its_tail_in_the_lines_set`,
  `the_cap_stays_put_when_the_first_letter_after_it_is_typed`,
  `the_drop_cap_stays_put_while_the_first_words_are_typed`.
- **A paragraph that ends beside its cap leaves the room there to the next ones** (`Float`): they
  go on beside the letter, narrower, until it ends (`DropCap::bottom`); one that runs past it goes
  on at full width below. Enter beside a cap used to send the new line below the whole letter.
  How much of a paragraph goes beside is worked out at page size (`Doc::beside_cap`) by
  `next_span` and `layout_page` alike, which carry the room from one paragraph to the next on the
  page; anything that changes how those two walk a page keeps them in step. A picture or another
  cap ends the room. Clicking right of the cap finds the piece beside it at that height (`hit`).
  Tests: `enter_beside_a_drop_cap_goes_on_to_the_next_line_beside_it` (with full pagination
  compared).
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
  start of a contents page is on another page. Swipes, the scrollbar and Page Up/Down use
  `turn_to`; a contents link moves the caret to its chapter's page (a story page), like a link.
- **Turning pages leaves the caret where it is**, as scrolling does in other word processors: the
  shown page (`target`) and the caret's page often differ. Only Shift+Page Down/Up (extending the
  selection) takes the caret along. Anything that acts at the caret must not assume it is on the
  page shown; what shows on the page shown without the caret (the scene, R14) comes from the page.
- **`set_caret` shows the caret's page**, so typing, arrow keys and edits bring it back.
- **A reflow that is not an edit keeps the page being read**, not the caret's: page settings
  (`set_setup`) show the page the shown page's first char goes to, at once, and only keep the
  caret in range, without `set_caret` (which turned to the caret's page: with the caret at the
  end, switching A4 to Letter showed the last page). Adding the contents turns to them.
- **A click only places the caret if the spot it hits is on the clicked page** (`editor_surface`).
- Anything per page that uses `spans[i].start` (selection, notes, the pen) must cope with a page
  that holds no text.
- **A selection dragged well past the top or bottom of the writing area turns the page**
  (`drag_past_edge`: `EDGE_PAST` page points into the margin, times the zoom; nearer, it only
  selects to the first or last line),
  the caret going to a story position on the next or previous page, so `set_caret` shows it. It
  never turns onto or from the contents. The drag is the app's own (`App::sel_drag`) and survives
  the turn, when the editor surface is not drawn and egui may drop its own. Held there, it turns
  again after `EDGE_REPEAT` on the settled page, not every frame.
- Tests: `page_keys_go_through_every_page_of_long_contents`,
  `swiping_goes_through_every_page_of_long_contents`,
  `the_scrollbar_reaches_every_page_of_long_contents`,
  `clicks_on_a_later_page_of_the_contents_stay_there_or_follow_a_line`,
  `the_caret_stays_in_the_story_around_long_contents`,
  `turning_pages_leaves_the_caret_where_it_is_until_typing_or_a_click`,
  `changing_the_paper_keeps_the_page_being_read_in_view`,
  `an_empty_page_between_two_breaks_exists_and_can_hold_the_caret`,
  `dragging_a_selection_past_the_bottom_of_the_page_goes_on_to_the_next_pages`,
  `dragging_a_selection_past_the_top_of_the_page_goes_back_to_the_pages_before`,
  `dragging_a_selection_past_the_top_of_the_story_does_not_turn_onto_the_contents`.

### R6 — Derived things must not depend on their own results

The contents list page numbers, and their page count shifts every page after them. So **how many
pages the contents take depends only on the chapter titles**, never on page numbers; the numbers
are filled in when a page is drawn (`chapters(true)` in `layout_page`, `chapters(false)` while
paginating).

- Same for anything new that shows page numbers or other layout results: decide its size from
  inputs that layout does not change.
- Tests: `long_contents_go_on_over_as_many_pages_as_they_take`,
  `the_contents_list_each_chapter_and_lead_to_its_page`.

### R7 — Notes and scenes are char positions outside the text

- `rebase_notes`, `rebase_scenes`, `rebase_boxes` and `rebase_number_marks` move them with every `Edit`. A text box pin
  moves like a scene pin. A scene pin is a point: text
  typed right at it goes after it (it still starts its paragraph or sentence), and deleted around, it stays
  where the text was. A rewrite of the text that bypasses `Doc::apply`
  (file migration) must move them too (`take_out_contents_chars` returns what it removed, so
  `into_doc` can).
- Neither changes where pages break.
- **Anything new pinned to the text** goes the same way: moved in `apply_raw`, and in `into_doc`.
- Tests: `notes_follow_edits`, `replacing_across_a_notes_edges`,
  `text_boxes_stay_with_their_char_and_where_it_was_once_it_is_deleted`,
  `deleting_a_notes_text_leaves_a_point_note`, `post_its_do_not_change_where_pages_break`,
  `scenes_stay_with_their_paragraph_through_edits_and_saving`,
  `files_with_the_old_contents_char_open_with_the_setting_and_post_its_and_scenes_in_place`.

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
- Converting old formats happens in `DocFile::into_doc`, and must move notes and scenes (R7).
  The one exception is the scene of older files (`DocFile::scene.svg`), which needs the app: it
  becomes a scene pinned at 0 in `App::open_scene`. `scene` now only holds the description and
  whether pictures show.
- Unsaved changes = a hash of the serialized `DocFile`, so anything saved counts automatically.
- `PageSetup::page_numbers` is read only from files from before footers: `into_doc` makes it a
  plain footer of the page number (`Band::page_numbers`) and it is never written again. (Older
  Caprice opening a newer file shows no page numbers.)
- Page numbering: `DocFile::numbers` (the changes pinned in the story, moved by `into_doc` like
  the boxes) and `PageSetup::contents_numbers`, both left out of the file while unused (R26).
- `DocFile::id` names the story for its versions (R22). Older files have none: they get one on
  opening, which counts as saved (it is written with the next save). `DocFile::created` likewise:
  older files take the file's own date (created, else modified) on opening, and keep it once saved.
- A look's peek (`Look::peek`, an SVG) and whether the cast peeks (`Doc::peeks`, on unless turned
  off: older files peek) are left out of the file while unused (R28).
- Tests: `roundtrip_keeps_text_styles_breaks_and_setup`, `version_1_files_still_load_as_separate_pages`,
  `a_peek_and_whether_the_cast_peeks_are_saved_and_older_files_peek`,
  `files_from_before_footers_keep_their_page_numbers_as_a_footer`,
  `chapter_titles_are_saved_and_older_files_have_none`,
  `the_contents_setting_is_saved_opened_and_exported_through_the_app`,
  `an_older_files_one_scene_opens_pinned_to_the_start_of_the_story`,
  `story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections`,
  `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`,
  `a_scene_that_names_someone_in_the_cast_is_painted_with_their_look_and_model_sheet`.

### R10 — Word export maps the model, not the pixels

Word lays the document out again by its own rules, so `export.rs` maps Caprice's model onto Word's
(table in its module comment): chapter titles → Heading 1, drop caps → Word drop caps, contents →
a `TOC` field (Caprice's page numbers are only its cached result, and Word updates them on opening).

- **A new formatting feature needs its Word mapping**, or a decision that it has none, in the same
  change. Check the output with LibreOffice (`CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`,
  then `soffice --headless --convert-to pdf out.docx`). Not yet checked in Microsoft Word itself.
- Scenes are not exported (decided: they are a writing aid, too faint to print).
- Story notes, the scratchpad and the cast are not exported (decided: notes to Claude and to
  oneself, not part of the book).
- Headers and footers are Word's header and footer parts, their fields Word's (`PAGE`,
  `STYLEREF "Heading 1"`), their looks paragraph borders, shading, a "❦" and an
  ellipse shape (module comment of `export.rs`); Facing pages are odd and even headers
  (`evenAndOddHeaders` in `settings.xml`). Known difference: before the first chapter, Caprice
  shows no chapter, and Word's `STYLEREF` shows the first one after.
- Text boxes are text box shapes (`wps:wsp`) anchored in the paragraph of their char, at their
  place on the page, `wrapNone` (in front of the text, as in Caprice), growing to their text
  (`spAutoFit`). Quote and sidebar lines are paragraph borders inside the box.
- Page numbering is sections (R26): each change starts one at its paragraph, its `sectPr` in the
  last paragraph before (`end_section`), starting a page where a hard break was (the break is
  taken by it) and `continuous` elsewhere. `pgNumType` holds the start and `lowerRoman`. A section
  without numbers refers to an empty header or footer part (`header3.xml`, `footer3.xml`) in
  place of one showing the page number: with no reference it would carry on the one before. The
  contents end their section in an empty paragraph of their own, as Word rewrites the `TOC`
  field's paragraphs. Known difference: Word changes the numbering of a continuous section from
  the next page that begins in it, Caprice from the page its char is on.
- Character formatting maps to run properties (`run_props`): bold, italic (`<w:i/>`), underline,
  and colour (`<w:color>`; none is Word's automatic colour, as it is Caprice's usual ink).
- The sample checks (`sample_docx`, `sample_pdf`) lay the sample out in a context that has its
  font loaded; laid out in a fresh `with_ctx` it panicked once any other test had loaded it.
- Tests: in `export.rs`, plus `notes_become_comments_over_the_same_text`,
  `italic_and_coloured_text_is_drawn_saved_and_exported`.

### R11 — egui workarounds depend on egui internals

| Problem in egui 0.36 | Workaround | Where |
|---|---|---|
| Zoomed line heights rounded to pixels | Rescale rows to page-size height × zoom, per line | `layout.rs::true_to_scale` |
| Text background fills the whole line height | Trim background quads to the font height | `layout.rs::tight_highlights` |
| No text flowing around a box | Drop cap paragraph as three pieces; text boxes lie in front of the text | `dropcap.rs`, `textbox.rs` |
| Only convex shapes are filled | Fleurons and circles built of convex pieces (`leaf`, `circle`); the emblem's tile a fan of triangles, its pencil quads | `bands.rs`, `emblem.rs` |
| A field's galley is shown from its left, whatever its alignment | Box text laid out left, rows moved within the width (`box_galley`) | `textbox.rs` |
| `TextEdit` caret height from its own font | Pass the post-it font | `notes.rs` |
| No bending of text | Tessellate galleys, move vertices | `render.rs`, `book.rs`, `notes.rs` |
| Modifiers unreliable on Hyprland | Track Ctrl from key events | `view.rs` |
| The UI font has no symbols like ✕ (drawn as an empty box) | Draw icons with lines, labelled for tests | `ui.rs::paint_icon`, `search.rs::bar_button` |
| `ctx.fonts_mut` inside `ctx.fonts_mut` hangs (the context's lock is taken twice) | Build a job (`paragraph_job`, `run_format`, which ask the fonts for a row's height) before taking the fonts to lay it out. Drop caps with line spacing hung: `a_new_novel_begins_on_book_pages_at_chapter_one_and_a_journal_at_today` | `dropcap.rs::drop_cap` |
| Justified rows are lined up by their first and last letters: whitespace in front goes out to the left, a tab or space at a paragraph's end gives the caret no room | `justified_edges` moves the glyphs (and their quads) back: the whitespace in front keeps its room, a stretched row still ends at the margin, the last row is as wide as all its chars. Underlines and highlights of such a row are not moved. `justified_text_keeps_a_tab_before_it_and_after_it` | `layout.rs::justified_edges` |
| Tab moves egui's keyboard focus to the next widget, even when the page took the key | The editor cancels the move when it types a tab (`move_focus(FocusDirection::None)`); the caret is hidden while a widget has the keyboard. `a_tab_typed_beside_a_drop_cap_moves_the_caret_at_once` | `editor.rs` (keys) |
| `set_fonts` takes effect the next frame; a family named before then panics | `FontBook::ensure` in mid-frame with `immediate: false`, and draw with `family_for` (the usual font until it is there). `fonts::LOADED` is one for the process, so tests running alongside share it: a test that loads a font for the first time is the only one to | `fonts.rs`, `start.rs::welcome_page` |

Re-check every one after upgrading egui.

### R12 — Tests never call Claude

`claude::run_claude` refuses under `cfg(test)` unless a test calls `allow_live()`; those tests are
`#[ignore]` (`live_*`). Calls run `claude -p` with no tools, user settings only, no MCP, no saved
session, in the temp dir.

### R13 — Asking Claude: circling with the pen, or selecting with the cursor

With Claude on (`App::pen`), `App::picking` says how a passage is picked, chosen in the toolbar's
Claude menu:

- **Circle:** `pen_surface` takes the drags and draws loops; the editor surface is drawn but not
  interactive (`circling()`), so drags never select. A loop stays on one page.
- **Cursor:** the editor works as always, so a selection can be dragged over several pages (R5).
  Letting go of a drag that left a selection (`sel_drag`, a double or triple click too) offers the
  commands for it (`offer_commands`), a `Lasso` with no points, only `caught`.
- Either way the commands come from the one `Lasso` menu; anything new there must work for both.
- **The menu's text fields take the typing, never the passage.** The passage is still selected
  while the menu is open, so a keystroke that reaches the editor replaces it. Whichever field the
  menu shows takes focus the first frame it shows (`menu_shown`). A button that swaps the menu for
  another step (Connect the scenes → `bridging`) must stop drawing the menu that frame (`return`),
  or a field further down takes the focus and is gone the next frame.
- **Claude is sent only the marked passage**, never other text of the document: the writer
  decides what Claude sees by what they circle or select. The Review and What next windows (R19)
  are the one exception, and keep its point: their button names what it sends before it is sent. Added to it are only what the writer
  wrote for Claude: the note typed for Connect the scenes, and the story notes (`Doc::story_notes`)
  with every command about the story (`knows_story`), not with corrections (Fix, Grammar). It is
  built in one place, `App::prompt`, and kept in `Answer::first`. Connecting the scenes
  (`Command::Bridge`) expects the passage to hold both scenes, and Claude finds the gap in it.
  (It once also sent 12,000 chars of story on each side, which for most stories was all of it.)
- **An answer is a conversation.** `claude -p` keeps nothing between calls (we pass
  `--no-session-persistence`), so every reply sends `Answer::sent()`: the first prompt again,
  then each earlier answer and reply. A new kind of follow-up adds its instruction there.
  `follow_up` stops the answer being written before starting the next.
- **An answer that can take the passage's place** is `Answer::rewrites()`: Fix and Grammar, or a
  draft (`draft`, from Draft it: the passage again with the gap filled, shown as a diff; a reply
  to a draft asks for it revised). Claude sometimes answers a draft with only the new part, which
  would replace both scenes: the request names the passage's first and last words, and Apply is
  off unless the draft keeps them (`keeps_ends`). Apply and
  Pin as note both need the passage unchanged since it was asked about (`unchanged`): the range
  is not moved with edits. A pinned answer is a post-it like any other (R7, saved, a Word comment).
- Tests: `claude_is_sent_only_the_marked_text_whatever_it_is_asked` (a new command or follow-up
  goes into it), `a_loop_drawn_with_the_pen_selects_the_text_inside_it`,
  `circling_with_the_pen_takes_drags_away_from_the_cursor`,
  `asking_claude_by_cursor_offers_the_commands_for_a_selection_over_two_pages`,
  `connecting_two_scenes_sends_the_marked_scenes_with_the_writers_note`,
  `replying_to_claude_sends_the_conversation_so_far`,
  `a_draft_fills_the_gap_in_the_marked_scenes_in_one_undo_step`,
  `a_reply_to_a_draft_asks_for_it_revised`,
  `pinning_an_answer_puts_it_on_a_post_it_over_the_passage_once`,
  `story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections`.

### R14 — Scenes: one per pin, each covering the story up to the next; the caret's shows

`Doc::scenes` are kept with the story (saved, R9), each pinned to a char: the start of the
paragraph it was painted for (a description, a selected passage), or, following the writing, the
sentence its part of a passage begins with. `start_scene` and `pin_painting` take the pin as it
is; callers that paint for a paragraph pass `para_start` themselves.

- **A scene covers the story from its pin up to the next scene's** (`Doc::scene_at(c)`: the last
  shown, painted scene pinned at or before `c`).
- **The caret's page follows the caret** (`App::scene_of_page`): it shows the scene of the part
  the caret is in, wherever it is drawn (shown, in a pile, turning), and moving the caret into
  another scene's part fades to that scene. Every other page shows the scene at its top
  (`Doc::page_scene(i)`). So a page looks the same while turning as once turned to, and turning
  back to the caret finds its picture as it was left. Every look (`render.rs`, `book.rs`, the
  settled page in `main.rs`) passes its page to `backdrop_shapes`.
- **Following the writing groups sentences into scenes by what they say, and paints each scene
  once the next begins.** The first edit opens a scene at the sentence written in (`open_at_edit`,
  from `History::last_at`); what was written before counts as drawn. After `PAUSE` without edits,
  the sentences finished since (`sentence_starts`) go to Claude numbered, with the open scene's
  text so far (`reading_prompt`), and it answers which begin a new scene (`parse_firsts`). At each,
  the scene before ends: it is queued with its text up to there as subject (`reading_done`) and
  painted, once; the new one is open. The open scene is never painted while open.
  - **Only content decides**, never layout: dialogue, thoughts and description belong to the scene
    they happen in, and line breaks and empty lines are no scene breaks (the prompt says so).
    Lines only end sentences, since a line of dialogue often has no full stop. A capital after
    ". " begins a sentence; lower case goes on it (`"Where?" he asked.` is one).
  - The open scene and each sentence being read are unpainted scenes pinned to their starts, so
    edits move them (R7). How many of the open scene's sentences were read is a count
    (`Open::read`), so a sentence put into the open scene's middle shifts which is read next by one.
  - If Claude cannot answer, the sentences go on the open scene. `Backdrop::drawing` covers the
    reading and the queue, and "Stop painting" (`stop_painting`) ends both; the open scene stays.
- **Painting a paragraph again adds a version** to its scene (`pin_painting` finds it by `at`)
  rather than another scene; `shown` picks one. Moving a scene onto a paragraph with one already
  (Pin here) takes the other's place.
- **A scene is made when painting starts**, with no versions, so its pin follows edits made while
  Claude paints (or, following the writing, while it is open, being read, or queued). It is not
  saved (`from_doc` skips it), never shows, and is dropped once its painting ends without a
  picture (`drop_unpainted_scenes`, which keeps the open, read and queued ones).
- **Drawings are rendered on threads, when needed:** those of pages within `NEAR` of the page
  shown, of scenes that begin on them (for the caret to reach), and of the caret's scene wherever
  the pages are, and small ones while the list is open. Textures beyond those are let go. Turned
  to, a page shows its drawing at once; a change on the page shown (caret, painted, another
  version, hidden) fades.
- **A painting carries the people its subject names** (R17), their look and model sheet before
  the passage in the prompt (`scene_prompt`), kept in `Backdrop::asked`.
- Moving, hiding, deleting and stepping versions change `Doc::scenes` directly: like page
  settings, they are not undoable (R1) and do not paginate.
- Tests: `the_picture_follows_the_caret_from_one_part_of_the_story_to_the_next`,
  `turning_back_to_the_caret_finds_its_picture_as_it_was_left`,
  `painting_a_paragraph_again_adds_a_version_and_keeps_the_others`,
  `a_scene_still_being_painted_is_not_saved_and_goes_if_stopped`,
  `following_the_writing_paints_each_scene_once_the_next_one_begins`,
  `following_the_writing_begins_with_the_sentence_written_in`,
  `live_reading` (ignored, calls Claude: dialogue stays in its scene), and those of R7 and R9.

### R15 — Post-its: one size, the text in sheets; a sheet shows its own rows and no more

A post-it is always `NOTE_SIZE` square. `sheets` breaks its text into sheets of whole rows (as
many as fit the text area above the counter), and two drawings must agree with it:

- **At rest** (`draw_notes`) the whole text is one `TextEdit`, shifted up to the sheet on top and
  clipped to that sheet's rows, not to the whole text area. The area has room for part of one
  more row, which would show cut in half and look like text that cannot be read.
- **Painted** (`note_shapes`: other pages, turning, the pad flipping) each sheet is laid out on
  its own (`sheet_galley`).
- A change to the font, padding, footer or row breaking goes through `sheets`, and both follow.
- **The sheet counter ("‹ 2/3 ›") is placed in one place, `sheet_counter`,** used both to draw it
  and to split the footer into back and forward at its middle. It sits at the footer's right, so
  split at the footer's own middle, both arrows turned forward.
- **A post-it goes wherever its page goes.** In a book, pages in the pile lie turned over and show
  their post-its' backs mirrored on the left (`stack`), so a page may only get there by turning
  over (`book_turn`, which carries its post-its). Adding a page in a book therefore slides the
  new page in from the right while the old one turns over onto the pile (`book_slide_in`); the
  paperstack slides it in while the old page goes into the pile as on any page forward
  (`slide_page`, with the new page as `under`), its post-its with it. Backspacing a page away is the same
  backwards: it slides out to the right while the page before turns back over it out of the pile
  (`book_slide_out`, at the same pace), and only then is that page drawn flat and editable. On a
  paperstack the page before comes back out of the pile as on any page back (`paper_slide_out`,
  through `slide_page`), and is likewise editable only once flat. Both looks take the same
  unhurried pace for a page coming or going (`book::NEW_PAGE_PACE`). Anything
  new that moves pages must carry the post-its with them, or they jump sides.
- Tests: `a_post_it_shows_whole_rows_and_the_rest_on_its_next_sheet` (checks what is painted,
  through `Harness::painted`), `a_post_its_sheets_turn_back_as_well_as_forward`,
  `in_a_book_a_new_page_slides_in_as_the_old_one_turns_over_with_its_post_it` (follows the
  post-it's colour and the new page's paper),
  `in_a_book_a_backspaced_page_slides_out_as_the_one_before_turns_back_with_its_post_it` and
  `in_a_book_a_page_backspaced_away_between_others_turns_the_one_before_back_too` (both follow the
  post-it, the leaving page and the pace, through `assert_turns_back`),
  `typing_while_a_backspaced_page_slides_out_goes_into_the_page_turning_back`,
  `on_a_paperstack_a_backspaced_page_slides_out_as_the_one_before_comes_back_with_its_post_it`,
  `on_a_paperstack_a_new_page_slides_in_as_the_old_one_goes_into_the_pile_with_its_post_it`.

### R16 — Windows float over the pages, and what happens over them is theirs

The scratchpad, the review, the ideas, Claude's answer, the story notes, the scene panels, the cast, Print, Versions and Document info are egui windows
over the pages. They belong to the window, not to a page, so turning pages leaves them as they are. The
scratchpad's text is the document's (`Doc::scratchpad`, saved, R9); whether it shows is the app's.

- **Input over a window is the window's.** The view acts on the wheel and touchpad only over the
  pages (`ctx.layer_id_at` is a `Background` layer): `swipe_pages` for sideways swipes, and
  `page_rect` for panning up and down. Anything new the view does with the pointer checks the same.
- Keys typed into a window's field never reach the story: the editor yields while
  `egui_wants_keyboard_input()`.
- **A window sizes itself to what it holds, so a row must not ask for more than it has.** A field
  given `available_width()` less a guess for the button after it widens the window a little every
  frame, without end. Lay such a row out right to left inside `horizontal` (the button first, then
  the field at `f32::INFINITY`); `with_layout` alone takes all the height left and centres the row
  in it. (Found in the Cast window, guarded by `someone_can_keep_several_looks_and_scenes_get_the_one_in_use`.)
- **Anything that takes clicks over the page is on a layer of its own** (a window, or an egui
  `Area` like the second row of tools). The editor takes a press whenever the pointer is in its
  rect on the background layer (`contains_pointer`), whatever else was drawn over it there: a
  click on the second row's tools, drawn on the background, also moved the caret in the text
  under it. Guarded by `tools_that_do_not_fit_open_in_a_second_row_under_the_toolbar`. Text
  boxes are drawn on the background too, after the editor, so the editor leaves a press alone
  where a box lies (`box_hits`, passed to `mouse`); guarded by
  `a_text_box_is_written_in_moved_and_resized_and_goes_where_its_text_goes`. The search
  bar is still drawn on the background; it lies over the top margin, which takes no clicks, unless
  the margin is very small.
- **The toolbar is in groups (`ui::GROUPS`), and those that do not fit go into "⋯"**, the last
  first, which opens them in a second dock under the toolbar (rows as needed). Each group is
  measured as drawn (`tool_w`) and the fit is worked out from last frame's widths, so a group only
  ever put away is measured when the second row is open. **A group's measured width must not
  depend on where it is drawn**: the line before the paragraph and page groups (`has_line`,
  `LINE_W`) is drawn outside the measured scope and added in the sums (`cost`). When the line was
  measured with its group, a group starting a row was narrower than the same group inside one: it
  ran past the second row's edge, and at some widths it moved back and forth between two rows
  every frame. Guarded by `the_toolbar_settles_at_every_width_and_its_second_row_holds_its_tools`.
  The second row closes on a click on the pages, but not on one in a menu opened from it (a popup
  is on a higher layer); the search bar goes under it (`tools_bottom`). All the tools take about
  1020 points, about 1090 while a scene is painted (the Claude group grows), so they fit in the
  test window (`Harness`, 1240 wide, where the dock is its widest, `DOCK_MAX_W`) and the tests
  reach every menu; `Harness::width` narrows it. A new tool that makes them too wide shows as a
  test failing to find a menu of the last group ("Scene"). A new toggle goes in the page bar (like Fit and Scratchpad)
  or into a menu.
- **The document's name is always at the toolbar's right end.** The tools get the room first, but
  the name keeps about four of its characters' width; when it has less than it needs it runs past
  in a loop (`NAME_SPEED`), resting with its start showing between passes (`NAME_PAUSE`); it
  repaints about 30 times a second while it moves. Guarded by
  `the_file_name_is_always_in_the_toolbar_and_runs_past_when_there_is_little_room`.
- **Nothing in a dock is drawn over anything else or past the dock's edge, at any window width.**
  The page bar lays out from the right (Scratchpad, Fit, the zoom, New page) and, as it gets
  shorter, drops the page count, shrinks New page to "+", drops the track, the zoom, Scratchpad,
  "+" and Fit, the track taking whatever room is left (guarded too by
  `the_page_bar_leaves_no_empty_room_left_of_its_buttons`); the sums in `scrollbar` must match the
  widths drawn. The toolbar drops "⋯" too when there is no room for it. `ui::MIN_WINDOW` keeps a
  floating window wide enough for the usual buttons, but a tiling window manager ignores it, so
  the docks must give way at any width. Something new in either dock needs room in these sums and
  a place in the order things go. (Buttons used to slide out past the bar's left end, and
  Scratchpad stayed in a window narrower than the bar.) The second row's rows are as wide as the
  toolbar, but a group wider than that gets a row reaching nearly across the window, and one wider
  still is left out. Guarded by `in_a_narrow_window_the_docks_hold_their_buttons_without_overlap`
  and, for everything that takes clicks in both docks and the second row at every width from 40 to
  1300 with a short and a long name, `at_any_width_the_docks_settle_and_hold_their_tools_apart`
  (in place, inside its dock, clear of the others, the same from one frame to the next). A new
  tool is covered by it without a change.
- **An item marked as chosen in a menu is `Button::selectable` (or `selectable_label`), never
  `.selected(true)` with `.frame(false)`**: the theme gives chosen items dark text meant for the
  accent highlight, and without a frame there is none, so it was dark on the dark menu (the text
  box's look). Guarded by `the_look_a_text_box_has_reads_clearly_in_its_menu` (`painted_contrast`).
- **A popup that needs more input (a colour picker) holds it in itself**, never in a popup of its
  own: a click in a popup opened from a popup counts as outside the first, which closes both (the
  "Other colour" picker did nothing). Guarded by
  `any_colour_can_be_picked_and_black_goes_back_to_the_usual_ink`.
- Tests: `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`,
  `scrolling_over_the_scratchpad_scrolls_it_and_not_the_page`.

### R17 — The cast: people are found by name in what is painted, or just before it, and sent with that scene only

`Doc::cast` holds the people of the story (`Character`: name, other names, and one or more
`Look`s, each words and a model sheet), saved with the document (R9), never exported (R10), not
undoable and not pinned to the text.

- **One look is in use at a time** (`Character::active`, `look()`), chosen by hand in the Cast
  window, and every painting from then on gets that one; others are kept, sheets and all, to switch
  back to. Everything that sends or draws a look goes through `look()` or a look's id, never
  `looks[0]`. A new look starts from the words of the one in use, and is not put in use. Deleting
  the look in use puts the next one in use; the last look cannot be deleted. Scenes already
  painted keep their pictures, and do not record which look they got.
- **A model sheet belongs to a look** (`SheetJob` holds the person's and the look's id), so a
  sheet arriving after its look was deleted is dropped, and one drawn for a look not in use waits
  there. Older files hold one look as `look`/`sheet` on the person; `into_doc` makes it their only
  look (`people_from_files_before_looks_open_with_their_one_look_in_use`).

- **Who is in a scene comes from its subject** (`Doc::named_in`: the passage or the description
  painted), from the paragraphs just before a passage that names no one (below), or from the
  writer's choice (`people_for`). Only the subject and the cast are sent, so, as in R13, Claude
  sees only what the writer marked plus what the writer wrote for Claude. Every way of painting
  goes through `paint_with` → `scene_prompt`; a new one must too.
- **A passage that names no one carries on those named just before it** (`carried_people`): the
  sentences before it in its own paragraph (a scene may begin partway into one), else the
  nearest paragraph before it that names anyone, `CARRY_BACK` paragraphs back at most, never across
  a break in the story (chapter title, page break, an empty or starred line). Only passages carry;
  a description is all the writer wants painted. It reads text the passage does not hold, but only
  to choose among the cast: that text is never sent (R13).
- **A scene keeps who it was painted with** (`Scene::people`, by name, saved), set when its
  painting arrives (`Job::people`), so a stopped or failed painting changes nothing. The list of
  scenes shows them; choosing others there (`chosen`) makes every later painting of that scene
  use them instead of looking again. "Paint again" paints with exactly those shown
  (`paint_with`). A renamed or deleted person is left out of the prompt (`cast_by_names`).
- **Names are whole words, case counts but for the first letter** (`find_word`): "Mara's" names
  Mara, "Maradona" does not, "rose" is not Rose, and "The miller" is "the miller". Changing this
  changes who appears in pictures already painted the next time they are repainted.
- **A scene gets everyone's look but at most `MOST_SHEETS` model sheets**, those named first: each
  sheet is 15 to 30 KB of SVG, and more people in one picture get mixed up. Sheets name their ids
  after the person (`slug`), so two in one picture do not clash.
- **The passage decides the pose** (`cast_prompt`): told only to reuse the sheet, Claude copied a
  standing figure into a passage where she sat. Sheets have a sitting view for the same reason.
  Known gap: a pose the sheet shows from another side (sitting with her back to us) still came
  out standing.
- A model sheet is kept only if it renders (`draw_sheet`). The example picture is taller than some
  graphics cards take, so it is scaled to `max_texture_side` when read.
- Tests: `a_scene_that_names_someone_in_the_cast_is_painted_with_their_look_and_model_sheet`,
  `a_model_sheet_is_drawn_from_the_look_and_the_example_opens_from_the_cast`,
  `a_passage_that_names_no_one_is_painted_with_those_named_just_before_it`,
  `the_list_of_scenes_says_who_each_was_painted_with_and_paints_again_with_those_chosen`,
  `someone_can_keep_several_looks_and_scenes_get_the_one_in_use`, and in `cast.rs`
  `people_are_found_by_any_of_their_names_as_whole_words`,
  `a_scene_tells_how_the_people_it_names_look_and_sends_the_first_sheets`,
  `a_model_sheet_is_asked_for_with_ids_from_the_name`.

### R18 — At a soft wrap one position is two places; `prefer_next` says which

Rows of a paragraph are contiguous (`PageLayout::rows`): a row ends where the next one starts, so
the position at a wrap is both the end of one line and the start of the next. `App::prefer_next`
says which: true (set by `set_caret`, Home) the start of the next line, false (End) the end of
this one.

- **Everything that asks which line the caret is on goes through `row_of` / `caret_rect` with
  `prefer_next`**, and they must agree with what is drawn: egui draws the caret by
  `CCursor::prefer_next_row` the same way. Taking the wrap as the end of the line above while the
  caret shows at the start of the next made Down stay put and Up skip a line.
- Something new that moves the caret line by line, or to a line's ends, sets `prefer_next` for
  where it means the caret to be.
- Tests: `up_and_down_from_the_start_of_a_wrapped_line_move_one_line`,
  `arrow_keys_treat_a_drop_cap_and_the_line_beside_it_as_one_line`.

### R19 — Review and What next: the button names what is read; findings are found again by their words

Two windows on the left (`review::left_dock`: the review above, the ideas below), opened from the
Claude menu, not saved as open, their results not saved either.

- **What is sent is named on the button that sends it**, so the writer still decides what Claude
  reads (R13). Review: the selection, else the caret's chapter (from its title to the next), else
  the whole story, at most `review::MOST` chars (whole paragraphs around the caret beyond that), in
  `review_stretch`, which both the label and `start_review` use. What next: the story before the
  caret (`ideas_from`, at most `ideas::BACK`, from a paragraph's start; the tip says how many
  words), the story notes, the writer's wish, and the scratchpad while "Use the scratchpad" is
  ticked. Never text after the caret.
- **A finding holds the words it is about, not a range.** The text is edited while the findings
  show, so `find_quote` looks for the quote each frame, the place nearest where the stretch began
  if it is there more than once; a finding whose words are gone cannot be shown or applied.
  Applying goes through `replace_range` (R1: one undo step). The review asks for a quote short
  enough to be found, and copied exactly.
- **Both read Claude's whole reply as a JSON array** (`claude::json_list`, tolerant of words or a
  code fence around it), through `claude::Pending`, a request on a thread that stops when dropped.
  Unknown kinds and empty entries are left out (`parse`).
- An idea is kept by adding it to the end of the scratchpad (saved, R9), not on a post-it: it is
  about where the story goes, not about a passage.
- Tests: `the_review_reads_the_carets_chapter_shows_what_it_found_and_applies_a_fix_in_one_undo_step`,
  `what_next_reads_the_story_before_the_caret_and_keeps_an_idea_in_the_scratchpad`, and in
  `review.rs` / `ideas.rs` the parsing (`findings_are_read_from_the_reply_whatever_is_around_it`,
  `ideas_are_read_from_the_reply_whatever_is_around_it`).

### R20 — Print draws what the screen draws, from the same layout

`App::pdf` lays every page out at page size (`layout_page(i, 1.0)`, no highlights) and draws
`PageLayout::ink`, the list `paint_layout` draws on screen, between `App::underlay` (text boxes
behind the text) and `App::furniture` (header, footer, text boxes in front), which every look
draws too, in that order (R24). So a
printed page breaks, wraps and justifies where the page shown does.

- **Anything new drawn on a page goes into `PageLayout::ink`**, not straight onto the painter in
  `paint_layout`, or it shows on screen and not on paper. A new kind of `Ink` needs drawing in both.
  Text is printed from the galleys' own sections (font, colour, underline, slant), so new character
  formatting done through `run_format` prints without more work; one drawn by other shapes does not.
- **Fonts are embedded from `FontBook::defs`**, the same files egui draws with, each char in the
  first face of its family's chain that has it, as egui picks. A font loaded some other way must
  go into `defs` too.
- **`Ink::Fill` and `Ink::Line`** are the shapes of headers, footers and text boxes: a fill must be
  convex (egui fills nothing else). On screen and on a turning page they go through `Ink::shape`;
  print draws them as paths (`draw_shape`).
- **Letters follow their own widths** (`places`): at page size egui puts each glyph on a whole
  point, which printed looked like "bul let". Lines keep egui's start and, justified, its end.
  Chars egui shaped into one glyph (a ligature) are printed one by one, sharing its width.
- Not printed (decided): the scene behind the page, post-its, search and note highlights, the
  paper's colour. Pictures are printed turned, from `export_media`.
- **Printers are CUPS's** (`lpstat`, `lp`, the same on macOS and Linux) behind the `Spooler` trait;
  setting one up is the system's (the window opens its printer settings). Asked on a thread each
  time the window opens; a job is written to a temporary PDF that `lp` reads before it answers.
  **Tests never reach the system's printers**: they put a `Spool` in `App::print.spooler`.
- Ctrl+P opens it, but not with Shift: Ctrl+Shift+P takes up the pen (guarded by
  `a_loop_drawn_with_the_pen_selects_the_text_inside_it`).
- Tests: `ctrl_p_prints_the_page_shown_on_the_default_printer`,
  `print_sends_copies_of_some_pages_to_the_printer_chosen_from_the_menu`,
  `with_no_printer_set_up_print_says_so_and_saves_every_page_as_a_pdf`,
  `each_printed_page_holds_the_text_of_that_page_and_its_number`, `a_picture_is_printed_with_its_page`,
  `a_header_and_footer_from_the_insert_menu_show_each_pages_chapter_and_number_and_print`,
  and in `print.rs` the reading of `lpstat` and `places`. Check the output by eye:
  `CAPRICE_SAMPLE_PDF=out.pdf cargo test sample_pdf`, then `pdftoppm -png out.pdf page`.

### R21 — A font is stored by name and drawn with what is installed: itself, or its twin

`Style::font` is the name the writer chose, saved (R9) and exported (R10) as it is. `FontBook::ensure`
loads the installed family that draws it (`drawn_with`): the font itself, else a twin from
`TWINS` made to take the same room char for char (Times New Roman, Liberation Serif, Tinos;
Arial, Liberation Sans, Arimo; Courier New, Liberation Mono, Cousine; Calibri, Carlito; Cambria,
Caladea). So a story written on a Mac in Times New Roman breaks into the same pages on Linux with
Liberation Serif, and prints the same (R20 embeds the twin, as drawn).

- **Fonts load under the stored name** (`face_key(font, …)`), whatever family draws them, and
  everything asks for them by that name (`family_for`). Never store the installed family's name.
- A font of the document that is not offered yet (not common, or a twin's name) is added to
  `families` on loading, so the toolbar shows it.
- With neither the font nor a twin installed, it is drawn with egui's own font, and the pages
  break differently. Only add a twin that is truly metric-compatible: one that is merely alike
  moves page breaks without saying so.
- Tests: `a_story_in_a_font_not_installed_takes_the_pages_it_takes_in_its_twin_and_keeps_its_name`
  (it finds a pair with only the twin installed, and passes quietly on a machine that has none),
  and in `fonts.rs` `a_missing_font_is_drawn_with_an_installed_twin_of_the_same_widths`.

### R22 — A story with a file saves itself; every story keeps versions of itself

`App::keep_safe` looks at the story every `CHECK_EVERY` seconds (it serializes the whole of it,
as `unsaved` does, so not every frame). Changes `SAVE_AFTER` old are saved to the story's file,
quietly. Opening another story or closing saves a story with a file first; then, if it changed
since it was opened or saved by hand (`App::as_saved`, which autosave leaves alone), it asks
whether to keep the changes or drop them back to that (`drop_changes`, also in the File menu). A
story never saved asks Save / Don't save as before. Closing to throw away an afternoon's changes
must keep working: writers do it. A version (the whole `.caprice` file) is kept at most every
`KEEP_EVERY` while the story changes, on opening, before going back to another, and on exit
(`on_exit`), so "Don't save" on a story never saved still leaves it in the versions.

- **Versions belong to the story's id**, not its path: `<data>/<id>/<unix seconds>.caprice`, with
  `story.json` saying its name and file (none: never saved, listed under "Stories never saved").
  Anything that starts another story (open, going back to one never saved) sets `App::story_id`
  and calls `Keeper::opened`, or its versions mix with the last story's.
- **The same state is not kept twice** (`kept_hash`), and a fresh story with nothing in it never.
  Two versions in one second get the next free second's name rather than writing over each other
  (dropping changes, then opening the list, did).
  Of older versions, `kept` keeps the newest `KEEP_NEWEST`, then the last of each day for
  `KEEP_DAYS` days. A story with many pictures takes room: each version holds them all.
- **The question says what is at stake, and keeps its word** (`leave.rs`): the story's words and
  chapters, how many words changed since it was saved by hand and when (`App::saved_at`, set with
  `as_saved`), or when it was begun; and that a version is kept whichever is chosen. So "Don't
  save" keeps a version (`keep_now`) before going on, when opening another story too, not only on
  exit; dropping changes keeps one already. What it tells is worked out once per document version
  (`App::leave_about`), not each frame. A story never saved is offered a file name from its first
  chapter's title or first words (`suggested_name`), in this question and in Save as.
  Tests: `leaving_a_story_never_saved_tells_what_is_at_stake_and_keeps_a_version`,
  `leaving_after_saving_tells_how_much_changed_since`.
- **Going back and dropping changes are not undoable** (like opening, R1): the story as it was
  is kept as a version first, so going back is undone by going back to that one. Going back to this story's version
  keeps its file, so autosave then writes the older text over it.
- Timing uses the frame clock (`ctx.input time`), so tests move it (`Harness::wait`); version names
  use the wall clock, or `Keeper::clock` in tests. Nothing waits on a frame: while saving or keeping
  is due, `keep_safe` asks for one.
- **Tests never write behind your back**: `Harness::new` turns autosave off and sets no folder (nor
  one for the stories last opened, R27); the tests about it turn them on with a folder of their own (`test_dir`).
- Tests: `a_story_with_a_file_saves_itself_half_a_minute_after_a_change`,
  `closing_a_story_with_a_file_asks_whether_to_keep_the_changes_already_saved`,
  `closing_can_drop_the_changes_since_saving_and_they_stay_in_the_versions`,
  `changes_since_saving_can_be_dropped_from_the_file_menu_and_a_story_never_saved_still_asks`,
  `versions_are_kept_while_writing_and_going_back_keeps_the_story_as_it_was`,
  `a_story_never_saved_is_kept_when_caprice_closes_and_opens_again_from_versions`, and in
  `versions.rs` `kept` and `ago`.

### R23 — Find and replace keep the story's formatting and undo as one; document info never touches the story

- **A replacement takes the style of the first char it replaces** (`App::replacement`), so a bold
  or coloured name stays so. Replace one goes through `apply_edit`; replace all applies every
  edit as one `Doc::apply_group` (one undo step, notes and scenes moved, R1, R7), from the last
  match back so the places found stay true, then `full_paginate`. After replacing one, the next
  match is the first from the end of the replacement (`resume_at`): the replacement may match too.
- Whole words: neither end of a match is inside a word (letters, digits, `_`), so "ann's" names
  Ann and "Anne" does not, as in R17. Finding starts at the first match from the caret.
- Ctrl+H is Ctrl on a Mac too: Cmd+H hides the app there.
- **Word counts are in File > Document info, not in sight while writing** (asked for: a count
  always showing distracts). The window also shows the file, when the story was begun
  (`App::created`, saved) and last saved (the file's time), the versions kept, and what the story
  holds. Something new to know about a story goes there.
- **Counting words changes nothing saved.** The story's count is cached by `Doc::version`; the
  daily goal and each story's count at its first sight today live in the data folder
  (`progress.json`), the writer's, not the story's. In the story's file they would make it
  unsaved on opening, and the goal is a habit across stories. "Today" is every story's words since
  first seen that day (deleting counts against it), the day being the system clock's local one
  (`date +%z`, asked once). `Harness::new` sets no file and an empty progress.
- Tests: `ctrl_h_renames_someone_once_and_everywhere_in_one_undo_step_keeping_the_bold`,
  `document_info_tells_the_words_of_the_story_and_selection_and_todays_towards_the_goal`,
  `a_story_keeps_when_it_was_begun_and_an_older_file_takes_its_own_date`, and in
  `search.rs` / `words.rs` finding, counting, the chapter around the caret and today's words.

### R24 — Headers, footers and text boxes lie on the page; they never move the text

Headers and footers (`PageSetup::header`, `footer`, a `Band` each: a look, a line of text, a
colour) sit in the middle of the top and bottom margins; text boxes (`Doc::boxes`) lie wherever
they were put, in front of the story's text or behind it. Neither changes where pages break, so
pagination and R3 do not know them, and a header or footer change does not paginate (`set_band`).

- **Everything they draw is `Ink`, from two places**: `App::underlay` (boxes behind the text) and
  `App::furniture` (header, footer, boxes in front). A page is drawn in layers: the scene, the
  underlay, the story's text and its pictures (pictures are in the text, so always level with
  it), then the furniture. Every path keeps that order: the page shown (`draw_bands`,
  `text_boxes_behind`, the editor, then `text_boxes`), the static and sliding pages
  (`draw_underlay`, `paint_layout`, `draw_furniture`), a book's turning page (the underlay in the
  text's mesh before the text; the furniture in a mesh of its own after the pictures, which are
  quads of their own) and print (R20). Something new drawn on a page that is not the story's text
  goes into one of them.
- **`Doc::boxes` is in stacking order**, bottom first; `behind` says which layer a box is in.
  "Bring to front" and "Send to back" (`stack_text_box`) move a box into the front or back layer
  and to the top or bottom of the list. Word gets the same: `behindDoc`, and `relativeHeight` from
  the box's place in the list.
- **A box behind the text leaves the text to the story.** It takes the pointer only where no row
  of text or picture lies (`PageLayout::covers`, `takes_pointer`), unless it is selected; the
  editor leaves presses alone where `box_hits` says a box takes them, so both ask the same thing.
  egui hit-tests with the last frame's widgets, so this is one frame behind the pointer.
- **Transparent (`clear`) changes the fill only**: none for a simple box, its tint for a shaded
  box or a sidebar laid on with alpha, so on blank paper it looks the same. Looks without a fill
  do not offer it. In Word: `noFill`, or `a:alpha` on the fill.
- **A band shows one thing**, chosen with radio buttons: the page number (`{page}`, as numbered
  by R26; a band showing it is not drawn at all, look and all, on a page without one), the
  chapter's title (`{chapter}`: the chapter that starts on the page, else the last before it,
  `chapter_of_page`), or text of the writer's own. The fields are filled in when drawn
  (`bands::filled`). There is no page count (not wanted). Their size never
  feeds back into layout (R6). A header showing the chapter asks for `chapters(false)` once a page;
  print asks once for all pages.
- **A text box is pinned to a char and shows on its page** (`boxes_on`): pushed onto the next
  page with its text, it goes along; moved by hand, it is pinned again to the char under its
  top-left corner (`pin_box`). Its height grows to hold its text (`box_height`, at page size, R4).
  Its text is one style; while a box is selected the toolbar's font, size, letters, colour and
  alignment format the whole box (`format_box`, `tool_style`), and Ctrl+B/I/U do too.
- **Clicking a box writes in it; the story keeps its caret.** A selected box not being written in
  takes Delete, Backspace and Escape (`box_key`); any other key, a typed letter or a click on the
  story lets it go. Changes to headers, footers and boxes are not undoable (R1).
- **A right click anywhere on a box opens its menu** (look, colour, transparent, front or back, delete). While it is written
  in, its text field lies over it and takes the pointer there, so the field gets the same menu
  (`box_menu` on both); with it on the box alone, only the thin edge around the text opened it.
- Tests: `a_header_and_footer_from_the_insert_menu_show_each_pages_chapter_and_number_and_print`,
  `a_text_box_is_written_in_moved_and_resized_and_goes_where_its_text_goes`,
  `a_right_click_anywhere_on_a_text_box_offers_its_menu_whether_written_in_or_not`,
  `a_text_box_sent_to_the_back_lies_behind_the_text_and_pictures_and_comes_to_the_front_again`,
  `text_boxes_stay_with_their_char_and_where_it_was_once_it_is_deleted`,
  `files_from_before_footers_keep_their_page_numbers_as_a_footer`, and in `bands.rs`
  `fields_are_filled_in_for_each_page`, `a_leaf_is_convex_and_reaches_from_end_to_end`.

### R25 — Copying formatting: the letters' look always, the paragraph's only onto whole paragraphs

The brush in the letters group (`toggle_brush`) takes up a `Style` in `App::brush`: the selected
text box's, else the look at the start of the selection (at the caret, the typing style), with
the paragraph's `ParaAttrs` in `para`. Letting go of the next mouse selection puts it down
(`paint_format`, from `mouse`), as does a click: on the word under it, or on a text box (its look
and alignment, `textbox.rs`). Then the brush is put down; Escape or the button again do so too.

- **The look goes through `Style::take_look`**: everything but `para` and `image`. A new field of
  `Style` that is character formatting is copied without more work; one that is not (like
  `image`) must be kept there.
- **Paragraph format goes only onto paragraphs selected whole** (first char to terminator, R2),
  all of `ParaAttrs`, chapter title too. A word painted inside a title would otherwise make its
  paragraph a chapter.
- It is one `Edit::Restyle` through `Doc::apply` (one undo step, R1) and `paginate_after` from
  the first paragraph touched, like `set_para`. No Word mapping of its own (R10): it copies
  formatting that has one.
- Taking up the selection with a key and pasting it (Word's Ctrl+Shift+C/V) is not offered:
  egui-winit turns any Ctrl+C/V into `Copy`/`Paste` events without the key, and sends no paste at
  all when the clipboard holds no text.
- Tests: `formatting_copied_from_one_place_goes_onto_the_text_selected_next_and_whole_paragraphs_take_its_alignment`,
  `formatting_copied_from_the_story_goes_onto_a_text_box_clicked`.

### R26 — Page numbering changes are pinned to the text and hold until the next one

Every page counts, from 1 on the first (contents included). A change (`numbers::Numbering`:
count on, start again at a number, or no numbers; figures or small roman) holds from the page it
is set on until the next change. "No numbers" hides them but the count goes on underneath, as in
Word. Set in Page > Page numbers for the page shown, which also lists every change.

- **A change is pinned to the first char of the page it was set on** (`NumberMark`, in
  `Doc::number_marks`, kept in the story's order) and holds from the page that char is on
  (`page_of`), so an appendix without numbers stays so wherever its text goes. It moves with
  edits like a text box (R7). The contents pages hold no text: theirs is
  `PageSetup::contents_numbers`, which holds from the first contents page; the story goes on from it
  unless a change is pinned at its start.
- **Everything that shows a page's number asks `Doc::page_numbers` / `page_number`**: bands
  (`bands_ink`, R24, and the Facing look's sides, by the count), the contents' entries
  (`Entry::number`, shown even for a page whose own number is hidden, as Word's `TOC` does), and
  Word (R10). The page bar and the Print window count pages as they lie (1 to n), not by these
  numbers. Something new showing a page number uses `page_number`, not `i + 1`.
- **Setting a change that changes nothing keeps nothing** (`set_numbering` compares the numbers
  with and without it), and a page has at most one; a page without one shows the numbering in
  force there (`numbering_on`). Not undoable, like page settings (R1).
- Numbers never change where pages break (R6): the contents keep room for four figures, and a
  wider roman number runs into the dots.
- Tests: `page_numbers_start_again_after_the_contents_and_leave_out_the_appendix_wherever_its_text_goes`,
  and in `numbers.rs` `numbering_holds_from_the_page_it_is_set_on_until_the_next_change`,
  `roman_numerals_are_written_small`. Check Word with `sample_docx` (its contents are roman, its
  story from 1, its last chapter without).

### R27 — Opened without a file, Caprice welcomes the writer; opening and saving remember the story

`main()` puts the welcome page up (`Start::open`) only when no file was named on the command line;
`App::new` and so `Harness` leave it down. While it is up, `frame` draws it **in place of the desk**
and returns: no toolbar, no page, no keys reach the story (only Ctrl+O, as its "Open another…").
It begins a new story of a `Kind` (`App::begin`), shows the file dialog, or opens a story last
opened through `open_path`. Escape begins a blank page, as Caprice used to start. Giving up on the
file dialog, or a story that cannot be read, leaves the page up.

- **Leaving it is a flight, not a cut** (`Start::flight`, `welcome_leaving`, drawn last in `frame`
  on a foreground layer): the desk is drawn as usual underneath, the welcome fades from over it,
  and the sheet chosen moves from its place on the page to `App::last_page_rect`, fading out over
  the page itself at the end. The story takes typing from the first frame of it. Anything that
  takes the welcome away calls `Start::leave` (or sets a flight), so it never just vanishes.
- **A `Kind` is the starting state of a story, not a setting**: its paper, margins, headers and
  footers, drop caps (`Kind::setup`), the look of its text (`Kind::body`: font from the first of
  its fonts offered, size, alignment, spacing) and its first chapter title (`Kind::title`). `begin`
  builds the flow from them and goes through `adopt`, like opening a file, then starts a new story
  id (R22), sets `saved_hash` so nothing counts as unsaved until written, and puts the caret in the
  paragraph after the title. A new kind: add it to `Kind::ALL` and each match; its sheet comes from
  its setup. Nothing about a kind is stored: the story just has those settings (R9).
- **The emblem beside the name** (`emblem.rs`) draws from when the page first showed
  (`Start::shown_at`): a pencil draws the icon's loop, it rests, fades and is drawn again
  (`ROUND`). While the page shows it asks for every frame; it stops with the page. It reaches
  `HEADROOM` above its tile, which the header leaves room for. Tests: in `emblem.rs`
  `the_pencil_draws_the_loop_then_it_rests_fades_and_starts_again`, and
  `the_welcome_shows_the_emblem_beside_the_name_its_pencil_drawing`.
- **The sheets are drawn from the page setup** (`Sheet::of`: shape, margins, title, drop cap,
  justified, spacing, header and footer looks), for the kinds and for stories last opened alike,
  so a new page setting that changes how a page looks can be shown there too.
- **`open_path` and `save_to` call `Start::remember`**, and `open_path` calls `Start::leave`. A new
  way to open or save a story goes through them, or it is missing from the list. Paths are kept
  canonical, so a story named on the command line is the same entry as from the dialog. Autosave
  saves through `save_to` every half minute: `remember` writes the list only when the story is not
  already on top, so it does not write every time.
- The list (`recent.json` in the data folder, beside `progress.json`) holds paths and when each
  was last opened (`Keeper::now`, so tests set the clock), newest first, at most `REMEMBER`. What
  the page tells about each (words, chapters, post-its, paper, first words, when changed) is read
  from the files once, when it first shows (`Start::stories`), never each frame. A file moved or
  deleted since is left out of what is shown, not out of the list.
- **Tests never write it behind your back**: `Harness::new` sets no file and an empty list; the
  tests about it use `Start::kept_in` with a folder of their own. What the page paints is read
  with `Harness::painted_texts` / `painted_at`; the kinds and stories are buttons for `click_widget`.
- Tests: `opened_without_a_file_caprice_welcomes_with_new_stories_and_the_ones_last_opened`,
  `the_sheet_chosen_flies_to_the_desk_while_the_welcome_fades`,
  `a_new_novel_begins_on_book_pages_at_chapter_one_and_a_journal_at_today`,
  `escape_on_the_welcome_page_begins_a_blank_story`.

### R28 — The cast peeks out: a puppet in parts, behind the page, only while the writer rests

Someone of the cast with a peek drawn (`Look::peek`) comes out from behind the side of the page
now and then, holds its edge, looks at the line that names them, blinks, glances at the writer and
ducks away (`peek.rs`). Claude draws the peek once per look, from its model sheet (`puppet_prompt`,
through the same job as model sheets, `SheetJob::peek`).

- **A puppet is its parts** (`PARTS`): groups with those ids in the drawing, rendered one by one
  (`cut`), each a texture placed where it lies in the drawing. Only a drawing with all of them
  (the body may be missing) is kept: `draw_sheet` cuts it before taking it. A new part needs its id
  in the prompt and in `PARTS`.
- **Behind the page, then the hand in front**: `draw_peek` is called in the settled page's branch
  of `frame`, before `paper` (body, head, irises, lids: the page hides what is behind its edge) and
  after the post-its (the hand, its fingers over the edge). It is drawn only on a settled page; a
  page turned ends it. Nothing of it is printed or exported: it is not on the page.
- **Its course is a function of time** (`pose`): rising with an overshoot, looking at the line,
  blinks at set times, a glance at the writer, ducking. Typing, clicking or scrolling makes it duck
  at once (`Peek::duck`). It is placed from the page's rect each frame (`SIZE` of its height), so
  zoom and window size need nothing more. It is worked out as if on the right side and mirrored
  about the page's middle on the left, so **it leans out, away from the page, on either side**: the
  whole figure turns on its hips (`LEAN`), its top outwards, and the head tilts only a little back
  towards the line, on its neck (it once leaned in, towards the page).
- **Who, when, where** (`update_peeks`): every `EVERY` seconds at random (the first after
  `FIRST`), once the writer has left keys and mouse alone for `QUIET`; someone named on the page,
  else in its chapter, else anyone with a peek; looking at where their name first stands on the
  page, else the caret, from the side nearer it. "Peek now" in the Cast window asks for someone at
  once. While a peek is due it asks for a frame then (`request_repaint_after`), and every frame while
  one is under way. "Cast peeks out" in the Scene menu turns it off (`Doc::peeks`, saved).
- Tests: in `peek.rs` `a_puppet_is_cut_into_its_parts_each_where_it_lies`,
  `a_peek_rises_looks_blinks_glances_and_ducks`; and
  `someone_named_on_the_page_peeks_out_looks_at_their_name_blinks_and_ducks_away`,
  `a_peek_leans_out_away_from_the_page_on_either_side`,
  `a_peek_blinks_and_ducks_at_once_when_the_writer_types`,
  `the_cast_peeks_every_minute_or_two_while_the_writer_rests_and_not_when_turned_off`,
  `only_someone_named_on_the_page_peeks_out_when_the_page_names_anyone`. The prompt with the real
  Claude: `live_peek` (ignored).

---

## 5. Checklists for common changes

**A new paragraph attribute** (like line spacing or chapter title): `ParaAttrs` → `run_format` /
`paragraph_job` → the toolbar (`ui.rs`) → file format default (R9) → Word mapping (R10) → whether
it changes drop caps (`has_drop_cap`) or the contents (`chapters`). The brush copies it with the
rest of `ParaAttrs` (R25).

**A new kind of character formatting** (like colour): a `Style` field (in `same_char`) →
`run_format` → the toolbar and `format_box` → Word's `run_props` (R10). It prints (R20) and the
brush copies it (R25) without more work.

**A new page setting:** `PageSetup` field + default → Page menu (`ui.rs`, applied through
`set_setup`) → pagination/layout → Word section or settings (R10) → round-trip test.

**A new look for headers, footers or text boxes:** a `BandLook`/`BoxLook` variant (in `ALL`, so
the gallery shows it) → its `Ink` in `band_ink`/`box_ink`, convex fills only → its Word mapping in
`band_xml`/`text_box_xml` (R10) → check with `sample_docx` and `sample_pdf`.

**Something new drawn on a page:** size at page size (R4) → pagination (`next_span`) and layout
(`layout_page`) agree → incremental equals full (R3) → both looks (`render.rs` and `book.rs`) →
hit-testing and caret (`PageLayout`) → into `PageLayout::ink`, so it prints (R20). Lying on the
page rather than in the text: into `App::underlay` or `App::furniture`, which layer decided (R24).

**Page navigation or a new way to move between pages:** use `turn_to` / `go_to_page` (R5); test it
on the contents pages with `Harness::long_contents`.

**The file format:** `serde(default)`, a migration in `into_doc` that moves notes and scenes, an old-file test.

**A new tool in the toolbar:** into a group in `tool_group` (`ui.rs`), what it does recorded in
`Tools` and done in `apply_tools` after all the tools are drawn, since a group can be drawn in
either row. The order of `GROUPS` is the order they go into "⋯", the last first.

**A new tool in a dock:** in the toolbar, inside a group in `tool_group`, nothing in it depending on
where the group is (R: the toolbar's groups); in the page bar, room in the sums in `scrollbar` and a
place in the order things go. `at_any_width_the_docks_settle_and_hold_their_tools_apart` then checks
it at every width.

**A new window over the pages:** a toggle in the page bar or a menu, not the toolbar (R16); its
scrolling must not move the pages (R16); its stored text in `Doc` and `DocFile` (R9).

**Something new sent with a scene** (like the cast): build it in `scene_prompt` from the subject
only (R13, R17), check it in `Backdrop::asked` in a test.

**A new kind of story on the welcome page:** a `Kind` variant in `ALL` and each match (`setup`,
`body`, `fonts`, `title`, `about`); a test that begins one and checks its pages and first text (R27).

**Something new drawn on the desk around the page** (like a peek): draw it in the settled
branch of `frame`, behind the paper or in front of it as it should hide or not; end it when the
page turns; leave it out of print (R20 is for what is on the page) (R28).

**Something new pinned to the text** (like notes and scenes): move it in `apply_raw` and in
`into_doc` (R7), save it (R9), decide its Word mapping (R10).

---

## 6. Tests

- `cargo test` runs everything headlessly (about 235 tests, ~13 s). They drive the real `App`
  through an egui `Context`: `layout::with_ctx` for model/layout tests, `Harness` (in `main.rs`)
  for app-level ones (`frames`, `key`, `type_text`, `click_in_page`, `scrub_to`, `long_contents`,
  `shown`, and `click_widget`, which finds a button by its label or a field by its hint through
  egui's AccessKit tree; `painted` holds the last frame's shapes, to check what is drawn and
  where it is clipped).
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
- **2026-10-07 — Scenes are part of the story, not one picture.** A new scene used to replace the
  one before, and it was saved beside the document. Now every scene is kept, pinned to the
  paragraph it was painted for like a note, and covers the story up to the next; the page
  shown shows the caret's (R14). Pinned to a char,
  not a page (pages reflow) or a chapter (too coarse, and not every story has them).
- **2026-10-07 — Turning pages leaves the caret.** Swipes, the scrollbar and Page Up/Down used to
  put the caret at the turned-to page's start. With scenes following the caret (R14), that changed
  the picture on every page turn, and it lost the writer's place. Now only a click, typing or
  moving the caret moves it (R5).
- **2026-10-08 — Brainstorming with Claude lives in the answer panel and on post-its.** Connecting
  scenes, replies and drafts are one conversation in the panel, resent in full each time rather
  than resumed (`--resume` would need Claude Code to keep sessions on disk). What is worth keeping
  is pinned as a post-it on the passage, so ideas stay with the story without a store of their
  own. What Claude should always know is the story notes, saved with the document (R13). Claude
  sees only the passage marked, never more of the document: the writer decides what it reads.
- **2026-10-09 — The same people in every scene, from SVG model sheets.** Looked at storing
  people as image embeddings: they only help find things, and an image model that keeps a face
  needs the reference picture itself (or a model trained on it), which Claude, painting in SVG,
  cannot use. Tried instead, on four passages that only name one woman: from the passage alone
  she was someone else in every picture; with her look in words her clothes held but her build
  and drawing changed; with a model sheet Claude copied its shapes and colours and changed only
  the pose. So the cast holds a look in words and a sheet in SVG, found by name (R17).
- **2026-10-09 — Someone's looks are switched by hand, not by place in the story.** A look could
  instead start at a paragraph, like a scene, so repainting chapter 1 later still finds the girl.
  Switching by hand came first as what the writer asked for, and is simpler; pinning a look to
  where it starts can come on top (a look pinned is in use from there on).
- **2026-10-14 — Review and What next read more than a marked passage.** A writer asking for a
  chapter's mistakes or for where the story could go next wants Claude to read the chapter, or the
  story so far; making them select it first would be busywork. The writer still decides: the
  button says what it sends (R13, R19). The scratchpad goes with the ideas only while ticked, as it
  is notes to oneself.
- **2026-10-15 — Printing makes its own PDF and hands it to CUPS.** The system print dialogs
  (GTK's, macOS's `NSPrintOperation`) would each need their own bindings and lay the pages out
  again by their rules. Drawing the PDF from Caprice's own page layouts prints exactly the pages
  shown, the same PDF serves "Save as PDF", and `lp` is there on both macOS and Linux. The cost: the
  window offers only what Caprice asks for (printer, pages, copies, both sides), and adding a
  printer is done in the system's settings (R20).
- **2026-10-16 — A missing font is drawn with its twin, under its own name.** Writing on a Mac
  and on Linux, Times New Roman was missing on one and Liberation Serif on the other, and each
  fell back to egui's font, so pages broke differently. Renaming the font in the document would
  have lost the writer's choice in Word; drawing it with a font of the same widths keeps both (R21).
- **2026-10-17 — Autosave, and versions outside the story's folder.** Work was lost to a crash
  or a wrong "Don't save". Now a story with a file saves itself, and versions go to the data
  folder by story id rather than beside the file: no clutter in the writer's folders, versions
  follow a renamed file, and a story never saved has them too. Leaving still asks, but whether
  to keep or drop the changes since saving by hand, as the file holds them already (R22).
- **2026-10-19 — Text boxes lie in front of the text, pinned to a char.** Word offers text that
  wraps around a box; egui cannot flow text around anything (R11), and making pagination aware of
  boxes would tie page breaks to where a box was dropped. So a box lies in front of the text
  (Word's "In front of text", which is how it is exported), and it is pinned to a char like a
  scene, not to a page, so it goes along when the text moves to another page. Headers and footers
  likewise take no room: they sit in the margins (R24). "Page numbers" became a footer.
- **2026-10-20 — Text boxes in front of the text or behind it; pictures stay in the text.** Asked
  for: a box's text before or behind a picture. Pictures are paragraphs of the story, so making
  them float would bring back what the box decision avoided (R11). Instead a box is in one of two
  layers, behind or in front of the story with its pictures, as Word's "Behind text" and "In front
  of text", and ordered among boxes by its place in the list. Transparent lets the text under a
  box in front show through (R24).
- **2026-10-21 — Page numbering is pinned to the text, not to page numbers.** Asked for: numbers
  starting again, and pages without them (the contents, an appendix). Ranges of pages ("none from
  page 120") would hide the wrong pages as soon as text before them grew or shrank, so a change
  is pinned to a char like a text box, and holds until the next one, as Word's sections do. Word
  gets sections; "no numbers" keeps counting, as Word does (R26).
- **2026-10-22 — Opened without a file, Caprice welcomes the writer.** It used to show an empty
  story at once, and going back to yesterday's story meant the file dialog every time. A small
  modal over the empty desk came first; asked for instead was a page of its own, telling more
  about each story, with ways to begin that set things up differently. So the welcome stands in
  for the desk, a new story is begun as a kind (its settings, nothing stored about the kind), and
  the sheet chosen flies to the desk so the welcome visibly turns into the page (R27).
- **2026-10-22 — The cast peeks out as puppets in parts.** Asked for: someone of the story
  looking out from behind the page now and then, as a proper animation. A model sheet is a still
  figure; cut from it, a person could only slide and tilt. So Claude draws each look once more,
  peeking, in named parts, and Caprice moves the parts: the eyes look about and blink, the head
  tilts, a hand holds the edge. It costs one more drawing per look, asked for in the Cast window,
  never by itself (R12, R28).
- **No `.docx` import.** Export only.
- **Contents heading is "Contents"** (English), in the app and in Word.
