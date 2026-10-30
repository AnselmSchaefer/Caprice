//! Little things hidden in the magic themes. In the Magic Garden a garden grows up from the bottom
//! of the desk, a plant for each chapter, taller the more pages it has, and flowering once the next
//! chapter has begun. In the Magic City a street of little houses stands there instead, a floor for
//! each page: a new floor is a light that flows out of a window and spreads into the floor. Some
//! windows are lit, a few of them flickering slightly; some houses have a little tower with a
//! pennant, a statue on the roof, balconies or arched windows. When the writer makes a paragraph a
//! chapter title, it is brought in: in the garden now and then a hot-air balloon floats down with
//! it hanging below its basket, sets it in place and rises away; in the Magic City every time a
//! dragon flies in with it held in its feet, flapping
//! its wings and breathing fire, looks round at the city, at the writer and at the page, sets the
//! title in place and flies away. Both follow one course in time (`pose`), each along its own way
//! (`rig`).
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
    /// In the Magic City, and in the Magic Garden; each from the right side or the left, the next
    /// from the other side.
    Dragon { right: bool },
    Balloon { right: bool },
}

impl Carrier {
    pub fn of(theme: Theme) -> Option<Carrier> {
        match theme {
            Theme::City => Some(Carrier::Dragon { right: true }),
            Theme::Garden => Some(Carrier::Balloon { right: true }),
            _ => None,
        }
    }

    /// How many times slower than the balloon it goes its course: the dragon is big and takes its
    /// time, so it can be seen.
    pub fn pace(self) -> f32 {
        match self {
            Carrier::Dragon { .. } => 1.7,
            Carrier::Balloon { .. } => 1.0,
        }
    }

    /// The chance that making a chapter title calls it: the dragon comes for every new chapter,
    /// the balloon now and then.
    pub fn odds(self) -> f64 {
        match self {
            Carrier::Dragon { .. } => 1.0,
            Carrier::Balloon { .. } => ODDS,
        }
    }
}

/// How tall a plant grows at most, in shares of the desk's height, and how many pages it takes to
/// grow most of the way: a chapter of `PAGES` pages is two thirds of the way up.
const TALLEST: f32 = 0.55;
const PAGES: f32 = 8.0;
/// How fast a plant grows to its height, in shares of its tallest a second.
const GROW: f32 = 0.5;
/// The chance that making a chapter title calls the balloon.
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

/// The dragon: its scales, lighter along its back and darker in its shadows, its belly and the
/// plates across it, its wings' skin in two shades and their bones, its claws, horns and teeth,
/// its eyes, the inside of its mouth, its fire from the outside in, and its smoke.
const SCALES: Color32 = Color32::from_rgb(58, 146, 92);
const SCALES_LIGHT: Color32 = Color32::from_rgb(96, 178, 116);
const SCALES_SHADE: Color32 = Color32::from_rgb(46, 124, 78);
const SCALES_DARK: Color32 = Color32::from_rgb(32, 92, 60);
const BELLY: Color32 = Color32::from_rgb(232, 200, 118);
const BELLY_DARK: Color32 = Color32::from_rgb(196, 160, 86);
const WING: Color32 = Color32::from_rgb(112, 178, 120);
const WING_SHADE: Color32 = Color32::from_rgb(92, 158, 104);
const WING_FAR: Color32 = Color32::from_rgb(44, 108, 72);
const BONE: Color32 = Color32::from_rgb(40, 100, 66);
pub const CLAW: Color32 = Color32::from_rgb(246, 236, 210);
const HORN: Color32 = Color32::from_rgb(214, 184, 120);
pub const EYE: Color32 = Color32::from_rgb(255, 210, 60);
const PUPIL: Color32 = Color32::from_rgb(30, 24, 20);
const MOUTH: Color32 = Color32::from_rgb(120, 30, 36);
pub const FIRE: [Color32; 4] = [
    Color32::from_rgb(214, 56, 32),
    Color32::from_rgb(244, 118, 36),
    Color32::from_rgb(252, 182, 58),
    Color32::from_rgb(255, 238, 160),
];
/// How tall the dragon is from the title it holds to the tips of its wings, risen on a beat, in its sizes; and
/// the seconds one beat of its wings takes, and one breath of fire.
const DRAGON_TALL: f32 = 3.8;
pub const FLAP: f32 = 0.8;
const BREATH: f32 = 0.9;
/// Seconds a slow wave takes to run down the dragon from its neck to its tail.
const GLIDE: f32 = 2.4;

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
/// The statues on the roofs, of stone or gilded, and the pennants on the towers and their poles.
const STONE: Color32 = Color32::from_rgb(198, 192, 216);
const GILT: Color32 = Color32::from_rgb(234, 190, 90);
const PENNANT: Color32 = Color32::from_rgb(232, 76, 84);
const POLE: Color32 = Color32::from_rgb(70, 70, 82);
/// Seconds a new floor takes: its light flows out of a window for the first `FLOW` of it, and
/// spreads into the floor for the rest.
pub const BUILD: f32 = 1.6;
pub const FLOW: f32 = 0.45;

/// Seconds the dragon or the balloon takes to come in from the side, carry the title over its
/// place, lower it, settle it, let go and go away; and to go away when the writer goes on
/// meanwhile.
const ENTER: f32 = 0.8;
const RUN: f32 = 0.7;
const LOWER: f32 = 0.9;
const SETTLE: f32 = 0.25;
const UNHOOK: f32 = 0.25;
const LEAVE: f32 = 0.9;
/// Seconds at the balloon's pace until it is over the title's place, and until it is gone; for
/// tests, to wait as long.
#[cfg(test)]
pub const OVER: f32 = ENTER + RUN;
#[cfg(test)]
pub const COURSE: f32 = ENTER + RUN + LOWER + SETTLE + UNHOOK + LEAVE;
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
    /// The dragon or the balloon at work, if one is.
    pub lift: Option<Lift>,
    /// Where the title being lifted lies on the screen, from the last page drawn; and the desk
    /// the pages lie on, which what brings it comes in from beyond and leaves past.
    pub place: Option<Rect>,
    pub desk: Rect,
    /// The chance that making a chapter title calls the dragon or the balloon, if not theirs
    /// (`Carrier::odds`); tests set it to 0 or 1.
    pub odds: Option<f64>,
    /// The side the last dragon or balloon came from: the next comes from the other.
    from_right: bool,
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

/// The dragon or the balloon bringing the chapter title that begins at `at` into place on page `page`.
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
        Self { grown_for: None, grown: Vec::new(), grown_at: 0.0, measured: None, lift: None, place: None, desk: Rect::NOTHING, odds: None, from_right: false, rng: seed | 1, plants: Vec::new(),
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

/// Where the dragon (or the balloon) is in its work at `t` seconds, the writer having gone on at `duck`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    /// 0: away beyond the side of the window, 1: beside the page.
    pub enter: f32,
    /// 0: beside the page, 1: over the title's place.
    pub run: f32,
    /// 0: the title hanging high, 1: in its place.
    pub lower: f32,
    /// Is the title still held?
    pub hooked: bool,
    /// 0: just let go of the title, 1: risen clear of it.
    pub hook: f32,
    pub done: bool,
    /// Where the dragon looks: 0 at the city, 1 at the writer, 2 at the page, 3 ahead on its way
    /// out, and between while it turns its head.
    pub look: f32,
    /// The seconds since it was called, for the beat of its wings and its fire.
    pub t: f32,
}

/// Where carrier `by` is at `t` seconds: along the course at its own pace, its wings, fire and
/// tail at the seconds as they are. The dragon goes on by its wings: each down beat pushes it
/// along, it slows while they come up, and glides on evenly.
pub fn pose_of(by: Carrier, t: f32, duck: Option<f32>) -> Pose {
    let k = by.pace();
    let along = |t: f32| match by {
        Carrier::Dragon { .. } => (t + 0.8 * surge(t)) / k,
        Carrier::Balloon { .. } => t / k,
    };
    Pose { t, ..pose(along(t), duck.map(along)) }
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
        // The title is set down at once, and the carrier hurries away from wherever it was.
        let was = pose(d, None).enter;
        let enter = was * (1.0 - ((t - d) / HURRY).clamp(0.0, 1.0));
        return Pose { enter, run: 1.0, lower: 1.0, hooked: false, hook: 1.0, done: t - d >= HURRY, look: 3.0, t };
    }
    // At the city while it comes, at the writer while it is over the title's place, at the page
    // while it lowers the title, and ahead once it lets go.
    let keys = [
        (0.0, 0.0),
        (run_at + RUN * 0.2, 0.0),
        (run_at + RUN * 0.55, 1.0),
        (lower_at + LOWER * 0.45, 1.0),
        (lower_at + LOWER * 0.75, 2.0),
        (unhook_at, 2.0),
        (leave_at, 3.0),
    ];
    let look = keys.windows(2).find(|w| t < w[1].0).map_or(3.0, |w| w[0].1 + (w[1].1 - w[0].1) * ease((t - w[0].0) / (w[1].0 - w[0].0)));
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
        look,
        t,
    }
}

