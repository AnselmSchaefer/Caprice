//! Little things hidden in the magic themes. In the Magic Garden a garden grows up from the bottom
//! of the desk, a plant for each chapter, taller the more pages it has, and flowering once the next
//! chapter has begun. In the Magic City a street of little houses stands there instead, a floor for
//! each page: a new floor is a light that flows out of a window and spreads into the floor, and
//! some windows are lit, flickering. Now and then, when the writer makes a paragraph a chapter title, it is
//! brought in: in the garden a hot-air balloon floats down with it hanging below its basket, sets it
//! in place and rises away; in the Magic City a crane drives in from the side, carries it over the
//! page on its hook, lowers it into place and drives away. Both follow one course in time (`pose`),
//! each along its own way (`rig`).
//!
//! None of it is on the page: the garden and the city lie on the desk behind the pages, and what brings a title
//! is drawn
//! over the page on the screen only, so nothing of it is printed or saved, and no page break moves
//! for it. Like the emblem, it is drawn from shapes (egui fills only convex ones, R11), and what is
//! shown is a function of the story and of time, so a test can look at any moment of it.

use std::f32::consts::PI;
use std::hash::{Hash, Hasher};

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

use crate::App;
use crate::theme::Theme;

/// What brings a new chapter title in, in which theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Carrier {
    /// In the Magic City.
    Crane,
    /// In the Magic Garden, from the right side or the left: each comes from the other side.
    Balloon { right: bool },
}

impl Carrier {
    pub fn of(theme: Theme) -> Option<Carrier> {
        match theme {
            Theme::City => Some(Carrier::Crane),
            Theme::Garden => Some(Carrier::Balloon { right: true }),
            _ => None,
        }
    }
}

/// How tall a plant grows at most, in shares of the desk's height, and how many pages it takes to
/// grow most of the way: a chapter of `PAGES` pages is two thirds of the way up.
const TALLEST: f32 = 0.55;
const PAGES: f32 = 8.0;
/// How fast a plant grows to its height, in shares of its tallest a second.
const GROW: f32 = 0.5;
/// The chance that making a chapter title calls the crane.
pub const ODDS: f64 = 1.0 / 3.0;

const STEM: Color32 = Color32::from_rgb(86, 142, 66);
const LEAF: Color32 = Color32::from_rgb(112, 172, 82);
const LEAF_DARK: Color32 = Color32::from_rgb(84, 140, 62);
const GRASS: Color32 = Color32::from_rgb(98, 158, 74);
const BUD: Color32 = Color32::from_rgb(132, 186, 96);
const PETALS: [Color32; 5] = [
    Color32::from_rgb(240, 122, 152),
    Color32::from_rgb(250, 200, 72),
    Color32::from_rgb(172, 132, 230),
    Color32::from_rgb(242, 142, 82),
    Color32::from_rgb(250, 248, 240),
];
const HEART: Color32 = Color32::from_rgb(232, 168, 40);

const STEEL: Color32 = Color32::from_rgb(242, 186, 36);
const STEEL_DARK: Color32 = Color32::from_rgb(196, 146, 28);
const WEIGHT: Color32 = Color32::from_rgb(118, 120, 132);
const CABLE: Color32 = Color32::from_rgb(70, 70, 82);

const SILK: [Color32; 2] = [Color32::from_rgb(232, 76, 84), Color32::from_rgb(252, 206, 74)];
const WICKER: Color32 = Color32::from_rgb(160, 110, 62);
const WICKER_DARK: Color32 = Color32::from_rgb(122, 80, 44);
pub const ROPE: Color32 = Color32::from_rgb(110, 92, 74);
/// How tall the balloon is from the bottom of its basket to the top of its silk, in its sizes.
const TALL: f32 = 2.92;

/// The Magic City's houses: their walls, roofs and doors, a window dark and lit, and the light a
/// new floor is made of.
const WALLS: [Color32; 5] = [
    Color32::from_rgb(96, 70, 134),
    Color32::from_rgb(54, 98, 122),
    Color32::from_rgb(74, 78, 146),
    Color32::from_rgb(132, 74, 116),
    Color32::from_rgb(60, 106, 100),
];
const ROOFS: [Color32; 3] = [Color32::from_rgb(44, 38, 78), Color32::from_rgb(150, 64, 96), Color32::from_rgb(36, 72, 96)];
const DOOR: Color32 = Color32::from_rgb(52, 34, 46);
const DARK: Color32 = Color32::from_rgb(34, 30, 58);
const GLOW: Color32 = Color32::from_rgb(255, 216, 124);
const GLOW_DIM: Color32 = Color32::from_rgb(224, 146, 58);
pub const LIGHT: Color32 = Color32::from_rgb(255, 238, 178);
const STAR: Color32 = Color32::from_rgb(255, 222, 112);
/// Seconds a new floor takes: its light flows out of a window for the first `FLOW` of it, and
/// spreads into the floor for the rest.
pub const BUILD: f32 = 1.6;
pub const FLOW: f32 = 0.45;

/// Seconds the crane takes to drive in, run the title out over the page, lower it, settle it,
/// lift its hook off, and drive away; and to drive away when the writer goes on meanwhile. The
/// balloon takes as long to come, float over, come down, let go and rise away.
const ENTER: f32 = 0.5;
const RUN: f32 = 0.7;
const LOWER: f32 = 0.9;
const SETTLE: f32 = 0.25;
const UNHOOK: f32 = 0.25;
const LEAVE: f32 = 0.5;
const HURRY: f32 = 0.4;

/// What is hidden, and how it is doing.
pub struct Eggs {
    /// The story the garden was grown for, and how far each plant has grown, in shares of its
    /// tallest. A plant never shrinks while its story is open: taking pages away stops it growing.
    grown_for: Option<String>,
    grown: Vec<f32>,
    grown_at: f64,
    /// The story, its version and its number of pages when the garden was last measured.
    measured: Option<(String, u64, usize)>,
    /// The crane or the balloon at work, if one is.
    pub lift: Option<Lift>,
    /// Where the title being lifted lies on the screen, from the last page drawn.
    pub place: Option<Rect>,
    /// The chance that making a chapter title calls the crane (`ODDS`; tests set it to 0 or 1).
    pub odds: f64,
    /// The side the last balloon came from: the next comes from the other.
    balloon_right: bool,
    rng: u64,
    /// Each plant drawn last frame: from its root to its tip, and whether it flowered; for tests.
    pub plants: Vec<(Rect, bool)>,
    /// The story the city was built for, and how many floors it has risen to, the last perhaps in
    /// part, while it is being built.
    built_for: Option<String>,
    built: f32,
    built_at: f64,
    /// Each house drawn last frame: its floors from the ground to the top of the last one built,
    /// and how many; and the light of a floor being built, if one is; for tests.
    pub houses: Vec<(Rect, usize)>,
    pub light: Option<Rect>,
}

