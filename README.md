# Caprice

**A word processing app for creative, fun story telling.**

Caprice is a word processor for writing stories, not reports. It gives you real pages that flip like a book, post-it notes for your ideas, and Claude as a helper: it can paint the scene you are writing behind the page, or read along when you circle a passage.

![Caprice: a story page with a lake at dusk painted behind the text, children walking towards a little house, and post-it notes at the edge](assets/Example.png)

## Why Caprice

- **Easy to use.** One small toolbar, real pages and no clutter. Start typing, and the story fills page after page.
- **Joy in writing.** Pages turn with a flip, and the pages you have already written pile up beside the one you are on, so you can watch the story grow.
- **Room for ideas.** Stick post-its to any line for plot twists, names and "what if…" thoughts. They stay readable in the page pile, and clicking one takes you back to its page.

## Features

- **Scenes behind the page.** Describe a place ("a village between the mountains and the sea, at dusk") and Claude paints it faintly behind your writing. Or let it follow along, sketching the last sentences you wrote as you go. You can save the picture as a PNG or SVG.
- **Circle to ask Claude.** Turn on the pen (Ctrl+Shift+P) and draw a loop around a passage. Claude can review it, fix awkward wording, check grammar, summarize it, or answer your own question about it. A rewrite can replace the passage, and one undo brings the original back.
- **Post-it notes** on any selection or line (Ctrl+Alt+N), in several colours.
- **Pictures** in your text that you can move, resize and rotate.
- **Formatting** that stays out of the way: fonts, sizes, bold, underline, alignment, lists and page setup.
- **Search** (Ctrl+F), **undo** and **zoom** (Ctrl + / − / 0).
- **Your work is safe.** Caprice asks before closing or opening another file when you have unsaved changes.
- **Export to Word** (.docx) when the story is ready to share.

## Getting started

Caprice is written in Rust and runs on Linux.

```sh
cargo run --release              # try it
cargo run --release -- story.caprice   # open a story directly
./install.sh                     # install it with an icon and a launcher entry
```

### Claude features

The scene painting and the pen use [Claude Code](https://claude.com/claude-code) on your machine. Install it and sign in by running `claude` once in a terminal. Caprice stores no keys of its own; it runs your local `claude` command with tools, project settings and session history turned off. Everything else in Caprice works without Claude.

## Files

Stories are saved as `.caprice` files: plain JSON holding the text, notes, pictures and the scene behind the pages.
