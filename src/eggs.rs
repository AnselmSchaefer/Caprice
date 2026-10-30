//! Little things hidden in the magic themes. In the Magic Garden a garden grows up from the bottom
//! of the desk, a plant for each chapter, taller the more pages it has, and flowering once the next
//! chapter has begun. In the Magic City a street of little houses stands there instead, a floor for
//! each page: a new floor is a light that flows out of a window and spreads into the floor. Some
//! windows are lit, a few of them flickering slightly; some houses have a little tower with a
//! pennant, a statue on the roof, balconies or arched windows. Now and then, when the writer makes a paragraph a chapter title, it is
//! brought in: in the garden a hot-air balloon floats down with it hanging below its basket, sets it
//! in place and rises away; in the Magic City a dragon flies in with it held in its feet, flapping
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
}

/// How tall a plant grows at most, in shares of the desk's height, and how many pages it takes to
/// grow most of the way: a chapter of `PAGES` pages is two thirds of the way up.
const TALLEST: f32 = 0.55;
const PAGES: f32 = 8.0;
/// How fast a plant grows to its height, in shares of its tallest a second.
const GROW: f32 = 0.5;
/// The chance that making a chapter title calls the dragon or the balloon.
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

/// The dragon: its scales, belly and wings, its claws and horns, its eyes, and its fire from
/// the outside in.
const SCALES: Color32 = Color32::from_rgb(58, 146, 92);
const SCALES_DARK: Color32 = Color32::from_rgb(36, 102, 66);
const BELLY: Color32 = Color32::from_rgb(236, 204, 116);
const WING: Color32 = Color32::from_rgb(104, 186, 120);
const WING_FAR: Color32 = Color32::from_rgb(46, 118, 78);
pub const CLAW: Color32 = Color32::from_rgb(246, 236, 210);
const HORN: Color32 = Color32::from_rgb(214, 184, 120);
pub const EYE: Color32 = Color32::from_rgb(255, 222, 70);
const PUPIL: Color32 = Color32::from_rgb(30, 24, 20);
pub const FIRE: [Color32; 3] = [Color32::from_rgb(228, 70, 38), Color32::from_rgb(250, 146, 40), Color32::from_rgb(255, 228, 120)];
/// How tall the dragon is from the title it holds to the tips of its wings, in its sizes; and
/// the seconds one beat of its wings takes, and one breath of fire.
const DRAGON_TALL: f32 = 3.4;
pub const FLAP: f32 = 0.55;
const BREATH: f32 = 0.9;

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
    /// The dragon or the balloon at work, if one is.
    pub lift: Option<Lift>,
    /// Where the title being lifted lies on the screen, from the last page drawn.
    pub place: Option<Rect>,
    /// The chance that making a chapter title calls the dragon or the balloon (`ODDS`; tests set it to 0 or 1).
    pub odds: f64,
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
        Self { grown_for: None, grown: Vec::new(), grown_at: 0.0, measured: None, lift: None, place: None, odds: ODDS, from_right: false, rng: seed | 1, plants: Vec::new(),
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

fn rig(by: Carrier, p: Pose, place: Rect, page: Rect) -> Rig {
    // In from the side at the title's height, a little above it; let go, it goes on and away past
    // the other side. A title is often high on its page, so the carrier hangs only as high as
    // there is room for below the top of the page, and is smaller there.
    let (right, tall, rope, sizes) = match by {
        Carrier::Balloon { right } => (right, TALL, page.height() * 0.02, (0.035, 0.06)),
        Carrier::Dragon { right } => (right, DRAGON_TALL, 0.0, (0.05, 0.1)),
    };
    let side = if right { 1.0 } else { -1.0 };
    let away = 1.0 - p.enter;
    let room = place.top() - (page.top() - page.height() * 0.03) - rope;
    let size = (room / tall).clamp(page.width() * sizes.0, page.width() * sizes.1);
    let hang = (room - size * tall).clamp(0.0, page.height() * 0.08);
    let (hook, offset) = if p.hooked {
        let x = place.center().x + side * ((1.0 - p.run) * page.width() * 0.35 + away * page.width() * 0.6);
        let bob = (p.run * PI).sin() * page.height() * 0.015;
        let load = pos2(x, place.center().y - hang * (1.0 - p.lower) - bob);
        (load - vec2(0.0, place.height() / 2.0 + rope), load - place.center())
    } else {
        let up = rope + page.height() * 0.04 * p.hook;
        (place.center_top() - vec2(0.0, up) + vec2(-side * 0.7 * page.width(), -0.1 * page.height()) * away, Vec2::ZERO)
    };
    Rig { hook, size, offset }
}

/// How far the title is from its place at `p`, brought `by` that.
pub fn offset(by: Carrier, p: Pose, place: Rect, page: Rect) -> Vec2 {
    rig(by, p, place, page).offset
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

/// Where the dragon is, and where it looks, at `p`.
pub struct Dragon {
    /// The middle of its body, and its size: the height of its body.
    pub body: Pos2,
    pub size: f32,
    /// 1 when it flies to the right, -1 to the left.
    pub ahead: f32,
    pub head: Pos2,
    /// Where its snout points, and how far it is turned away from the writer: 0 looking at them, 1
    /// in profile.
    pub aim: Vec2,
    pub turned: f32,
    pub mouth: Pos2,
    /// Its feet, at the title's top while it holds it.
    pub feet: [Pos2; 2],
}

/// Where it looks at `look` (`Pose::look`) flying `ahead`: back down at the houses on the side
/// it came from, out at the writer, down ahead at the page, and ahead on its way.
fn gaze(look: f32, ahead: f32) -> (Vec2, f32) {
    let keys = [(vec2(-ahead * 0.8, 0.6), 1.0), (vec2(0.0, 1.0), 0.0), (vec2(ahead * 0.45, 0.9), 1.0), (vec2(ahead, -0.15), 1.0)];
    let k = (look.max(0.0) as usize).min(2);
    let s = (look - k as f32).clamp(0.0, 1.0);
    let ((a, ta), (b, tb)) = (keys[k], keys[k + 1]);
    ((a + (b - a) * s).normalized(), ta + (tb - ta) * s)
}

pub fn dragon_at(by: Carrier, p: Pose, place: Rect, page: Rect) -> Dragon {
    let r = rig(by, p, place, page);
    let s = r.size;
    // It flies away from the side it came from.
    let ahead = if matches!(by, Carrier::Dragon { right: true }) { -1.0 } else { 1.0 };
    let body = r.hook - vec2(0.0, 0.85 * s);
    let (aim, turned) = gaze(p.look, ahead);
    let head = body + vec2(ahead * 1.15 * s, -0.7 * s) + aim * 0.12 * s;
    let hr = 0.32 * s;
    let snout = head + aim * hr * 0.75 * turned + vec2(0.0, hr * 0.35) * (1.0 - turned);
    let mouth = snout + aim * hr * (0.5 + 0.35 * turned) * turned + vec2(0.0, hr * 0.3) * (1.0 - turned);
    let feet = if p.hooked {
        let load = place.translate(r.offset);
        let grip = (load.width() * 0.4).min(0.55 * s);
        [load.center_top() + vec2(ahead * grip, 0.0), load.center_top() - vec2(ahead * grip, 0.0)]
    } else {
        // Tucked in under it.
        [body + vec2(ahead * 0.2 * s, 0.6 * s), body + vec2(-ahead * 0.55 * s, 0.6 * s)]
    };
    Dragon { body, size: s, ahead, head, aim, turned, mouth, feet }
}

/// The dragon at `p`, the title's place at `place`: its wings beating, its tail swaying, the title
/// in its claws while it holds it, its head turned where it looks, and fire from its mouth.
pub fn dragon(by: Carrier, p: Pose, place: Rect, page: Rect) -> Vec<Shape> {
    let d = dragon_at(by, p, place, page);
    let (b, s, f, t) = (d.body, d.size, d.ahead, p.t);
    let line = (page.width() * 0.003).max(1.0);
    let mut out = Vec::new();
    // A wing from its shoulder: a fan of convex pieces from the shoulder to its scalloped edge,
    // up and down with the beat.
    let lift = 0.32 + 0.68 * (2.0 * PI * t / FLAP).sin();
    let wing = |out: &mut Vec<Shape>, shoulder: Pos2, scale: f32, color: Color32| {
        let tip = shoulder + vec2(-f * 0.9 * s, -2.2 * s * lift) * scale;
        let elbow = shoulder + (tip - shoulder) * 0.45 + vec2(f * 0.3 * s, 0.0) * scale;
        let root = b + vec2(-f * 0.8 * s, -0.1 * s);
        let edge: Vec<Pos2> = (1..4).map(|j| {
            let q = tip + (root - tip) * (j as f32 / 4.0);
            q + (q - shoulder).normalized() * 0.2 * s * scale
        }).collect();
        let points = [elbow, tip, edge[0], edge[1], edge[2], root];
        for w in points.windows(2) {
            out.push(Shape::convex_polygon(vec![shoulder, w[0], w[1]], color, Stroke::NONE));
        }
        let bone = Stroke::new(line, SCALES_DARK);
        out.push(Shape::line(vec![shoulder, elbow, tip], bone));
        for q in &edge {
            out.push(Shape::line_segment([elbow, *q], Stroke::new(line * 0.6, SCALES_DARK)));
        }
    };
    let shoulder = b + vec2(f * 0.1 * s, -0.28 * s);
    wing(&mut out, shoulder + vec2(f * 0.2 * s, -0.05 * s), 0.85, WING_FAR);
    // The tail, swaying, thinner towards its spade.
    let tail: Vec<Pos2> = (0..=6).map(|k| {
        let k = k as f32;
        b + vec2(-f * (0.8 + 0.24 * k) * s, (0.06 * k + 0.1 * (3.0 * t + k * 0.9).sin() * k / 6.0) * s)
    }).collect();
    for (k, w) in tail.windows(2).enumerate() {
        let width = 0.38 * s * (1.0 - k as f32 / 7.5);
        out.push(Shape::line_segment([w[0], w[1]], Stroke::new(width, SCALES)));
        out.push(Shape::circle_filled(w[1], width / 2.0, SCALES));
    }
    let end = tail[6];
    out.push(Shape::convex_polygon(vec![end + vec2(-f * 0.32 * s, 0.04 * s), end + vec2(0.0, 0.16 * s), end + vec2(0.0, -0.16 * s)], SCALES_DARK, Stroke::NONE));
    // Its legs, down to the title's top, the claws curled over it.
    for (hip, foot) in [(b + vec2(f * 0.4 * s, 0.2 * s), d.feet[0]), (b + vec2(-f * 0.35 * s, 0.2 * s), d.feet[1])] {
        let knee = hip + (foot - hip) * 0.5 + vec2(-f * 0.12 * s, 0.0);
        out.push(Shape::line_segment([hip, knee], Stroke::new(0.18 * s, SCALES)));
        out.push(Shape::line_segment([knee, foot], Stroke::new(0.1 * s, SCALES)));
        for dx in [-0.1, 0.0, 0.1] {
            out.push(Shape::line_segment([foot, foot + vec2(dx * s, 0.12 * s)], Stroke::new(line, CLAW)));
        }
    }
    let along = vec2(f, -0.15).normalized();
    out.push(oval(b, along, 0.9 * s, 0.45 * s, SCALES));
    out.push(oval(b + vec2(0.0, 0.14 * s), along, 0.7 * s, 0.2 * s, BELLY));
    // Spikes along its back.
    for k in 0..4 {
        let at = b + along * (0.45 - 0.3 * k as f32) * s + vec2(-along.y, along.x) * -f * 0.36 * s;
        let w = vec2(along.x, along.y) * 0.1 * s;
        out.push(Shape::convex_polygon(vec![at - w, at + w, at - w * 0.4 - vec2(0.0, 0.2 * s)], SCALES_DARK, Stroke::NONE));
    }
    wing(&mut out, shoulder, 1.0, WING);
    // The neck up to the head: wide at the body, narrower at the head.
    let neck = b + along * 0.65 * s;
    let n = (d.head - neck).normalized();
    let (wn, wh) = (vec2(-n.y, n.x) * 0.24 * s, vec2(-n.y, n.x) * 0.15 * s);
    out.push(Shape::convex_polygon(vec![neck + wn, d.head + wh, d.head - wh, neck - wn], SCALES, Stroke::NONE));
    // Its head, turned where it looks: in profile one eye shows and the snout is long; looking at
    // the writer, both eyes, and the snout short towards them.
    let (h, hr, aim, turned) = (d.head, 0.32 * s, d.aim, d.turned);
    let back = vec2(-aim.x.signum() * turned, 0.0);
    for w in [-1.0, 1.0] {
        let base = h + vec2(w * 0.3 * hr * (1.0 - turned), -0.75 * hr) + back * 0.2 * hr;
        let tip = base + vec2(w * 0.3 * hr * (1.0 - turned), -0.8 * hr) + back * 0.6 * hr;
        out.push(Shape::convex_polygon(vec![base - vec2(0.16 * hr, 0.0), tip, base + vec2(0.16 * hr, 0.0)], HORN, Stroke::NONE));
    }
    out.push(Shape::circle_filled(h, hr, SCALES));
    let snout = h + aim * hr * 0.75 * turned + vec2(0.0, hr * 0.35) * (1.0 - turned);
    let muzzle = if turned < 0.05 { vec2(1.0, 0.0) } else { aim };
    out.push(oval(snout, muzzle, hr * (0.5 + 0.35 * turned), hr * 0.5, SCALES));
    let across = vec2(1.0, 0.0) * (1.0 - turned);
    for w in [-1.0, 1.0] {
        let nostril = snout + aim * hr * 0.5 * turned + across * w * 0.2 * hr;
        out.push(Shape::circle_filled(nostril, 0.06 * s, SCALES_DARK));
    }
    let eye = h + vec2(0.0, -0.25 * hr) - aim * 0.1 * hr * turned;
    let eyes: &[f32] = if turned < 0.6 { &[-1.0, 1.0] } else { &[0.0] };
    for w in eyes {
        let at = eye + vec2(w * 0.42 * hr * (1.0 - turned), 0.0);
        out.push(Shape::circle_filled(at, 0.16 * hr, EYE));
        out.push(Shape::circle_filled(at + aim * 0.05 * hr, 0.07 * hr, PUPIL));
    }
    // Fire: always a little, and a great breath now and then; it flickers. Looking back over
    // itself at the city it only puffs, so as not to breathe fire over its own back.
    let back = (1.0 + aim.x * f).clamp(0.25, 1.0);
    let breath = (0.35 + 0.65 * (2.0 * PI * t / BREATH).sin().max(0.0)) * back;
    let dir = Vec2::angled(aim.angle() + 0.08 * (17.0 * t).sin());
    let len = s * 1.6 * breath * (0.6 + 0.4 * turned);
    let width = 0.18 * (1.0 + 0.6 * (1.0 - turned));
    for (k, color) in FIRE.iter().enumerate() {
        let share = 1.0 - 0.28 * k as f32;
        let flicker = 1.0 + 0.12 * (31.0 * t + k as f32 * 2.1).sin();
        out.push(flame(d.mouth, dir, len * share * flicker, len * share * width, *color));
    }
    for k in 0..3 {
        let spark = d.mouth + dir * len * (1.05 + 0.15 * k as f32) + vec2(-dir.y, dir.x) * len * 0.12 * (9.0 * t + k as f32 * 2.0).sin();
        out.push(Shape::circle_filled(spark, 0.035 * s * breath, FIRE[1]));
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

    /// The paragraph at the caret was just made a chapter title: now and then, in a magic theme,
    /// the dragon or the balloon comes to bring it into place.
    pub fn maybe_lift_title(&mut self, ctx: &egui::Context) {
        let Some(mut by) = Carrier::of(self.themes.theme) else { return };
        let (at, _) = self.doc.para_start(self.selection().0);
        // Only a title with words in it: an empty one is no load.
        if !self.doc.chapters(false).iter().any(|e| e.at == at) || self.eggs.roll() >= self.eggs.odds {
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

    /// The dragon or the balloon over page `i` lying at `page`, if it is at work there.
    pub fn draw_carrier(&self, ui: &egui::Ui, page: Rect, i: usize) {
        let (Some((by, p, _)), Some(place)) = (self.lifting(ui.ctx(), i), self.eggs.place) else { return };
        ui.painter().extend(match by {
            Carrier::Dragon { .. } => dragon(by, p, place, page),
            Carrier::Balloon { .. } => balloon(by, p, place, page),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // A title high on its page: the carrier hangs lower and is smaller, its top still near the page.
        let high = Rect::from_min_size(pos2(500.0, 120.0), vec2(200.0, 30.0));
        for (by, tall) in [(Carrier::Balloon { right: true }, TALL), (Carrier::Dragon { right: true }, DRAGON_TALL)] {
            let r = rig(by, pose(ENTER + RUN, None), high, page);
            assert!(r.hook.y - r.size * tall >= page.top() - page.height() * 0.05, "{by:?}: {:?} {}", r.hook, r.size);
        }
        let p = pose(ENTER + RUN, None);
        let top = dragon(Carrier::Dragon { right: true }, p, high, page).iter().fold(Rect::NOTHING, |r, s| r | s.visual_bounding_rect()).top();
        assert!(top >= page.top() - page.height() * 0.05, "the dragon's wings stay near the page: {top}");
    }

    /// The shapes of the dragon at `t` seconds bringing a title at `place`, in `colors`.
    fn dragon_in(t: f32, place: Rect, page: Rect, colors: &[Color32]) -> Vec<Shape> {
        let by = Carrier::Dragon { right: true };
        dragon(by, pose(t, None), place, page).into_iter().filter(|s| match s {
            Shape::Path(path) => colors.contains(&path.fill),
            Shape::Circle(c) => colors.contains(&c.fill),
            _ => false,
        }).collect()
    }

    #[test]
    fn the_dragon_beats_its_wings_and_breathes_fire_from_its_mouth() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let over = ENTER + RUN;
        let tip = |t: f32| dragon_in(t, place, page, &[WING]).iter().map(|s| s.visual_bounding_rect().top()).fold(f32::MAX, f32::min);
        let (up, down) = (tip(over + FLAP / 4.0), tip(over + FLAP * 3.0 / 4.0));
        let d = dragon_at(Carrier::Dragon { right: true }, pose(over, None), place, page);
        assert!(down - up > d.size, "up and down: {up} {down}");
        for k in 0..8 {
            let t = k as f32 * 0.37;
            let d = dragon_at(Carrier::Dragon { right: true }, pose(t, None), place, page);
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
    fn the_dragon_looks_at_the_city_then_at_the_writer_then_at_the_page() {
        let page = Rect::from_min_size(pos2(300.0, 50.0), vec2(600.0, 800.0));
        let place = Rect::from_min_size(pos2(500.0, 400.0), vec2(200.0, 30.0));
        let by = Carrier::Dragon { right: true };
        let at = |t: f32| dragon_at(by, pose(t, None), place, page);
        let eyes = |t: f32| dragon_in(t, place, page, &[EYE]).len();
        // In from the right, flying left: the houses it passed are behind it, to the right.
        let coming = at(ENTER * 0.8);
        assert!(coming.ahead < 0.0 && coming.aim.x > 0.5 && coming.aim.y > 0.0 && coming.turned == 1.0, "back down at the city: {:?}", coming.aim);
        assert_eq!(eyes(ENTER * 0.8), 1, "in profile");
        let over = ENTER + RUN + LOWER * 0.2;
        assert!(at(over).turned == 0.0 && eyes(over) == 2, "at the writer, with both eyes");
        let lowering = ENTER + RUN + LOWER;
        let p = at(lowering);
        assert!(p.aim.x < -0.3 && p.aim.y > 0.7 && p.turned == 1.0 && eyes(lowering) == 1, "down ahead at the page: {:?}", p.aim);
        // And it held the title by its feet all the while, at its top, either side of its middle.
        for t in [0.0, ENTER, over, lowering] {
            let d = at(t);
            let load = place.translate(offset(by, pose(t, None), place, page));
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