/// The crane or the balloon bringing the chapter title that begins at `at` into place on page `page`.
#[derive(Clone, Copy, Debug)]
pub struct Lift {
    pub by: Carrier,
    pub at: usize,
    pub page: usize,
    pub started: f64,
    /// When the writer went on, if they did before it was over.
    pub duck: Option<f64>,
}

impl Default for Eggs {
    fn default() -> Self {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(7, |d| d.as_nanos() as u64);
        Self { grown_for: None, grown: Vec::new(), grown_at: 0.0, measured: None, lift: None, place: None, odds: ODDS, balloon_right: false, rng: seed | 1, plants: Vec::new(),
            built_for: None, built: 0.0, built_at: 0.0, houses: Vec::new(), light: None }
    }
}

impl Eggs {
    /// A number from 0 to 1, at random.
    fn roll(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10_000) as f64 / 10_000.0
    }
}

/// How far up its tallest a plant of `pages` pages grows.
pub fn height_of(pages: usize) -> f32 {
    1.0 - (-(pages as f32) / PAGES).exp()
}

/// A little generator of numbers, the same for the same seed, so a story grows the same garden.
struct Seeds(u64);

impl Seeds {
    fn of(story: &str, k: usize) -> Seeds {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (story, k).hash(&mut h);
        Seeds(h.finish() | 1)
    }

    /// A number from `lo` to `hi`.
    fn next(&mut self, lo: f32, hi: f32) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        lo + (hi - lo) * (self.0 % 10_000) as f32 / 10_000.0
    }
}

/// A leaf from `at`, pointing `dir`, this long: a lens, which is convex.
fn leaf(at: Pos2, dir: Vec2, len: f32, color: Color32) -> Shape {
    let side = vec2(-dir.y, dir.x) * len * 0.22;
    let points = (0..=12)
        .map(|k| {
            let s = k as f32 / 12.0;
            let (s, w) = if k <= 6 { (s * 2.0, 1.0) } else { (2.0 - s * 2.0, -1.0) };
            at + dir * len * s + side * w * (s * PI).sin()
        })
        .collect();
    Shape::convex_polygon(points, color, Stroke::NONE)
}

/// One plant: its stem from `root`, `height` tall, its leaves where they have grown, and at its
/// top a flower or a bud. `unit` is the size of things, 1 on a desk 900 points high.
pub fn plant(story: &str, k: usize, root: Pos2, height: f32, flower: bool, unit: f32) -> Vec<Shape> {
    let mut seeds = Seeds::of(story, k);
    let (sway, wave, phase) = (seeds.next(6.0, 14.0) * unit, seeds.next(90.0, 160.0) * unit, seeds.next(0.0, 2.0 * PI));
    // Where the stem is at a height: the same at every height however tall it is, so growing
    // only adds to its top.
    let at = |y: f32| pos2(root.x + sway * ((y / wave) * 2.0 * PI + phase).sin() - sway * phase.sin(), root.y - y);
    let steps = (height / (4.0 * unit)).ceil().max(1.0) as usize;
    let stem: Vec<Pos2> = (0..=steps).map(|s| at(height * s as f32 / steps as f32)).collect();
    let mut out = vec![Shape::line(stem, Stroke::new(3.0 * unit, STEM))];
    let mut y = seeds.next(16.0, 28.0) * unit;
    let mut left = seeds.next(0.0, 1.0) < 0.5;
    while y < height - 6.0 * unit {
        // Young leaves near the top are smaller.
        let len = (24.0 * unit * seeds.next(0.8, 1.2)).min((height - y) * 0.9);
        let tilt = seeds.next(0.35, 0.75);
        let dir = egui::Vec2::angled(if left { -PI + tilt } else { -tilt });
        out.push(leaf(at(y), dir, len, if left { LEAF } else { LEAF_DARK }));
        y += seeds.next(26.0, 38.0) * unit;
        left = !left;
    }
    let top = at(height);
    if flower {
        let color = PETALS[(seeds.next(0.0, PETALS.len() as f32) as usize).min(PETALS.len() - 1)];
        let n = 5 + (seeds.next(0.0, 2.0) as usize);
        let turn = seeds.next(0.0, PI);
        for p in 0..n {
            let a = turn + p as f32 * 2.0 * PI / n as f32;
            out.push(Shape::circle_filled(top + egui::Vec2::angled(a) * 7.0 * unit, 5.5 * unit, color));
        }
        out.push(Shape::circle_filled(top, 4.5 * unit, HEART));
    } else {
        out.push(Shape::circle_filled(top + vec2(0.0, -2.0 * unit), 3.5 * unit, BUD));
    }
    out
}

/// Blades of grass along the bottom of the desk.
fn grass(story: &str, area: Rect, unit: f32) -> Vec<Shape> {
    let mut seeds = Seeds::of(story, usize::MAX);
    let mut out = Vec::new();
    let mut x = area.left() + seeds.next(0.0, 10.0) * unit;
    while x < area.right() {
        let (h, lean) = (seeds.next(6.0, 16.0) * unit, seeds.next(-4.0, 4.0) * unit);
        out.push(Shape::line(vec![pos2(x, area.bottom()), pos2(x + lean, area.bottom() - h)], Stroke::new(1.5 * unit, GRASS)));
        x += seeds.next(6.0, 14.0) * unit;
    }
    out
}

/// Where the crane (or the balloon) is in its work at `t` seconds, the writer having gone on at `duck`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    /// 0: away beyond the side of the window, 1: standing beside the page.
    pub enter: f32,
    /// 0: the hook by the crane's tower, 1: over the title's place.
    pub run: f32,
    /// 0: the title hanging high, 1: in its place.
    pub lower: f32,
    /// Is the title still on the hook?
    pub hooked: bool,
    /// 0: the hook at the title, 1: lifted clear of it.
    pub hook: f32,
    pub done: bool,
}

pub fn pose(t: f32, duck: Option<f32>) -> Pose {
    let ease = |s: f32| {
        let s = s.clamp(0.0, 1.0);
        s * s * (3.0 - 2.0 * s)
    };
    let (run_at, lower_at) = (ENTER, ENTER + RUN);
    let (settle_at, unhook_at) = (lower_at + LOWER, lower_at + LOWER + SETTLE);
    let leave_at = unhook_at + UNHOOK;
    if let Some(d) = duck.filter(|&d| d < leave_at) {
        // The title is set down at once, and the crane hurries away from wherever it was.
        let was = pose(d, None).enter;
        let enter = was * (1.0 - ((t - d) / HURRY).clamp(0.0, 1.0));
        return Pose { enter, run: 1.0, lower: 1.0, hooked: false, hook: 1.0, done: t - d >= HURRY };
    }
    let mut lower = ease((t - lower_at) / LOWER);
    if (settle_at..unhook_at).contains(&t) {
        // It touches down, and bounces a little.
        lower = 1.0 - 0.06 * (PI * (t - settle_at) / SETTLE).sin();
    }
    Pose {
        enter: ease(t / ENTER) * (1.0 - ease((t - leave_at) / LEAVE)),
        run: ease((t - run_at) / RUN),
        lower,
        hooked: t < unhook_at,
        hook: ease((t - unhook_at) / UNHOOK),
        done: t >= leave_at + LEAVE,
    }
}

