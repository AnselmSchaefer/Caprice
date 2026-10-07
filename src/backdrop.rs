//! Scenes behind the pages. Claude paints a scene as SVG (through the Claude Code CLI), and it is
//! kept with the story, pinned to the start of the paragraph it was painted for, like a note. It
//! covers the story up to the next scene, and the page shown shows the one the caret is in, faintly
//! behind its text, so the pictures change as the caret moves on through the story. A scene is one
//! the writer describes, a passage they select, or follows the writing: each finished sentence
//! brings a picture of the newest one. Painting again for the same paragraph adds a version to its
//! scene rather than replacing it. The list of scenes shows them all, to go to, show again, step
//! through versions, hide or delete.
//!
//! Drawings are rendered on a background thread when a page near the one shown needs them, and
//! only those near are kept as textures.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, Color32, ColorImage, Id, Key, Modifiers, Pos2, Rect, Shape, TextureHandle, TextureOptions, epaint::Mesh, pos2};

use crate::App;
use crate::claude::{Outcome, run_claude};
use crate::model::{Doc, IMAGE_CHAR, PAGE_BREAK, Scene};
use crate::theme::TEXT_DIM;

/// How strongly a scene shows through the paper.
pub const OPACITY: f32 = 0.2;
/// How many of the last finished sentences a scene following the writing is drawn from.
const SENTENCES: usize = 3;
/// Seconds to wait after a sentence is finished before drawing, in case another follows.
const PAUSE: f64 = 1.5;
/// Seconds one scene takes to fade into the next.
const FADE: f64 = 2.5;
/// Width in pixels scenes are rendered at, behind the pages and in the list of scenes.
const RENDER_WIDTH: f32 = 1000.0;
const THUMB_WIDTH: f32 = 120.0;
/// How many pages either side of the one shown get their scene rendered ahead.
const NEAR: usize = 2;
/// How many full-size drawings are kept rendered at most, besides those near the page shown.
const KEEP: usize = 6;

/// The model that paints the scenes.
const MODEL: &str = "claude-opus-5-5";

const SYSTEM: &str = "You are a skilled illustrator who paints with SVG code. You make detailed, \
    atmospheric illustrations for the pages of a story, in the manner of a classic book \
    illustration: believable light, depth and texture rather than cartoon shapes.";

/// A painted scene: its drawing, and the drawing rendered.
pub type Painted = (String, ColorImage);

/// One drawing of one scene: the scene's id and which of its versions.
pub type DrawingKey = (u64, usize);

/// A painting under way, for the scene with id `scene`.
struct Job {
    rx: Receiver<Result<Painted, String>>,
    cancel: Arc<AtomicBool>,
    scene: u64,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// A drawing rendered on a background thread, full size or small for the list.
struct Rendered {
    key: DrawingKey,
    thumb: bool,
    image: Result<ColorImage, String>,
}

/// Where new scenes come from.
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
    /// A passage the writer selected.
    Passage(String),
    /// The last few finished sentences, one per line, the newest last.
    Sentences(String),
}

impl Subject {
    fn text(&self) -> &str {
        match self {
            Subject::Description(t) | Subject::Passage(t) | Subject::Sentences(t) => t,
        }
    }
}