/// Where the balloon's basket is, or the dragon's grip, for a title lying at `place`, on a page
/// lying at `page`.
struct Rig {
    /// The bottom of the balloon's basket, or the middle of the title's top, where the dragon holds it.
    hook: Pos2,
    /// The balloon's size, the radius of its silk; or the dragon's, the height of its body.
    size: f32,
    /// How far the title is from its place.
    offset: Vec2,
}

fn rig(by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect) -> Rig {
    // In from the side at the title's height, a little above it; let go, it goes on and away past
    // the other side. A title is often high on its page, so the carrier hangs only as high as
    // there is room for below the top of the page, and is smaller there.
    // It comes in from beyond the desk's edge and leaves past the other, far enough that none of
    // it shows there: its reach, in its sizes, from where it holds the title to its furthest
    // part (the balloon's silk; the dragon's tail behind, its head and fire ahead), and coming,
    // the title it carries.
    let (right, tall, rope, sizes, reach) = match by {
        Carrier::Balloon { right } => (right, TALL, page.height() * 0.02, (0.035, 0.06), 1.5),
        Carrier::Dragon { right } => (right, DRAGON_TALL, 0.0, (0.045, 0.1), 5.0),
    };
    let side = if right { 1.0 } else { -1.0 };
    let away = 1.0 - p.enter;
    let room = place.top() - (page.top() - page.height() * 0.03) - rope;
    let size = (room / tall).clamp(page.width() * sizes.0, page.width() * sizes.1);
    let hang = (room - size * tall).clamp(0.0, page.height() * 0.08);
    // The balloon bobs as it floats over; the dragon rises and sinks with its wings, the title
    // with it, less and less as it lowers the title into place.
    let heave = match by {
        Carrier::Balloon { .. } => 0.0,
        Carrier::Dragon { .. } => heave(p.t) * size,
    };
    // How far beyond the title's middle the desk's edge is on the side it comes from, and on the
    // side it leaves by; the page's edges when there is no desk (in tests).
    let desk = if desk.is_positive() { desk } else { page };
    let edge = |side: f32| if side > 0.0 { desk.right() - place.center().x } else { place.center().x - desk.left() };
    let come = (edge(side) + reach * size + place.width() / 2.0 - page.width() * 0.35).max(page.width() * 0.6);
    let go = (edge(-side) + reach * size).max(page.width() * 0.7);
    let (hook, offset) = if p.hooked {
        let x = place.center().x + side * ((1.0 - p.run) * page.width() * 0.35 + away * come);
        let bob = match by {
            Carrier::Balloon { .. } => (p.run * PI).sin() * page.height() * 0.015,
            Carrier::Dragon { .. } => heave * (1.0 - p.lower),
        };
        let load = pos2(x, place.center().y - hang * (1.0 - p.lower) - bob);
        (load - vec2(0.0, place.height() / 2.0 + rope), load - place.center())
    } else {
        let up = rope + page.height() * 0.04 * p.hook;
        let up = up + heave * p.hook;
        (place.center_top() - vec2(0.0, up) + vec2(-side * go, -0.1 * page.height()) * away, Vec2::ZERO)
    };
    Rig { hook, size, offset }
}

/// How far the title is from its place at `p`, brought `by` that.
pub fn offset(by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect) -> Vec2 {
    rig(by, p, place, page, desk).offset
}

/// An oval around `c`, `a` long along `along` and `b` across it: convex.
fn oval(c: Pos2, along: Vec2, a: f32, b: f32, color: Color32) -> Shape {
    let across = vec2(-along.y, along.x);
    let points = (0..20).map(|k| {
        let r = k as f32 / 20.0 * 2.0 * PI;
        c + along * a * r.cos() + across * b * r.sin()
    });
    Shape::convex_polygon(points.collect(), color, Stroke::NONE)
}

/// A tongue of fire from `at` along `dir`, `len` long: a lens, which is convex.
fn flame(at: Pos2, dir: Vec2, len: f32, width: f32, color: Color32) -> Shape {
    let side = vec2(-dir.y, dir.x) * width;
    let points = (0..=16).map(|k| {
        let (s, w) = if k <= 8 { (k as f32 / 8.0, 1.0) } else { (2.0 - k as f32 / 8.0, -1.0) };
        at + dir * len * s + side * w * (s * PI).sin()
    });
    Shape::convex_polygon(points.collect(), color, Stroke::NONE)
}

/// The smallest convex shape around `points`, so that anything can be filled (R11).
fn hull(mut points: Vec<Pos2>, color: Color32) -> Shape {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let turn = |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut out: Vec<Pos2> = Vec::new();
    for pass in 0..2 {
        let start = out.len();
        let mut walk = |q: Pos2| {
            while out.len() >= start + 2 && turn(out[out.len() - 2], out[out.len() - 1], q) <= 0.0 {
                out.pop();
            }
            out.push(q);
        };
        if pass == 0 { points.iter().for_each(|&q| walk(q)) } else { points.iter().rev().for_each(|&q| walk(q)) }
        out.pop();
    }
    Shape::convex_polygon(out, color, Stroke::NONE)
}

/// A body tapering along `spine`, `widths` thick at its points: a convex piece for each stretch
/// and a round at each joint, so it bends smoothly.
fn ribbon(spine: &[Pos2], widths: &[f32], color: Color32) -> Vec<Shape> {
    let mut out = Vec::new();
    for (k, w) in spine.windows(2).enumerate() {
        let n = (w[1] - w[0]).normalized().rot90();
        let (a, b) = (widths[k] / 2.0, widths[k + 1] / 2.0);
        out.push(Shape::convex_polygon(vec![w[0] + n * a, w[1] + n * b, w[1] - n * b, w[0] - n * a], color, Stroke::NONE));
    }
    out.extend(spine.iter().zip(widths).map(|(&p, &w)| Shape::circle_filled(p, w / 2.0, color)));
    out
}

/// A point along the curve from `a` to `b` pulled towards `c`, at `s` from 0 to 1.
fn bend(a: Pos2, c: Pos2, b: Pos2, s: f32) -> Pos2 {
    let m = |p: Pos2, q: Pos2| p + (q - p) * s;
    m(m(a, c), m(c, b))
}

/// The head turned `yaw` from facing the writer (a quarter turn is in profile, to the right when
/// it is positive) and `pitch` down: a point `[forward, up, right]` of it, in head sizes, comes
/// out on the screen at `x`, `y`, and `z` towards the writer.
#[derive(Clone, Copy)]
struct Turn {
    forward: [f32; 3],
    up: [f32; 3],
    right: [f32; 3],
}

impl Turn {
    fn new(yaw: f32, pitch: f32) -> Turn {
        let forward = [yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()];
        // Up on the screen is -y, made square to where the head points.
        let lean = -forward[1];
        let up = [-lean * forward[0], -1.0 - lean * forward[1], -lean * forward[2]];
        let len = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
        let up = up.map(|c| c / len);
        let right = [
            up[1] * forward[2] - up[2] * forward[1],
            up[2] * forward[0] - up[0] * forward[2],
            up[0] * forward[1] - up[1] * forward[0],
        ];
        Turn { forward, up, right }
    }

    fn at(&self, p: [f32; 3]) -> [f32; 3] {
        let axes = [self.forward, self.up, self.right];
        [0, 1, 2].map(|i| axes.iter().zip(p).map(|(a, c)| a[i] * c).sum())
    }

    /// Where the snout points on the screen, and how long it looks there: 0 pointing at the writer.
    fn aim(&self) -> (Vec2, f32) {
        let v = vec2(self.forward[0], self.forward[1]);
        (v.normalized(), v.length())
    }
}

/// Points round an egg of the head: around `c`, `r` across in each of its directions, its long
/// axis tipped by `tilt` from forward to up.
fn egg(c: [f32; 3], r: [f32; 3], tilt: f32) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for i in 0..7 {
        let lat = PI * (i as f32 / 6.0 - 0.5);
        for j in 0..12 {
            let lon = 2.0 * PI * j as f32 / 12.0;
            let (a, b) = (r[0] * lat.cos() * lon.cos(), r[1] * lat.sin());
            let (a, b) = (a * tilt.cos() - b * tilt.sin(), a * tilt.sin() + b * tilt.cos());
            out.push([c[0] + a, c[1] + b, c[2] + r[2] * lat.cos() * lon.sin()]);
        }
    }
    out
}

