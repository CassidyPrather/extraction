//! What's said, where it can be read without looking down (#90). Every
//! line is on the wrist (`render::comfort`); a line told for the view
//! (`raid::Raid::tell`) is also put in front of the eyes:
//!
//! - a line about a thing in the hand, as a callout beside it: it follows
//!   the thing on a damped spring, a little behind, never over the sights;
//! - a line to act on now, as a caption in the lower view: it stays where
//!   it is while you glance about, and comes round when you turn away; it
//!   keeps clear of what you're holding.
//!
//! Both are cool off-white on a dark slate (`ScreenStyle::Slate`), lit
//! only as much as the eyes are open (`grade::Adaptation`): near black at
//! any hour, never a glow. An urgent line has an orange-red bar down its
//! left edge, the game's warning colour, and no later line takes its
//! place inside its reading time. The Captions card (`Comfort::in_view`)
//! puts them back on the wrist only.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::captions::{Caption, Speaker, parse};
use crate::raid::{Raid, Show};
use crate::render::legibility::{TextSource, X_HEIGHT};
use crate::render::screen::PanelLight;

/// The caption: how far out (m), how far below the eye line (degrees),
/// and how high its smallest letters stand (degrees: over
/// `legibility::MUST_READ_DEG`'s 0.55).
pub const CAPTION_AT: f32 = 1.4;
pub const CAPTION_DOWN: f32 = 12.0;
pub const CAPTION_X_DEG: f32 = 0.9;
/// Letters a caption's line: the radio's measure (three lines of 26).
pub const CAPTION_COLS: usize = 26;
/// The caption keeps this clear (degrees) of what's in a hand, moving up
/// toward the eye line as far as `CAPTION_HIGHEST` below it, else down
/// as far as `CAPTION_LOWEST`.
pub const CAPTION_CLEAR: f32 = 3.0;
pub const CAPTION_HIGHEST: f32 = 6.0;
pub const CAPTION_LOWEST: f32 = 20.0;
/// The caption's soft lock: it stays put till the head turns this far
/// from it (degrees), then comes round with this time constant (s).
pub const LOCK_YAW: f32 = 18.0;
pub const LOCK_PITCH: f32 = 10.0;
pub const LOCK_TAU: f32 = 0.4;

/// A callout: about how far from the eye it's read (m: the thing held at
/// a forearm's reach), its letters (degrees), letters a line, and the gap
/// between the thing and it (m).
pub const CALLOUT_AT: f32 = 0.5;
pub const CALLOUT_X_DEG: f32 = 0.65;
pub const CALLOUT_COLS: usize = 20;
pub const CALLOUT_GAP: f32 = 0.04;
/// Its follow: a critically damped spring (rad/s: about 0.13 s behind,
/// no overshoot), its turn eased (s), and a hand's tremor under this
/// (m) not followed at all.
pub const CALLOUT_OMEGA: f32 = 8.0;
pub const CALLOUT_TURN_TAU: f32 = 0.15;
pub const CALLOUT_STILL: f32 = 0.01;
/// A callout's centre keeps this far (degrees) off a held gun's sight
/// line; the gun at the eye fades its own callout in `SIGHT_FADE` (s).
pub const SIGHT_CLEAR: f32 = 12.0;
pub const SIGHT_FADE: f32 = 0.15;

/// How long a line shows: a second and a half, and about 200 words a
/// minute, between 3 and 9 s.
pub fn reading_time(text: &str) -> f32 {
    (1.5 + 0.06 * text.chars().count() as f32).clamp(3.0, 9.0)
}
/// Looked at (within `GAZE_CONE` degrees), a line holds up to this much
/// longer (s); then it fades over `FADE` (s).
pub const GAZE_HOLD: f32 = 4.0;
pub const GAZE_CONE: f32 = 10.0;
pub const FADE: f32 = 0.3;
/// A line shows at least this long (s) before another takes its place.
pub const MIN_SHOW: f32 = 1.5;
/// The urgent bar's width (m, at the caption's distance).
pub const URGENT_BAR: f32 = 0.012;

pub struct SaidPlugin;

impl Plugin for SaidPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptionLines>()
            .add_systems(Update, (take_shown, show_caption, show_callouts).chain());
    }
}

/// One line in the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub header: Option<String>,
    pub words: String,
    pub urgent: bool,
    /// Seconds shown, and seconds it may show (looking at it adds some).
    pub shown: f32,
    pub time: f32,
}

impl Line {
    pub fn new(header: Option<String>, words: String, urgent: bool) -> Self {
        let time = reading_time(&words);
        Self {
            header,
            words,
            urgent,
            shown: 0.0,
            time,
        }
    }

    fn done(&self) -> bool {
        self.shown >= self.time
    }
}

/// The caption's lines: the one shown and those waiting their turn.
#[derive(Resource, Default, Debug)]
pub struct CaptionLines {
    pub now: Option<Line>,
    pub waiting: VecDeque<Line>,
}

/// At most this many wait; past it the oldest that isn't urgent goes.
const WAITING_MAX: usize = 3;

impl CaptionLines {
    pub fn push(&mut self, line: Line) {
        self.waiting.push_back(line);
        if self.waiting.len() > WAITING_MAX {
            let i = self.waiting.iter().position(|l| !l.urgent).unwrap_or(0);
            self.waiting.remove(i);
        }
    }