pub struct Backdrop {
    /// The dialog to describe the scene is open.
    pub panel: bool,
    /// The list of scenes is open.
    pub list: bool,
    pub mode: Mode,
    /// The pictures show behind the pages.
    pub visible: bool,
    /// What the writer wants to see.
    pub description: String,
    /// Drawings rendered: full size behind the pages, small in the list.
    textures: HashMap<DrawingKey, TextureHandle>,
    thumbs: HashMap<DrawingKey, TextureHandle>,
    /// Drawings being rendered, or that could not be, so they are not tried again and again.
    rendering: HashSet<(DrawingKey, bool)>,
    rendered: (Sender<Rendered>, Receiver<Rendered>),
    /// The page shown and the drawing behind it, and the drawing it fades from and since when.
    /// Turning to another page shows its drawing at once; only a change on the same page fades.
    on_target: Option<(usize, Option<DrawingKey>)>,
    fade: Option<(Option<DrawingKey>, f64)>,
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
            list: false,
            mode: Mode::Described,
            visible: true,
            description: String::new(),
            textures: HashMap::new(),
            thumbs: HashMap::new(),
            rendering: HashSet::new(),
            rendered: channel(),
            on_target: None,
            fade: None,
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

/// Render an SVG to an image `width` pixels wide.
fn rasterize(svg: &str, width: f32) -> Result<ColorImage, String> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the picture could not be read: {e}"))?;
    let size = tree.size();
    let scale = width / size.width();
    let (w, h) = (width.round() as u32, (size.height() * scale).round().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("the picture has no size")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

/// Ask Claude for a picture of `subject` on a page `size` points large, and render it.
fn draw_scene(subject: &Subject, size: egui::Vec2, cancel: &AtomicBool) -> Result<Option<Painted>, String> {
    let (w, h) = (size.x.round(), size.y.round());
    let what = match subject {
        Subject::Description(d) => format!("<scene>\n{d}\n</scene>\n\nPaint one illustration of this scene"),
        Subject::Passage(p) => format!("<passage>\n{p}\n</passage>\n\nPaint one illustration of the scene this passage of a story describes"),
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
            rasterize(svg, RENDER_WIDTH).map(|image| Some((svg.to_owned(), image)))
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

/// The scenes' settings as kept in a document file. Older files also hold their one scene's
/// drawing here; it opens as a scene pinned to the start of the story.
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

impl Doc {
    /// The scene behind char `c`: the one pinned last at or before it that has a drawing and shows.
    pub fn scene_at(&self, c: usize) -> Option<&Scene> {
        self.scenes.iter().filter(|s| s.at <= c && !s.hidden && s.svg().is_some()).max_by_key(|s| (s.at, s.id))
    }

    /// The scene behind page `i` when the caret is not on it: the one of the text at its top, as
    /// the page is when turned to. The contents pages, holding no text, show the story's first.
    pub fn page_scene(&self, i: usize) -> Option<&Scene> {
        self.scene_at(self.spans[i].start)
    }

    pub fn page_scene_key(&self, i: usize) -> Option<DrawingKey> {
        self.page_scene(i).map(|s| (s.id, s.shown))
    }

    fn scene_mut(&mut self, id: u64) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.id == id)
    }
}

impl App {
    /// The scene behind the page shown: the one of the part of the story the caret is in, so
    /// moving the caret into the next part brings its picture. On a page without the caret (the
    /// contents), the one at its top.
    pub fn scene_here(&self) -> Option<&Scene> {
        if self.doc.page_of(self.caret) == self.target {
            self.doc.scene_at(self.caret)
        } else {
            self.doc.page_scene(self.target)
        }
    }
}

impl Backdrop {
    pub fn drawing(&self) -> bool {
        self.job.is_some()
    }

    /// What is kept in the document file besides the scenes, if anything.
    pub fn to_file(&self) -> Option<SceneFile> {
        (!self.description.trim().is_empty() || !self.visible).then(|| SceneFile { svg: None, description: self.description.clone(), hidden: !self.visible })
    }

    /// Following the writing: the sentences waiting to be drawn.
    #[cfg(test)]
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_ref().map(|(p, _)| p.as_str())
    }

    /// Render the drawing `svg` of `key` on a background thread, unless it is or has been already.
    fn render(&mut self, ctx: &egui::Context, key: DrawingKey, svg: &str, thumb: bool) {
        let have = if thumb { &self.thumbs } else { &self.textures };
        if have.contains_key(&key) || !self.rendering.insert((key, thumb)) {
            return;
        }
        let (tx, ctx, svg) = (self.rendered.0.clone(), ctx.clone(), svg.to_owned());
        std::thread::spawn(move || {
            let image = rasterize(&svg, if thumb { THUMB_WIDTH } else { RENDER_WIDTH });
            let _ = tx.send(Rendered { key, thumb, image });
            ctx.request_repaint();
        });
    }
}

impl App {
    /// Start painting the described scene, for the paragraph the selection starts in.
    pub fn draw_backdrop(&mut self, ctx: &egui::Context) {
        let description = self.backdrop.description.trim().to_owned();
        if !description.is_empty() {
            self.start_scene(ctx, Subject::Description(description), self.selection().0);
        }
    }

    /// Paint the scene the passage `a..b` describes, for the paragraph it starts in.
    pub fn paint_passage(&mut self, ctx: &egui::Context, a: usize, b: usize) {
        let passage = self.selected_passage(a, b);
        if !passage.trim().is_empty() {
            self.start_scene(ctx, Subject::Passage(passage), a);
        }
    }

    fn selected_passage(&self, a: usize, b: usize) -> String {
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        self.doc.flow.text[ba..bb].chars().filter(|&c| c != IMAGE_CHAR).map(|c| if c == PAGE_BREAK { '\n' } else { c }).collect()
    }

