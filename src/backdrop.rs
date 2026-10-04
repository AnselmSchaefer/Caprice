//! Scenes behind the page: Claude paints a scene, and the picture fades in faintly behind the text
//! of every page, replacing the one before. The scene is either one the writer describes and asks
//! for, or follows the writing: each finished sentence brings a picture of the newest sentence.
//! The pictures are SVG drawn through the Claude Code CLI and rendered here; they are not part of
//! the document.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Color32, ColorImage, Id, Key, Modifiers, Pos2, Rect, Shape, TextureHandle, TextureOptions, epaint::Mesh, pos2};

use crate::App;
use crate::claude::{Outcome, run_claude};
use crate::model::{IMAGE_CHAR, PAGE_BREAK};
use crate::theme::TEXT_DIM;

/// How strongly a scene shows through the paper.
pub const OPACITY: f32 = 0.2;
/// How many of the last finished sentences a scene following the writing is drawn from.
const SENTENCES: usize = 3;
/// Seconds to wait after a sentence is finished before drawing, in case another follows.
const PAUSE: f64 = 1.5;
/// Seconds one scene takes to fade into the next.
const FADE: f64 = 2.5;
/// Width in pixels scenes are rendered at.
const RENDER_WIDTH: f32 = 1000.0;

/// The model that paints the scenes.
const MODEL: &str = "claude-opus-5-5";

const SYSTEM: &str = "You are a skilled illustrator who paints with SVG code. You make detailed, \
    atmospheric illustrations for the pages of a story, in the manner of a classic book \
    illustration: believable light, depth and texture rather than cartoon shapes.";

/// A scene on screen, and when it began to fade in.
struct Shown {
    texture: TextureHandle,
    since: f64,
    /// The drawing itself, kept to save it.
    svg: Arc<str>,
}

/// A painted scene: its drawing, and the drawing rendered.
type Painted = (String, ColorImage);

struct Job {
    rx: Receiver<Result<Painted, String>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Where the scene comes from.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The writer's description, drawn when they ask.
    #[default]
    Described,
    /// The newest finished sentence, drawn after each one.
    Writing,
}

/// What a picture is to show.
enum Subject {
    Description(String),
    /// The last few finished sentences, one per line, the newest last.
    Sentences(String),
}

pub struct Backdrop {
    /// The dialog to describe the scene is open.
    pub panel: bool,
    pub mode: Mode,
    /// The picture shows behind the pages.
    pub visible: bool,
    /// What the writer wants to see.
    pub description: String,
    current: Option<Shown>,
    previous: Option<Shown>,
    job: Option<Job>,
    /// Put the keyboard on the description when the dialog opens.
    focus_field: bool,
    /// Following the writing: the sentences drawn (or being drawn) last, the newest ones waiting
    /// and when they were seen, and the document version they were read at (only edits count,
    /// not moving the caret).
    drawn_for: String,
    pending: Option<(String, f64)>,
    read_at: Option<u64>,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            panel: false,
            mode: Mode::Described,
            visible: true,
            description: String::new(),
            current: None,
            previous: None,
            job: None,
            focus_field: false,
            drawn_for: String::new(),
            pending: None,
            read_at: None,
        }
    }
}

/// The last `n` finished sentences of `text`, one per line, the newest last. A sentence is finished
/// by `.`, `!`, `?` or `…` followed by a space or line break, or by the end of its paragraph.
pub fn last_sentences(text: &str, n: usize) -> String {
    let chars: Vec<char> = text.chars().filter(|&c| c != IMAGE_CHAR).map(|c| if c == PAGE_BREAK { '\n' } else { c }).collect();
    let mut done: Vec<String> = Vec::new();
    let mut current = String::new();
    for (k, &c) in chars.iter().enumerate() {
        if c == '\n' {
            if !current.trim().is_empty() {
                done.push(current.trim().to_owned());
            }
            current.clear();
            continue;
        }
        current.push(c);
        let next_is_space = chars.get(k + 1).is_some_and(|n| n.is_whitespace());
        if matches!(c, '.' | '!' | '?' | '…') && next_is_space && !current.trim().is_empty() {
            done.push(current.trim().to_owned());
            current.clear();
        }
    }
    let from = done.len().saturating_sub(n);
    done[from..].join("\n")
}

