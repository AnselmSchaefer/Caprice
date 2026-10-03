//! Asking Claude about a passage. With the pen on, drawing a loop around text selects it and
//! opens a menu of commands (review, fix, grammar, summary, or a question of one's own). The answer
//! streams into a floating panel, from where a corrected passage can be put in place of the old one.
//!
//! The request goes through the installed Claude Code CLI (`claude -p`) on a background thread.
//! The CLI uses its own sign-in, which Caprice never sees, and streams the answer as it is written.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Id, Key, Pos2, Rect, Sense, Shape, Stroke, text::LayoutJob, vec2};
use serde_json::Value;

use crate::App;
use crate::model::{IMAGE_CHAR, PAGE_BREAK};
use crate::theme::{ACCENT, TEXT_DIM};

const MODEL: &str = "claude-opus-5-5";
const NO_CLI: &str = "Caprice could not find Claude Code. Install it and sign in by running `claude` once \
    in a terminal, then try again.";

const SYSTEM: &str = "You help a writer working in a word processor. They circled a passage of their \
    document and picked a command. Answer in the language of the passage. Write plain text only: no \
    Markdown, no headings, no bullet symbols other than simple dashes.";

#[derive(Clone, PartialEq)]
pub enum Command {
    Review,
    Fix,
    Grammar,
    Summarize,
    Ask(String),
}

impl Command {
    fn title(&self) -> &str {
        match self {
            Command::Review => "Review",
            Command::Fix => "Fix",
            Command::Grammar => "Grammar check",
            Command::Summarize => "Summary",
            Command::Ask(_) => "Answer",
        }
    }

    fn instruction(&self) -> &str {
        match self {
            Command::Review => "Review this passage: what works, what does not, and concrete suggestions. Be concise.",
            Command::Fix => "Rewrite this passage to fix errors and awkward or unclear wording, keeping its meaning, \
                tone and voice. Reply with only the rewritten passage, nothing before or after it.",
            Command::Grammar => "Correct only spelling, grammar and punctuation in this passage; change nothing else. \
                Reply with only the corrected passage, nothing before or after it. If nothing needs \
                correcting, reply with the passage unchanged.",
            Command::Summarize => "Summarize this passage in a few sentences.",
            Command::Ask(q) => q,
        }
    }

    /// Is the answer a new version of the passage, that can replace it?
    fn rewrites(&self) -> bool {
        matches!(self, Command::Fix | Command::Grammar)
    }
}

/// The loop being drawn (or drawn) around text, in page points of page `page`.
pub struct Lasso {
    pub page: usize,
    pub points: Vec<Pos2>,
    /// The flow range it caught, and where on screen its menu opens, once the loop is closed.
    pub caught: Option<(usize, usize, Pos2)>,
    /// What the user types into the menu's "Ask" field, and whether the menu has been shown yet.
    pub question: String,
    pub menu_shown: bool,
}

enum Msg {
    Text(String),
    Done,
    Failed(String),
    Refused,
}

#[derive(PartialEq)]
enum State {
    Waiting,
    Streaming,
    Done,
    Failed(String),
    Refused,
}

/// A question to Claude and its answer, as shown in the panel.
pub struct Answer {
    command: Command,
    /// The flow range asked about and its text then, to check it is unchanged before replacing it.
    range: (usize, usize),
    original: String,
    text: String,
    state: State,
    rx: Receiver<Msg>,
    cancel: Arc<AtomicBool>,
    at: Pos2,
}