/// Where the dragon is, and where it looks, at `p`.
pub struct Dragon {
    /// The middle of its body, and its size: the height of its body.
    pub body: Pos2,
    pub size: f32,
    /// 1 when it flies to the right, -1 to the left.
    pub ahead: f32,
    pub head: Pos2,
    /// Where its snout points on the screen, and how far it is turned away from the writer: 0
    /// looking at them, 1 in profile.
    pub aim: Vec2,
    pub turned: f32,
    turn: Turn,
    pub mouth: Pos2,
    /// How far its jaw is open, from 0 shut; and how great its breath of fire.
    jaw: f32,
    breath: f32,
    /// Its feet, at the title's top while it holds it.
    pub feet: [Pos2; 2],
}

/// How its head is turned at `look` (`Pose::look`) flying `ahead`: back down at the houses on the
/// side it came from, round at the writer, down ahead at the page, and ahead on its way. Turning
/// from one to the next, it turns through facing the writer, as a head does.
fn gaze(look: f32, ahead: f32) -> Turn {
    let keys = [(-ahead * 0.42 * PI, 0.6), (0.0, 0.25), (ahead * 0.45 * PI, 0.85), (ahead * 0.5 * PI, -0.1)];
    let k = (look.max(0.0) as usize).min(2);
    let s = (look - k as f32).clamp(0.0, 1.0);
    let ((ya, pa), (yb, pb)) = (keys[k], keys[k + 1]);
    Turn::new(ya + (yb - ya) * s, pa + (pb - pa) * s)
}

/// How far ahead of an even pace the dragon's wings have pushed it at `t`, in seconds: the
/// push is strongest in the middle of each down beat, and nothing while it glides. Over a beat
/// it comes to nothing, so it keeps its pace; less than a second's worth, so it never goes back.
fn surge(t: f32) -> f32 {
    let beat = (t / FLAP).rem_euclid(3.0);
    if beat < 2.0 { -FLAP / (2.0 * PI) * (2.0 * PI * beat).sin() } else { 0.0 }
}

/// How high the dragon's wings have lifted it at `t`, in its sizes: lowest with its wings up,
/// highest with them down, half way while it glides.
fn heave(t: f32) -> f32 {
    let beat = (t / FLAP).rem_euclid(3.0);
    let up = if beat < 2.0 { 0.5 - 0.5 * (2.0 * PI * beat).sin() } else { 0.5 };
    0.2 * up
}

/// Its wings at `t`, from -0.5 down to 1 up, and how far spread: two beats, spread fuller on the
/// down beat, then a glide on wings spread wide; and again.
fn wingbeat(t: f32) -> (f32, f32) {
    let beat = (t / FLAP).rem_euclid(3.0);
    if beat < 2.0 {
        let phase = 2.0 * PI * beat;
        (0.25 + 0.75 * phase.sin(), 0.82 - 0.18 * phase.cos())
    } else {
        // They open, hold still a while, and fold back into the next beat.
        let ease = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let g = ease((beat - 2.0) / 0.3) * ease((3.0 - beat) / 0.3);
        (0.25 + 0.2 * g, 0.64 + 0.36 * g)
    }
}

/// How far the slow wave lifts the dragon at `u` along it, from 0 at its head to 1 at its tail's
/// tip, at `t`, in its sizes.
fn wave(u: f32, t: f32) -> f32 {
    (0.05 + 0.22 * u * u) * (2.0 * PI * t / GLIDE - 3.2 * u).sin()
}

/// The head's size, in the dragon's.
const HEAD: f32 = 0.36;

pub fn dragon_at(by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect) -> Dragon {
    let r = rig(by, p, place, page, desk);
    let s = r.size;
    // It flies away from the side it came from.
    let ahead = if matches!(by, Carrier::Dragon { right: true }) { -1.0 } else { 1.0 };
    let body = r.hook - vec2(0.0, 0.85 * s);
    let turn = gaze(p.look, ahead);
    let (aim, _) = turn.aim();
    // The head rides the slow wave, a little behind the neck.
    let head = body + vec2(ahead * 1.3 * s, -0.95 * s) + vec2(0.0, 0.8 * wave(0.0, p.t - 0.3)) * s;
    // Fire always a little, and a great breath now and then. Looking back over itself at the city
    // it only puffs, so as not to breathe fire over its own back.
    let back = (1.0 + aim.x * ahead).clamp(0.25, 1.0);
    let breath = (0.35 + 0.65 * (2.0 * PI * p.t / BREATH).sin().max(0.0)) * back;
    let jaw = 0.1 + 0.4 * breath;
    let tip = turn.at([1.45, -0.2 - 0.6 * jaw, 0.0]);
    let mouth = head + vec2(tip[0], tip[1]) * HEAD * s;
    let feet = if p.hooked {
        let load = place.translate(r.offset);
        let grip = (load.width() * 0.4).min(0.55 * s);
        [load.center_top() + vec2(ahead * grip, 0.0), load.center_top() - vec2(ahead * grip, 0.0)]
    } else {
        // Tucked in under it.
        [body + vec2(ahead * 0.3 * s, 0.55 * s), body + vec2(-ahead * 0.5 * s, 0.6 * s)]
    };
    let turned = turn.forward[0].abs() / turn.forward[0].hypot(turn.forward[2]).max(1e-6);
    Dragon { body, size: s, ahead, head, aim, turned, turn, mouth, jaw, breath, feet }
}

/// The dragon's head, its parts drawn from the furthest from the writer to the nearest, so it
/// turns as a head does: the far eye and horn go behind it in profile.
fn dragon_head(d: &Dragon) -> Vec<Shape> {
    let (h, size, turn, jaw) = (d.head, HEAD * d.size, d.turn, d.jaw);
    let mut parts: Vec<(f32, Shape)> = Vec::new();
    let mut part = |points: Vec<[f32; 3]>, color: Color32, nearer: f32| {
        let seen: Vec<[f32; 3]> = points.into_iter().map(|q| turn.at(q)).collect();
        let depth = seen.iter().map(|q| q[2]).sum::<f32>() / seen.len() as f32 + nearer;
        parts.push((depth, hull(seen.iter().map(|q| h + vec2(q[0], q[1]) * size).collect(), color)));
    };
    // The lower jaw opens about its hinge below the back of the head.
    let open = |q: [f32; 3]| {
        let (a, b) = (q[0], q[1] + 0.15);
        [a * jaw.cos() + b * jaw.sin(), b * jaw.cos() - a * jaw.sin() - 0.15, q[2]]
    };
    part(vec![[0.0, -0.15, 0.2], [0.0, -0.15, -0.2], [1.45, -0.2, 0.0], open([1.35, -0.3, 0.0])], MOUTH, -0.3);
    part(egg([0.1, -0.4, 0.0], [0.45, 0.25, 0.34], 0.0), BELLY, -0.05);
    part(egg([0.7, -0.3, 0.0], [0.72, 0.13, 0.27], 0.0).into_iter().map(open).collect(), SCALES_SHADE, 0.0);
    for c in [-0.22, 0.22] {
        for k in 0..4 {
            let a = 0.55 + 0.24 * k as f32;
            part(vec![[a - 0.05, -0.24, c], [a + 0.05, -0.24, c], [a, -0.38, c]], CLAW, 0.05);
        }
    }
    part(egg([0.0, 0.05, 0.0], [0.62, 0.55, 0.5], 0.0), SCALES, 0.0);
    part(egg([0.85, -0.04, 0.0], [0.72, 0.28, 0.32], 0.08), SCALES, 0.02);
    for side in [-1.0, 1.0] {
        let c = 0.3 * side;
        part(egg([0.35, 0.3, side * 0.27], [0.3, 0.09, 0.12], 0.15), SCALES_DARK, 0.05);
        // Horns sweeping back, in two pieces, and spikes down the cheek.
        let ring = |a: f32, b: f32, c: f32, r: f32| (0..8).map(move |k| {
            let t = k as f32 * PI / 4.0;
            [a + r * t.cos(), b + r * t.sin(), c]
        });
        part(ring(-0.25, 0.45, c, 0.12).chain(ring(-0.75, 0.78, c * 1.3, 0.07)).collect(), HORN, 0.0);
        part(ring(-0.75, 0.78, c * 1.3, 0.07).chain([[-1.3, 0.85, c * 1.45]]).collect(), HORN, 0.0);
        for k in 0..3 {
            let b = -0.15 + 0.17 * k as f32;
            part(vec![[-0.3, b, side * 0.4], [-0.3, b + 0.12, side * 0.4], [-0.78, b + 0.02, side * 0.5]], SCALES_DARK, 0.0);
        }
        part(egg([1.5, 0.08, side * 0.12], [0.05, 0.04, 0.03], 0.0), PUPIL, 0.06);
    }
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<Shape> = parts.into_iter().map(|(_, s)| s).collect();
    // The eyes, each only when it faces the writer: in profile only the near one shows. A slit of
    // a pupil, and a glint.
    for side in [-1.0, 1.0] {
        let normal = turn.at([0.3, 0.3, side]);
        if normal[2] <= 0.05 {
            continue;
        }
        let c = turn.at([0.32, 0.15, side * 0.4]);
        let at = h + vec2(c[0], c[1]) * size;
        out.push(Shape::circle_filled(at, 0.14 * size, EYE));
        out.push(oval(at, vec2(0.0, 1.0), 0.11 * size, 0.035 * size, PUPIL));
        out.push(Shape::circle_filled(at + vec2(-0.05, -0.05) * size, 0.03 * size, CLAW));
    }
    out
}