/// Where the crane's parts are, or the balloon's, for a title lying at `place`, on a page lying at
/// `page`.
struct Rig {
    /// The crane's tower's middle, and the height of its arm.
    mast_x: f32,
    jib_y: f32,
    /// The arm's two ends.
    jib: (f32, f32),
    /// The crane's hook, or the bottom of the balloon's basket.
    hook: Pos2,
    /// The balloon's size: the radius of its silk.
    size: f32,
    /// How far the title is from its place.
    offset: Vec2,
}

fn rig(by: Carrier, p: Pose, place: Rect, page: Rect) -> Rig {
    let hang = page.height() * 0.16;
    match by {
        Carrier::Crane => {
            let away = (1.0 - p.enter) * page.width() * 0.9;
            let mast_x = page.right() + page.width() * 0.12 + away;
            let jib_y = place.top() - hang - page.height() * 0.08;
            let start = mast_x - page.width() * 0.1;
            let hook_x = start + (place.center().x + away - start) * p.run;
            let load = pos2(hook_x, place.center().y - hang * (1.0 - p.lower));
            let offset = if p.hooked { load - place.center() } else { Vec2::ZERO };
            let hook_y = if p.hooked { load.y - place.height() / 2.0 - 10.0 } else { place.top() - 10.0 - hang * 0.5 * p.hook };
            let jib = (place.left().min(start) + away - 20.0, mast_x + page.width() * 0.14);
            Rig { mast_x, jib_y, jib, hook: pos2(hook_x, hook_y), size: 0.0, offset }
        }
        Carrier::Balloon { right } => {
            // In from the side at the title's height, a little above it; let go, it floats on and
            // away past the other side. A title is often high on its page, so the balloon hangs
            // only as high as there is room for below the top of the page, and is smaller there.
            let side = if right { 1.0 } else { -1.0 };
            let away = 1.0 - p.enter;
            let rope = page.height() * 0.02;
            let room = place.top() - (page.top() - page.height() * 0.03) - rope;
            let size = (room / TALL).clamp(page.width() * 0.035, page.width() * 0.06);
            let hang = (room - size * TALL).clamp(0.0, page.height() * 0.08);
            let (basket, offset) = if p.hooked {
                let x = place.center().x + side * ((1.0 - p.run) * page.width() * 0.35 + away * page.width() * 0.6);
                let bob = (p.run * PI).sin() * page.height() * 0.015;
                let load = pos2(x, place.center().y - hang * (1.0 - p.lower) - bob);
                (load - vec2(0.0, place.height() / 2.0 + rope), load - place.center())
            } else {
                let up = rope + page.height() * 0.04 * p.hook;
                (place.center_top() - vec2(0.0, up) + vec2(-side * 0.7 * page.width(), -0.1 * page.height()) * away, Vec2::ZERO)
            };
            Rig { mast_x: 0.0, jib_y: 0.0, jib: (0.0, 0.0), hook: basket, size, offset }
        }
    }
}

/// How far the title is from its place at `p`, brought `by` that.
pub fn offset(by: Carrier, p: Pose, place: Rect, page: Rect) -> Vec2 {
    rig(by, p, place, page).offset
}

/// A beam of the crane between `a` and `b`, `w` thick: two rails and the lattice between them.
fn beam(a: Pos2, b: Pos2, w: f32, line: f32) -> Vec<Shape> {
    let along = b - a;
    let side = vec2(-along.y, along.x).normalized() * w / 2.0;
    let stroke = Stroke::new(line, STEEL);
    let mut out = vec![Shape::line_segment([a + side, b + side], stroke), Shape::line_segment([a - side, b - side], stroke)];
    let n = (along.length() / w).ceil().max(1.0) as usize;
    let thin = Stroke::new(line * 0.6, STEEL_DARK);
    for k in 0..n {
        let (s, e) = (a + along * (k as f32 / n as f32), a + along * ((k + 1) as f32 / n as f32));
        let flip = if k % 2 == 0 { 1.0 } else { -1.0 };
        out.push(Shape::line_segment([s + side * flip, e - side * flip], thin));
    }
    out
}

/// The crane at `p`, standing on the desk's bottom `ground`, the title's place at `place`.
pub fn crane(p: Pose, place: Rect, page: Rect, ground: f32) -> Vec<Shape> {
    let r = rig(Carrier::Crane, p, place, page);
    let w = page.width() * 0.035;
    let line = (page.width() * 0.004).max(1.0);
    let mut out = beam(pos2(r.mast_x, ground), pos2(r.mast_x, r.jib_y), w, line);
    out.extend(beam(pos2(r.jib.0, r.jib_y), pos2(r.jib.1, r.jib_y), w * 0.8, line));
    // The peak above the tower, tied to both ends of the arm.
    let peak = pos2(r.mast_x, r.jib_y - w * 2.2);
    out.push(Shape::convex_polygon(vec![peak, pos2(r.mast_x + w / 2.0, r.jib_y), pos2(r.mast_x - w / 2.0, r.jib_y)], STEEL, Stroke::NONE));
    for end in [r.jib.0 + w, r.jib.1] {
        out.push(Shape::line_segment([peak, pos2(end, r.jib_y - w * 0.4)], Stroke::new(line * 0.6, CABLE)));
    }
    let weight = Rect::from_min_size(pos2(r.jib.1 - w * 1.6, r.jib_y + w * 0.4), vec2(w * 1.4, w * 1.2));
    out.push(Shape::rect_filled(weight, 1.0, WEIGHT));
    let cab = Rect::from_min_size(pos2(r.mast_x - w * 0.9, r.jib_y + w * 0.4), vec2(w * 0.9, w * 0.8));
    out.push(Shape::rect_filled(cab, 2.0, STEEL_DARK));
    // The trolley on the arm, its cable down to the hook, and the slings to the title.
    let trolley = Rect::from_center_size(pos2(r.hook.x, r.jib_y + w * 0.5), vec2(w * 0.8, w * 0.35));
    out.push(Shape::rect_filled(trolley, 1.0, WEIGHT));
    out.push(Shape::line_segment([trolley.center_bottom(), r.hook], Stroke::new(line * 0.7, CABLE)));
    out.push(Shape::circle_stroke(r.hook + vec2(0.0, w * 0.2), w * 0.2, Stroke::new(line, CABLE)));
    if p.hooked {
        let load = place.translate(r.offset);
        for corner in [load.left_top() + vec2(load.width() * 0.1, 0.0), load.right_top() - vec2(load.width() * 0.1, 0.0)] {
            out.push(Shape::line_segment([r.hook + vec2(0.0, w * 0.4), corner], Stroke::new(line * 0.6, CABLE)));
        }
    }
    out
}