    /// Time passes (`gazed`: the shown line is being looked at). A waiting
    /// line takes the place of the shown one once that has had its
    /// `MIN_SHOW`, unless it's urgent: an urgent line has its whole time.
    /// An urgent line waiting goes before the others.
    pub fn tick(&mut self, dt: f32, gazed: bool) {
        if let Some(now) = &mut self.now {
            now.shown += dt;
            if gazed {
                now.time = now
                    .time
                    .max((now.shown + dt).min(reading_time(&now.words) + GAZE_HOLD));
            }
        }
        let yield_now = match &self.now {
            None => true,
            Some(l) if l.done() => true,
            Some(l) => !l.urgent && l.shown >= MIN_SHOW && !self.waiting.is_empty(),
        };
        if yield_now {
            let next = match self.waiting.iter().position(|l| l.urgent) {
                Some(i) => self.waiting.remove(i),
                None => self.waiting.pop_front(),
            };
            match next {
                Some(l) => self.now = Some(l),
                None if self.now.as_ref().is_some_and(Line::done) => self.now = None,
                None => {}
            }
        }
    }

    /// How much of the shown line shows, 0..=1: it fades in over `FADE`
    /// and out over the last `FADE` of its time.
    pub fn alpha(&self) -> f32 {
        self.now.as_ref().map_or(0.0, |l| {
            (l.shown / FADE)
                .min((l.time - l.shown) / FADE)
                .clamp(0.0, 1.0)
        })
    }
}