/// A wing from `shoulder`: its arm up to the wrist, four long fingers fanning from there, and
/// the skin between them, scalloped, down to the body at `root`. It beats with `lift` (from -0.5
/// down to 1 up) and is spread fuller on the down beat (`spread`).
#[allow(clippy::too_many_arguments)]
fn wing(out: &mut Vec<Shape>, shoulder: Pos2, root: Pos2, f: f32, k: f32, lift: f32, spread: f32, skin: [Color32; 2]) {
    let angle = (-0.45 + 1.95 * (lift + 0.5) / 1.5).min(1.5);
    let dir = |a: f32| vec2(-f * a.cos(), -a.sin());
    let wrist = shoulder + dir(angle) * 1.05 * k * spread + vec2(f * 0.15 * k, 0.0);
    let elbow = shoulder + dir(angle + 0.5) * 0.5 * k + vec2(f * 0.22 * k, -0.05 * k);
    let tips: Vec<Pos2> = [1.3, 1.25, 1.1, 0.85].iter().enumerate()
        .map(|(i, len)| wrist + dir(angle - (0.15 + i as f32 * 0.42) * spread) * *len * k * spread.sqrt())
        .collect();
    let scallop = |a: Pos2, b: Pos2| a + (b - a) * 0.5 + (wrist - (a + (b - a) * 0.5)) * 0.16;
    out.push(Shape::convex_polygon(vec![shoulder, elbow, wrist], skin[0], Stroke::NONE));
    let mut edge = tips.clone();
    edge.push(root);
    for (i, w) in edge.windows(2).enumerate() {
        let m = scallop(w[0], w[1]);
        let color = skin[i % 2];
        out.push(Shape::convex_polygon(vec![wrist, w[0], m], color, Stroke::NONE));
        out.push(Shape::convex_polygon(vec![wrist, m, w[1]], color, Stroke::NONE));
    }
    out.push(Shape::convex_polygon(vec![wrist, root, shoulder], skin[0], Stroke::NONE));
    // The bones: the arm thick, the fingers thinning, and a claw at the wrist.
    out.push(Shape::line(vec![shoulder, elbow, wrist], Stroke::new(0.09 * k, BONE)));
    for &tip in &tips {
        let mid = wrist + (tip - wrist) * 0.5;
        out.push(Shape::line_segment([wrist, mid], Stroke::new(0.05 * k, BONE)));
        out.push(Shape::line_segment([mid, tip], Stroke::new(0.03 * k, BONE)));
    }
    let up = dir(angle + 0.9);
    out.push(Shape::convex_polygon(vec![wrist - up.rot90() * 0.04 * k, wrist + up * 0.16 * k, wrist + up.rot90() * 0.04 * k], CLAW, Stroke::NONE));
}

/// The dragon at `p`, the title's place at `place`: its wings beating, its tail swaying, the title
/// in its claws while it holds it, its head turned where it looks, fire from its jaws and smoke
/// from its nostrils.
pub fn dragon(by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect) -> Vec<Shape> {
    let d = dragon_at(by, p, place, page, desk);
    let (b, s, f, t) = (d.body, d.size, d.ahead, p.t);
    let mut out = Vec::new();
    let (lift, spread) = wingbeat(t);
    let shoulder = b + vec2(f * 0.25 * s, -0.3 * s);
    let root = b + vec2(-f * 0.65 * s, -0.2 * s);
    wing(&mut out, shoulder + vec2(f * 0.22 * s, -0.08 * s), root, f, 0.9 * s, lift, spread, [WING_FAR, WING_FAR]);

    // One spine from the head down the neck, along the body and out to the tip of the tail; the
    // tail sways in a wave running down it.
    // The slow wave runs down it: the chest and hips rise and fall in turn, the tail most.
    let chest = b + vec2(f * 0.75 * s, (-0.1 + wave(0.15, t)) * s);
    let hips = b + vec2(-f * 0.75 * s, (0.05 + wave(0.4, t)) * s);
    let mut spine = Vec::new();
    let mut widths = Vec::new();
    for k in 0..8 {
        let u = k as f32 / 8.0;
        spine.push(bend(d.head, b + vec2(f * 1.45 * s, (-0.2 + wave(0.05, t)) * s), chest, u));
        widths.push((0.3 + 0.2 * u) * s);
    }
    let neck = spine.len();
    for k in 0..10 {
        let u = k as f32 / 10.0;
        spine.push(bend(chest, b + vec2(0.0, (0.14 + wave(0.28, t)) * s), hips, u));
        widths.push((0.5 + 0.18 * (u * PI).sin() - 0.05 * u) * s);
    }
    let tail = spine.len();
    for k in 0..=14 {
        let u = k as f32 / 14.0;
        let sway = wave(0.4 + 0.6 * u, t) - wave(0.4, t);
        spine.push(hips + vec2(-f * 2.4 * u, 0.5 * u + sway) * s);
        widths.push((0.45 * (1.0 - u) + 0.04) * s);
    }
    // Which way is its back at each point.
    let top: Vec<Vec2> = (0..spine.len()).map(|k| {
        let (a, c) = (spine[k.saturating_sub(1)], spine[(k + 1).min(spine.len() - 1)]);
        (c - a).normalized().rot90() * -f
    }).collect();

    // Its legs: the far ones behind, in shadow; the near ones hold the title.
    let leg = |out: &mut Vec<Shape>, hip: Pos2, foot: Pos2, hind: bool, color: Color32| {
        let bend_to = if hind { f * 0.22 } else { -f * 0.16 };
        let knee = hip + (foot - hip) * 0.45 + vec2(bend_to * s, 0.0);
        let ankle = foot + vec2(-f * if hind { 0.16 } else { 0.06 } * s, -0.16 * s);
        if hind {
            out.push(oval(hip + (knee - hip) * 0.4, (knee - hip).normalized(), 0.34 * s, 0.2 * s, color));
        }
        out.extend(ribbon(&[hip, knee, ankle, foot], &[0.24 * s, 0.14 * s, 0.1 * s, 0.08 * s], color));
    };
    let (front_hip, hind_hip) = (chest + vec2(-f * 0.05 * s, 0.2 * s), hips + vec2(f * 0.2 * s, 0.15 * s));
    leg(&mut out, front_hip + vec2(f * 0.12 * s, -0.05 * s), front_hip + vec2(f * 0.35 * s, 0.4 * s), false, SCALES_DARK);
    leg(&mut out, hind_hip + vec2(f * 0.12 * s, -0.05 * s), hind_hip + vec2(-f * 0.1 * s, 0.45 * s), true, SCALES_DARK);

    out.extend(ribbon(&spine, &widths, SCALES));
    // Its back catches the light, the scales show along its flank, plates cross its belly.
    out.push(Shape::line(spine[..tail + 4].iter().zip(&top).zip(&widths).map(|((&q, &n), &w)| q + n * w * 0.3).collect(), Stroke::new(0.06 * s, SCALES_LIGHT)));
    for k in neck..tail + 6 {
        for row in [-0.05, 0.12] {
            let at = spine[k] + top[k] * widths[k] * row + (spine[k + 1] - spine[k]) * 0.5 * (row * 4.0);
            out.push(Shape::circle_filled(at, 0.035 * s, SCALES_SHADE));
        }
    }
    let belly: Vec<Pos2> = (2..tail + 5).map(|k| spine[k] - top[k] * widths[k] * 0.24).collect();
    let thick: Vec<f32> = (2..tail + 5).map(|k| widths[k] * 0.48).collect();
    out.extend(ribbon(&belly, &thick, BELLY));
    for (k, (&c, &w)) in belly.iter().zip(&thick).enumerate().skip(1) {
        let n = (c - belly[k - 1]).normalized().rot90();
        out.push(Shape::line_segment([c + n * w * 0.45, c - n * w * 0.45], Stroke::new(0.02 * s, BELLY_DARK)));
    }
    // Spikes down its back, smaller towards the tail, leaning back; a spade at the tail's tip.
    for k in (3..spine.len() - 1).step_by(2) {
        let (n, w) = (top[k], widths[k]);
        let along = (spine[k + 1] - spine[k]).normalized();
        let base = spine[k] + n * w * 0.42;
        let h = 0.1 * s + 0.22 * w;
        out.push(Shape::convex_polygon(vec![base - along * 0.25 * h, base + n * h + along * 0.35 * h, base + along * 0.45 * h], SCALES_DARK, Stroke::NONE));
    }
    let (end, before) = (spine[spine.len() - 1], spine[spine.len() - 2]);
    let (on, across) = ((end - before).normalized(), (end - before).normalized().rot90());
    out.push(Shape::convex_polygon(vec![end + on * 0.45 * s, end + across * 0.2 * s, end - on * 0.08 * s, end - across * 0.2 * s], SCALES_DARK, Stroke::NONE));

    // The near legs, down to the title's top, three claws of each foot curled over it.
    leg(&mut out, front_hip, d.feet[0], false, SCALES);
    leg(&mut out, hind_hip, d.feet[1], true, SCALES);
    for foot in d.feet {
        for dx in [-0.1, 0.0, 0.1] {
            out.push(Shape::line_segment([foot, foot + vec2(dx * s, 0.13 * s)], Stroke::new(0.04 * s, CLAW)));
        }
    }
    wing(&mut out, shoulder, root, f, s, lift, spread, [WING, WING_SHADE]);
    out.extend(dragon_head(&d));

    // Fire: tongues from its jaws, each flickering on its own, the hottest in the middle; sparks
    // flying off, and smoke curling up from its nostrils.
    let long = d.turn.aim().1;
    let dir = (d.aim * long + vec2(0.0, 0.3) * (1.0 - long)).normalized();
    let len = s * 1.7 * d.breath * (0.45 + 0.55 * long);
    for (k, color) in FIRE.iter().enumerate() {
        for tongue in 0..3 - k.min(2) {
            let wobble = 0.12 * (13.0 * t + tongue as f32 * 2.4 + k as f32).sin() * (tongue as f32 - 1.0);
            let share = (1.0 - 0.22 * k as f32) * (1.0 + 0.12 * (29.0 * t + k as f32 * 2.1 + tongue as f32).sin());
            let width = 0.22 * (1.0 + 0.25 * (1.0 - d.turned)) * (1.0 - 0.15 * k as f32);
            out.push(flame(d.mouth, Vec2::angled(dir.angle() + wobble), len * share, len * share * width, *color));
        }
    }
    for k in 0..5 {
        let age = (1.7 * t + k as f32 * 0.2).fract();
        let drift = dir.rot90() * (0.25 * (7.0 * t + k as f32 * 1.7).sin()) - vec2(0.0, 0.3 * age);
        let spark = d.mouth + (dir * (0.7 + 0.6 * age) + drift) * len;
        out.push(Shape::circle_filled(spark, 0.04 * s * d.breath * (1.0 - age), FIRE[2]));
    }
    for k in 0..4 {
        let age = (0.9 * t + k as f32 * 0.25).fract();
        let nostril = d.turn.at([1.5, 0.1, 0.0]);
        let at = d.head + vec2(nostril[0], nostril[1]) * HEAD * s + vec2(0.12 * (3.0 * t + k as f32).sin(), -0.9 * age) * s;
        let grey = (90.0 * (1.0 - age)) as u8;
        out.push(Shape::circle_filled(at, (0.05 + 0.14 * age) * s, Color32::from_rgba_unmultiplied(130, 128, 136, grey)));
    }
    out
}