impl Drop for Answer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Is `p` inside the polygon `poly` (even-odd rule)?
fn inside(poly: &[Pos2], p: Pos2) -> bool {
    let mut odd = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// What the stream says after one of its events.
enum Flow {
    Go,
    Stop,
}

/// Pass on one event of the answer's stream.
fn on_event(event: &Value, tx: &Sender<Msg>, ctx: &egui::Context) -> Result<Flow, String> {
    match event["type"].as_str() {
        Some("content_block_delta") if event["delta"]["type"] == "text_delta" => {
            let _ = tx.send(Msg::Text(event["delta"]["text"].as_str().unwrap_or_default().to_owned()));
            ctx.request_repaint();
        }
        Some("message_delta") if event["delta"]["stop_reason"] == "refusal" => {
            let _ = tx.send(Msg::Refused);
            return Ok(Flow::Stop);
        }
        Some("error") => return Err(event["error"]["message"].as_str().unwrap_or("Unknown error").to_owned()),
        Some("message_stop") => return Ok(Flow::Stop),
        _ => {}
    }
    Ok(Flow::Go)
}

/// Stream Claude's answer to `passage` and `command` into `tx`, until done or `cancel` is set.
fn ask(passage: &str, command: &Command, tx: &Sender<Msg>, ctx: &egui::Context, cancel: &AtomicBool) -> Result<(), String> {
    let prompt = format!("<passage>\n{passage}\n</passage>\n\n{}", command.instruction());
    let cli = find_cli().ok_or(NO_CLI)?;
    ask_cli(&cli, &prompt, tx, ctx, cancel)?;
    if !cancel.load(Ordering::Relaxed) {
        let _ = tx.send(Msg::Done);
    }
    Ok(())
}

/// The Claude Code CLI: on the PATH, or where installers put it (the app launcher's PATH is often short).
fn find_cli() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let on_path = std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).map(|d| d.join("claude"));
    let usual = [".local/bin/claude", ".claude/local/claude", ".local/share/mise/shims/claude", ".local/share/mise/installs/claude/latest/claude"]
        .map(|p| home.join(p));
    on_path.chain(usual).find(|p| p.is_file())
}

/// Ask through `claude -p`: no tools, no settings or project files, nothing saved, answer streamed.
fn ask_cli(cli: &std::path::Path, prompt: &str, tx: &Sender<Msg>, ctx: &egui::Context, cancel: &AtomicBool) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command as Process, Stdio};
    let mut child = Process::new(cli)
        .args(["-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages"])
        .args(["--tools", "", "--setting-sources", "", "--strict-mcp-config", "--no-session-persistence"])
        .args(["--model", MODEL, "--effort", "medium", "--system-prompt", SYSTEM])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start Claude Code ({}): {e}", cli.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).map_err(|e| format!("Could not talk to Claude Code: {e}"))?;
    }
    let stdout = child.stdout.take().ok_or("Claude Code gave no output")?;
    let mut failure = None;
    for line in BufReader::new(stdout).lines() {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Ok(());
        }
        let Ok(line) = line else { break };
        let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
        match msg["type"].as_str() {
            // After the message ends the CLI still sends its closing summary, so keep reading.
            Some("stream_event") => {
                on_event(&msg["event"], tx, ctx)?;
            }
            Some("result") if msg["is_error"] == true => {
                failure = Some(msg["result"].as_str().unwrap_or("Claude Code reported an error").to_owned());
            }
            _ => {}
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if let Some(f) = failure {
        return Err(format!("Claude Code: {f}"));
    }
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        let err = err.trim();
        return Err(if err.is_empty() { format!("Claude Code stopped ({status})") } else { format!("Claude Code: {err}") });
    }
    Ok(())
}

/// Splits text into runs of word characters and runs of everything else, for comparing.
fn tokens(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut kind = None;
    for (b, c) in s.char_indices() {
        let k = c.is_alphanumeric();
        if kind.is_some_and(|w| w != k) {
            out.push(&s[start..b]);
            start = b;
        }
        kind = Some(k);
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// The new text with what changed from the old one marked: removed words struck through in red,
/// added ones on green. None if the texts are too long to compare.
fn diff_job(old: &str, new: &str, font: FontId, ink: Color32) -> Option<LayoutJob> {
    let (a, b) = (tokens(old), tokens(new));
    if a.len() * b.len() > 4_000_000 {
        return None;
    }
    // Longest common subsequence table, filled from the end.
    let w = b.len() + 1;
    let mut lcs = vec![0u32; (a.len() + 1) * w];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i * w + j] = if a[i] == b[j] { lcs[(i + 1) * w + j + 1] + 1 } else { lcs[(i + 1) * w + j].max(lcs[i * w + j + 1]) };
        }
    }
    let same = egui::TextFormat { font_id: font.clone(), color: ink, ..Default::default() };
    let removed = egui::TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(235, 110, 110),
        strikethrough: Stroke::new(1.0, Color32::from_rgb(235, 110, 110)),
        ..Default::default()
    };
    let added = egui::TextFormat { font_id: font, color: ink, background: Color32::from_rgba_unmultiplied(80, 190, 110, 90), ..Default::default() };
    let mut job = LayoutJob::default();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            job.append(a[i], 0.0, same.clone());
            (i, j) = (i + 1, j + 1);
        } else if j < b.len() && (i == a.len() || lcs[i * w + j + 1] >= lcs[(i + 1) * w + j]) {
            job.append(b[j], 0.0, added.clone());
            j += 1;
        } else {
            job.append(a[i], 0.0, removed.clone());
            i += 1;
        }
    }
    Some(job)
}