/// Words wrapped to `cols` a line (a line's own breaks are spaces: the
/// panel's measure decides).
pub fn wrap(words: &str, cols: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for w in words.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + w.chars().count() > cols {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(w);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// A panel's text and size (m) for `header` and `words`, `at` metres from
/// the eye, its smallest letters `x_deg` high, `cols` a line: the panel is
/// as wide as its longest line and as tall as its lines, so its letters
/// come out at `x_deg` and no smaller.
pub fn panel(header: Option<&str>, words: &str, cols: usize, at: f32, x_deg: f32) -> ScreenText {
    let mut lines: Vec<String> = header.map(|h| h.to_uppercase()).into_iter().collect();
    lines.extend(wrap(words, cols));
    let widest = lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let em = at * x_deg.to_radians().tan() / X_HEIGHT;
    let n = lines.len().max(1) as f32;
    ScreenText {
        text: lines.join("\n"),
        // `screen::fit_font`'s metrics: 0.64 em a letter, 1.2 em a line,
        // 0.8 em of margin a side; a little over, so it fits.
        size: Vec2::new(em * (widest * 0.64 + 1.6), em * (n * 1.2 + 1.6)) * 1.02,
        style: ScreenStyle::Slate,
    }
}

/// The soft lock: where the caption is anchored (yaw, pitch: radians) and
/// whether it's coming round to the head.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoftLock {
    pub yaw: f32,
    pub pitch: f32,
    pub chasing: bool,
    pub set: bool,
}

/// The shortest signed turn from `a` to `b` (radians).
fn turn(a: f32, b: f32) -> f32 {
    (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

impl SoftLock {
    /// Follows a head looking at `yaw`, `pitch` (radians) over `dt`.
    pub fn step(&mut self, yaw: f32, pitch: f32, dt: f32) {
        if !self.set {
            *self = Self {
                yaw,
                pitch,
                chasing: false,
                set: true,
            };
            return;
        }
        let (dy, dp) = (turn(self.yaw, yaw), pitch - self.pitch);
        if dy.abs() > LOCK_YAW.to_radians() || dp.abs() > LOCK_PITCH.to_radians() {
            self.chasing = true;
        }
        if self.chasing {
            let k = 1.0 - (-dt / LOCK_TAU).exp();
            self.yaw += dy * k;
            self.pitch += dp * k;
            if dy.abs() < 1f32.to_radians() && dp.abs() < 1f32.to_radians() {
                self.chasing = false;
            }
        }
    }
}

/// A critically damped follow (`CALLOUT_OMEGA`), a hand's tremor
/// (`CALLOUT_STILL`) left out.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Follow {
    pub at: Vec3,
    pub vel: Vec3,
    pub goal: Vec3,
    pub set: bool,
}

impl Follow {
    pub fn step(&mut self, goal: Vec3, dt: f32) -> Vec3 {
        if !self.set {
            *self = Self {
                at: goal,
                vel: Vec3::ZERO,
                goal,
                set: true,
            };
            return goal;
        }
        if goal.distance(self.goal) > CALLOUT_STILL {
            self.goal = goal;
        }
        let w = CALLOUT_OMEGA;
        // Integrated in small steps: stable at any frame time.
        let n = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            let acc = -2.0 * w * self.vel - w * w * (self.at - self.goal);
            self.vel += acc * h;
            self.at += self.vel * h;
        }
        self.at
    }
}

/// `at` moved round the eye, the least it takes, to keep `SIGHT_CLEAR`
/// off the sight line from `eye` along `line`.
pub fn off_the_line(eye: Vec3, line: Vec3, at: Vec3) -> Vec3 {
    let to = at - eye;
    let d = to.length();
    let line = line.normalize_or(Vec3::NEG_Z);
    if d < 1e-4 {
        return at;
    }
    let dir = to / d;
    let clear = SIGHT_CLEAR.to_radians();
    if dir.angle_between(line) >= clear {
        return at;
    }
    // Away from the line in the plane of the two; straight down off it
    // if it's dead on.
    let away = (dir - line * dir.dot(line))
        .normalize_or(Vec3::NEG_Y.reject_from(line).normalize_or(Vec3::X));
    eye + (line * clear.cos() + away * clear.sin()) * d
}

/// How far below the eye line (degrees) the caption sits, given what's
/// held as seen from its anchor: each thing's outline (its least and most
/// degrees right and below), and the caption's angular half width and
/// half height. `CAPTION_DOWN` if that's clear by `CAPTION_CLEAR`; else
/// the nearest clear spot up to `CAPTION_HIGHEST`, else down to
/// `CAPTION_LOWEST`; else above it, as high as the eye line; if none is
/// clear, the least crowded of them.
pub fn caption_down(held: &[(Vec2, Vec2)], half: Vec2) -> f32 {
    // How far (degrees) the caption at `down` is from clearing everything
    // held: 0 when it does.
    let short = |down: f32| {
        held.iter()
            .map(|(lo, hi)| {
                let dx = (lo.x - half.x).max(-half.x - hi.x).max(0.0);
                let dy = (lo.y - (down + half.y))
                    .max((down - half.y) - hi.y)
                    .max(0.0);
                (CAPTION_CLEAR - (dx * dx + dy * dy).sqrt()).max(0.0)
            })
            .fold(0.0, f32::max)
    };
    let ups = (0..)
        .map(|i| CAPTION_DOWN - 0.5 * i as f32)
        .take_while(|d| *d >= CAPTION_HIGHEST);
    let downs = (1..)
        .map(|i| CAPTION_DOWN + 0.5 * i as f32)
        .take_while(|d| *d <= CAPTION_LOWEST);
    // Last, above what's held as far as the eye line: never hidden
    // behind it (a pistol raised to look at fills the whole range).
    let over = (1..)
        .map(|i| CAPTION_HIGHEST - 0.5 * i as f32)
        .take_while(|d| *d >= 0.0);
    let tries: Vec<f32> = ups.chain(downs).chain(over).collect();
    // The nearest clear spot, up first; none clear, the least crowded.
    tries
        .iter()
        .copied()
        .find(|d| short(*d) == 0.0)
        .unwrap_or_else(|| {
            tries
                .iter()
                .copied()
                .min_by(|a, b| short(*a).total_cmp(&short(*b)))
                .unwrap_or(CAPTION_DOWN)
        })
}

/// What a glove adds round a thing held (m), and a glove's own reach
/// from its middle, cuff and all.
const FIST: f32 = 0.05;
const GLOVE: f32 = 0.12;

/// A panel's light for the eye's adaptation: a screen's own by day, as
/// much less as the eyes open, so the slate stays near black.
pub fn glow(stops: f32) -> f32 {
    2f32.powf(-stops.max(0.0))
}

/// The caption's panel.
#[derive(Component)]
struct CaptionPanel;

/// Its urgent bar.
#[derive(Component)]
struct UrgentBar;

/// A callout beside a thing in hand.
#[derive(Component, Debug)]
pub struct Callout {
    pub item: Entity,
    pub line: Line,
    pub follow: Follow,
    /// Faded by the sights, 0..=1 (1: shows).
    pub sights: f32,
}

/// Screenshots (`dev`): lines stay up, however slowly the frames come
/// (a software render's few a second would read a line out before the
/// picture's taken).
#[derive(Resource, Default)]
pub struct HoldForShot;

/// A thing a dev pose has in hand (screenshots): counted as held by the
/// local player in that hand.
#[derive(Component, Clone, Copy, Debug)]
pub struct Posed(pub crate::rig::Side);

/// The eye: the free camera's (a screenshot, the tour), else the local
/// player's head.
fn the_eye(
    free: &Query<&GlobalTransform, With<crate::render::FreeCamera>>,
    heads: &Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    locals: &Query<Entity, With<crate::rig::LocalPlayer>>,
) -> Option<GlobalTransform> {
    if let Some(g) = free.iter().next() {
        return Some(*g);
    }
    let me = locals.iter().next()?;
    heads
        .iter()
        .find(|(_, p)| p.parent() == me)
        .map(|(g, _)| *g)
}

type HeldQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static GlobalTransform,
        &'static crate::items::Item,
        Option<&'static crate::interact::Held>,
        Option<&'static Posed>,
    ),
    Or<(With<crate::interact::Held>, With<Posed>)>,
>;

/// The local player's things in hand: (entity, where, half size, hand).
fn in_hand(
    held: &HeldQuery,
    locals: &Query<Entity, With<crate::rig::LocalPlayer>>,
    catalog: &crate::items::Catalog,
) -> Vec<(Entity, Transform, Vec3, crate::rig::Side)> {
    let me = locals.iter().next();
    held.iter()
        .filter_map(|(e, g, item, h, posed)| {
            let side = match (h, posed) {
                (Some(h), _) if Some(h.player) == me => h.side,
                (_, Some(p)) => p.0,
                _ => return None,
            };
            let half = Vec3::from(catalog.get(item.kind()).size_m()) * 0.5;
            Some((e, g.compute_transform(), half, side))
        })
        .collect()
}

/// The words of a said line and who says them, for a header.
fn header_and_words(line: &str) -> (Option<String>, String) {
    let (speaker, words) = parse(line);
    let header = Caption {
        speaker,
        text: String::new(),
        from: None,
    }
    .header(None)
    .filter(|_| speaker != Speaker::None);
    (header, words)
}