    /// Paint `subject` for the paragraph char `c` is in (replacing a painting still in progress).
    /// A scene already pinned to that paragraph gets another version; otherwise a new scene is
    /// pinned there, which shows once its drawing is done.
    fn start_scene(&mut self, ctx: &egui::Context, subject: Subject, c: usize) {
        let (tx, cancel) = self.pin_painting(subject.text(), c);
        let (ctx, size) = (ctx.clone(), self.doc.setup.size());
        std::thread::spawn(move || {
            if let Some(result) = draw_scene(&subject, size, &cancel).transpose() {
                let _ = tx.send(result);
            }
            ctx.request_repaint();
        });
    }

    /// The scene a painting for char `c` goes to, made if need be, and the painting as the job
    /// under way: where to send the result, and the flag that stops it.
    fn pin_painting(&mut self, subject: &str, c: usize) -> (Sender<Result<Painted, String>>, Arc<AtomicBool>) {
        self.backdrop.job = None;
        self.drop_unpainted_scenes();
        let at = self.doc.para_start(c).0;
        let id = match self.doc.scenes.iter_mut().find(|s| s.at == at) {
            Some(s) => {
                s.subject = subject.to_owned();
                s.id
            }
            None => {
                let id = self.doc.next_scene_id;
                self.doc.next_scene_id += 1;
                self.doc.scenes.push(Scene { id, at, versions: Vec::new(), shown: 0, subject: subject.to_owned(), hidden: false });
                id
            }
        };
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.backdrop.job = Some(Job { rx, cancel: cancel.clone(), scene: id });
        (tx, cancel)
    }

    /// A painting of `svg` for the paragraph of char `c`, as if Claude had just finished it.
    #[cfg(test)]
    pub fn fake_painting(&mut self, c: usize, svg: &str) {
        let (tx, _) = self.pin_painting("a test", c);
        tx.send(Ok((svg.to_owned(), rasterize(svg, RENDER_WIDTH).unwrap()))).unwrap();
    }

    /// Start painting for char `c` without finishing, as if Claude were still at it.
    #[cfg(test)]
    pub fn fake_painting_under_way(&mut self, c: usize) -> Sender<Result<Painted, String>> {
        self.pin_painting("a test", c).0
    }

    /// The list of scenes has the small picture of drawing `key`.
    #[cfg(test)]
    pub fn has_thumb_of(&self, key: DrawingKey) -> bool {
        self.backdrop.thumbs.contains_key(&key)
    }

    /// The drawing shown behind the page shown, once rendered.
    #[cfg(test)]
    pub fn shown_scene(&self) -> Option<DrawingKey> {
        self.backdrop.on_target.and_then(|(page, key)| (page == self.target).then_some(key)?)
    }

    /// A scene whose first drawing was never finished (stopped, failed) is no scene.
    fn drop_unpainted_scenes(&mut self) {
        let painting = self.backdrop.job.as_ref().map(|j| j.scene);
        self.doc.scenes.retain(|s| !s.versions.is_empty() || Some(s.id) == painting);
    }

    /// The sentences written last, before the caret.
    fn sentences_now(&self) -> String {
        let upto = self.doc.char_to_byte(self.caret);
        last_sentences(&self.doc.flow.text[..upto], SENTENCES)
    }

    /// Take in the scene settings of a document just opened (its scenes came with the `Doc`). An
    /// older file's one drawing becomes a scene pinned to the story's start. Following the
    /// writing, what is already written counts as drawn, so only new sentences bring a picture.
    pub fn open_scene(&mut self, file: Option<SceneFile>) {
        let file = file.unwrap_or_default();
        self.backdrop = Backdrop { mode: self.backdrop.mode, description: file.description, visible: !file.hidden, ..Backdrop::default() };
        if let Some(svg) = file.svg.filter(|_| self.doc.scenes.is_empty()) {
            let id = self.doc.next_scene_id;
            self.doc.next_scene_id += 1;
            self.doc.scenes.push(Scene { id, at: 0, versions: vec![svg], shown: 0, subject: String::new(), hidden: false });
        }
        if !self.doc.scenes.is_empty() {
            self.backdrop.drawn_for = self.sentences_now();
            self.backdrop.read_at = Some(self.doc.version);
        }
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
            self.start_scene(ctx, Subject::Sentences(sentences), self.caret);
        }
    }

