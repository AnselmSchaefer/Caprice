use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, FontFamily};

/// Fonts egui knows about right now (by UI name, plus "<name>:bold" for bold faces).
pub static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn family_for(font: &str, bold: bool) -> FontFamily {
    let loaded = LOADED.lock().unwrap();
    if loaded.iter().any(|f| f == font) {
        let key = format!("{font}:bold");
        if bold && loaded.contains(&key) {
            return FontFamily::Name(key.into());
        }
        return FontFamily::Name(font.into());
    }
    FontFamily::Proportional
}

/// Common document fonts, most popular first; the first ten that are installed get offered.
const COMMON_FONTS: [&str; 24] = [
    "Times New Roman", "Arial", "Calibri", "Helvetica", "Georgia", "Verdana", "Courier New", "Cambria", "Garamond",
    "Palatino Linotype", "Liberation Serif", "Liberation Sans", "Liberation Mono", "Noto Serif", "Noto Sans",
    "DejaVu Serif", "DejaVu Sans", "Nimbus Roman", "Nimbus Sans", "P052", "Nimbus Mono PS", "Tinos", "Arimo",
    "Cousine",
];

pub struct FontBook {
    db: fontdb::Database,
    /// (name shown in the UI and stored in files, installed family that renders it)
    pub families: Vec<(String, String)>,
    defs: egui::FontDefinitions,
    requested: BTreeSet<String>,
    pending: Vec<String>,
}

impl FontBook {
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let installed: BTreeSet<String> = db
            .faces()
            .filter(|f| f.style == fontdb::Style::Normal)
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        let mut families: Vec<(String, String)> = COMMON_FONTS
            .iter()
            .filter(|f| installed.contains(**f))
            .take(10)
            .map(|f| ((*f).to_owned(), (*f).to_owned()))
            .collect();
        if families.is_empty() {
            families.push(("Default".into(), "Default".into()));
        }
        Self {
            db,
            families,
            defs: egui::FontDefinitions::default(),
            requested: BTreeSet::new(),
            pending: Vec::new(),
        }
    }

    pub fn preferred_default(&self) -> String {
        self.families[0].0.clone()
    }

    /// Register `font` with egui. It becomes usable on the next frame.
    pub fn ensure(&mut self, ctx: &egui::Context, font: &str, immediate: bool) {
        if font == "Default" || !self.requested.insert(font.to_owned()) {
            return;
        }
        let Some((_, actual)) = self.families.iter().find(|(n, _)| n == font).cloned() else { return };
        let families = [fontdb::Family::Name(&actual)];
        let query = |bold: bool| fontdb::Query {
            families: &families,
            weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            ..Default::default()
        };
        let mut keys = Vec::new();
        for bold in [false, true] {
            let Some(id) = self.db.query(&query(bold)) else { continue };
            let Some(info) = self.db.face(id) else { continue };
            if bold && info.weight.0 < 600 {
                continue;
            }
            let Some((bytes, index)) = self.db.with_face_data(id, |d, i| (d.to_vec(), i)) else { continue };
            let key = if bold { format!("{font}:bold") } else { font.to_owned() };
            let mut data = egui::FontData::from_owned(bytes);
            data.index = index;
            self.defs.font_data.insert(key.clone(), Arc::new(data));
            let mut chain = vec![key.clone()];
            chain.extend(self.defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
            self.defs.families.insert(FontFamily::Name(key.clone().into()), chain);
            keys.push(key);
        }
        if keys.is_empty() {
            return;
        }
        ctx.set_fonts(self.defs.clone());
        if immediate {
            LOADED.lock().unwrap().extend(keys);
        } else {
            self.pending.extend(keys);
        }
    }

    pub fn activate_pending(&mut self) {
        if !self.pending.is_empty() {
            LOADED.lock().unwrap().append(&mut self.pending);
        }
    }
}