/// Takes what's told for the view: a line about a thing in hand to its
/// callout, the rest to the caption. With captions off the view, they're
/// on the wrist alone.
#[allow(clippy::too_many_arguments)]
fn take_shown(
    mut commands: Commands,
    mut raid: ResMut<Raid>,
    mut lines: ResMut<CaptionLines>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    held: HeldQuery,
    locals: Query<Entity, With<crate::rig::LocalPlayer>>,
    catalog: Res<crate::items::Catalog>,
    callouts: Query<(Entity, &Callout)>,
) {
    if raid.shown.is_empty() {
        return;
    }
    let shown: Vec<_> = raid.shown.drain(..).collect();
    if !comfort.is_none_or(|c| c.in_view()) {
        return;
    }
    let hands = in_hand(&held, &locals, &catalog);
    for (text, show) in shown {
        let (header, words) = header_and_words(&text);
        match show {
            Show::Wrist => {}
            Show::Item(item) if hands.iter().any(|h| h.0 == item) => {
                let name = held
                    .get(item)
                    .map(|(_, _, i, _, _)| catalog.get(i.kind()).name.clone())
                    .ok();
                let line = Line::new(name, words, false);
                // One callout a thing: a new line replaces its last, a new
                // panel sized to it, where the last one was.
                let mut follow = Follow::default();
                for (e, c) in &callouts {
                    if c.item == item {
                        follow = c.follow;
                        commands.entity(e).despawn();
                    }
                }
                commands.spawn((
                    Callout {
                        item,
                        line,
                        follow,
                        sights: 1.0,
                    },
                    Transform::default(),
                    Visibility::Hidden,
                ));
            }
            Show::Item(_) | Show::View => lines.push(Line::new(header, words, false)),
            Show::Urgent => lines.push(Line::new(header, words, true)),
        }
    }
}

/// Head yaw and pitch (radians) from a head's rotation.
fn yaw_pitch(rot: Quat) -> (f32, f32) {
    let (y, p, _) = rot.to_euler(EulerRot::YXZ);
    (y, p)
}

/// Where (degrees right, degrees below) `at` is, seen from `eye` in the
/// frame of `yaw` with the horizon level.
fn seen(eye: Vec3, yaw: f32, at: Vec3) -> Vec2 {
    let local = Quat::from_rotation_y(-yaw) * (at - eye);
    let right = local.x.atan2(-local.z).to_degrees();
    let below = (-local.y)
        .atan2((local.x * local.x + local.z * local.z).sqrt())
        .to_degrees();
    Vec2::new(right, below)
}

