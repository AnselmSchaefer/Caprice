//! The start screen: Caprice opened without a file asks what to write in, a new story, one found
//! with the file dialog, or one of the stories opened lately, rather than handing over an empty page.
//!
//! The stories opened or saved lately are a list of paths, with when each was last opened, in the
//! app's data folder beside the versions; the newest first. A file moved or deleted since is left
//! out of the list shown.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Id, RichText, vec2};
use serde::{Deserialize, Serialize};

use crate::App;
use crate::theme;

/// How many stories the list remembers.
const REMEMBER: usize = 8;

/// A story opened lately.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lately {
    pub path: PathBuf,
    /// When it was last opened (or first saved), in seconds since 1970.
    pub opened: u64,
}

/// The start screen, and the stories opened lately.
pub struct Start {
    /// The start screen shows (Caprice was opened without a file).
    pub open: bool,
    /// Where the list is kept (None: only while Caprice runs).
    pub file: Option<PathBuf>,
    /// The stories opened or saved lately, newest first.
    pub recent: Vec<Lately>,
}

impl Start {
    pub fn new() -> Self {
        Self::kept_in(crate::versions::default_dir().and_then(|d| d.parent().map(|p| p.join("recent.json"))))
    }

    /// The list kept in `file`.
    pub fn kept_in(file: Option<PathBuf>) -> Self {
        let recent = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|j| serde_json::from_str(&j).ok());
        Self { open: false, file, recent: recent.unwrap_or_default() }
    }

    /// A story was opened or saved, at `now`: it goes to the top of the list.
    pub fn remember(&mut self, path: &Path, now: u64) {
        // The full path, so a story opened from the command line is the same as from the dialog.
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        // Autosave saves often: saving the story already on top changes nothing.
        if self.recent.first().is_some_and(|l| l.path == path) {
            return;
        }
        self.recent.retain(|l| l.path != path);
        self.recent.insert(0, Lately { path, opened: now });
        self.recent.truncate(REMEMBER);
        if let (Some(file), Ok(json)) = (&self.file, serde_json::to_string(&self.recent)) {
            let _ = file.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(file, json);
        }
    }
}

/// "~/Writing" for a folder in the home folder, to read at a glance.
fn folder_name(path: &Path) -> String {
    let folder = path.parent().unwrap_or(path);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.as_ref().and_then(|h| folder.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    }
}

impl App {
    /// The start screen, while it shows: New, Open…, or a story opened lately.
    pub fn start_screen(&mut self, ctx: &egui::Context) {
        if !self.start.open || self.leaving.is_some() {
            return;
        }
        let now = self.keep.now();
        let (mut new, mut browse, mut chosen) = (false, false, None);
        let modal = egui::Modal::new(Id::new("start_screen")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("What would you like to write in?");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                new = ui.button("New story").on_hover_text("Begin on an empty page").clicked();
                let waiting = self.dialog.is_some();
                browse = ui.add_enabled(!waiting, egui::Button::new("Open\u{2026}")).on_hover_text("Find a story in your files").clicked();
            });
            let shown: Vec<&Lately> = self.start.recent.iter().filter(|l| l.path.is_file()).collect();
            if shown.is_empty() {
                return;
            }
            ui.add_space(10.0);
            ui.label(RichText::new("Last opened").color(theme::TEXT_DIM));
            for l in shown {
                let name = l.path.file_stem().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                ui.horizontal(|ui| {
                    if ui.button(&name).on_hover_text(l.path.display().to_string()).clicked() {
                        chosen = Some(l.path.clone());
                    }
                    let about = format!("{}, {}", crate::versions::ago(l.opened, now), folder_name(&l.path));
                    ui.label(RichText::new(about).color(theme::TEXT_DIM));
                });
            }
            ui.allocate_space(vec2(ui.available_width(), 0.0));
        });
        // Escape, or a click beside it, begins on the empty page as before.
        if new || modal.should_close() {
            self.start.open = false;
        } else if browse {
            // The screen stays until a story is opened, so giving up on the dialog comes back here.
            self.open(ctx);
        } else if let Some(path) = chosen {
            // A story that cannot be read says so and leaves the screen up to choose another.
            self.open_path(ctx, path);
        }
    }
}