/// The balloon at `p`, the title's place at `place`: its silk in stripes, the basket below, and
/// while it carries the title, ropes down to it.
pub fn balloon(by: Carrier, p: Pose, place: Rect, page: Rect, desk: Rect) -> Vec<Shape> {
    let r = rig(by, p, place, page, desk);
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
    /// A little tower at a corner of its roof, on the left or the right; a statue at the top of
    /// its roof, which kind and of what; and windows round at the top.
    tower: Option<bool>,
    statue: Option<(usize, Color32)>,
    arched: bool,
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
        let roof = (ROOFS[pick(&mut seeds, ROOFS.len())], pick(&mut seeds, 5));
        // Drawn after where it stands and how high it gets, so those stay what they were.
        let tower = (seeds.next(0.0, 1.0) < 0.4).then(|| seeds.next(0.0, 1.0) < 0.5);
        let statue = (roof.1 == 3 || seeds.next(0.0, 1.0) < 0.15).then(|| (pick(&mut seeds, 3), [STONE, GILT][pick(&mut seeds, 2)]));
        let arched = seeds.next(0.0, 1.0) < 0.4;
        House {
            left: 0.0,
            width,
            ground,
            storey: 22.0 * unit,
            most,
            floors: 0,
            building: 0.0,
            seed,
            windows,
            wall,
            roof,
            tower,
            statue,
            arched,
            unit,
        }
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

    /// The colour of window `f`, `w` at `time`: some are lit and the rest dark, the same windows
    /// for the same story. Most lit ones burn steadily, each a little warmer or paler; a few
    /// flicker slightly, as if by a candle.
    pub fn light_in(&self, story: &str, f: usize, w: usize, time: f64) -> Color32 {
        let mut seeds = Seeds::of(story, self.seed * 1024 + f * 8 + w);
        if seeds.next(0.0, 1.0) > 0.45 {
            return DARK;
        }
        let (warmth, phase, speed) = (seeds.next(0.75, 0.95), seeds.next(0.0, 2.0 * PI), seeds.next(0.8, 1.3));
        if seeds.next(0.0, 1.0) > 0.35 {
            return GLOW_DIM.lerp_to_gamma(GLOW, warmth);
        }
        let t = (time % 1000.0) as f32 * speed;
        let wave = (t * 2.3 + phase).sin() * 0.5 + (t * 5.9 + phase * 1.7).sin() * 0.3 + (t * 12.7 + phase * 2.9).sin() * 0.2;
        GLOW_DIM.lerp_to_gamma(GLOW, warmth + 0.08 * wave)
    }

    /// Whether floor `f` has a little balcony under one of its windows, and which.
    fn balcony(&self, story: &str, f: usize) -> Option<usize> {
        let mut seeds = Seeds::of(story, self.seed * 1024 + f * 8 + 7);
        (f > 0 && seeds.next(0.0, 1.0) < 0.22).then(|| (seeds.next(0.0, self.windows as f32) as usize).min(self.windows - 1))
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
                    out.extend(window(r, self.light_in(story, f, w, time), self.arched, u));
                }
            }
            if let Some(r) = self.balcony(story, f).and_then(|w| self.window(f, w)) {
                out.extend(balcony(r, self.wall.lerp_to_gamma(Color32::WHITE, 0.3), u));
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
            out.extend(self.roof(story, roof_at, time));
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
                        out.extend(window(r, self.wall.lerp_to_gamma(lit, (b - 0.6) / 0.4), self.arched, u));
                    }
                }
            }
        }
        (out, light)
    }

    /// Its roof, sitting at `y`: a pointed one, a tall hat, a dome, a flat one with battlements,
    /// or an onion; at its top a star or a statue, and perhaps a little tower at a corner. All of
    /// it stays above the house, no wider than its eaves, so it never reaches the next house.
    fn roof(&self, story: &str, y: f32, time: f64) -> Vec<Shape> {
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
            2 => {
                let radius = self.width * 0.42;
                let points = (0..=12).map(|k| pos2(mid, y) + egui::Vec2::angled(PI + PI * k as f32 / 12.0) * radius).collect();
                out.push(Shape::convex_polygon(points, color, Stroke::NONE));
                let tip = pos2(mid, y - radius - 7.0 * u);
                out.push(Shape::line_segment([pos2(mid, y - radius), tip], Stroke::new(1.2 * u, color)));
                tip
            }
            3 => {
                // A parapet with battlements, the statue standing in the middle.
                let top = y - 4.0 * u;
                out.push(Shape::rect_filled(Rect::from_min_max(pos2(l, top), pos2(r, y)), 0.0, color));
                let n = ((r - l) / (6.0 * u)) as usize | 1;
                let step = (r - l) / n as f32;
                for k in (0..n).step_by(2) {
                    let x = l + k as f32 * step;
                    out.push(Shape::rect_filled(Rect::from_min_max(pos2(x, top - 3.5 * u), pos2(x + step, top)), 0.0, color));
                }
                pos2(mid, top)
            }
            _ => {
                // An onion: a round body and a point drawn up from it.
                let radius = self.width * 0.3;
                let c = pos2(mid, y - radius * 0.7);
                out.push(Shape::rect_filled(Rect::from_min_max(pos2(mid - radius * 0.7, c.y), pos2(mid + radius * 0.7, y)), 0.0, color));
                out.push(Shape::circle_filled(c, radius, color));
                let tip = pos2(mid, c.y - radius * 1.9);
                let shoulder = c.y - radius * 0.7;
                out.push(Shape::convex_polygon(
                    vec![pos2(mid - radius * 0.72, shoulder), tip, pos2(mid + radius * 0.72, shoulder)],
                    color,
                    Stroke::NONE,
                ));
                tip
            }
        };
        match self.statue {
            Some((which, stone)) => out.extend(statue(which, tip, stone, u)),
            None => out.push(Shape::circle_filled(tip, 2.0 * u, STAR)),
        }
        if let Some(left) = self.tower {
            out.extend(self.tower(story, left, y, time));
        }
        out
    }

    /// A little round tower at the left or right corner of the roof at `y`, in front of it: a
    /// lit or dark window, a pointed cap, and a pennant fluttering at its top.
    fn tower(&self, story: &str, left: bool, y: f32, time: f64) -> Vec<Shape> {
        let u = self.unit;
        let w = self.width * 0.26;
        let x = if left { self.left - 2.0 * u } else { self.left + self.width + 2.0 * u - w };
        let body = Rect::from_min_max(pos2(x, y - self.storey * 0.95), pos2(x + w, y + 2.0 * u));
        let edge = self.wall.lerp_to_gamma(Color32::BLACK, 0.3);
        let mut out = vec![
            Shape::rect_filled(body, 0.0, self.wall.lerp_to_gamma(Color32::WHITE, 0.08)),
            Shape::rect_filled(Rect::from_min_max(body.left_bottom() - vec2(1.0 * u, 2.5 * u), body.right_bottom() + vec2(1.0 * u, 0.0)), 0.0, edge),
        ];
        let pane = Rect::from_center_size(body.center() - vec2(0.0, self.storey * 0.08), vec2(w * 0.4, self.storey * 0.36));
        out.extend(window(pane, self.light_in(story, 127, 0, time), true, u));
        let (color, _) = self.roof;
        let tip = pos2(body.center().x, body.top() - w * 1.15);
        let cap = vec![pos2(body.left() - 1.5 * u, body.top()), tip, pos2(body.right() + 1.5 * u, body.top())];
        out.push(Shape::convex_polygon(cap, color, Stroke::NONE));
        // The pennant flies away from the house, and flutters.
        let pole = tip - vec2(0.0, 7.0 * u);
        out.push(Shape::line_segment([tip, pole], Stroke::new(1.0 * u, POLE)));
        let away = if left { -1.0 } else { 1.0 };
        let flutter = ((time % 1000.0) as f32 * 3.1 + self.seed as f32).sin() * 0.8 * u;
        let flag = vec![pole, pos2(pole.x + away * 6.0 * u, pole.y + 1.8 * u + flutter), pole + vec2(0.0, 3.6 * u)];
        out.push(Shape::convex_polygon(flag, PENNANT, Stroke::NONE));
        out
    }
}