    /// Set where new scenes come from.
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
        self.drop_unpainted_scenes();
    }

    /// Take in finished paintings and renderings, keep the drawings near the page shown rendered,
    /// and fade between drawings on the page shown.
    pub fn update_backdrop(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.backdrop.mode == Mode::Writing {
            self.follow_writing(ctx, now);
        }
        self.take_painting(ctx);
        while let Ok(r) = self.backdrop.rendered.1.try_recv() {
            match r.image {
                Ok(image) => {
                    self.backdrop.rendering.remove(&(r.key, r.thumb));
                    let texture = ctx.load_texture("scene", image, TextureOptions::LINEAR);
                    if r.thumb { &mut self.backdrop.thumbs } else { &mut self.backdrop.textures }.insert(r.key, texture);
                }
                Err(e) => self.status = format!("- a scene could not be shown: {e}"),
            }
        }

        // The drawings of the pages near the one shown, and of the list, rendered ahead.
        let (lo, hi) = (self.target.min(self.pos as usize).saturating_sub(NEAR), (self.target.max(self.pos.ceil() as usize) + NEAR).min(self.last()));
        // Those at the pages' tops, and those that begin on them, for the caret to come to.
        let (from, to) = (self.doc.spans[lo].start, self.doc.spans[hi].end);
        let begin_on = self.doc.scenes.iter().filter(|s| (from..=to).contains(&s.at) && s.svg().is_some()).map(|s| (s.id, s.shown));
        let near: Vec<DrawingKey> = (lo..=hi).filter_map(|i| self.doc.page_scene_key(i)).chain(begin_on).collect();
        for &key in &near {
            if let Some(svg) = self.doc.scenes.iter().find(|s| s.id == key.0).and_then(|s| s.versions.get(key.1)).cloned() {
                self.backdrop.render(ctx, key, &svg, false);
            }
        }
        if self.backdrop.list {
            let all: Vec<(DrawingKey, String)> = self.doc.scenes.iter().filter_map(|s| Some(((s.id, s.shown), s.svg()?.to_owned()))).collect();
            for (key, svg) in all {
                self.backdrop.render(ctx, key, &svg, true);
            }
        }

        // On the page shown, a new drawing (the caret moved into another scene's part, painted,
        // another version, hidden) fades in. Turned to, a page first shows what it showed while
        // turning, the scene at its top, and fades from there to the caret's.
        let wanted = self.scene_here().map(|s| (s.id, s.shown));
        let top = self.doc.page_scene_key(self.target);
        let b = &mut self.backdrop;
        let ready = wanted.filter(|k| b.textures.contains_key(k));
        match b.on_target {
            Some((page, shown)) if page == self.target => {
                if shown != ready && (ready.is_some() || wanted.is_none()) {
                    b.fade = Some((shown, now));
                    b.on_target = Some((page, ready));
                }
            }
            _ => {
                b.on_target = Some((self.target, top.filter(|k| b.textures.contains_key(k))));
                b.fade = None;
            }
        }
        if b.fade.is_some_and(|(_, since)| now - since < FADE) {
            ctx.request_repaint();
        } else {
            b.fade = None;
        }

        // Only the drawings near the page shown, and the ones fading, stay rendered.
        if b.textures.len() > near.len() + KEEP {
            let keep: HashSet<DrawingKey> = near.into_iter().chain(b.on_target.and_then(|t| t.1)).chain(b.fade.and_then(|f| f.0)).collect();
            b.textures.retain(|k, _| keep.contains(k));
        }
    }

    /// A painting finished: it becomes its scene's newest version, shown.
    fn take_painting(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.backdrop.job else { return };
        let id = job.scene;
        match job.rx.try_recv() {
            Ok(Ok((svg, image))) => {
                self.backdrop.job = None;
                if let Some(scene) = self.doc.scene_mut(id) {
                    scene.versions.push(svg);
                    scene.shown = scene.versions.len() - 1;
                    scene.hidden = false;
                    let texture = ctx.load_texture("scene", image, TextureOptions::LINEAR);
                    self.backdrop.textures.insert((id, scene.shown), texture);
                    self.backdrop.visible = true;
                }
            }
            Ok(Err(e)) => {
                self.status = format!("- no picture: {e}");
                self.backdrop.job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.backdrop.job = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
        }
        self.drop_unpainted_scenes();
    }

    /// The toolbar's Scene menu: follow the writing, describe a scene, the list of scenes, show or
    /// hide the pictures.
    pub fn scene_menu(&mut self, ui: &mut egui::Ui) {
        let count = self.doc.scenes.iter().filter(|s| !s.versions.is_empty()).count();
        let has_picture = self.scene_here().is_some();
        let b = &mut self.backdrop;
        let title = if b.job.is_some() { "Scene \u{b7} painting\u{2026}" } else { "Scene" };
        let (mut mode, mut describe, mut stop, mut save) = (b.mode, false, false, false);
        let tip = "Pictures Claude paints faintly behind the pages, each for its part of the story";
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
            let list_tip = "Every scene painted, by where it is in the story";
            if ui.add_enabled(count > 0, egui::Button::new(format!("All scenes ({count})\u{2026}")).frame(false)).on_hover_text(list_tip).clicked() {
                b.list = true;
                ui.close();
            }
            ui.separator();
            ui.add_enabled(count > 0, egui::Checkbox::new(&mut b.visible, "Show pictures"));
            if ui.add_enabled(has_picture, egui::Button::new("Save picture as\u{2026}").frame(false)).clicked() {
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
            self.drop_unpainted_scenes();
        }
        if describe {
            let b = &mut self.backdrop;
            b.panel = true;
            b.focus_field = true;
        }
        self.set_scene_mode(mode);
        if save {
            self.save_scene_picture(&ui.ctx().clone());
        }
    }

    /// Write the picture shown now to a PNG (or, given a .svg name, an SVG) file the user picks.
    fn save_scene_picture(&mut self, ctx: &egui::Context) {
        if self.scene_here().is_none() {
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
        let Some(svg) = self.scene_here().and_then(|s| s.svg()).map(str::to_owned) else { return };
        let is_svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        let written = if is_svg {
            std::fs::write(&path, svg.as_bytes()).map_err(|e| e.to_string())
        } else {
            rasterize(&svg, RENDER_WIDTH).and_then(|image| {
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
                ui.label(egui::RichText::new("What should Claude paint behind this part of the story?").color(TEXT_DIM));
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
                let can = !b.description.trim().is_empty();
                let tip = "Have Claude paint this scene, from the caret's paragraph on (Ctrl+Enter)";
                draw |= ui.add_enabled(can, egui::Button::new("Draw")).on_hover_text(tip).clicked();
            });
        if !open || draw {
            b.panel = false;
        }
        if draw && !b.description.trim().is_empty() {
            self.set_scene_mode(Mode::Described);
            self.draw_backdrop(ctx);
        }
    }

    /// The list of scenes, in story order: each to go to, pin at the caret, step through its
    /// versions, show or hide, or delete.
    pub fn scene_list(&mut self, ctx: &egui::Context) {
        if !self.backdrop.list {
            return;
        }
        enum Do {
            GoTo(usize),
            PinHere(u64),
            Step(u64, isize),
            Hidden(u64, bool),
            Delete(u64),
        }
        let mut action = None;
        let mut open = true;
        let mut scenes: Vec<&Scene> = self.doc.scenes.iter().filter(|s| !s.versions.is_empty()).collect();
        scenes.sort_by_key(|s| (s.at, s.id));
        let here = self.scene_here().map(|s| s.id);
        egui::Window::new("Scenes")
            .id(Id::new("scene_list"))
            .open(&mut open)
            .default_width(380.0)
            .default_height(480.0)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    for s in &scenes {
                        let key = (s.id, s.shown);
                        ui.horizontal(|ui| {
                            let size = egui::vec2(THUMB_WIDTH * 0.6, THUMB_WIDTH * 0.6 * 1.414);
                            match self.backdrop.thumbs.get(&key) {
                                Some(t) => {
                                    let tint = if s.hidden { Color32::from_white_alpha(90) } else { Color32::WHITE };
                                    ui.add(egui::Image::new((t.id(), size)).tint(tint).bg_fill(Color32::WHITE));
                                }
                                None => {
                                    let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                    ui.painter().rect_filled(r, 2.0, Color32::from_gray(235));
                                }
                            }
                            ui.vertical(|ui| {
                                let page = self.doc.page_of(s.at) + 1;
                                let mark = if here == Some(s.id) { "  \u{b7} shown now" } else { "" };
                                ui.label(egui::RichText::new(format!("Page {page}{mark}")).size(12.0).color(TEXT_DIM));
                                ui.label(self.scene_snippet(s.at)).on_hover_text(&s.subject);
                                ui.horizontal(|ui| {
                                    if ui.small_button("Go to").on_hover_text("Turn to its place in the story").clicked() {
                                        action = Some(Do::GoTo(s.at));
                                    }
                                    if ui.small_button("Pin here").on_hover_text("Show it from the caret's paragraph on").clicked() {
                                        action = Some(Do::PinHere(s.id));
                                    }
                                    if s.versions.len() > 1 {
                                        if ui.add_enabled(s.shown > 0, egui::Button::new("\u{2039}").small()).clicked() {
                                            action = Some(Do::Step(s.id, -1));
                                        }
                                        ui.label(format!("{}/{}", s.shown + 1, s.versions.len()));
                                        if ui.add_enabled(s.shown + 1 < s.versions.len(), egui::Button::new("\u{203a}").small()).clicked() {
                                            action = Some(Do::Step(s.id, 1));
                                        }
                                    }
                                    let mut shows = !s.hidden;
                                    if ui.checkbox(&mut shows, "Show").changed() {
                                        action = Some(Do::Hidden(s.id, !shows));
                                    }
                                    if ui.small_button("Delete").on_hover_text("Delete this scene and all its versions").clicked() {
                                        action = Some(Do::Delete(s.id));
                                    }
                                });
                            });
                        });
                        ui.separator();
                    }
                });
            });
        self.backdrop.list = open;
        let caret_para = self.doc.para_start(self.caret).0;
        match action {
            Some(Do::GoTo(at)) => self.set_caret(ctx, at, false),
            Some(Do::PinHere(id)) => {
                // Another scene pinned to that paragraph already gives way.
                self.doc.scenes.retain(|s| s.id == id || s.at != caret_para);
                if let Some(s) = self.doc.scene_mut(id) {
                    s.at = caret_para;
                }
            }
            Some(Do::Step(id, by)) => {
                if let Some(s) = self.doc.scene_mut(id) {
                    s.shown = s.shown.saturating_add_signed(by).min(s.versions.len() - 1);
                }
            }
            Some(Do::Hidden(id, hidden)) => {
                if let Some(s) = self.doc.scene_mut(id) {
                    s.hidden = hidden;
                }
            }
            Some(Do::Delete(id)) => self.doc.scenes.retain(|s| s.id != id),
            None => {}
        }
    }

    /// The first words of the paragraph a scene is pinned to.
    fn scene_snippet(&self, at: usize) -> String {
        let text: String = self.selected_passage(at, self.doc.total_chars()).lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_owned();
        let mut words = String::new();
        for w in text.split_whitespace() {
            if words.len() + w.len() > 60 {
                words.push('\u{2026}');
                break;
            }
            if !words.is_empty() {
                words.push(' ');
            }
            words.push_str(w);
        }
        if words.is_empty() { "(empty)".into() } else { words }
    }

    /// The scene behind page `i` at `rect` (on screen through `map`), at `shade` and `alpha`. On
    /// the page shown, a new drawing fades in over the one before.
    pub fn backdrop_shapes(&self, ctx: &egui::Context, i: usize, rect: Rect, map: &dyn Fn(Pos2) -> Pos2, shade: f32, alpha: f32) -> Vec<Shape> {
        let b = &self.backdrop;
        if !b.visible {
            return Vec::new();
        }
        let tint = |a: f32| {
            let v = (255.0 * shade).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(v, v, v).gamma_multiply(a * alpha * OPACITY)
        };
        let mesh = |key: Option<DrawingKey>, a: f32| key.and_then(|k| b.textures.get(&k)).map(|t| Shape::mesh(strip_mesh(t, rect, tint(a), map)));
        match b.on_target {
            Some((page, shown)) if page == i => {
                let Some((from, since)) = b.fade else { return mesh(shown, 1.0).into_iter().collect() };
                let t = ((ctx.input(|inp| inp.time) - since) / FADE).clamp(0.0, 1.0) as f32;
                let t = t * t * (3.0 - 2.0 * t);
                mesh(from, 1.0 - t).into_iter().chain(mesh(shown, t)).collect()
            }
            _ => mesh(self.doc.page_scene_key(i), 1.0).into_iter().collect(),
        }
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
        let image = rasterize(svg, RENDER_WIDTH).unwrap();
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
        crate::claude::allow_live();
        // Two unrelated sentences: only the newest, the storm at sea, should be drawn.
        let sentences = "The village lay quiet in the valley, smoke rising from its chimneys.\nFar away, a lone ship fought its way through a storm on the open sea.";
        let (_, image) = draw_scene(&Subject::Sentences(sentences.into()), egui::vec2(595.0, 842.0), &AtomicBool::new(false)).unwrap().unwrap();
        let path = std::env::temp_dir().join("caprice-live-scene.png");
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8).unwrap();
        println!("scene saved to {}", path.display());
    }
}