/// The SVG in Claude's reply (it may wrap it in a code fence).
fn extract_svg(reply: &str) -> Option<&str> {
    let start = reply.find("<svg")?;
    let end = reply.rfind("</svg>")? + "</svg>".len();
    (end > start).then(|| &reply[start..end])
}

/// Render an SVG to an image `RENDER_WIDTH` pixels wide.
fn rasterize(svg: &str) -> Result<ColorImage, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the picture could not be read: {e}"))?;
    let size = tree.size();
    let scale = RENDER_WIDTH / size.width();
    let (w, h) = (RENDER_WIDTH.round() as u32, (size.height() * scale).round().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("the picture has no size")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

/// Ask Claude for a picture of `subject` on a page `size` points large, and render it.
fn draw_scene(subject: &Subject, size: egui::Vec2, cancel: &AtomicBool) -> Result<Option<Painted>, String> {
    let (w, h) = (size.x.round(), size.y.round());
    let what = match subject {
        Subject::Description(d) => format!("<scene>\n{d}\n</scene>\n\nPaint one illustration of this scene"),
        Subject::Sentences(sentences) => {
            let (earlier, latest) = sentences.rsplit_once('\n').unwrap_or(("", sentences));
            let context = if earlier.is_empty() {
                String::new()
            } else {
                format!(
                    "<earlier_sentences>\n{earlier}\n</earlier_sentences>\n\n\
                    The earlier sentences come just before it. Use them only where they describe the \
                    same scene (the same place and moment), to add details to it. If they describe \
                    another place or moment, ignore them completely.\n\n"
                )
            };
            format!(
                "<latest_sentence>\n{latest}\n</latest_sentence>\n\n{context}\
                Paint one illustration of the scene the latest sentence of this story describes"
            )
        }
    };
    let prompt = format!(
        "{what}, to be shown faintly behind the text of a page.\n\
        - One standalone SVG with viewBox=\"0 0 {w} {h}\", the scene filling the whole area, sky included.\n\
        - Realistic and detailed: correct proportions and perspective; several planes of depth with \
          atmospheric perspective (far things paler and bluer); one clear light direction with \
          highlights and cast shadows; varied, natural shapes (no identical copies); fine detail \
          where the eye goes, such as textured rock, foliage clusters, roof tiles, ripples.\n\
        - Painterly technique: layered linear and radial gradients, soft edges with feGaussianBlur, \
          natural textures with feTurbulence, fine ink or pencil detail on top.\n\
        - A harmonious, natural palette with enough contrast to read when shown faintly.\n\
        - No text, letters or numbers. No scripts, links, external references or embedded images.\n\
        - At most about 30 KB.\n\
        Reply with only the SVG code."
    );
    let mut reply = String::new();
    match run_claude(MODEL, SYSTEM, &prompt, "medium", cancel, &mut |t| reply.push_str(t))? {
        Outcome::Cancelled => Ok(None),
        Outcome::Refused => Err("Claude declined to draw this scene".into()),
        Outcome::Finished => {
            let svg = extract_svg(&reply).ok_or("Claude's reply held no picture")?;
            rasterize(svg).map(|image| Some((svg.to_owned(), image)))
        }
    }
}

/// `image` stretched over `rect` through `map`, in strips so it bends with a turning page.
fn strip_mesh(texture: &TextureHandle, rect: Rect, tint: Color32, map: &dyn Fn(Pos2) -> Pos2) -> Mesh {
    const STRIPS: usize = 12;
    let mut mesh = Mesh::with_texture(texture.id());
    for k in 0..=STRIPS {
        let f = k as f32 / STRIPS as f32;
        let x = rect.left() + rect.width() * f;
        mesh.vertices.push(egui::epaint::Vertex { pos: map(pos2(x, rect.top())), uv: pos2(f, 0.0), color: tint });
        mesh.vertices.push(egui::epaint::Vertex { pos: map(pos2(x, rect.bottom())), uv: pos2(f, 1.0), color: tint });
        if k > 0 {
            let b = (k * 2) as u32;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    mesh
}

/// The scene as kept in a document file.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SceneFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

impl Backdrop {
    pub fn drawing(&self) -> bool {
        self.job.is_some()
    }

    /// What is kept in the document file, if there is anything.
    pub fn to_file(&self) -> Option<SceneFile> {
        let svg = self.current.as_ref().map(|s| s.svg.to_string());
        (svg.is_some() || !self.description.trim().is_empty()).then(|| SceneFile { svg, description: self.description.clone(), hidden: !self.visible })
    }

    /// The scene of a document just opened (or none): it replaces whatever was shown or painted.
    fn load(&mut self, ctx: &egui::Context, file: Option<SceneFile>) -> Result<(), String> {
        let file = file.unwrap_or_default();
        *self = Backdrop { mode: self.mode, description: file.description, visible: !file.hidden, ..Backdrop::default() };
        let Some(svg) = file.svg else { return Ok(()) };
        let image = rasterize(&svg)?;
        let texture = ctx.load_texture("scene", image, TextureOptions::LINEAR);
        self.current = Some(Shown { texture, since: ctx.input(|i| i.time), svg: svg.into() });
        Ok(())
    }

    /// Following the writing: the sentences waiting to be drawn.
    #[cfg(test)]
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_ref().map(|(p, _)| p.as_str())
    }
}

impl App {
    /// Start painting the described scene (replacing a drawing still in progress).
    pub fn draw_backdrop(&mut self, ctx: &egui::Context) {
        let description = self.backdrop.description.trim().to_owned();
        if !description.is_empty() {
            self.start_scene(ctx, Subject::Description(description));
        }
    }

    fn start_scene(&mut self, ctx: &egui::Context, subject: Subject) {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (ctx2, cancel2, size) = (ctx.clone(), cancel.clone(), self.doc.setup.size());
        std::thread::spawn(move || {
            if let Some(result) = draw_scene(&subject, size, &cancel2).transpose() {
                let _ = tx.send(result);
            }
            ctx2.request_repaint();
        });
        let b = &mut self.backdrop;
        b.job = Some(Job { rx, cancel });
    }

    /// The sentences written last, before the caret.
    fn sentences_now(&self) -> String {
        let upto = self.doc.char_to_byte(self.caret);
        last_sentences(&self.doc.flow.text[..upto], SENTENCES)
    }

    /// Show the scene of a document just opened. Following the writing, a saved picture counts as
    /// the picture of what is written, so only new sentences bring a new one.
    pub fn open_scene(&mut self, ctx: &egui::Context, file: Option<SceneFile>) -> Result<(), String> {
        let shown = self.backdrop.load(ctx, file);
        if self.backdrop.current.is_some() {
            self.backdrop.drawn_for = self.sentences_now();
            self.backdrop.read_at = Some(self.doc.version);
        }
        shown
    }

    /// Following the writing: notice finished sentences and draw the newest after a pause.
    fn follow_writing(&mut self, ctx: &egui::Context, now: f64) {
        if self.backdrop.read_at != Some(self.doc.version) {
            self.backdrop.read_at = Some(self.doc.version);
            let latest = self.sentences_now();
            let b = &mut self.backdrop;
            if latest.is_empty() || latest == b.drawn_for {
                b.pending = None;
            } else if b.pending.as_ref().is_none_or(|(p, _)| *p != latest) {
                b.pending = Some((latest, now));
            }
        }
        let b = &mut self.backdrop;
        let (Some((sentences, seen)), true) = (b.pending.clone(), b.job.is_none()) else { return };
        let wait = seen + PAUSE - now;
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        } else {
            b.pending = None;
            b.drawn_for = sentences.clone();
            self.start_scene(ctx, Subject::Sentences(sentences));
        }
    }

    /// Set where the scene comes from.
    pub fn set_scene_mode(&mut self, mode: Mode) {
        let b = &mut self.backdrop;
        if b.mode == mode {
            return;
        }
        b.mode = mode;
        b.job = None;
        b.pending = None;
        // Following the writing starts with a picture of what is already written.
        b.drawn_for.clear();
        b.read_at = None;
    }

    /// Take in a finished picture and keep a fade going.
    pub fn update_backdrop(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.backdrop.mode == Mode::Writing {
            self.follow_writing(ctx, now);
        }
        let b = &mut self.backdrop;
        if let Some(job) = &b.job {
            match job.rx.try_recv() {
                Ok(Ok((svg, image))) => {
                    let texture = ctx.load_texture("scene", image, TextureOptions::LINEAR);
                    b.previous = b.current.take();
                    b.current = Some(Shown { texture, since: now, svg: svg.into() });
                    b.visible = true;
                    b.job = None;
                }
                Ok(Err(e)) => {
                    self.status = format!("- no picture: {e}");
                    b.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => b.job = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if b.current.as_ref().is_some_and(|s| now - s.since < FADE) {
            ctx.request_repaint();
        } else {
            b.previous = None;
        }
    }

    /// The toolbar's Scene menu: follow the writing, describe the scene, show or hide the picture.
    pub fn scene_menu(&mut self, ui: &mut egui::Ui) {
        let b = &mut self.backdrop;
        let title = if b.job.is_some() { "Scene \u{b7} painting\u{2026}" } else { "Scene" };
        let (mut mode, mut describe, mut stop, mut save) = (b.mode, false, false, false);
        let tip = "Pictures Claude paints faintly behind the page";
        egui::containers::menu::MenuButton::new(title).ui(ui, |ui| {
            ui.set_min_width(230.0);
            let mut follow = mode == Mode::Writing;
            let follow_tip = "Each time you finish a sentence, Claude paints what the newest one describes";
            if ui.checkbox(&mut follow, "Follow my writing").on_hover_text(follow_tip).changed() {
                mode = if follow { Mode::Writing } else { Mode::Described };
            }
            if ui.add(egui::Button::new("Describe the scene\u{2026}").frame(false)).clicked() {
                describe = true;
                ui.close();
            }
            ui.separator();
            ui.add_enabled(b.current.is_some(), egui::Checkbox::new(&mut b.visible, "Show picture"));
            if ui.add_enabled(b.current.is_some(), egui::Button::new("Save picture as\u{2026}").frame(false)).clicked() {
                save = true;
                ui.close();
            }
            if b.job.is_some() && ui.add(egui::Button::new("Stop painting").frame(false)).clicked() {
                stop = true;
            }
        })
        .0
        .on_hover_text(tip);
        if stop {
            b.job = None;
        }
        if describe {
            b.panel = true;
            b.focus_field = true;
        }
        self.set_scene_mode(mode);
        if save {
            self.save_scene_picture(&ui.ctx().clone());
        }
    }

    /// Write the picture shown to a PNG (or, given a .svg name, an SVG) file the user picks.
    fn save_scene_picture(&mut self, ctx: &egui::Context) {
        if self.backdrop.current.is_none() {
            return;
        }
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map_or("Scene".to_owned(), |s| format!("{} scene", s.to_string_lossy()));
        self.ask_path(ctx, crate::fileio::DialogFor::ScenePicture, move || {
            rfd::FileDialog::new()
                .add_filter("PNG picture", &["png"])
                .add_filter("SVG drawing", &["svg"])
                .set_file_name(format!("{stem}.png"))
                .save_file()
        });
    }

    pub fn save_scene_picture_to(&mut self, path: std::path::PathBuf) {
        let Some(svg) = self.backdrop.current.as_ref().map(|s| s.svg.clone()) else { return };
        let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let written = if is_svg {
            std::fs::write(&path, svg.as_bytes()).map_err(|e| e.to_string())
        } else {
            rasterize(&svg).and_then(|image| {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8).map_err(|e| e.to_string())
            })
        };
        let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
        self.status = match written {
            Ok(()) => format!("- picture saved as {name}"),
            Err(e) => format!("- picture not saved: {e}"),
        };
    }

    /// The dialog to describe the scene. It closes once the picture is asked for.
    pub fn scene_panel(&mut self, ctx: &egui::Context) {
        if !self.backdrop.panel {
            return;
        }
        let (mut open, mut draw) = (true, false);
        let b = &mut self.backdrop;
        egui::Window::new("Describe the scene")
            .id(Id::new("scene_panel"))
            .open(&mut open)
            .default_width(340.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("What should Claude paint behind the page?").color(TEXT_DIM));
                let field = egui::TextEdit::multiline(&mut b.description)
                    .hint_text("A rural village in a valley between the mountains and the sea, at dusk\u{2026}")
                    .desired_rows(5)
                    .desired_width(f32::INFINITY);
                let field = ui.add(field);
                if std::mem::take(&mut b.focus_field) {
                    field.request_focus();
                }
                if field.has_focus() && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter)) {
                    draw = true;
                }
                ui.add_space(4.0);
                let label = if b.current.is_some() { "Redraw" } else { "Draw" };
                let can = !b.description.trim().is_empty();
                draw |= ui.add_enabled(can, egui::Button::new(label)).on_hover_text("Have Claude paint this scene (Ctrl+Enter)").clicked();
            });
        if !open || draw {
            b.panel = false;
        }
        if draw && !b.description.trim().is_empty() {
            self.set_scene_mode(Mode::Described);
            self.draw_backdrop(ctx);
        }
    }

    /// The scene behind a page at `rect` (on screen through `map`), faded in, at `shade` and `alpha`.
    pub fn backdrop_shapes(&self, ctx: &egui::Context, rect: Rect, map: &dyn Fn(Pos2) -> Pos2, shade: f32, alpha: f32) -> Vec<Shape> {
        let b = &self.backdrop;
        let Some(current) = b.current.as_ref().filter(|_| b.visible) else { return Vec::new() };
        let t = ((ctx.input(|i| i.time) - current.since) / FADE).clamp(0.0, 1.0) as f32;
        let t = t * t * (3.0 - 2.0 * t);
        let tint = |a: f32| {
            let v = (255.0 * shade).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(v, v, v).gamma_multiply(a * alpha * OPACITY)
        };
        let mut out = Vec::new();
        if let Some(prev) = &b.previous {
            out.push(Shape::mesh(strip_mesh(&prev.texture, rect, tint(1.0 - t), map)));
        }
        out.push(Shape::mesh(strip_mesh(&current.texture, rect, tint(t), map)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_finished_sentences_count() {
        assert_eq!(last_sentences("One. Two! Three? Four", 3), "One.\nTwo!\nThree?");
        assert_eq!(last_sentences("One. Two. Three. Four. ", 2), "Three.\nFour.");
        assert_eq!(last_sentences("A heading\nThe story begins", 3), "A heading", "a paragraph's end finishes it");
        assert_eq!(last_sentences("Dr.Who is here", 3), "", "no space after the point: not an end");
        assert_eq!(last_sentences("", 3), "");
    }

    #[test]
    fn the_drawing_is_found_and_rendered() {
        let reply = "```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 60 85\"><circle cx=\"30\" cy=\"40\" r=\"20\" fill=\"#36c\"/></svg>\n```";
        let svg = extract_svg(reply).unwrap();
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        let image = rasterize(svg).unwrap();
        assert_eq!(image.size, [1000, 1417]);
        let middle = image.pixels[708 * 1000 + 500];
        assert!(middle.b() > 150 && middle.a() == 255, "the circle is drawn: {middle:?}");
        assert_eq!(image.pixels[0].a(), 0, "the background stays transparent");
        assert!(extract_svg("no drawing here").is_none());
    }

    /// Asks the real Claude for a scene through the Claude Code CLI: `cargo test live_scene -- --ignored`.
    #[test]
    #[ignore]
    fn live_scene() {
        // Two unrelated sentences: only the newest, the storm at sea, should be drawn.
        let sentences = "The village lay quiet in the valley, smoke rising from its chimneys.\nFar away, a lone ship fought its way through a storm on the open sea.";
        let (_, image) = draw_scene(&Subject::Sentences(sentences.into()), egui::vec2(595.0, 842.0), &AtomicBool::new(false)).unwrap().unwrap();
        let path = std::env::temp_dir().join("caprice-live-scene.png");
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8).unwrap();
        println!("scene saved to {}", path.display());
    }
}