/// A little statue standing on `at`, on a plinth: an owl, a figure holding up a star, or a cat
/// sitting with its tail curled round.
fn statue(which: usize, at: Pos2, stone: Color32, u: f32) -> Vec<Shape> {
    let shade = stone.lerp_to_gamma(Color32::BLACK, 0.25);
    let plinth = Rect::from_min_max(at - vec2(4.5 * u, 3.0 * u), at + vec2(4.5 * u, 0.0));
    let mut out = vec![Shape::rect_filled(plinth, 0.5 * u, shade)];
    let base = plinth.center_top();
    let oval = |c: Pos2, rx: f32, ry: f32| {
        let points = (0..16).map(|k| c + egui::Vec2::angled(2.0 * PI * k as f32 / 16.0) * vec2(rx, ry)).collect();
        Shape::convex_polygon(points, stone, Stroke::NONE)
    };
    let ears = |c: Pos2, r: f32, out: &mut Vec<Shape>| {
        for side in [-1.0, 1.0] {
            let ear = vec![c + vec2(side * r * 0.9, -r * 0.2), c + vec2(side * r * 0.85, -r * 1.6), c + vec2(side * r * 0.1, -r * 0.8)];
            out.push(Shape::convex_polygon(ear, stone, Stroke::NONE));
        }
    };
    match which {
        0 => {
            // An owl, its eyes alight.
            out.push(oval(base - vec2(0.0, 4.5 * u), 3.6 * u, 4.6 * u));
            let head = base - vec2(0.0, 10.0 * u);
            out.push(Shape::circle_filled(head, 3.2 * u, stone));
            ears(head, 3.2 * u, &mut out);
            for side in [-1.0, 1.0] {
                out.push(Shape::circle_filled(head + vec2(side * 1.4 * u, 0.0), 1.0 * u, GLOW));
            }
        }
        1 => {
            // A figure in a long robe, holding a star up high.
            let robe = vec![base + vec2(-3.6 * u, 0.0), base + vec2(-1.4 * u, -10.0 * u), base + vec2(1.4 * u, -10.0 * u), base + vec2(3.6 * u, 0.0)];
            out.push(Shape::convex_polygon(robe, stone, Stroke::NONE));
            let head = base - vec2(0.0, 12.2 * u);
            out.push(Shape::circle_filled(head, 2.3 * u, stone));
            let hand = base + vec2(4.5 * u, -16.0 * u);
            out.push(Shape::line_segment([base + vec2(1.0 * u, -9.0 * u), hand], Stroke::new(1.3 * u, stone)));
            out.push(Shape::circle_filled(hand - vec2(0.0, 1.5 * u), 2.0 * u, STAR));
        }
        _ => {
            // A cat sitting, its tail curled round its feet.
            out.push(oval(base - vec2(0.0, 4.0 * u), 3.4 * u, 4.2 * u));
            let head = base - vec2(-0.6 * u, 9.5 * u);
            out.push(Shape::circle_filled(head, 2.6 * u, stone));
            ears(head, 2.6 * u, &mut out);
            let tail = [base + vec2(2.6 * u, -u), base + vec2(5.2 * u, -2.0 * u), base + vec2(5.6 * u, -5.5 * u)];
            out.push(Shape::line_segment([tail[0], tail[1]], Stroke::new(1.3 * u, stone)));
            out.push(Shape::line_segment([tail[1], tail[2]], Stroke::new(1.3 * u, stone)));
        }
    }
    out
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

/// A window at `r` of `color`, round at the top if `arched`, a soft glow about it when it is lit.
fn window(r: Rect, color: Color32, arched: bool, u: f32) -> Vec<Shape> {
    let mut out = Vec::new();
    let top = if arched { (r.width() / 2.0) as u8 } else { u as u8 };
    let round = egui::CornerRadius { nw: top, ne: top, sw: u as u8, se: u as u8 };
    if color != DARK {
        out.push(Shape::rect_filled(r.expand(2.0 * u), round + (2.0 * u) as u8, glow(color, 0.18)));
    }
    out.push(Shape::rect_filled(r, round, color));
    out
}

/// A little balcony under the window at `r`: a floor and a railing of `color`.
fn balcony(r: Rect, color: Color32, u: f32) -> Vec<Shape> {
    let (l, rt, y) = (r.left() - 3.0 * u, r.right() + 3.0 * u, r.bottom() + 1.5 * u);
    let mut out = vec![Shape::rect_filled(Rect::from_min_max(pos2(l, y), pos2(rt, y + 1.8 * u)), 0.5 * u, color)];
    let rail = y - 4.0 * u;
    out.push(Shape::line_segment([pos2(l, rail), pos2(rt, rail)], Stroke::new(1.0 * u, color)));
    let n = ((rt - l) / (2.6 * u)) as usize;
    for k in 0..=n {
        let x = l + (rt - l) * k as f32 / n.max(1) as f32;
        out.push(Shape::line_segment([pos2(x, rail), pos2(x, y)], Stroke::new(0.7 * u, color)));
    }
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

    /// The paragraph at the caret was just made a chapter title: in the Magic City the dragon comes
    /// to bring it into place, in the Magic Garden now and then the balloon.
    pub fn maybe_lift_title(&mut self, ctx: &egui::Context) {
        let Some(mut by) = Carrier::of(self.themes.theme) else { return };
        let (at, _) = self.doc.para_start(self.selection().0);
        // Only a title with words in it: an empty one is no load.
        if !self.doc.chapters(false).iter().any(|e| e.at == at) || self.eggs.roll() >= self.eggs.odds.unwrap_or(by.odds()) {
            return;
        }
        let (Carrier::Balloon { right } | Carrier::Dragon { right }) = &mut by;
        self.eggs.from_right = !self.eggs.from_right;
        *right = self.eggs.from_right;
        let started = ctx.input(|i| i.time);
        self.eggs.lift = Some(Lift { by, at, page: self.doc.page_of(at), started, duck: None });
        ctx.request_repaint();
    }

    /// Once a frame: the dragon or the balloon hurries away when the writer goes on, and is gone when it is done or
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
        let p = pose_of(lift.by, (now - lift.started) as f32, lift.duck.map(|d| (d - lift.started) as f32));
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
        let p = pose_of(lift.by, (now - lift.started) as f32, lift.duck.map(|d| (d - lift.started) as f32));
        let start = self.doc.spans[i].start;
        let (end, _) = self.doc.term_from(lift.at, self.doc.char_to_byte(lift.at));
        Some((lift.by, p, lift.at.saturating_sub(start)..end.saturating_sub(start)))
    }

    /// The dragon or the balloon over page `i` lying at `page`, if it is at work there.
    pub fn draw_carrier(&self, ui: &egui::Ui, page: Rect, i: usize) {
        let (Some((by, p, _)), Some(place)) = (self.lifting(ui.ctx(), i), self.eggs.place) else { return };
        ui.painter().extend(match by {
            Carrier::Dragon { .. } => dragon(by, p, place, page, self.eggs.desk),
            Carrier::Balloon { .. } => balloon(by, p, place, page, self.eggs.desk),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desk the tests' pages lie on: a wide window.
    fn desk() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))
    }

    #[test]
    fn the_title_is_brought_in_carried_over_its_place_lowered_and_let_go() {
        let at = |t: f32| pose(t, None);
        assert!(at(0.0).enter == 0.0 && at(0.0).hooked && at(0.0).lower == 0.0, "away, the title on its hook, hanging high");
        assert!(at(ENTER).enter == 1.0 && at(ENTER).run == 0.0, "beside the page");
        let over = ENTER + RUN;
        assert!(at(over).run == 1.0 && at(over).lower == 0.0, "over its place");
        let down = over + LOWER;
        assert!(at(down - 0.01).lower > 0.9 && at(down - 0.01).hooked, "lowered");
        assert!(at(down + SETTLE / 2.0).lower < 1.0, "it bounces as it touches down");
        let free = down + SETTLE + UNHOOK;
        assert!(!at(free).hooked && at(free).hook == 1.0 && at(free).enter == 1.0, "let go of it");
        assert!(at(free + LEAVE / 2.0).enter < 1.0 && at(free + LEAVE).enter == 0.0, "gone away");
        assert!(at(free + LEAVE).done);
    }

    #[test]
    fn the_balloon_and_the_dragon_bring_the_title_in_from_the_side_and_go_away_past_the_other() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        for by in [true, false].into_iter().flat_map(|right| [Carrier::Balloon { right }, Carrier::Dragon { right }]) {
            let (Carrier::Balloon { right } | Carrier::Dragon { right }) = by;
            let side = if right { 1.0 } else { -1.0 };
            let at = |t: f32| offset(by, pose(t, None), place, page, desk());
            let start = at(0.0);
            assert!(start.x * side > 400.0 && start.y.abs() < 100.0, "{right}: from the side, near the title's height: {start:?}");
            let over = at(ENTER + RUN);
            assert!(over.x.abs() < 0.01 && over.y < -20.0, "{right}: a little above its place: {over:?}");
            assert_eq!(at(ENTER + RUN + LOWER + SETTLE + UNHOOK), Vec2::ZERO, "{right}: let go in its place");
            let p = pose(ENTER + RUN + LOWER + SETTLE + UNHOOK + LEAVE / 2.0, None);
            let going = rig(by, p, place, page, desk()).hook;
            assert!((going.x - place.center().x) * side < -100.0, "{right}: away past the other side: {going:?}");
        }
        // A title high on its page: the carrier hangs lower and is smaller, its top still near the page.
        let high = Rect::from_min_size(pos2(500.0, 120.0), vec2(200.0, 30.0));
        for (by, tall) in [(Carrier::Balloon { right: true }, TALL), (Carrier::Dragon { right: true }, DRAGON_TALL)] {
            let r = rig(by, pose(ENTER + RUN, None), high, page, desk());
            assert!(r.hook.y - r.size * tall >= page.top() - page.height() * 0.05, "{by:?}: {:?} {}", r.hook, r.size);
        }
        let p = pose(ENTER + RUN, None);
        let top = dragon(Carrier::Dragon { right: true }, p, high, page, desk()).iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect()).top();
        assert!(top >= page.top() - page.height() * 0.05, "the dragon's wings stay near the page: {top}");
    }

    /// The shapes of the dragon at `t` seconds bringing a title at `place`, in `colors`.
    fn dragon_in(t: f32, place: Rect, page: Rect, colors: &[Color32]) -> Vec<Shape> {
        let by = Carrier::Dragon { right: true };
        dragon(by, pose(t, None), place, page, desk()).into_iter().filter(|s| match s {
            Shape::Path(path) => colors.contains(&path.fill),
            Shape::Circle(c) => colors.contains(&c.fill),
            _ => false,
        }).collect()
    }

    #[test]
    fn the_dragon_beats_its_wings_and_breathes_fire_from_its_mouth() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        // Over the title's place, at moments of the wings' own time.
        let over = ENTER + RUN;
        let at = |t: f32| Pose { t, ..pose(over, None) };
        let by = Carrier::Dragon { right: true };
        let tip = |t: f32| {
            let skin = dragon(by, at(t), place, page, desk()).into_iter().filter(|s| matches!(s, Shape::Path(p) if [WING, WING_SHADE].contains(&p.fill)));
            skin.map(|s| s.visual_bounding_rect().top()).fold(f32::MAX, f32::min)
        };
        let (up, down) = (tip(FLAP / 4.0), tip(FLAP * 3.0 / 4.0));
        let d = dragon_at(by, at(0.0), place, page, desk());
        assert!(down - up > d.size, "up and down: {up} {down}");
        // Two beats, then a glide: the wings hardly move for a while, spread.
        let swing = |from: f32, to: f32| {
            let tips: Vec<f32> = (0..=10).map(|k| tip(from + (to - from) * k as f32 / 10.0)).collect();
            tips.iter().fold(f32::MIN, |a, &b| a.max(b)) - tips.iter().fold(f32::MAX, |a, &b| a.min(b))
        };
        assert!(swing(2.35 * FLAP, 2.65 * FLAP) < 0.05 * swing(0.0, FLAP), "a glide after two beats");
        for k in 0..8 {
            let t = k as f32 * 0.37;
            let d = dragon_at(Carrier::Dragon { right: true }, pose(t, None), place, page, desk());
            let fire = dragon_in(t, place, page, &FIRE);
            assert!(fire.len() >= 3, "{t}: fire");
            let near = fire.iter().map(|s| s.visual_bounding_rect().distance_to_pos(d.mouth)).fold(f32::MAX, f32::min);
            assert!(near < 1.0, "{t}: from its mouth");
            let reach = fire.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect()).center() - d.mouth;
            assert!(reach.dot(d.aim) > 0.0, "{t}: the way it looks: {reach:?} {:?}", d.aim);
        }
        let big = |t: f32| dragon_in(t, place, page, &FIRE[..1]).iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect()).area();
        // Both while it looks at the writer.
        assert!(big(BREATH * 1.25) > 2.0 * big(BREATH * 1.75), "a great breath now and then: {} {}", big(BREATH * 1.25), big(BREATH * 1.75));
    }

    #[test]
    fn the_dragon_takes_its_time_and_flies_by_its_wings_carrying_the_title_up_and_down_with_them() {
        let by = Carrier::Dragon { right: true };
        let balloon = Carrier::Balloon { right: true };
        let over = ENTER + RUN;
        assert!(pose_of(balloon, over, None).run == 1.0 && pose_of(by, over, None).run < 0.6, "slower than the balloon");
        let course = (ENTER + RUN + LOWER + SETTLE + UNHOOK + LEAVE) * by.pace();
        assert!(course > 5.0 && !pose_of(by, course - 0.3, None).done && pose_of(by, course + 0.3, None).done, "{course}");
        // It goes on fastest in the middle of a down beat, slowest as its wings come up, and
        // evenly while it glides.
        let speed = |beat: f32| (surge((beat + 0.01) * FLAP) - surge((beat - 0.01) * FLAP)) / (0.02 * FLAP);
        assert!(speed(0.5) > 0.5 && speed(1.0) < -0.5 && speed(2.5) == 0.0, "{} {} {}", speed(0.5), speed(1.0), speed(2.5));
        assert!(0.8 * speed(1.0) > -1.0, "never back");

        // Over its place, through a few seconds of the wings' own time.
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let at = |t: f32| dragon_at(by, Pose { t, ..pose(over, None) }, place, page, desk());
        let title = |t: f32| offset(by, Pose { t, ..pose(over, None) }, place, page, desk()).y;
        let s = at(0.0).size;
        // Its wings up at a quarter beat, down at three quarters: the title rises with the down beat.
        assert!(title(FLAP * 0.75) < title(FLAP * 0.25) - 0.15 * s, "{} {}", title(FLAP * 0.25), title(FLAP * 0.75));
        let wings = |t: f32| wingbeat(t).0;
        assert!(wings(FLAP * 0.25) > wings(FLAP * 0.75));
        for k in 0..30 {
            let (t, d) = (k as f32 * 0.1, at(k as f32 * 0.1));
            assert!((d.feet[0].y - (place.top() + title(t))).abs() < 0.01 && (d.body.y - d.feet[0].y + 0.85 * s).abs() < 0.01, "{t}: held");
            // Set down, it lies in its place whatever the wings do.
            let down = Pose { t, ..pose(ENTER + RUN + LOWER, None) };
            assert!(offset(by, down, place, page, desk()).y.abs() < 0.01, "{t}: in its place");
        }
        // The head rides a slow wave, a little behind the neck; the tail swings most.
        let seen: Vec<Dragon> = (0..30).map(|k| at(k as f32 * 0.1)).collect();
        let span = |v: Vec<f32>| v.iter().fold(f32::MIN, |a, &b| a.max(b)) - v.iter().fold(f32::MAX, |a, &b| a.min(b));
        let rise = span(seen.iter().map(|d| d.body.y).collect());
        assert!(rise > 0.15 * s && rise < 0.3 * s, "it rises and sinks, gently: {rise}");
        assert!(span(seen.iter().map(|d| d.head.y - d.body.y).collect()) > 0.05 * s, "the head on its own");
        let tail = |t: f32| {
            let shapes = dragon(by, Pose { t, ..pose(over, None) }, place, page, desk());
            let mut spade = shapes.iter().filter_map(|s| match s { Shape::Path(p) if p.fill == SCALES_DARK && p.points.len() == 4 => Some(p.points[0].y), _ => None });
            spade.next_back().unwrap() - at(t).body.y
        };
        let swing = span((0..30).map(|k| tail(k as f32 * 0.1)).collect());
        assert!(swing > 0.3 * s, "the tail sways: {swing}");
    }

    #[test]
    fn what_brings_a_title_comes_from_beyond_the_desk_and_leaves_until_none_of_it_shows() {
        let page = Rect::from_min_size(pos2(400.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(600.0, 300.0), vec2(200.0, 30.0));
        for right in [true, false] {
            for by in [Carrier::Balloon { right }, Carrier::Dragon { right }] {
                for t in [0.0, COURSE - 0.001] {
                    let p = pose(t, None);
                    let shapes = match by {
                        Carrier::Dragon { .. } => dragon(by, p, place, page, desk()),
                        Carrier::Balloon { .. } => balloon(by, p, place, page, desk()),
                    };
                    let seen = shapes.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect());
                    assert!(!seen.intersects(desk()), "{by:?} at {t}: {seen:?}");
                }
            }
        }
    }

    #[test]
    fn the_dragon_looks_at_the_city_then_at_the_writer_then_at_the_page() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let by = Carrier::Dragon { right: true };
        let at = |t: f32| dragon_at(by, pose(t, None), place, page, desk());
        let eyes = |t: f32| dragon_in(t, place, page, &[EYE]).len();
        // In from the right, flying left: the houses it passed are behind it, to the right.
        let coming = at(ENTER * 0.8);
        assert!(coming.ahead < 0.0 && coming.aim.x > 0.5 && coming.aim.y > 0.0 && coming.turned > 0.9, "back down at the city: {:?}", coming.aim);
        assert_eq!(eyes(ENTER * 0.8), 1, "in profile");
        let over = ENTER + RUN + LOWER * 0.2;
        assert!(at(over).turned == 0.0 && eyes(over) == 2, "at the writer, with both eyes");
        let lowering = ENTER + RUN + LOWER;
        let p = at(lowering);
        assert!(p.aim.x < -0.3 && p.aim.y > 0.7 && p.turned > 0.9 && eyes(lowering) == 1, "down ahead at the page: {:?}", p.aim);
        // And it held the title by its feet all the while, at its top, either side of its middle.
        for t in [0.0, ENTER, over, lowering] {
            let d = at(t);
            let load = place.translate(offset(by, pose(t, None), place, page, desk()));
            for foot in d.feet {
                assert!((foot.y - load.top()).abs() < 0.01 && load.x_range().contains(foot.x), "{t}: {foot:?} on {load:?}");
            }
            assert!((d.feet[0].x - load.center().x) * (d.feet[1].x - load.center().x) < 0.0, "{t}: one foot each side");
        }
        let gone = ENTER + RUN + LOWER + SETTLE + UNHOOK + 0.1;
        assert!(at(gone).feet.iter().all(|f| f.y < place.top() - 5.0), "let go");
    }

    #[test]
    fn the_balloon_s_stripes_take_turns_in_colour_all_the_way_across() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let by = Carrier::Balloon { right: true };
        let p = pose(ENTER + RUN, None);
        let r = rig(by, p, place, page, desk());
        let silk: Vec<_> = balloon(by, p, place, page, desk()).into_iter().filter_map(|s| match s {
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
    fn the_title_is_set_down_at_once_when_the_writer_goes_on() {
        let p = pose(1.0, Some(0.9));
        assert!(!p.hooked && p.lower == 1.0, "the title in its place");
        assert!(p.enter > 0.0 && !p.done && p.look == 3.0, "the carrier on its way out, looking ahead");
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
    fn some_windows_are_lit_a_few_flickering_slightly_the_same_ones_for_the_same_story() {
        let (mut steady, mut flickering) = (0, 0);
        for i in 0..6 {
            let h = House::of("a", i, 900.0, 1.0);
            let windows: Vec<(usize, usize)> = (0..8).flat_map(|f| (0..3).map(move |w| (f, w))).collect();
            let lit: Vec<(usize, usize)> = windows.iter().copied().filter(|&(f, w)| h.light_in("a", f, w, 0.0) != DARK).collect();
            assert!(!lit.is_empty() && lit.len() < windows.len(), "some lit, some dark: {lit:?}");
            for (f, w) in lit {
                let seen: Vec<Color32> = (0..20).map(|t| h.light_in("a", f, w, t as f64 * 0.1)).collect();
                assert!(seen.iter().all(|c| c.r() > 200 && *c != DARK), "lit all the while: {seen:?}");
                let greens: Vec<u8> = seen.iter().map(|c| c.g()).collect();
                let swing = greens.iter().max().unwrap() - greens.iter().min().unwrap();
                assert!(swing < 20, "only slightly: {greens:?}");
                if swing == 0 { steady += 1 } else { flickering += 1 }
                assert_eq!(h.light_in("a", f, w, 0.5), House::of("a", i, 900.0, 1.0).light_in("a", f, w, 0.5));
            }
        }
        assert!(flickering > 0 && steady > flickering, "most burn steadily, a few flicker: {steady} {flickering}");
    }

    #[test]
    fn some_houses_have_a_little_tower_or_a_statue_and_none_reaches_the_next_house() {
        let houses: Vec<House> = (0..40).map(|i| House::of("a", i, 900.0, 1.0)).collect();
        assert!(houses.iter().any(|h| h.tower.is_some()) && houses.iter().any(|h| h.tower.is_none()), "towers on some");
        assert!(houses.iter().any(|h| h.statue.is_some()) && houses.iter().any(|h| h.statue.is_none()), "statues on some");
        assert!(houses.iter().any(|h| h.arched) && (1..20).any(|f| houses[0].balcony("a", f).is_some()), "arches and balconies");
        for (i, mut h) in houses.into_iter().enumerate() {
            assert_eq!(h.tower, House::of("a", i, 900.0, 1.0).tower, "the same for the same story");
            (h.left, h.floors) = (100.0, 3);
            let drawn = h.draw("a", None, 1.0).0.iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect());
            // Houses stand 8 apart: each may reach a little less than half the way.
            assert!(drawn.left() > h.left - 4.0 && drawn.right() < h.left + h.width + 4.0, "{i}: within its eaves: {drawn:?}");
            let shapes = |h: &House| h.draw("a", None, 1.0).0;
            let top = |s: &[Shape]| s.iter().map(|s| s.visual_bounding_rect().top()).fold(f32::MAX, f32::min);
            if h.statue.is_some() {
                let (with, plain) = (House { tower: None, ..h.clone() }, House { tower: None, statue: None, ..h.clone() });
                assert!(top(&shapes(&with)) < top(&shapes(&plain)) - 6.0, "{i}: the statue stands up at the top of the roof");
            }
            if h.tower.is_some() {
                // The tower is drawn last, in front of the roof.
                let (all, plain) = (shapes(&h), shapes(&House { tower: None, ..h.clone() }));
                assert!(top(&all[plain.len()..]) < h.floor(2).top() - h.storey, "{i}: the tower rises above the roof's eaves");
            }
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