/// The balloon at `p`, the title's place at `place`: its silk in stripes, the basket below, and
/// while it carries the title, ropes down to it.
pub fn balloon(by: Carrier, p: Pose, place: Rect, page: Rect) -> Vec<Shape> {
    let r = rig(by, p, place, page);
    let size = r.size;
    let line = (page.width() * 0.003).max(1.0);
    let basket = Rect::from_min_max(r.hook - vec2(size * 0.32, size * 0.42), r.hook + vec2(size * 0.32, 0.0));
    let middle = basket.center_top() - vec2(0.0, size * 0.5 + size);
    let mouth = basket.center_top() - vec2(0.0, size * 0.5);
    let lip = size * 0.22;
    let mut out = Vec::new();
    // From the basket's corners up to the silk's mouth.
    for (x, end) in [(basket.left() + line, mouth - vec2(lip, 0.0)), (basket.right() - line, mouth + vec2(lip, 0.0))] {
        out.push(Shape::line_segment([pos2(x, basket.top()), end], Stroke::new(line * 0.7, ROPE)));
    }
    // The silk: a round top tapering to its mouth, drawn as stripes running from the top down to
    // the mouth, each a convex piece, so the colours take turns all the way round.
    let n = 8;
    let at = |a: f32| middle + egui::Vec2::angled(a) * size;
    let lip_at = |j: usize| mouth + vec2(lip * (2.0 * j as f32 / n as f32 - 1.0), 0.0);
    for k in 0..n {
        let (a, b) = (PI * (0.85 + 1.3 * k as f32 / n as f32), PI * (0.85 + 1.3 * (k + 1) as f32 / n as f32));
        let mut points: Vec<Pos2> = (0..=4).map(|j| at(a + (b - a) * j as f32 / 4.0)).collect();
        points.extend([lip_at(k + 1), lip_at(k)]);
        out.push(Shape::convex_polygon(points, SILK[k % 2], Stroke::NONE));
    }
    out.push(Shape::rect_filled(basket, 2.0, WICKER));
    out.push(Shape::rect_filled(Rect::from_min_size(basket.min, vec2(basket.width(), basket.height() * 0.25)), 2.0, WICKER_DARK));
    // From the basket's bottom corners down to the ends of the title.
    if p.hooked {
        let load = place.translate(r.offset);
        let ends = [(basket.left_bottom(), load.left_top()), (basket.right_bottom(), load.right_top())];
        for (from, to) in ends {
            out.push(Shape::line_segment([from, to], Stroke::new(line * 0.7, ROPE)));
        }
    }
    out
}

/// A house of the Magic City: where it stands, how it looks, and how far it is built.
#[derive(Clone, Debug)]
pub struct House {
    pub left: f32,
    pub width: f32,
    pub ground: f32,
    /// A floor's height.
    pub storey: f32,
    /// The most floors it gets while there is room on the desk for another house; once there is
    /// none, it grows on.
    pub most: usize,
    /// Floors built, and how far the next one is (0 when none is being built).
    pub floors: usize,
    pub building: f32,
    /// What its windows, its colours and its roof come from.
    seed: usize,
    windows: usize,
    wall: Color32,
    roof: (Color32, usize),
    unit: f32,
}

impl House {
    /// The city's house `i`, the same for the same story, standing on `ground`.
    pub fn of(story: &str, i: usize, ground: f32, unit: f32) -> House {
        let seed = (1 << 20) + i;
        let mut seeds = Seeds::of(story, seed);
        let pick = |seeds: &mut Seeds, n: usize| (seeds.next(0.0, n as f32) as usize).min(n - 1);
        let width = seeds.next(34.0, 52.0) * unit;
        let most = 3 + pick(&mut seeds, 5);
        let windows = 2 + pick(&mut seeds, 2);
        let wall = WALLS[pick(&mut seeds, WALLS.len())];
        let roof = (ROOFS[pick(&mut seeds, ROOFS.len())], pick(&mut seeds, 3));
        House { left: 0.0, width, ground, storey: 22.0 * unit, most, floors: 0, building: 0.0, seed, windows, wall, roof, unit }
    }

    /// Floor `f`, counted from the ground.
    pub fn floor(&self, f: usize) -> Rect {
        let bottom = self.ground - f as f32 * self.storey;
        Rect::from_min_max(pos2(self.left, bottom - self.storey), pos2(self.left + self.width, bottom))
    }

    /// Window `w` of floor `f`. The ground floor has a door in the middle, so only its windows
    /// clear of it are there.
    pub fn window(&self, f: usize, w: usize) -> Option<Rect> {
        let floor = self.floor(f);
        let x = floor.left() + floor.width() * (w as f32 + 0.5) / self.windows as f32;
        if f == 0 && self.windows % 2 == 1 && w == self.windows / 2 {
            return None;
        }
        Some(Rect::from_center_size(pos2(x, floor.center().y - self.storey * 0.04), vec2(self.width * 0.16, self.storey * 0.44)))
    }

    /// The colour of window `f`, `w` at `time`: some are lit, flickering as if by candles, and
    /// the rest dark; the same windows for the same story.
    pub fn light_in(&self, story: &str, f: usize, w: usize, time: f64) -> Color32 {
        let mut seeds = Seeds::of(story, self.seed * 1024 + f * 8 + w);
        if seeds.next(0.0, 1.0) > 0.45 {
            return DARK;
        }
        let (phase, speed) = (seeds.next(0.0, 2.0 * PI), seeds.next(0.8, 1.3));
        let t = (time % 1000.0) as f32 * speed;
        let wave = (t * 2.3 + phase).sin() * 0.5 + (t * 5.9 + phase * 1.7).sin() * 0.3 + (t * 12.7 + phase * 2.9).sin() * 0.2;
        GLOW_DIM.lerp_to_gamma(GLOW, 0.7 + 0.3 * wave)
    }

    /// Where the light for its next floor comes from: a window of its top floor, a lit one if it
    /// has one.
    fn source(&self, story: &str) -> Option<Pos2> {
        let f = self.floors.checked_sub(1)?;
        let windows: Vec<(usize, Rect)> = (0..self.windows).filter_map(|w| Some((w, self.window(f, w)?))).collect();
        let lit = windows.iter().find(|(w, _)| self.light_in(story, f, *w, 0.0) != DARK);
        lit.or(windows.first()).map(|(_, r)| r.center())
    }