#[allow(clippy::too_many_arguments)]
fn show_caption(
    mut commands: Commands,
    mut lines: ResMut<CaptionLines>,
    mut lock: Local<SoftLock>,
    mut showing: Local<Option<(Entity, Option<Entity>, String)>>,
    time: Res<Time>,
    adapt: Option<Res<crate::render::grade::Adaptation>>,
    free: Query<&GlobalTransform, With<crate::render::FreeCamera>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    locals: Query<Entity, With<crate::rig::LocalPlayer>>,
    held: HeldQuery,
    catalog: Res<crate::items::Catalog>,
    mut panels: Query<&mut Transform, Or<(With<CaptionPanel>, With<UrgentBar>)>>,
    hold: Option<Res<HoldForShot>>,
    gloves: Query<(&GlobalTransform, &InheritedVisibility), With<crate::render::props::HandVisual>>,
) {
    let dt = time.delta_secs();
    let Some(eye) = the_eye(&free, &heads, &locals) else {
        return;
    };
    let gloves: Vec<GlobalTransform> = gloves
        .iter()
        .filter(|(g, v)| v.get() && g.translation().distance(eye.translation()) < 1.5)
        .map(|(g, _)| *g)
        .collect();
    let (yaw, pitch) = yaw_pitch(eye.rotation());
    lock.step(yaw, pitch, dt);
    // Looked at: the caption's middle within `GAZE_CONE` of the gaze.
    let fwd = eye.rotation() * Vec3::NEG_Z;
    let gazed = showing
        .as_ref()
        .and_then(|(e, _, _)| panels.get(*e).ok())
        .is_some_and(|t| {
            (t.translation - eye.translation()).angle_between(fwd) < GAZE_CONE.to_radians()
        });
    lines.tick(dt, gazed);
    if hold.is_some() {
        if let Some(l) = &mut lines.now {
            l.time = l.time.max(l.shown + 10.0);
        }
    }
    let want = lines
        .now
        .as_ref()
        .map(|l| (l.header.clone(), l.words.clone(), l.urgent));
    let key = want.as_ref().map(|(h, w, u)| format!("{h:?}{w}{u}"));
    // A new line: a new panel, sized to it.
    if showing.as_ref().map(|s| &s.2) != key.as_ref() {
        if let Some((p, bar, _)) = showing.take() {
            commands.entity(p).despawn();
            if let Some(b) = bar {
                commands.entity(b).despawn();
            }
        }
        if let (Some((header, words, urgent)), Some(key)) = (want, key) {
            let screen = panel(
                header.as_deref(),
                &words,
                CAPTION_COLS,
                CAPTION_AT,
                CAPTION_X_DEG,
            );
            let size = screen.size;
            let p = commands
                .spawn((
                    CaptionPanel,
                    TextSource::Caption,
                    Name::new("caption"),
                    screen,
                    PanelLight {
                        glow: 0.0,
                        alpha: 0.0,
                    },
                    Transform::default(),
                ))
                .id();
            let bar = urgent.then(|| {
                commands
                    .spawn((
                        UrgentBar,
                        TextSource::Caption,
                        crate::raid::bench::ScreenText {
                            text: " ".into(),
                            size: Vec2::new(URGENT_BAR, size.y),
                            style: ScreenStyle::Slate,
                        },
                        crate::render::screen::Ink {
                            ground: Some(Color::srgb(1.0, 0.24, 0.06)),
                            ink: Color::srgb(1.0, 0.24, 0.06),
                        },
                        PanelLight {
                            glow: 0.0,
                            alpha: 0.0,
                        },
                        Transform::default(),
                    ))
                    .id()
            });
            debug!("said: caption {key}");
            *showing = Some((p, bar, key));
        }
    }
    let Some((p, bar, _)) = showing.as_ref() else {
        return;
    };
    // Clear of what's in hand, seen from where the caption's anchored.
    let hands = in_hand(&held, &locals, &catalog);
    let anchor_eye = eye.translation();
    let size = lines.now.as_ref().map_or(Vec2::ONE, |l| {
        panel(
            l.header.as_deref(),
            &l.words,
            CAPTION_COLS,
            CAPTION_AT,
            CAPTION_X_DEG,
        )
        .size
    });
    let half = Vec2::new(
        (size.x * 0.5 / CAPTION_AT).atan().to_degrees(),
        (size.y * 0.5 / CAPTION_AT).atan().to_degrees(),
    );
    // Each thing's outline as seen, below the anchor's eye line, grown by
    // the glove round it.
    let mut things: Vec<(Vec2, Vec2)> = hands
        .iter()
        .map(|(_, place, h, _)| {
            let mut lo = Vec2::splat(f32::INFINITY);
            let mut hi = Vec2::splat(f32::NEG_INFINITY);
            for i in 0..8 {
                let corner = Vec3::new(
                    if i & 1 == 0 { -h.x } else { h.x },
                    if i & 2 == 0 { -h.y } else { h.y },
                    if i & 4 == 0 { -h.z } else { h.z },
                );
                let c = seen(anchor_eye, lock.yaw, place.transform_point(corner));
                let c = Vec2::new(c.x, c.y + lock.pitch.to_degrees());
                lo = lo.min(c);
                hi = hi.max(c);
            }
            let d = place.translation.distance(anchor_eye).max(0.05);
            let fist = (FIST / d).atan().to_degrees();
            (lo - fist, hi + fist)
        })
        .collect();
    // And the gloves themselves, holding or not, as drawn: a glove with
    // its cuff is bigger than the thing in it, and hangs below it.
    for g in &gloves {
        let at = g.translation();
        let d = at.distance(anchor_eye).max(0.05);
        let c = seen(anchor_eye, lock.yaw, at);
        let c = Vec2::new(c.x, c.y + lock.pitch.to_degrees());
        let r = (GLOVE / d).atan().to_degrees();
        things.push((c - r, c + r));
    }
    let down = caption_down(&things, half);
    let rot = Quat::from_euler(EulerRot::YXZ, lock.yaw, lock.pitch - down.to_radians(), 0.0);
    let at = anchor_eye + rot * Vec3::NEG_Z * CAPTION_AT;
    let alpha = lines.alpha();
    let g = glow(adapt.as_ref().map_or(0.0, |a| a.stops));
    let place = Transform::from_translation(at).with_rotation(rot);
    if let Ok(mut t) = panels.get_mut(*p) {
        *t = place;
    }
    commands.entity(*p).insert(PanelLight { glow: g, alpha });
    if let Some(b) = bar {
        if let Ok(mut t) = panels.get_mut(*b) {
            *t = place * Transform::from_xyz(-size.x * 0.5 - URGENT_BAR * 0.5, 0.0, 0.0);
        }
        commands.entity(*b).insert(PanelLight { glow: g, alpha });
    }
}

