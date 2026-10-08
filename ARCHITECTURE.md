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
(the CLI) can review a passage drawn around with the pen or selected with the cursor, and paint scenes that show faintly behind the pages, each
for its part of the story.

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
scenes (a char + drawings)   ──scene_at(caret) / page_scene(i)──▶ the drawing behind a page
                             export::to_docx(&Doc) ─▶ .docx (Word lays it out again itself)
```

- **`Flow`** (`model.rs`) is one `String` plus one `Style` per char (font, size, bold, underline,
  image id, paragraph attributes). Paragraph formatting sits on the paragraph's **terminator** char.
- **Pages are not stored.** `Doc::spans` is a cache: per page, a char/byte range of the flow, whether
  a hard break ends it, and, for a contents page, which part of the contents it shows.
- **Notes** are char ranges kept beside the text, not in it; **scenes** are chars likewise.
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
| `claude.rs` | Pen loop or cursor selection → passage → `claude -p` → streamed answer panel, a conversation; story notes |
| `backdrop.rs` | Scenes Claude paints (SVG), pinned to the story, faintly behind the pages; the Scene menu and list |
| `scratchpad.rs` | The scratchpad: plain text saved with the document, in a window on the right |
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
  start of a contents page is on another page. Swipes, the scrollbar and Page Up/Down use
  `turn_to`; a contents link moves the caret to its chapter's page (a story page), like a link.
- **Turning pages leaves the caret where it is**, as scrolling does in other word processors: the
  shown page (`target`) and the caret's page often differ. Only Shift+Page Down/Up (extending the
  selection) takes the caret along. Anything that acts at the caret must not assume it is on the
  page shown; what shows on the page shown without the caret (the scene, R14) comes from the page.
- **`set_caret` shows the caret's page**, so typing, arrow keys and edits bring it back.
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

- `rebase_notes` and `rebase_scenes` move them with every `Edit`. A scene pin is a point: text
  typed right at it goes after it (it still starts its paragraph), and deleted around, it stays
  where the text was. A rewrite of the text that bypasses `Doc::apply`
  (file migration) must move them too (`take_out_contents_chars` returns what it removed, so
  `into_doc` can).
- Neither changes where pages break.
- **Anything new pinned to the text** goes the same way: moved in `apply_raw`, and in `into_doc`.
- Tests: `notes_follow_edits`, `replacing_across_a_notes_edges`,
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
- Tests: `roundtrip_keeps_text_styles_breaks_and_setup`, `version_1_files_still_load_as_separate_pages`,
  `chapter_titles_are_saved_and_older_files_have_none`,
  `the_contents_setting_is_saved_opened_and_exported_through_the_app`,
  `an_older_files_one_scene_opens_pinned_to_the_start_of_the_story`,
  `story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections`,
  `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`.

### R10 — Word export maps the model, not the pixels

Word lays the document out again by its own rules, so `export.rs` maps Caprice's model onto Word's
(table in its module comment): chapter titles → Heading 1, drop caps → Word drop caps, contents →
a `TOC` field (Caprice's page numbers are only its cached result, and Word updates them on opening).

- **A new formatting feature needs its Word mapping**, or a decision that it has none, in the same
  change. Check the output with LibreOffice (`CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`,
  then `soffice --headless --convert-to pdf out.docx`). Not yet checked in Microsoft Word itself.
- Scenes are not exported (decided: they are a writing aid, too faint to print).
- Story notes and the scratchpad are not exported (decided: notes to Claude and to oneself, not
  part of the book).
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
  decides what Claude sees by what they circle or select. Added to it are only what the writer
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

### R14 — Scenes: one per paragraph, each covering the story up to the next; the caret's shows

`Doc::scenes` are kept with the story (saved, R9), each pinned to the start of the paragraph it
was painted for (`start_scene`: the selection's or caret's paragraph, or the passage's).

- **A scene covers the story from its pin up to the next scene's** (`Doc::scene_at(c)`: the last
  shown, painted scene pinned at or before `c`).
- **The caret's page follows the caret** (`App::scene_of_page`): it shows the scene of the part
  the caret is in, wherever it is drawn (shown, in a pile, turning), and moving the caret into
  another scene's part fades to that scene. Every other page shows the scene at its top
  (`Doc::page_scene(i)`). So a page looks the same while turning as once turned to, and turning
  back to the caret finds its picture as it was left. Every look (`render.rs`, `book.rs`, the
  settled page in `main.rs`) passes its page to `backdrop_shapes`.
- **Following the writing paints each finished paragraph once** (`finished_paragraph`: the last
  one with text before the caret's paragraph, if it has no scene yet), after `PAUSE`, pinned to
  it. Finished sentences alone paint nothing.
- **Painting a paragraph again adds a version** to its scene (`pin_painting` finds it by `at`)
  rather than another scene; `shown` picks one. Moving a scene onto a paragraph with one already
  (Pin here) takes the other's place.
- **A scene is made when painting starts**, with no versions, so its pin follows edits made while
  Claude paints. It is not saved (`from_doc` skips it), never shows, and is dropped once its
  painting ends without a picture (`drop_unpainted_scenes`).
- **Drawings are rendered on threads, when needed:** those of pages within `NEAR` of the page
  shown, of scenes that begin on them (for the caret to reach), and of the caret's scene wherever
  the pages are, and small ones while the list is open. Textures beyond those are let go. Turned
  to, a page shows its drawing at once; a change on the page shown (caret, painted, another
  version, hidden) fades.
- Moving, hiding, deleting and stepping versions change `Doc::scenes` directly: like page
  settings, they are not undoable (R1) and do not paginate.
- Tests: `the_picture_follows_the_caret_from_one_part_of_the_story_to_the_next`,
  `turning_back_to_the_caret_finds_its_picture_as_it_was_left`,
  `painting_a_paragraph_again_adds_a_version_and_keeps_the_others`,
  `a_scene_still_being_painted_is_not_saved_and_goes_if_stopped`,
  `following_the_writing_paints_each_paragraph_once_it_is_finished`, and those of R7 and R9.

### R15 — Post-its: one size, the text in sheets; a sheet shows its own rows and no more

A post-it is always `NOTE_SIZE` square. `sheets` breaks its text into sheets of whole rows (as
many as fit the text area above the counter), and two drawings must agree with it:

- **At rest** (`draw_notes`) the whole text is one `TextEdit`, shifted up to the sheet on top and
  clipped to that sheet's rows, not to the whole text area. The area has room for part of one
  more row, which would show cut in half and look like text that cannot be read.
- **Painted** (`note_shapes`: other pages, turning, the pad flipping) each sheet is laid out on
  its own (`sheet_galley`).
- A change to the font, padding, footer or row breaking goes through `sheets`, and both follow.
- Tests: `a_post_it_shows_whole_rows_and_the_rest_on_its_next_sheet` (checks what is painted,
  through `Harness::painted`).

### R16 — Windows float over the pages, and what happens over them is theirs

The scratchpad, Claude's answer, the story notes and the scene panels are egui windows over the
pages. They belong to the window, not to a page, so turning pages leaves them as they are. The
scratchpad's text is the document's (`Doc::scratchpad`, saved, R9); whether it shows is the app's.

- **Input over a window is the window's.** The view acts on the wheel and touchpad only over the
  pages (`ctx.layer_id_at` is a `Background` layer): `swipe_pages` for sideways swipes, and
  `page_rect` for panning up and down. Anything new the view does with the pointer checks the same.
- Keys typed into a window's field never reach the story: the editor yields while
  `egui_wants_keyboard_input()`.
- The toolbar is full at about 1100 points wide: a new toggle goes in the page bar (like Fit and
  Scratchpad) or into a menu, or the last toolbar menus run off the window.
- Tests: `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`,
  `scrolling_over_the_scratchpad_scrolls_it_and_not_the_page`.

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

**The file format:** `serde(default)`, a migration in `into_doc` that moves notes and scenes, an old-file test.

**A new window over the pages:** a toggle in the page bar or a menu, not the toolbar (R16); its
scrolling must not move the pages (R16); its stored text in `Doc` and `DocFile` (R9).

**Something new pinned to the text** (like notes and scenes): move it in `apply_raw` and in
`into_doc` (R7), save it (R9), decide its Word mapping (R10).

---

## 6. Tests

- `cargo test` runs everything headlessly (about 130 tests, ~6 s). They drive the real `App`
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
- **No `.docx` import.** Export only.
- **Contents heading is "Contents"** (English), in the app and in Word.