impl App {
    /// The text of the flow chars `a..b`, as sent to Claude: pictures left out, page breaks as line breaks.
    fn passage(&self, a: usize, b: usize) -> String {
        let (ba, bb) = (self.doc.char_to_byte(a), self.doc.char_to_byte(b));
        self.doc.flow.text[ba..bb].chars().filter(|&c| c != IMAGE_CHAR).map(|c| if c == PAGE_BREAK { '\n' } else { c }).collect()
    }

    pub fn toggle_pen(&mut self) {
        self.pen = !self.pen;
        self.lasso = None;
    }

    /// With the pen on, page `i` at `page_rect` takes drags as loops drawn around text.
    pub fn pen_surface(&mut self, ui: &mut egui::Ui, page_rect: Rect, i: usize) {
        if self.lasso.as_ref().is_some_and(|l| l.page != i) {
            self.lasso = None;
        }
        if self.pen && !self.ctrl_down {
            let ctx = ui.ctx().clone();
            let sc = self.scale_of(page_rect);
            let resp = ui.interact(page_rect.expand(40.0), Id::new("pen"), Sense::drag());
            if resp.hovered() || resp.dragged() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
            }
            let to_page = |p: Pos2| ((p - page_rect.min) / sc).to_pos2();
            if resp.drag_started() {
                self.lasso = Some(Lasso { page: i, points: Vec::new(), caught: None, question: String::new(), menu_shown: false });
            }
            let drawing = resp.dragged() || resp.drag_started();
            if let (Some(l), Some(p), true) = (&mut self.lasso, resp.interact_pointer_pos(), drawing) {
                let q = to_page(p);
                if l.caught.is_none() && l.points.last().is_none_or(|&last| (last - q).length() * sc > 2.0) {
                    l.points.push(q);
                }
            }
            if resp.drag_stopped() {
                self.close_lasso(&ctx, page_rect, i);
            }
        }
        self.paint_lasso(ui.painter(), page_rect);
        self.lasso_menu(ui.ctx());
    }

    /// The loop is finished: select the text inside it and offer the commands.
    fn close_lasso(&mut self, ctx: &egui::Context, page_rect: Rect, i: usize) {
        let Some(mut l) = self.lasso.take() else { return };
        let origin = self.doc.setup.margin_origin().to_pos2();
        let start = self.doc.spans[i].start;
        let caught: Vec<usize> = if l.points.len() < 3 {
            Vec::new()
        } else {
            let layout = self.doc.layout_page(ctx, i, 1.0, &[]);
            layout.char_centers().into_iter().filter(|&(_, c)| inside(&l.points, origin + c.to_vec2())).map(|(k, _)| start + k).collect()
        };
        let (Some(&a), Some(&b)) = (caught.iter().min(), caught.iter().max()) else {
            self.status = "- draw a loop around the text to ask Claude about".into();
            return;
        };
        let sc = self.scale_of(page_rect);
        let end = l.points.last().map_or(page_rect.center(), |&p| page_rect.min + p.to_vec2() * sc);
        l.caught = Some((a, b + 1, end));
        self.lasso = Some(l);
        self.anchor = a;
        self.set_caret(ctx, b + 1, true);
        self.target = i; // the end of the passage can be the start of the next page
    }

    fn paint_lasso(&self, painter: &egui::Painter, page_rect: Rect) {
        let Some(l) = &self.lasso else { return };
        let sc = self.scale_of(page_rect);
        let pts: Vec<Pos2> = l.points.iter().map(|&p| page_rect.min + p.to_vec2() * sc).collect();
        let stroke = Stroke::new(2.2, ACCENT.gamma_multiply(0.85));
        if l.caught.is_some() {
            painter.add(Shape::closed_line(pts, stroke));
        } else {
            painter.add(Shape::line(pts, stroke));
        }
    }

    /// The commands for a closed loop.
    fn lasso_menu(&mut self, ctx: &egui::Context) {
        let Some(l) = &mut self.lasso else { return };
        let Some((a, b, at)) = l.caught else { return };
        let mut chosen = None;
        let area = egui::Area::new(Id::new("lasso_menu")).order(egui::Order::Foreground).fixed_pos(at + vec2(10.0, 10.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(230.0);
                ui.label(egui::RichText::new("Ask Claude about this passage").size(12.0).color(TEXT_DIM));
                for c in [Command::Review, Command::Fix, Command::Grammar, Command::Summarize] {
                    let label = match c {
                        Command::Summarize => "Sum it up",
                        _ => c.title(),
                    };
                    if ui.add(egui::Button::new(label).frame(false).min_size(vec2(220.0, 24.0))).clicked() {
                        chosen = Some(c);
                    }
                }
                ui.separator();
                let field = ui.add(egui::TextEdit::singleline(&mut l.question).hint_text("Ask something else…").desired_width(220.0));
                // Typing goes into the question, never over the circled text.
                if !l.menu_shown {
                    field.request_focus();
                    l.menu_shown = true;
                }
                if field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && !l.question.trim().is_empty() {
                    chosen = Some(Command::Ask(l.question.trim().to_owned()));
                }
            });
        });
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p)));
        let dismissed = clicked_outside || ctx.input(|i| i.key_pressed(Key::Escape));
        if let Some(command) = chosen {
            self.lasso = None;
            self.ask_claude(ctx, a, b, command, at);
        } else if dismissed {
            self.lasso = None;
        }
    }

    /// Send the passage `a..b` with `command`; the answer shows in the panel next to `at`.
    pub fn ask_claude(&mut self, ctx: &egui::Context, a: usize, b: usize, command: Command, at: Pos2) {
        let passage = self.passage(a, b);
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let (ctx, cancel, command, passage) = (ctx.clone(), cancel.clone(), command.clone(), passage.clone());
            std::thread::spawn(move || {
                if let Err(e) = ask(&passage, &command, &tx, &ctx, &cancel) {
                    let _ = tx.send(Msg::Failed(e));
                }
                ctx.request_repaint();
            });
        }
        let original = self.doc.flow.text[self.doc.char_to_byte(a)..self.doc.char_to_byte(b)].to_owned();
        self.answer = Some(Answer { command, range: (a, b), original, text: String::new(), state: State::Waiting, rx, cancel, at });
    }

    /// The floating panel with Claude's answer.
    pub fn answer_panel(&mut self, ctx: &egui::Context) {
        let Some(ans) = &mut self.answer else { return };
        while let Ok(msg) = ans.rx.try_recv() {
            match msg {
                Msg::Text(t) => {
                    ans.text.push_str(&t);
                    ans.state = State::Streaming;
                }
                Msg::Done if matches!(ans.state, State::Waiting | State::Streaming) => ans.state = State::Done,
                Msg::Done => {}
                Msg::Failed(e) => ans.state = State::Failed(e),
                Msg::Refused => ans.state = State::Refused,
            }
        }
        let applicable = self.applicable();
        let Some(ans) = &mut self.answer else { return };
        let mut open = true;
        let mut apply = false;
        let title = format!("Claude · {}", ans.command.title());
        egui::Window::new(title)
            .id(Id::new("claude_answer"))
            .open(&mut open)
            .default_pos(ans.at + vec2(16.0, 16.0))
            .default_width(380.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                match &ans.state {
                    State::Waiting => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Claude is thinking…");
                        });
                    }
                    State::Failed(e) => {
                        ui.colored_label(Color32::from_rgb(235, 110, 110), e);
                    }
                    State::Refused => {
                        ui.colored_label(Color32::from_rgb(235, 110, 110), "Claude declined to answer this request.");
                    }
                    State::Streaming | State::Done => {}
                }
                if !ans.text.is_empty() && ans.state != State::Refused {
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        let font = FontId::proportional(14.0);
                        let ink = ui.visuals().text_color();
                        let job = (ans.command.rewrites() && ans.state == State::Done)
                            .then(|| diff_job(&ans.original, ans.text.trim(), font.clone(), ink))
                            .flatten()
                            .unwrap_or_else(|| LayoutJob::simple(ans.text.clone(), font, ink, f32::INFINITY));
                        ui.add(egui::Label::new(job).wrap().selectable(true));
                    });
                }
                if ans.state == State::Streaming {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(egui::RichText::new("writing…").color(TEXT_DIM));
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ans.command.rewrites() {
                        let why = applicable.err().unwrap_or("Put this text in place of the circled passage (Ctrl+Z undoes it)");
                        let button = ui.add_enabled(applicable.is_ok(), egui::Button::new("Apply"));
                        apply = button.on_hover_text(why).on_disabled_hover_text(why).clicked();
                    }
                    if ui.add_enabled(!ans.text.is_empty(), egui::Button::new("Copy")).clicked() {
                        ui.ctx().copy_text(ans.text.trim().to_owned());
                    }
                });
            });
        if apply {
            self.apply_answer(ctx);
        } else if !open {
            self.answer = None;
        }
    }

    /// Can the answer replace the passage it is about? If not, why not.
    fn applicable(&self) -> Result<(), &'static str> {
        let Some(ans) = &self.answer else { return Err("No answer") };
        let (a, b) = ans.range;
        if !ans.command.rewrites() || ans.state != State::Done || ans.text.trim().is_empty() {
            return Err("Claude has not finished a new version of the passage yet");
        }
        if ans.original.contains([IMAGE_CHAR, PAGE_BREAK]) {
            return Err("The passage holds a picture or page break; copy the text instead");
        }
        let now = (b <= self.doc.total_chars()).then(|| &self.doc.flow.text[self.doc.char_to_byte(a)..self.doc.char_to_byte(b)]);
        if now != Some(ans.original.as_str()) {
            return Err("The passage has been edited since; copy the text instead");
        }
        Ok(())
    }

    /// Put the answer in place of the passage it is about (one undo step), and close the panel.
    pub fn apply_answer(&mut self, ctx: &egui::Context) -> bool {
        if self.applicable().is_err() {
            return false;
        }
        let Some(ans) = self.answer.take() else { return false };
        let (a, b) = ans.range;
        self.set_caret(ctx, a, false);
        self.replace_range(ctx, a, b, ans.text.trim());
        true
    }

    /// An answer that has already arrived in full, as if from Claude.
    #[cfg(test)]
    pub fn fake_answer(&mut self, a: usize, b: usize, command: Command, text: &str) {
        let (tx, rx) = channel();
        tx.send(Msg::Text(text.to_owned())).unwrap();
        tx.send(Msg::Done).unwrap();
        let original = self.doc.flow.text[self.doc.char_to_byte(a)..self.doc.char_to_byte(b)].to_owned();
        let cancel = Arc::new(AtomicBool::new(false));
        self.answer = Some(Answer { command, range: (a, b), original, text: String::new(), state: State::Waiting, rx, cancel, at: Pos2::ZERO });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::pos2;

    /// Asks the real Claude through the Claude Code CLI: `cargo test live -- --ignored`.
    #[test]
    #[ignore]
    fn live_grammar_check() {
        let (tx, rx) = channel();
        let ctx = egui::Context::default();
        ask("Their is a dog in the gardn.", &Command::Grammar, &tx, &ctx, &AtomicBool::new(false)).unwrap();
        let text: String = rx.try_iter().filter_map(|m| if let Msg::Text(t) = m { Some(t) } else { None }).collect();
        println!("answer: {text}");
        assert!(text.contains("There") && text.contains("garden"));
    }

    #[test]
    fn points_inside_a_loop_are_found() {
        let square = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0), pos2(0.0, 10.0)];
        assert!(inside(&square, pos2(5.0, 5.0)));
        assert!(!inside(&square, pos2(15.0, 5.0)));
        assert!(!inside(&square, pos2(5.0, -1.0)));
    }

    #[test]
    fn diff_marks_only_what_changed() {
        let job = diff_job("the qick fox", "the quick fox", FontId::proportional(10.0), Color32::BLACK).unwrap();
        let parts: Vec<(&str, bool)> = job
            .sections
            .iter()
            .map(|s| (&job.text[s.byte_range.start.0..s.byte_range.end.0], s.format.background != Color32::TRANSPARENT))
            .collect();
        let marked = |word: &str| parts.iter().find(|(t, _)| t.contains(word)).map(|&(_, m)| m);
        assert_eq!(marked("quick"), Some(true));
        assert_eq!((marked("the"), marked("fox")), (Some(false), Some(false)));
        assert!(job.text.contains("qick"), "the removed word is shown struck through");
    }
}