    /// The house at `time`, and the light of its floor being built, if one is. A house without a
    /// floor yet takes the light for its first from `from`, a window of its neighbour.
    pub fn draw(&self, story: &str, from: Option<Pos2>, time: f64) -> (Vec<Shape>, Option<Rect>) {
        let u = self.unit;
        let mut out = Vec::new();
        let edge = self.wall.lerp_to_gamma(Color32::BLACK, 0.3);
        for f in 0..self.floors {
            let floor = self.floor(f);
            out.push(Shape::rect_filled(floor, 0.0, self.wall));
            out.push(Shape::line_segment([floor.left_top(), floor.right_top()], Stroke::new(1.2 * u, edge)));
            for w in 0..self.windows {
                if let Some(r) = self.window(f, w) {
                    out.extend(window(r, self.light_in(story, f, w, time), u));
                }
            }
        }
        if self.floors > 0 {
            let door = Rect::from_center_size(self.floor(0).center_bottom() - vec2(0.0, self.storey * 0.3), vec2(self.width * 0.2, self.storey * 0.6));
            out.push(Shape::rect_filled(door, egui::CornerRadius { nw: (3.0 * u) as u8, ne: (3.0 * u) as u8, sw: 0, se: 0 }, DOOR));
        }
        let next = self.floor(self.floors);
        let ease = |s: f32| {
            let s = s.clamp(0.0, 1.0);
            s * s * (3.0 - 2.0 * s)
        };
        let (mut light, mut front) = (None, Vec::new());
        let mut roof_at = next.bottom();
        if self.building > 0.0 {
            let start = self.source(story).or(from).unwrap_or(pos2(next.center().x, self.ground));
            if self.building < FLOW {
                // The light flows out of the window and up in an arc to where the floor will be,
                // a trail of light behind it.
                let a = ease(self.building / FLOW);
                let bend = pos2((start.x + next.center().x) / 2.0, start.y.min(next.center().y) - self.storey);
                let at = |a: f32| {
                    let b = 1.0 - a;
                    (start.to_vec2() * b * b + bend.to_vec2() * 2.0 * b * a + next.center().to_vec2() * a * a).to_pos2()
                };
                let r = (1.5 + 3.5 * a) * u;
                for k in (1..=5).rev() {
                    let back = (a - k as f32 * 0.05).max(0.0);
                    front.push(Shape::circle_filled(at(back), r * (1.0 - k as f32 * 0.15), glow(GLOW, 0.6 - k as f32 * 0.1)));
                }
                front.push(Shape::circle_filled(at(a), r * 2.4, glow(GLOW, 0.3)));
                front.push(Shape::circle_filled(at(a), r, LIGHT));
                light = Some(Rect::from_center_size(at(a), Vec2::splat(r * 2.0)));
            } else {
                // It spreads into the floor, lifting the roof, and cools into the wall; its windows
                // come last.
                let b = ease((self.building - FLOW) / (1.0 - FLOW));
                let r = 5.0 * u;
                let spread = Rect::from_min_max(
                    (next.center() - Vec2::splat(r)).lerp(next.min, b),
                    (next.center() + Vec2::splat(r)).lerp(next.max, b),
                );
                roof_at = next.bottom() + (next.top() - next.bottom()) * b;
                light = Some(spread);
            }
        }
        if self.floors > 0 || self.building >= FLOW {
            out.extend(self.roof(roof_at));
        }
        // The light goes in front of the roof it flows past.
        out.extend(front);
        if let (Some(spread), true) = (light, self.building >= FLOW) {
            let b = ease((self.building - FLOW) / (1.0 - FLOW));
            out.push(Shape::rect_filled(spread.expand(3.0 * u), 2.0 * u, glow(GLOW, 0.4 * (1.0 - b))));
            out.push(Shape::rect_filled(spread, 2.0 * u * (1.0 - b), LIGHT.lerp_to_gamma(self.wall, b)));
            if b > 0.6 {
                for w in 0..self.windows {
                    if let Some(r) = self.window(self.floors, w) {
                        let lit = self.light_in(story, self.floors, w, time);
                        out.extend(window(r, self.wall.lerp_to_gamma(lit, (b - 0.6) / 0.4), u));
                    }
                }
            }
        }
        (out, light)
    }

    /// Its roof, sitting at `y`: a pointed one, a tall hat, or a dome, a star at its top.
    fn roof(&self, y: f32) -> Vec<Shape> {
        let u = self.unit;
        let (color, kind) = self.roof;
        let (l, r, mid) = (self.left - 3.0 * u, self.left + self.width + 3.0 * u, self.left + self.width / 2.0);
        let mut out = Vec::new();
        let tip = match kind {
            0 => {
                let tip = pos2(mid, y - self.width * 0.55);
                out.push(Shape::convex_polygon(vec![pos2(l, y), tip, pos2(r, y)], color, Stroke::NONE));
                tip
            }
            1 => {
                let tip = pos2(mid, y - self.width * 0.95);
                let (l, r) = (self.left + self.width * 0.12, self.left + self.width * 0.88);
                out.push(Shape::convex_polygon(vec![pos2(l, y), tip, pos2(r, y)], color, Stroke::NONE));
                tip
            }
            _ => {
                let radius = self.width * 0.42;
                let points = (0..=12).map(|k| pos2(mid, y) + egui::Vec2::angled(PI + PI * k as f32 / 12.0) * radius).collect();
                out.push(Shape::convex_polygon(points, color, Stroke::NONE));
                let tip = pos2(mid, y - radius - 7.0 * u);
                out.push(Shape::line_segment([pos2(mid, y - radius), tip], Stroke::new(1.2 * u, color)));
                tip
            }
        };
        out.push(Shape::circle_filled(tip, 2.0 * u, STAR));
        out
    }
}

/// The houses of a city of `built` floors (the last perhaps in part) on the desk `area`, beside
/// the page lying at `page`: as many as there is room for, taking turns on the left and right of
/// the page, from its edges outwards, and on whether each stands on the left. Each is built up to
/// its `most` before the next is begun; once all are, the next floors go to each in turn, so the
/// houses grow ever higher. A floor once built stays on its house.
pub fn city(story: &str, area: Rect, page: Rect, unit: f32, built: f32) -> Vec<(bool, House)> {
    let gap = 8.0 * unit;
    let gutter = [page.left() - area.left(), area.right() - page.right()];
    // How far out from the page's edge each side is taken.
    let mut used = [gap, gap];
    let mut out: Vec<(bool, House)> = Vec::new();
    let (mut left, mut full) = (true, [false, false]);
    while !(full[0] && full[1]) {
        let side = usize::from(!left);
        let mut h = House::of(story, out.len(), area.bottom(), unit);
        if full[side] || used[side] + h.width + gap > gutter[side] {
            full[side] = true;
        } else {
            h.left = if left { page.left() - used[side] - h.width } else { page.right() + used[side] };
            used[side] += h.width + gap;
            out.push((left, h));
        }
        left = !left;
    }
    let (whole, part) = (built.floor() as usize, built.fract());
    let room: usize = out.iter().map(|(_, h)| h.most).sum();
    let mut next = None;
    let mut rest = whole;
    for (k, (_, h)) in out.iter_mut().enumerate() {
        h.floors = rest.min(h.most);
        rest -= h.floors;
        if next.is_none() && h.floors < h.most {
            next = Some(k);
        }
    }
    if !out.is_empty() && whole >= room {
        // Every house is as high as it gets while there is room: now each grows in turn.
        let (n, over) = (out.len(), whole - room);
        for (k, (_, h)) in out.iter_mut().enumerate() {
            h.floors = h.most + over / n + usize::from(k < over % n);
        }
        next = Some(over % n);
    }
    if let Some(k) = next.filter(|_| part > 0.0) {
        out[k].1.building = part;
    }
    out
}