#[allow(clippy::too_many_arguments)]
fn show_callouts(
    mut commands: Commands,
    mut callouts: Query<(Entity, &mut Callout, &mut Transform, Option<&ScreenText>)>,
    time: Res<Time>,
    adapt: Option<Res<crate::render::grade::Adaptation>>,
    free: Query<&GlobalTransform, With<crate::render::FreeCamera>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    locals: Query<Entity, With<crate::rig::LocalPlayer>>,
    held: HeldQuery,
    catalog: Res<crate::items::Catalog>,
    guns: Query<(&crate::weapons::Gun, &GlobalTransform)>,
    hold: Option<Res<HoldForShot>>,
) {
    let dt = time.delta_secs();
    let Some(eye) = the_eye(&free, &heads, &locals) else {
        return;
    };
    let hands = in_hand(&held, &locals, &catalog);
    let (yaw, pitch) = yaw_pitch(eye.rotation());
    let face = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    let right = face * Vec3::X;
    let fwd = eye.rotation() * Vec3::NEG_Z;
    let g = glow(adapt.as_ref().map_or(0.0, |a| a.stops));
    // The sight lines of the guns in hand, and whether the eye's at them.
    let sights: Vec<(Entity, Vec3, bool)> = hands
        .iter()
        .filter_map(|(e, ..)| {
            let (gun, gt) = guns.get(*e).ok()?;
            let at_eye = gt.affine().inverse().transform_point3(eye.translation());
            let line = gt.rotation() * gun.layout.sight.rotation * Vec3::NEG_Z;
            Some((*e, line, crate::weapons::feel::at_sights(gun, at_eye)))
        })
        .collect();
    for (e, mut c, mut t, screen) in &mut callouts {
        let Some(&(item, place, half, side)) = hands.iter().find(|h| h.0 == c.item) else {
            // Put down or put away: its line goes with it.
            commands.entity(e).despawn();
            continue;
        };
        if screen.is_none() {
            let s = panel(
                c.line.header.as_deref(),
                &c.line.words,
                CALLOUT_COLS,
                CALLOUT_AT,
                CALLOUT_X_DEG,
            );
            commands.entity(e).insert((
                TextSource::Callout,
                Name::new("callout"),
                s,
                PanelLight {
                    glow: 0.0,
                    alpha: 0.0,
                },
                Visibility::Inherited,
            ));
            continue;
        }
        let size = screen.map_or(Vec2::ONE, |s| s.size);
        // Beside it on the hand's outer side, clear of its bounds.
        let out = right * side.sign();
        let reach = half.x.max(half.y).max(half.z);
        let at = place.translation;
        let mut goal = at + out * (reach + CALLOUT_GAP + size.x * 0.5);
        // Off every gun's sight line; the gun at the eye fades its own.
        let mut at_sights = false;
        for (gun, line, at_eye) in &sights {
            if *gun == item && *at_eye {
                at_sights = true;
            }
            goal = off_the_line(eye.translation(), *line, goal);
        }
        let now = c.follow.step(goal, dt);
        let k = 1.0 - (-dt / CALLOUT_TURN_TAU).exp();
        t.translation = now;
        t.rotation = t
            .rotation
            .slerp(face, if t.rotation == Quat::IDENTITY { 1.0 } else { k });
        c.sights = if at_sights {
            (c.sights - dt / SIGHT_FADE).max(0.0)
        } else {
            (c.sights + dt / SIGHT_FADE).min(1.0)
        };
        c.line.shown += dt;
        if hold.is_some() {
            c.line.time = c.line.time.max(c.line.shown + 10.0);
        }
        let gazed = (now - eye.translation()).angle_between(fwd) < GAZE_CONE.to_radians();
        if gazed {
            c.line.time = c
                .line
                .time
                .max((c.line.shown + dt).min(reading_time(&c.line.words) + GAZE_HOLD));
        }
        if c.line.done() {
            commands.entity(e).despawn();
            continue;
        }
        let fade = (c.line.shown / FADE)
            .min((c.line.time - c.line.shown) / FADE)
            .clamp(0.0, 1.0);
        commands.entity(e).insert(PanelLight {
            glow: g,
            alpha: fade * c.sights,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_shows_for_its_length_between_three_and_nine_seconds() {
        assert_eq!(reading_time("Copy."), 3.0);
        let long = "Move off quiet, then go still. Something heard you.";
        let t = reading_time(long);
        assert!((t - (1.5 + 0.06 * long.chars().count() as f32)).abs() < 1e-4);
        assert_eq!(reading_time(&"word ".repeat(60)), 9.0);
    }

    fn line(w: &str, urgent: bool) -> Line {
        Line::new(None, w.into(), urgent)
    }

    fn run(c: &mut CaptionLines, secs: f32) {
        for _ in 0..(secs * 90.0) as usize {
            c.tick(1.0 / 90.0, false);
        }
    }

    /// A line to act on takes the place of the last once that's had its
    /// moment; an urgent one is never taken over inside its time, and
    /// what comes meanwhile waits its turn.
    #[test]
    fn an_urgent_line_holds_for_its_time_and_later_ones_wait() {
        let mut c = CaptionLines::default();
        c.push(line("Heard you. Two minutes.", false));
        run(&mut c, 0.5);
        c.push(line("Wait by the road.", false));
        run(&mut c, 0.5);
        assert_eq!(
            c.now.as_ref().unwrap().words,
            "Heard you. Two minutes.",
            "not before MIN_SHOW"
        );
        run(&mut c, 0.6);
        assert_eq!(
            c.now.as_ref().unwrap().words,
            "Wait by the road.",
            "after MIN_SHOW"
        );

        let mut c = CaptionLines::default();
        let urgent = "Move off quiet, then go still. Something heard you.";
        c.push(line(urgent, true));
        run(&mut c, 0.1);
        c.push(line("Heard you. Two minutes.", false));
        let time = reading_time(urgent);
        run(&mut c, time - 0.3);
        assert_eq!(
            c.now.as_ref().unwrap().words,
            urgent,
            "held for its reading time"
        );
        run(&mut c, 0.5);
        assert_eq!(
            c.now.as_ref().unwrap().words,
            "Heard you. Two minutes.",
            "then the next"
        );
        // An urgent line waiting goes first.
        let mut c = CaptionLines::default();
        c.push(line("One.", false));
        run(&mut c, 0.1);
        c.push(line("Two.", false));
        c.push(line("Danger.", true));
        run(&mut c, MIN_SHOW + 0.1);
        assert_eq!(c.now.as_ref().unwrap().words, "Danger.");
        // Shown out, nothing waiting: the caption goes.
        run(&mut c, 10.0);
        run(&mut c, 10.0);
        assert!(c.now.is_none());
    }

    #[test]
    fn looking_at_a_line_holds_it_a_while_longer() {
        let mut c = CaptionLines::default();
        c.push(line("Copy.", false));
        for _ in 0..(12.0 * 90.0) as usize {
            c.tick(1.0 / 90.0, true);
        }
        assert!(c.now.is_none(), "held no more than GAZE_HOLD past its time");
        let mut c = CaptionLines::default();
        c.push(line("Copy.", false));
        for _ in 0..(5.0 * 90.0) as usize {
            c.tick(1.0 / 90.0, true);
        }
        assert!(c.now.is_some(), "held past its 3 s while looked at");
    }

    /// The caption stays put while you glance about, and comes round when
    /// you turn away, no further than the head.
    #[test]
    fn the_caption_stays_put_inside_its_dead_zone_and_follows_out_of_it() {
        let mut l = SoftLock::default();
        l.step(0.0, 0.0, 0.01);
        for _ in 0..200 {
            l.step(15f32.to_radians(), 8f32.to_radians(), 0.01);
        }
        assert_eq!((l.yaw, l.pitch), (0.0, 0.0), "a glance doesn't drag it");
        let mut max = 0f32;
        for _ in 0..400 {
            l.step(60f32.to_radians(), 0.0, 0.01);
            max = max.max(l.yaw);
        }
        assert!(
            (l.yaw - 60f32.to_radians()).abs() < 1.5f32.to_radians(),
            "came round"
        );
        assert!(max <= 60f32.to_radians() + 1e-4, "no overshoot");
        // Round past behind: the short way.
        let mut l = SoftLock::default();
        l.step(170f32.to_radians(), 0.0, 0.01);
        for _ in 0..400 {
            l.step(-170f32.to_radians(), 0.0, 0.01);
        }
        assert!(turn(l.yaw, -170f32.to_radians()).abs() < 1.5f32.to_radians());
    }

    /// The callout follows its thing a little behind, settles without
    /// overshoot, and doesn't swim with a hand's tremor.
    #[test]
    fn a_callout_follows_damped_and_ignores_a_tremor() {
        let mut f = Follow::default();
        f.step(Vec3::ZERO, 1.0 / 90.0);
        let goal = Vec3::new(0.2, 0.0, 0.0);
        let mut max = 0f32;
        let mut at_tenth = 0.0;
        for i in 0..180 {
            let at = f.step(goal, 1.0 / 90.0);
            max = max.max(at.x);
            if i == 8 {
                at_tenth = at.x;
            }
        }
        assert!(at_tenth < 0.2 * 0.5, "it lags: {at_tenth}");
        assert!(max <= 0.2 + 1e-4, "no overshoot: {max}");
        assert!((f.at.x - 0.2).abs() < 1e-3, "settled");
        // A tremor of half a centimetre: it stays.
        let rest = f.at;
        for i in 0..90 {
            let jitter = Vec3::new(
                0.005 * (i as f32).sin(),
                0.004 * (i as f32 * 1.7).cos(),
                0.0,
            );
            f.step(goal + jitter, 1.0 / 90.0);
        }
        assert!(f.at.distance(rest) < 1e-3, "{:?}", f.at - rest);
    }

    /// A callout's centre stays `SIGHT_CLEAR` off a sight line, moved the
    /// least it takes; one already clear isn't moved.
    #[test]
    fn a_callout_keeps_off_the_sight_line() {
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let line = Vec3::NEG_Z;
        for at in [
            eye + Vec3::new(0.02, -0.01, -0.5),
            eye + Vec3::new(0.0, 0.0, -0.5),
            eye + Vec3::new(-0.05, 0.03, -0.4),
        ] {
            let moved = off_the_line(eye, line, at);
            let a = (moved - eye).angle_between(line).to_degrees();
            assert!(a >= SIGHT_CLEAR - 1e-3, "{a} deg off the line");
            assert!(
                ((moved - eye).length() - (at - eye).length()).abs() < 1e-4,
                "same distance"
            );
        }
        let clear = eye + Vec3::new(0.2, -0.1, -0.5);
        assert_eq!(off_the_line(eye, line, clear), clear);
    }

    /// The caption moves up off a thing held under it, else down; with
    /// nothing in the way it sits at `CAPTION_DOWN`.
    #[test]
    fn the_caption_keeps_clear_of_what_is_held() {
        let circle = |c: Vec2, r: f32| (c - r, c + r);
        let half = Vec2::new(13.0, 4.0);
        assert_eq!(caption_down(&[], half), CAPTION_DOWN);
        // A pistol's muzzle just under it, middle of the view.
        let d = caption_down(&[circle(Vec2::new(2.0, 18.0), 3.0)], half);
        assert!(d < CAPTION_DOWN && d >= CAPTION_HIGHEST, "up: {d}");
        // Something right where it would sit and up to the eye line: down.
        let d = caption_down(&[circle(Vec2::new(0.0, 8.0), 4.0)], half);
        assert!(d > CAPTION_DOWN && d <= CAPTION_LOWEST, "down: {d}");
        // Nothing clears it from 6 to 20 degrees down: above it, toward
        // the eye line, never behind it.
        let d = caption_down(&[circle(Vec2::new(5.0, 14.0), 2.5)], half);
        assert!((0.0..CAPTION_HIGHEST).contains(&d), "above it: {d}");
        // Nothing clears it anywhere: the least crowded spot, in range.
        let d = caption_down(&[circle(Vec2::new(0.0, 10.0), 12.0)], half);
        assert!((0.0..=CAPTION_LOWEST).contains(&d), "{d}");
        // A spot that clears keeps CAPTION_CLEAR.
        let (lo, hi) = circle(Vec2::new(5.0, 15.0), 1.0);
        let d = caption_down(&[(lo, hi)], half);
        let dx = (lo.x - half.x).max(-half.x - hi.x).max(0.0);
        let dy = (lo.y - (d + half.y)).max((d - half.y) - hi.y).max(0.0);
        assert!((dx * dx + dy * dy).sqrt() >= CAPTION_CLEAR, "{d}");
    }

    /// Its letters come out at their size whatever the line, and over the
    /// floor for what must be read (`legibility`).
    #[test]
    fn caption_and_callout_letters_clear_the_floor_at_their_size() {
        use crate::render::legibility::{MUST_READ_DEG, check_panel};
        for (words, header) in [
            ("Copy.", Some("LEV, RADIO")),
            (
                "Move off quiet, then go still. Something heard you.",
                Some("LEV, RADIO"),
            ),
            ("The Surge is here.", None),
            (
                &"A long line about nothing much at all, said twice over. ".repeat(2) as &str,
                None,
            ),
        ] {
            let s = panel(header, words, CAPTION_COLS, CAPTION_AT, CAPTION_X_DEG);
            let deg = check_panel(&s, TextSource::Caption).unwrap();
            assert!(
                deg >= MUST_READ_DEG && deg >= CAPTION_X_DEG * 0.97,
                "caption {deg} {words}"
            );
            assert!(s.text.lines().all(|l| l.chars().count() <= CAPTION_COLS));
            let s = panel(
                Some("PM pistol"),
                words,
                CALLOUT_COLS,
                CALLOUT_AT,
                CALLOUT_X_DEG,
            );
            let deg = check_panel(&s, TextSource::Callout).unwrap();
            assert!(
                deg >= MUST_READ_DEG && deg >= CALLOUT_X_DEG * 0.97,
                "callout {deg} {words}"
            );
        }
    }

    /// Lit only as much as the eyes are open: by night, the slate isn't
    /// lifted to grey.
    #[test]
    fn the_slate_dims_as_the_eyes_open() {
        assert_eq!(glow(0.0), 1.0);
        assert!(glow(3.0) <= 0.125 + 1e-6);
        assert!(glow(5.0) < glow(3.0));
    }

    /// Every yard step's words, as a callout wraps them, still name each
    /// control its code accepts (`onboarding::Goal::accepts`): the
    /// callout's measure doesn't lose or split what the step is checked
    /// against.
    #[test]
    fn a_yard_step_as_a_callout_still_names_its_controls() {
        let comfort = crate::rig::comfort::Comfort::default();
        for g in crate::onboarding::all_goals() {
            let how = g.how_for(false, &comfort);
            let s = panel(
                Some("PM pistol"),
                &how,
                CALLOUT_COLS,
                CALLOUT_AT,
                CALLOUT_X_DEG,
            );
            let flat = s
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            for k in g.accepts(false) {
                assert!(flat.contains(k), "{g:?}: {k:?} not in {:?}", s.text);
            }
        }
    }

    /// A callout says only what was told, under the thing's own name: it
    /// has no words of its own to be untrue (#90; its lines are checked
    /// where they're written, the yard's against `Goal::accepts`, a first
    /// pickup's against what the thing does).
    #[test]
    fn a_callout_says_the_told_line_under_the_things_name() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.add_plugins(SaidPlugin);
        app.finish();
        app.cleanup();
        app.update();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let kind = catalog.kind("pm_pistol").unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let pm = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_xyz(0.1, 1.4, -0.4),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        app.world_mut().entity_mut(pm).insert((
            Posed(crate::rig::Side::Right),
            avian3d::prelude::RigidBody::Static,
        ));
        app.world_mut().spawn((
            crate::render::FreeCamera,
            Transform::from_xyz(0.0, 1.7, 0.0),
            GlobalTransform::from(Transform::from_xyz(0.0, 1.7, 0.0)),
        ));
        let how = crate::onboarding::Goal::Rack.how(false);
        app.world_mut()
            .resource_mut::<Raid>()
            .tell(how, Show::Item(pm));
        app.world_mut()
            .resource_mut::<Raid>()
            .tell("Truck: \"Heard you. Two minutes.\"", Show::View);
        for _ in 0..3 {
            app.update();
        }
        let mut q = app.world_mut().query::<(&Callout, &ScreenText)>();
        let (c, s) = q.single(app.world()).expect("one callout, beside the PM");
        assert_eq!(c.item, pm);
        let want = panel(
            Some("PM pistol"),
            how,
            CALLOUT_COLS,
            CALLOUT_AT,
            CALLOUT_X_DEG,
        );
        assert_eq!(s.text, want.text);
        assert!(s.text.starts_with("PM PISTOL\n"), "{}", s.text);
        // The truck's line went to the caption, its speaker over it.
        let lines = app.world().resource::<CaptionLines>();
        let now = lines.now.as_ref().expect("the caption");
        assert_eq!(now.words, "Heard you. Two minutes.");
        assert_eq!(now.header.as_deref(), Some("LEV, RADIO"));
        // Off the view: neither.
        app.world_mut()
            .resource_mut::<crate::rig::comfort::Comfort>()
            .next_captions();
        let mut raid = app.world_mut().resource_mut::<Raid>();
        raid.tell("Something new to read, beside it.", Show::Item(pm));
        app.update();
        app.update();
        let mut q = app.world_mut().query::<&Callout>();
        let words: Vec<_> = q.iter(app.world()).map(|c| c.line.words.clone()).collect();
        assert_eq!(words, vec![how.to_string()], "the new line wasn't shown");
    }
}
