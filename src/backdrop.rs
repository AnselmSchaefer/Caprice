//! Scenes behind the page: whenever a sentence is finished, Claude sketches what the last few
//! sentences describe, and the sketch fades in faintly behind the text of every page, replacing
//! the one before. The sketches are SVG drawn through the Claude Code CLI and rendered here; they
//! are not part of the document.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Shape, TextureHandle, TextureOptions, epaint::Mesh, pos2};

use crate::App;
use crate::claude::{Outcome, run_claude};
use crate::model::{IMAGE_CHAR, PAGE_BREAK};

/// How strongly a scene shows through the paper.
pub const OPACITY: f32 = 0.2;
/// How many of the last finished sentences a scene is drawn from.
const SENTENCES: usize = 3;
/// Seconds to wait after a sentence is finished before drawing, in case another follows.
const PAUSE: f64 = 1.5;
/// Seconds one scene takes to fade into the next.
const FADE: f64 = 2.5;
/// Width in pixels scenes are rendered at.
const RENDER_WIDTH: f32 = 1000.0;

const SYSTEM: &str = "You are an illustrator who draws with SVG code. You make simple, evocative \
    hand-drawn sketches for the pages of a story as it is being written.";

/// A scene on screen, and when it began to fade in.
struct Shown {
    texture: TextureHandle,
    since: f64,
}

struct Job {
    rx: Receiver<Result<ColorImage, String>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct Backdrop {
    pub on: bool,
    current: Option<Shown>,
    previous: Option<Shown>,
    /// The sentences of the scene being drawn or shown last.
    drawn_for: String,
    /// The newest finished sentences and when they were first seen.
    pending: Option<(String, f64)>,
    job: Option<Job>,
    /// The document version the sentences were last read at: only edits can start a scene, not
    /// moving the caret around.
    read_at: Option<u64>,
}

/// The last `n` finished sentences of `text`, joined. A sentence is finished by `.`, `!`, `?` or `…`
/// followed by a space or line break, or by the end of its paragraph.
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
    done[from..].join(" ")
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
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| format!("the sketch could not be read: {e}"))?;
    let size = tree.size();
    let scale = RENDER_WIDTH / size.width();
    let (w, h) = (RENDER_WIDTH.round() as u32, (size.height() * scale).round().max(1.0) as u32);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("the sketch has no size")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data()))
}

/// Ask Claude for a sketch of `passage` on a page `size` points large, and render it.
fn draw_scene(passage: &str, size: egui::Vec2, cancel: &AtomicBool) -> Result<Option<ColorImage>, String> {
    let (w, h) = (size.x.round(), size.y.round());
    let prompt = format!(
        "<passage>\n{passage}\n</passage>\n\n\
        Draw one sketch of the scene this passage of a story describes, to be shown faintly behind \
        the text of a page.\n\
        - One standalone SVG with viewBox=\"0 0 {w} {h}\", filling the whole area.\n\
        - Pencil lines that wobble a little (an feTurbulence + feDisplacementMap filter), some \
          hatching, and light, uneven watercolour washes in a few soft colours.\n\
        - Transparent background: no paper or background rectangle.\n\
        - No text, letters or numbers. No scripts, links, external references or embedded images.\n\
        - Keep it simple and under about 15 KB.\n\
        Reply with only the SVG code."
    );
    let mut reply = String::new();
    match run_claude(SYSTEM, &prompt, "medium", cancel, &mut |t| reply.push_str(t))? {
        Outcome::Cancelled => Ok(None),
        Outcome::Refused => Err("Claude declined to draw this scene".into()),
        Outcome::Finished => {
            let svg = extract_svg(&reply).ok_or("Claude's reply held no drawing")?;
            rasterize(svg).map(Some)
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

impl Backdrop {
    /// The sentences waiting to be drawn.
    #[cfg(test)]
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_ref().map(|(p, _)| p.as_str())
    }
}

impl App {
    pub fn toggle_backdrop(&mut self) {
        let b = &mut self.backdrop;
        b.on = !b.on;
        b.job = None;
        b.pending = None;
        // Turned on, it draws the scene of what is already written.
        b.drawn_for.clear();
        b.read_at = None;
        if !b.on {
            b.current = None;
            b.previous = None;
        }
    }

    /// Notice finished sentences, start drawing after a pause, and take in finished sketches.
    pub fn update_backdrop(&mut self, ctx: &egui::Context) {
        if !self.backdrop.on {
            return;
        }
        let now = ctx.input(|i| i.time);
        if self.backdrop.read_at != Some(self.doc.version) {
            self.backdrop.read_at = Some(self.doc.version);
            // The sentences written last, before the caret.
            let upto = self.doc.char_to_byte(self.caret);
            let latest = last_sentences(&self.doc.flow.text[..upto], SENTENCES);
            let b = &mut self.backdrop;
            if latest.is_empty() || latest == b.drawn_for {
                b.pending = None;
            } else if b.pending.as_ref().is_none_or(|(p, _)| *p != latest) {
                b.pending = Some((latest, now));
            }
        }
        let b = &mut self.backdrop;

        if let Some(job) = &b.job {
            match job.rx.try_recv() {
                Ok(Ok(image)) => {
                    let texture = ctx.load_texture("scene", image, TextureOptions::LINEAR);
                    b.previous = b.current.take();
                    b.current = Some(Shown { texture, since: now });
                    b.job = None;
                }
                Ok(Err(e)) => {
                    self.status = format!("- no scene: {e}");
                    b.job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => b.job = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }

        let idle = b.job.is_none();
        if let (true, Some((passage, seen))) = (idle, b.pending.clone()) {
            let wait = seen + PAUSE - now;
            if wait > 0.0 {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            } else {
                b.pending = None;
                b.drawn_for = passage.clone();
                let (tx, rx) = channel();
                let cancel = Arc::new(AtomicBool::new(false));
                let (ctx2, cancel2, size) = (ctx.clone(), cancel.clone(), self.doc.setup.size());
                std::thread::spawn(move || {
                    if let Some(result) = draw_scene(&passage, size, &cancel2).transpose() {
                        let _ = tx.send(result);
                    }
                    ctx2.request_repaint();
                });
                b.job = Some(Job { rx, cancel });
            }
        }

        if b.current.as_ref().is_some_and(|s| now - s.since < FADE) {
            ctx.request_repaint();
        } else {
            b.previous = None;
        }
    }

    /// The scene behind a page at `rect` (on screen through `map`), faded in, at `shade` and `alpha`.
    pub fn backdrop_shapes(&self, ctx: &egui::Context, rect: Rect, map: &dyn Fn(Pos2) -> Pos2, shade: f32, alpha: f32) -> Vec<Shape> {
        let b = &self.backdrop;
        let Some(current) = &b.current else { return Vec::new() };
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
        assert_eq!(last_sentences("One. Two! Three? Four", 3), "One. Two! Three?");
        assert_eq!(last_sentences("One. Two. Three. Four. ", 2), "Three. Four.");
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
        let passage = "The story is about a magical land hidden in the mountains and a rural human land in the valley between the mountains and the sea.";
        let image = draw_scene(passage, egui::vec2(595.0, 842.0), &AtomicBool::new(false)).unwrap().unwrap();
        let path = std::env::temp_dir().join("caprice-live-scene.png");
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        image::save_buffer(&path, &rgba, image.size[0] as u32, image.size[1] as u32, image::ColorType::Rgba8).unwrap();
        println!("scene saved to {}", path.display());
    }
}