/// `color`, see-through by `alpha`.
fn glow(color: Color32, alpha: f32) -> Color32 {
    let [r, g, b, _] = color.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// A window at `r` of `color`, a soft glow about it when it is lit.
fn window(r: Rect, color: Color32, u: f32) -> Vec<Shape> {
    let mut out = Vec::new();
    if color != DARK {
        out.push(Shape::rect_filled(r.expand(2.0 * u), 2.0 * u, glow(color, 0.18)));
    }
    out.push(Shape::rect_filled(r, 1.0 * u, color));
    out
}

impl App {
    /// The garden kept with the story (`Doc::garden`) grown to the pages each chapter has now, if
    /// it has more: in every theme, so the Magic Garden shows all the story's growth. It never
    /// shrinks, and keeps a plant for a chapter taken out. Measured again only when the text or the
    /// pages have changed.
    pub fn grow_garden(&mut self) {
        let now = (self.doc.version, self.doc.spans.len());
        if self.eggs.measured.as_ref().is_some_and(|(id, v, n)| *id == self.story_id && (*v, *n) == now) {
            return;
        }
        self.eggs.measured = Some((self.story_id.clone(), now.0, now.1));
        // A plant a chapter, as many pages as it has; a story without chapters grows one.
        let starts: Vec<usize> = self.doc.chapters(false).iter().map(|e| self.doc.page_of(e.at)).collect();
        let pages = self.doc.pages();
        let mut sizes: Vec<usize> = (0..starts.len()).map(|k| starts.get(k + 1).copied().unwrap_or(pages).saturating_sub(starts[k]).max(1)).collect();
        if sizes.is_empty() {
            sizes.push(pages);
        }
        let garden = &mut self.doc.garden;
        if garden.len() < sizes.len() {
            garden.resize(sizes.len(), 0);
        }
        for (g, n) in garden.iter_mut().zip(sizes) {
            *g = (*g).max(n as u32);
        }
    }

    /// The story `id` comes in with `garden` in place of the one open: if it is the same story (a
    /// version of it, or its file again), it keeps the plants it has grown since.
    pub fn keep_garden(&self, id: &str, garden: &mut Vec<u32>) {
        if self.eggs.measured.as_ref().is_none_or(|(was, _, _)| was != id) {
            return;
        }
        if garden.len() < self.doc.garden.len() {
            garden.resize(self.doc.garden.len(), 0);
        }
        for (g, &had) in garden.iter_mut().zip(&self.doc.garden) {
            *g = (*g).max(had);
        }
    }

    /// The garden on the desk, behind the pages lying at `page`, in the Magic Garden only.
    pub fn draw_garden(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        self.eggs.plants.clear();
        if self.themes.theme != Theme::Garden {
            return;
        }
        let now = ui.ctx().input(|i| i.time);
        // As tall as the most pages each has had (`grow_garden`).
        let targets: Vec<f32> = self.doc.garden.iter().map(|&n| height_of(n as usize)).collect();
        let chapters = self.doc.chapters(false).len().max(1);
        let e = &mut self.eggs;
        if e.grown_for.as_deref() != Some(&self.story_id) {
            // A story just opened comes with its garden grown.
            (e.grown_for, e.grown) = (Some(self.story_id.clone()), targets.clone());
        }
        e.grown.resize(targets.len(), 0.0);
        // No more than a tenth of a second at a time, so a plant grows to its new height in sight.
        let dt = (now - e.grown_at).clamp(0.0, 0.1) as f32;
        e.grown_at = now;
        let mut growing = false;
        for (g, &t) in e.grown.iter_mut().zip(&targets) {
            if *g < t {
                *g = (*g + GROW * dt).min(t);
                growing = true;
            }
        }
        if growing {
            ui.ctx().request_repaint();
        }

        let unit = (area.height() / 900.0).clamp(0.6, 1.6);
        let painter = ui.painter();
        painter.extend(grass(&self.story_id, area, unit));
        let n = e.grown.len();
        for (k, &g) in e.grown.iter().enumerate() {
            // Taking turns on either side of the page, from its edge outwards.
            let left = k % 2 == 0;
            let (j, on_side) = (k / 2, if left { n.div_ceil(2) } else { n / 2 });
            let gutter = if left { page.left() - area.left() } else { area.right() - page.right() };
            let gap = (70.0 * unit).min(gutter / (on_side as f32 + 0.5)).max(12.0);
            let x = if left { page.left() - gap * (j as f32 + 0.6) } else { page.right() + gap * (j as f32 + 0.6) };
            let height = (g * TALLEST * area.height()).max(4.0 * unit);
            let root = pos2(x, area.bottom());
            // A chapter flowers once the next one has begun; the plant of one taken out has too.
            let flower = k + 1 != chapters;
            painter.extend(plant(&self.story_id, k, root, height, flower, unit));
            e.plants.push((Rect::from_min_max(pos2(x, root.y - height), root), flower));
        }
    }

    /// The city on the desk, behind the pages lying at `page`, in the Magic City only, with a floor
    /// for each page each chapter has had (`Doc::garden`, so it never comes down either).
    pub fn draw_city(&mut self, ui: &egui::Ui, area: Rect, page: Rect) {
        (self.eggs.houses, self.eggs.light) = (Vec::new(), None);
        if self.themes.theme != Theme::City {
            return;
        }
        let now = ui.ctx().input(|i| i.time);
        let target = self.doc.garden.iter().sum::<u32>() as f32;
        let e = &mut self.eggs;
        if e.built_for.as_deref() != Some(&self.story_id) {
            // A story just opened comes with its city built.
            (e.built_for, e.built) = (Some(self.story_id.clone()), target);
        }
        let dt = (now - e.built_at).clamp(0.0, 0.1) as f32;
        e.built_at = now;
        // Lit windows flicker: a candle needs no more than this.
        if e.built < target {
            // A floor at a time, quicker when many are waiting.
            e.built = (e.built + (target - e.built).max(3.0) / 3.0 / BUILD * dt).min(target);
            ui.ctx().request_repaint();
        } else {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
        }

        let unit = (area.height() / 900.0).clamp(0.6, 1.6);
        let story = &self.story_id;
        let houses = city(story, area, page, unit, e.built);
        let painter = ui.painter();
        for (i, (left, h)) in houses.iter().enumerate() {
            if h.floors == 0 && h.building == 0.0 {
                continue;
            }
            // A house begun takes its light from the window of its neighbour nearer the page.
            let from = houses[..i].iter().rev().find(|(l, _)| l == left).and_then(|(_, n)| n.source(story));
            let (shapes, light) = h.draw(story, from, now);
            painter.extend(shapes);
            let top = h.floor(h.floors).bottom();
            e.houses.push((Rect::from_min_max(pos2(h.left, top), pos2(h.left + h.width, h.ground)), h.floors));
            e.light = e.light.or(light);
        }
    }

    /// The paragraph at the caret was just made a chapter title: now and then, in a magic theme,
    /// the crane or the balloon comes to bring it into place.
    pub fn maybe_lift_title(&mut self, ctx: &egui::Context) {
        let Some(mut by) = Carrier::of(self.themes.theme) else { return };
        let (at, _) = self.doc.para_start(self.selection().0);
        // Only a title with words in it: an empty one is no load.
        if !self.doc.chapters(false).iter().any(|e| e.at == at) || self.eggs.roll() >= self.eggs.odds {
            return;
        }
        if let Carrier::Balloon { right } = &mut by {
            self.eggs.balloon_right = !self.eggs.balloon_right;
            *right = self.eggs.balloon_right;
        }
        let started = ctx.input(|i| i.time);
        self.eggs.lift = Some(Lift { by, at, page: self.doc.page_of(at), started, duck: None });
        ctx.request_repaint();
    }

    /// Once a frame: the crane or the balloon hurries away when the writer goes on, and is gone when it is done or
    /// the page is turned.
    pub fn update_lift(&mut self, ctx: &egui::Context) {
        let Some(lift) = &mut self.eggs.lift else { return };
        let now = ctx.input(|i| i.time);
        let busy = ctx.input(|i| {
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::PointerButton { .. } | egui::Event::Paste(_) | egui::Event::MouseWheel { .. })
            })
        });
        if busy && lift.duck.is_none() {
            lift.duck = Some(now);
        }
        let p = pose((now - lift.started) as f32, lift.duck.map(|d| (d - lift.started) as f32));
        if p.done || self.target != lift.page || lift.at > self.doc.total_chars() {
            self.eggs.lift = None;
        } else {
            ctx.request_repaint();
        }
    }

    /// What brings the title, where it is now, and the range of page `i`'s text (page-local) it is lifting.
    pub fn lifting(&self, ctx: &egui::Context, i: usize) -> Option<(Carrier, Pose, std::ops::Range<usize>)> {
        let lift = self.eggs.lift.filter(|l| l.page == i)?;
        let now = ctx.input(|inp| inp.time);
        let p = pose((now - lift.started) as f32, lift.duck.map(|d| (d - lift.started) as f32));
        let start = self.doc.spans[i].start;
        let (end, _) = self.doc.term_from(lift.at, self.doc.char_to_byte(lift.at));
        Some((lift.by, p, lift.at.saturating_sub(start)..end.saturating_sub(start)))
    }

    /// The crane or the balloon over page `i` lying at `page`, if it is at work there.
    pub fn draw_carrier(&self, ui: &egui::Ui, page: Rect, i: usize, area: Rect) {
        let (Some((by, p, _)), Some(place)) = (self.lifting(ui.ctx(), i), self.eggs.place) else { return };
        ui.painter().extend(match by {
            Carrier::Crane => crane(p, place, page, area.bottom()),
            by @ Carrier::Balloon { .. } => balloon(by, p, place, page),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crane_drives_in_runs_the_title_out_lowers_it_and_drives_away() {
        let at = |t: f32| pose(t, None);
        assert!(at(0.0).enter == 0.0 && at(0.0).hooked && at(0.0).lower == 0.0, "away, the title on its hook, hanging high");
        assert!(at(ENTER).enter == 1.0 && at(ENTER).run == 0.0, "beside the page");
        let over = ENTER + RUN;
        assert!(at(over).run == 1.0 && at(over).lower == 0.0, "over its place");
        let down = over + LOWER;
        assert!(at(down - 0.01).lower > 0.9 && at(down - 0.01).hooked, "lowered");
        assert!(at(down + SETTLE / 2.0).lower < 1.0, "it bounces as it touches down");
        let free = down + SETTLE + UNHOOK;
        assert!(!at(free).hooked && at(free).hook == 1.0 && at(free).enter == 1.0, "the hook lifted off it");
        assert!(at(free + LEAVE / 2.0).enter < 1.0 && at(free + LEAVE).enter == 0.0, "driven away");
        assert!(at(free + LEAVE).done);

        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 120.0), vec2(200.0, 30.0));
        assert!(offset(Carrier::Crane, at(0.0), place, page).x > page.width(), "the title comes with the crane, from far to the side");
        assert!(offset(Carrier::Crane, at(over), place, page).x.abs() < 0.01 && offset(Carrier::Crane, at(over), place, page).y < -50.0, "above its place");
        assert_eq!(offset(Carrier::Crane, at(free), place, page), Vec2::ZERO, "in its place");
    }

    #[test]
    fn the_balloon_floats_the_title_in_from_the_side_and_away_past_the_other() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        for right in [true, false] {
            let by = Carrier::Balloon { right };
            let side = if right { 1.0 } else { -1.0 };
            let at = |t: f32| offset(by, pose(t, None), place, page);
            let start = at(0.0);
            assert!(start.x * side > 400.0 && start.y.abs() < 100.0, "{right}: from the side, near the title's height: {start:?}");
            let over = at(ENTER + RUN);
            assert!(over.x.abs() < 0.01 && over.y < -20.0, "{right}: a little above its place: {over:?}");
            assert_eq!(at(ENTER + RUN + LOWER + SETTLE + UNHOOK), Vec2::ZERO, "{right}: let go in its place");
            let p = pose(ENTER + RUN + LOWER + SETTLE + UNHOOK + LEAVE / 2.0, None);
            let going = rig(by, p, place, page).hook;
            assert!((going.x - place.center().x) * side < -100.0, "{right}: away past the other side: {going:?}");
        }
        // A title high on its page: the balloon hangs lower and is smaller, its top still near the page.
        let high = Rect::from_min_size(pos2(500.0, 120.0), vec2(200.0, 30.0));
        let r = rig(Carrier::Balloon { right: true }, pose(ENTER + RUN, None), high, page);
        assert!(r.hook.y - r.size * TALL >= page.top() - page.height() * 0.05, "{:?} {}", r.hook, r.size);
    }

    #[test]
    fn the_balloon_s_stripes_take_turns_in_colour_all_the_way_across() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let by = Carrier::Balloon { right: true };
        let p = pose(ENTER + RUN, None);
        let r = rig(by, p, place, page);
        let silk: Vec<_> = balloon(by, p, place, page).into_iter().filter_map(|s| match s {
            Shape::Path(path) if SILK.contains(&path.fill) => Some(path),
            _ => None,
        }).collect();
        let inside = |points: &[Pos2], q: Pos2| {
            let side = |i: usize| {
                let (e, d) = (points[(i + 1) % points.len()] - points[i], q - points[i]);
                e.x * d.y - e.y * d.x
            };
            let sides: Vec<f32> = (0..points.len()).map(side).collect();
            sides.iter().all(|&c| c >= 0.0) || sides.iter().all(|&c| c <= 0.0)
        };
        // Across the silk low down, where it tapers to its mouth, and across its widest.
        let mouth = r.hook.y - r.size * 0.92;
        for y in [mouth - r.size * 0.3, mouth - r.size] {
            let mut met: Vec<usize> = Vec::new();
            for i in 0..=400 {
                let q = pos2(r.hook.x - r.size * 1.2 + r.size * 2.4 * i as f32 / 400.0, y);
                if let Some(k) = silk.iter().rposition(|s| inside(&s.points, q)) && met.last() != Some(&k) {
                    met.push(k);
                }
            }
            assert!(met.len() >= 4, "{y}: crossed {met:?}");
            for w in met.windows(2) {
                assert_ne!(silk[w[0]].fill, silk[w[1]].fill, "{y}: two stripes of one colour side by side: {met:?}");
            }
        }
    }

    #[test]
    fn the_crane_sets_the_title_down_at_once_when_the_writer_goes_on() {
        let p = pose(1.0, Some(0.9));
        assert!(!p.hooked && p.lower == 1.0, "the title in its place");
        assert!(p.enter > 0.0 && !p.done, "the crane on its way out");
        assert!(pose(0.9 + HURRY + 0.01, Some(0.9)).done);
        // After it set the title down, going on changes nothing.
        let late = ENTER + RUN + LOWER + SETTLE + UNHOOK + 0.1;
        assert_eq!(pose(late + 0.1, Some(late)), pose(late + 0.1, None));
    }

    #[test]
    fn a_new_floor_is_a_light_that_flows_out_of_a_window_and_spreads_into_the_floor() {
        let mut h = House::of("a", 0, 900.0, 1.0);
        (h.left, h.floors) = (100.0, 2);
        let window = h.source("a").unwrap();
        assert!(h.floor(1).contains(window), "a window of the top floor");
        let next = h.floor(2);
        let light = |s: f32| {
            let mut h = h.clone();
            h.building = s;
            h.draw("a", None, 0.0).1
        };
        assert!(light(0.0).is_none(), "nothing being built");
        assert!(light(0.01).unwrap().center().distance(window) < 1.0, "out of the window");
        let flown = light(FLOW - 0.001).unwrap();
        assert!(flown.center().distance(next.center()) < 1.0 && flown.width() < next.width() / 2.0, "a little light where the floor will be");
        let spread = light(0.999).unwrap();
        assert!(spread.min.distance(next.min) < 0.5 && spread.max.distance(next.max) < 0.5, "spread into the floor: {spread:?} {next:?}");
        // A house's first floor takes its light from its neighbour's window.
        let mut first = House::of("a", 1, 900.0, 1.0);
        first.building = 0.01;
        let from = pos2(50.0, 850.0);
        assert!(first.draw("a", Some(from), 0.0).1.unwrap().center().distance(from) < 1.0);
    }

    #[test]
    fn the_houses_take_turns_on_either_side_of_the_page_as_many_as_there_is_room_for() {
        let (area, page) = (Rect::from_min_size(Pos2::ZERO, vec2(1240.0, 900.0)), Rect::from_min_size(pos2(320.0, 40.0), vec2(600.0, 820.0)));
        let houses = city("a", area, page, 1.0, 1000.0);
        assert!(houses.len() >= 8, "{}", houses.len());
        assert!(houses[0].0 && !houses[1].0 && houses[2].0, "left, right, left");
        for (k, (left, h)) in houses.iter().enumerate() {
            let r = h.floor(0);
            assert!(area.contains_rect(r), "{k}: on the desk: {r:?}");
            assert!(if *left { r.right() < page.left() } else { r.left() > page.right() }, "{k}: beside the page on its side: {r:?}");
            for (_, o) in &houses[..k] {
                assert!(!r.intersects(o.floor(0)), "{k}: clear of the others");
            }
        }
        // The first house alone while it has room for its floors.
        let small = city("a", area, page, 1.0, 2.0);
        assert_eq!(small.iter().map(|(_, h)| h.floors).collect::<Vec<_>>()[..2], [2, 0]);
    }

    #[test]
    fn once_the_desk_is_full_the_houses_grow_higher_a_floor_each_in_turn() {
        let (area, page) = (Rect::from_min_size(Pos2::ZERO, vec2(1240.0, 900.0)), Rect::from_min_size(pos2(320.0, 40.0), vec2(600.0, 820.0)));
        let floors = |n: f32| city("a", area, page, 1.0, n).into_iter().map(|(_, h)| h.floors).collect::<Vec<_>>();
        let most: Vec<usize> = city("a", area, page, 1.0, 0.0).into_iter().map(|(_, h)| h.most).collect();
        let room: usize = most.iter().sum();
        assert_eq!(floors(room as f32), most, "each as high as it gets while there is room");
        let n = most.len();
        let high = floors((room + 3 * n) as f32);
        assert!(high.iter().zip(&most).all(|(f, m)| *f == m + 3), "and then higher: {high:?}");
        for k in 0..room + 2 * n {
            let (was, now) = (floors(k as f32), floors(k as f32 + 1.0));
            let grew: Vec<usize> = (0..n).filter(|&i| now[i] != was[i]).collect();
            assert!(grew.len() == 1 && now[grew[0]] == was[grew[0]] + 1, "{k}: one floor on one house, the rest stay: {was:?} {now:?}");
            let half = city("a", area, page, 1.0, k as f32 + 0.5);
            assert!(half[grew[0]].1.building == 0.5, "{k}: the floor being built is on that house");
        }
    }

    #[test]
    fn some_windows_are_lit_and_flicker_the_same_ones_for_the_same_story() {
        let h = House::of("a", 0, 900.0, 1.0);
        let windows: Vec<(usize, usize)> = (0..8).flat_map(|f| (0..3).map(move |w| (f, w))).collect();
        let lit: Vec<(usize, usize)> = windows.iter().copied().filter(|&(f, w)| h.light_in("a", f, w, 0.0) != DARK).collect();
        assert!(!lit.is_empty() && lit.len() < windows.len(), "some lit, some dark: {lit:?}");
        for (f, w) in lit {
            let seen: Vec<Color32> = (0..20).map(|t| h.light_in("a", f, w, t as f64 * 0.1)).collect();
            assert!(seen.iter().all(|c| c.r() > 200 && *c != DARK), "lit all the while: {seen:?}");
            assert!(seen.windows(2).filter(|p| p[0] != p[1]).count() > 10, "it flickers: {seen:?}");
            assert_eq!(h.light_in("a", f, w, 0.5), House::of("a", 0, 900.0, 1.0).light_in("a", f, w, 0.5));
        }
    }

    #[test]
    fn a_plant_is_the_same_for_the_same_story_and_grows_only_at_its_top() {
        let shapes = |story: &str, h: f32| format!("{:?}", plant(story, 0, pos2(0.0, 900.0), h, false, 1.0));
        assert_eq!(shapes("a", 200.0), shapes("a", 200.0));
        assert_ne!(shapes("a", 200.0), shapes("b", 200.0), "another story grows another plant");
        // The leaves low on a plant stay where they were as it grows.
        let leaves = |h: f32| plant("a", 0, pos2(0.0, 900.0), h, false, 1.0).into_iter().filter(|s| matches!(s, Shape::Path(_))).skip(1).take(2).map(|s| format!("{s:?}")).collect::<Vec<_>>();
        assert_eq!(leaves(200.0), leaves(400.0));
        assert!(height_of(1) < height_of(4) && height_of(40) < 1.0, "taller with more pages, never past its tallest");
    }
}
