//! One body for every line a machine hangs by or throws (#101): the
//! dirigible's pod cable, the spotter's tether, the grappler's line. A
//! [`Line`] says what it is (rope or steel cable), how worn it is, and
//! where its weak link and grip sleeves sit; a [`LineSpan`] says where it
//! runs (world space, written by its owner each frame it moves). This
//! draws it (client only):
//!
//! - **Gauge, at a glance:** rope is pale, fibrous and fat, and sags;
//!   cable is dark, thin and glints, and hangs taut.
//! - **Fraying, on the line itself:** whole; nicked, a strand springs out;
//!   half through, thinned with bright cut ends; nearly parted, a thread
//!   with the ends splayed. The progress shows where it's worked, not on
//!   a bar.
//! - **The weak link:** a shackle in the one shoot-here orange
//!   (`weak_points::SHOOT_HERE`), quicker to shoot or pry than the line.
//! - **Grip sleeves:** dark bands where hands hold and cutters bite.
//!
//! Wear comes from rounds on the line (`Hit` on the line's own entity);
//! what parts it, and when, is its owner's rule.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (wear_lines, (add_looks, draw_lines).chain()));
    }
}

/// What a line is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
pub enum Gauge {
    /// Pale, fibrous, fat; sags; one cut takes it.
    Rope,
    /// Dark steel, thin, glinting; hangs taut; strands to get through.
    Cable,
}

impl Gauge {
    /// Its radius (m).
    pub fn radius(self) -> f32 {
        match self {
            Gauge::Rope => 0.022,
            Gauge::Cable => 0.013,
        }
    }

    /// How much a round wears it (of its whole, 1).
    pub fn wear_per_damage(self) -> f32 {
        match self {
            Gauge::Rope => 1.0 / 40.0,
            Gauge::Cable => 1.0 / 110.0,
        }
    }
}

/// A line: what it is, how worn, its weak link and its grip sleeves.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Line {
    pub gauge: Gauge,
    /// 0 whole .. 1 parting: whole under 0.15, nicked, half through from
    /// 0.5, a thread from 0.85 ([`Fray`]).
    pub wear: f32,
    /// Where along it (0 at `from` .. 1 at `to`) it frays: where it's
    /// worked.
    pub fray_at: f32,
    /// Where its weak link sits, if it has one (0..1 along).
    pub weak_link: Option<f32>,
    /// Where its grip sleeves sit (0..1 along).
    pub grips: Vec<f32>,
}

impl Line {
    pub fn new(gauge: Gauge) -> Self {
        Self {
            gauge,
            wear: 0.0,
            fray_at: 0.5,
            weak_link: None,
            grips: Vec::new(),
        }
    }

    pub fn with_weak_link(mut self, at: f32) -> Self {
        self.weak_link = Some(at);
        self
    }

    pub fn with_grip(mut self, at: f32) -> Self {
        self.grips.push(at);
        self
    }

    pub fn fray(&self) -> Fray {
        Fray::of(self.wear)
    }
}

/// Where a line runs (world space) and how much it sags: 0 taut, 1 a
/// tenth of its length at the middle. Its owner writes it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LineSpan {
    pub from: Vec3,
    pub to: Vec3,
    pub slack: f32,
}

impl LineSpan {
    /// The point `f` (0..1) along it, sag and all.
    pub fn at(&self, f: f32) -> Vec3 {
        let len = self.from.distance(self.to);
        let sag = self.slack * 0.1 * len * 4.0 * f * (1.0 - f);
        self.from.lerp(self.to, f) - Vec3::Y * sag
    }
}

/// How far through a line is, as it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fray {
    Whole,
    /// A strand springs out.
    Nicked,
    /// Thinned, bright cut ends.
    Half,
    /// A thread, the ends splayed.
    Parting,
}

impl Fray {
    pub fn of(wear: f32) -> Self {
        if wear < 0.15 {
            Fray::Whole
        } else if wear < 0.5 {
            Fray::Nicked
        } else if wear < 0.85 {
            Fray::Half
        } else {
            Fray::Parting
        }
    }

    /// How much of the line is left where it frays (its thickness there).
    pub fn left(self) -> f32 {
        match self {
            Fray::Whole | Fray::Nicked => 1.0,
            Fray::Half => 0.45,
            Fray::Parting => 0.15,
        }
    }

    /// Cut ends springing out of it.
    pub fn tufts(self) -> usize {
        match self {
            Fray::Whole => 0,
            Fray::Nicked => 1,
            Fray::Half => 3,
            Fray::Parting => 5,
        }
    }
}

/// Segments a sagging line is drawn in (a taut one is one).
const SAG_SEGMENTS: usize = 8;
/// The worn stretch's length (m), where it frays: a hand's length.
const FRAY_LEN: f32 = 0.2;

/// A line's look: its pieces, drawn separately (its owner's transform is
/// anything), despawned with it.
#[derive(Component)]
struct LineLook {
    segments: Vec<Entity>,
    worn: Entity,
    tufts: Vec<Entity>,
    shackle: Option<Entity>,
    grips: Vec<Entity>,
}

/// A piece of a line's look, and whose.
#[derive(Component)]
struct LinePiece(Entity);

#[derive(Resource)]
struct LineAssets {
    tube: Handle<Mesh>,
    tuft: Handle<Mesh>,
    ring: Handle<Mesh>,
    sleeve: Handle<Mesh>,
    rope: Handle<StandardMaterial>,
    cable: Handle<StandardMaterial>,
    rope_ends: Handle<StandardMaterial>,
    cable_ends: Handle<StandardMaterial>,
    shackle: Handle<StandardMaterial>,
    sleeve_mat: Handle<StandardMaterial>,
}

fn assets(
    commands: &mut Commands,
    have: Option<&LineAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> LineAssets {
    if let Some(a) = have {
        return LineAssets {
            tube: a.tube.clone(),
            tuft: a.tuft.clone(),
            ring: a.ring.clone(),
            sleeve: a.sleeve.clone(),
            rope: a.rope.clone(),
            cable: a.cable.clone(),
            rope_ends: a.rope_ends.clone(),
            cable_ends: a.cable_ends.clone(),
            shackle: a.shackle.clone(),
            sleeve_mat: a.sleeve_mat.clone(),
        };
    }
    let a = LineAssets {
        // A unit tube along Y, from 0 to 1 (scaled to a segment).
        tube: meshes.add(
            Cylinder::new(1.0, 1.0)
                .mesh()
                .resolution(8)
                .build()
                .translated_by(Vec3::Y * 0.5),
        ),
        tuft: meshes.add(
            Cylinder::new(0.01, 1.0)
                .mesh()
                .resolution(4)
                .build()
                .translated_by(Vec3::Y * 0.5),
        ),
        ring: meshes.add(Torus::new(0.035, 0.05)),
        sleeve: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(10).build()),
        rope: materials.add(StandardMaterial {
            base_color: Color::srgb(0.66, 0.59, 0.44),
            perceptual_roughness: 0.95,
            ..default()
        }),
        cable: materials.add(StandardMaterial {
            base_color: Color::srgb(0.29, 0.31, 0.33),
            metallic: 0.85,
            perceptual_roughness: 0.32,
            ..default()
        }),
        // Fresh-cut fibre: near white against the weathered rope.
        rope_ends: materials.add(StandardMaterial {
            base_color: Color::srgb(0.98, 0.95, 0.86),
            emissive: LinearRgba::rgb(0.45, 0.42, 0.36),
            perceptual_roughness: 0.9,
            ..default()
        }),
        // Fresh-cut steel: bright, catching any light.
        cable_ends: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.62),
            metallic: 1.0,
            perceptual_roughness: 0.18,
            emissive: LinearRgba::rgb(0.9, 0.7, 0.35),
            ..default()
        }),
        shackle: materials.add(StandardMaterial {
            base_color: super::weak_points::SHOOT_HERE,
            emissive: LinearRgba::from(super::weak_points::SHOOT_HERE) * 0.6,
            perceptual_roughness: 0.5,
            ..default()
        }),
        sleeve_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.14, 0.15),
            perceptual_roughness: 0.85,
            ..default()
        }),
    };
    commands.insert_resource(LineAssets {
        tube: a.tube.clone(),
        tuft: a.tuft.clone(),
        ring: a.ring.clone(),
        sleeve: a.sleeve.clone(),
        rope: a.rope.clone(),
        cable: a.cable.clone(),
        rope_ends: a.rope_ends.clone(),
        cable_ends: a.cable_ends.clone(),
        shackle: a.shackle.clone(),
        sleeve_mat: a.sleeve_mat.clone(),
    });
    a
}

/// A round on a line wears it, by its gauge.
fn wear_lines(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    mut lines: Query<(&mut Line, Option<&LineSpan>)>,
) {
    for hit in hits.read() {
        let Ok((mut line, span)) = lines.get_mut(hit.entity) else {
            continue;
        };
        let worn = (line.wear + hit.damage.max(0.0) * line.gauge.wear_per_damage()).min(1.0);
        // It frays where it's hit (the first time: after that, there).
        if line.wear <= 0.0 {
            if let Some(s) = span {
                let d = s.to - s.from;
                let f =
                    ((hit.point - s.from).dot(d) / d.length_squared().max(1e-6)).clamp(0.05, 0.95);
                line.fray_at = f;
            }
        }
        line.wear = worn;
    }
}

fn add_looks(
    mut commands: Commands,
    new: Query<(Entity, &Line), Added<Line>>,
    have: Option<Res<LineAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if new.is_empty() {
        return;
    }
    let a = assets(&mut commands, have.as_deref(), &mut meshes, &mut materials);
    for (e, line) in &new {
        let (body, ends) = match line.gauge {
            Gauge::Rope => (&a.rope, &a.rope_ends),
            Gauge::Cable => (&a.cable, &a.cable_ends),
        };
        let mut piece = |mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>| {
            commands
                .spawn((
                    Name::new("Line piece"),
                    LinePiece(e),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    NotShadowCaster,
                ))
                .id()
        };
        let segments = (0..SAG_SEGMENTS).map(|_| piece(&a.tube, body)).collect();
        let worn = piece(&a.tube, body);
        let tufts = (0..Fray::Parting.tufts())
            .map(|_| piece(&a.tuft, ends))
            .collect();
        let shackle = line.weak_link.map(|_| piece(&a.ring, &a.shackle));
        let grips = line
            .grips
            .iter()
            .map(|_| piece(&a.sleeve, &a.sleeve_mat))
            .collect();
        commands.entity(e).insert(LineLook {
            segments,
            worn,
            tufts,
            shackle,
            grips,
        });
    }
}

/// The transform putting the unit tube from `a` to `b`, `r` thick.
fn tube(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(1e-4);
    Transform::from_translation(a)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len))
        .with_scale(Vec3::new(r, len, r))
}

#[allow(clippy::type_complexity)]
fn draw_lines(
    mut commands: Commands,
    lines: Query<
        (&Line, &LineSpan, &LineLook, Option<&InheritedVisibility>),
        Or<(
            Changed<Line>,
            Changed<LineSpan>,
            Added<LineLook>,
            Changed<InheritedVisibility>,
        )>,
    >,
    all: Query<(Entity, &LineLook)>,
    owners: Query<(), With<Line>>,
    pieces: Query<(Entity, &LinePiece)>,
    mut xf: Query<(&mut Transform, &mut Visibility), With<LinePiece>>,
    mut gone: RemovedComponents<Line>,
) {
    // A line gone takes its look with it.
    for e in gone.read() {
        if owners.contains(e) {
            continue;
        }
        for (p, of) in &pieces {
            if of.0 == e {
                commands.entity(p).despawn();
            }
        }
    }
    let _ = all;
    for (line, span, look, shown) in &lines {
        let shown = shown.is_none_or(|v| v.get());
        let mut set = |e: Entity, t: Option<Transform>| {
            if let Ok((mut x, mut v)) = xf.get_mut(e) {
                match t {
                    Some(t) if shown => {
                        *x = t;
                        v.set_if_neq(Visibility::Visible);
                    }
                    _ => {
                        v.set_if_neq(Visibility::Hidden);
                    }
                }
            }
        };
        let r = line.gauge.radius();
        let len = span.from.distance(span.to);
        let fray = line.fray();
        // The worn stretch, cut out of the line where it frays.
        let half = if fray == Fray::Whole || len < 1e-3 {
            0.0
        } else {
            (FRAY_LEN * 0.5 / len).min(0.2)
        };
        let (w0, w1) = (line.fray_at - half, line.fray_at + half);
        // The line, in segments (one if taut), skipping the worn stretch.
        let n = if span.slack > 0.01 { SAG_SEGMENTS } else { 1 };
        let mut k = 0;
        for i in 0..n {
            let (f0, f1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            // Split round the worn stretch: the part before it here, the
            // part after it as the next piece (taut lines have room: a
            // sagging line's segments are many).
            let parts: Vec<(f32, f32)> = if half > 0.0 && f0 < w1 && f1 > w0 {
                [(f0, w0.max(f0)), (w1.min(f1), f1)]
                    .into_iter()
                    .filter(|(a, b)| b - a > 1e-4)
                    .collect()
            } else {
                vec![(f0, f1)]
            };
            for (a, b) in parts {
                if k < look.segments.len() {
                    set(look.segments[k], Some(tube(span.at(a), span.at(b), r)));
                    k += 1;
                }
            }
        }
        for &s in &look.segments[k..] {
            set(s, None);
        }
        // The worn stretch: thinned as far as it's through.
        if half > 0.0 {
            set(
                look.worn,
                Some(tube(span.at(w0), span.at(w1), r * fray.left())),
            );
        } else {
            set(look.worn, None);
        }
        // Cut ends springing out round it, fixed per line (no flicker).
        let mid = span.at(line.fray_at);
        let along = (span.at((line.fray_at + 0.01).min(1.0))
            - span.at((line.fray_at - 0.01).max(0.0)))
        .normalize_or(Vec3::Y);
        let side = along.any_orthogonal_vector().normalize();
        for (i, &t) in look.tufts.iter().enumerate() {
            if i < fray.tufts() {
                let a = i as f32 * 2.4;
                let out = Quat::from_axis_angle(along, a) * side;
                let up = if i % 2 == 0 { 1.0 } else { -1.0 };
                let from = mid + along * up * FRAY_LEN * 0.5 + out * r * 0.6;
                let to =
                    from + (out * 0.9 + along * up * 0.4).normalize() * (0.09 + 0.02 * i as f32);
                let d = to - from;
                set(
                    t,
                    Some(
                        Transform::from_translation(from)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Y, d.normalize()))
                            .with_scale(Vec3::new(1.0, d.length(), 1.0)),
                    ),
                );
            } else {
                set(t, None);
            }
        }
        // The weak link: a shackle on the line, its eye across it.
        if let (Some(s), Some(at)) = (look.shackle, line.weak_link) {
            let p = span.at(at);
            let d = (span.at((at + 0.01).min(1.0)) - span.at((at - 0.01).max(0.0)))
                .normalize_or(Vec3::Y);
            // The torus lies in its XZ plane: turn its axis across the line.
            let across = d.any_orthogonal_vector().normalize();
            set(
                s,
                Some(
                    Transform::from_translation(p)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, across))
                        .with_scale(Vec3::new(1.0, 1.6, 1.0)),
                ),
            );
        }
        // Grip sleeves: dark bands, a hand's width.
        for (&g, &at) in look.grips.iter().zip(&line.grips) {
            let p = span.at(at);
            let d = (span.at((at + 0.01).min(1.0)) - span.at((at - 0.01).max(0.0)))
                .normalize_or(Vec3::Y);
            set(
                g,
                Some(
                    Transform::from_translation(p)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d))
                        .with_scale(Vec3::new(r * 2.4, 0.22, r * 2.4)),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stages read in order as it wears, thinning and fraying more.
    #[test]
    fn a_line_frays_in_order() {
        let stages: Vec<Fray> = [0.0, 0.2, 0.6, 0.9].into_iter().map(Fray::of).collect();
        assert_eq!(
            stages,
            vec![Fray::Whole, Fray::Nicked, Fray::Half, Fray::Parting]
        );
        for w in stages.windows(2) {
            assert!(w[1].left() <= w[0].left());
            assert!(w[1].tufts() > w[0].tufts());
        }
    }

    /// Rope reads fat beside cable, and a round wears it faster.
    #[test]
    fn rope_is_fat_and_soft_cable_thin_and_hard() {
        assert!(Gauge::Rope.radius() > Gauge::Cable.radius() * 1.5);
        assert!(Gauge::Rope.wear_per_damage() > Gauge::Cable.wear_per_damage() * 2.0);
    }

    /// A slack line sags at its middle, not at its ends.
    #[test]
    fn a_slack_line_sags_in_the_middle() {
        let s = LineSpan {
            from: Vec3::ZERO,
            to: Vec3::new(10.0, 0.0, 0.0),
            slack: 1.0,
        };
        assert_eq!(s.at(0.0), Vec3::ZERO);
        assert!((s.at(1.0) - s.to).length() < 1e-5);
        assert!((s.at(0.5).y + 1.0).abs() < 1e-4, "{}", s.at(0.5));
    }

    /// A round on the line wears it by its gauge, and it frays where it
    /// was hit.
    #[test]
    fn a_round_wears_the_line_where_it_hits() {
        let mut app = App::new();
        app.add_message::<crate::weapons::ballistics::Hit>()
            .add_systems(Update, wear_lines);
        let e = app
            .world_mut()
            .spawn((
                Line::new(Gauge::Rope),
                LineSpan {
                    from: Vec3::ZERO,
                    to: Vec3::new(0.0, -10.0, 0.0),
                    slack: 0.0,
                },
            ))
            .id();
        app.world_mut()
            .write_message(crate::weapons::ballistics::Hit {
                entity: e,
                point: Vec3::new(0.0, -3.0, 0.0),
                normal: Vec3::X,
                velocity: Vec3::NEG_X,
                damage: 20.0,
                shooter: None,
                mass_g: 8.0,
                load: extraction_core::ammo::Load::Standard,
                blow: None,
            });
        app.update();
        let l = app.world().get::<Line>(e).unwrap();
        assert!((l.wear - 0.5).abs() < 1e-4, "{}", l.wear);
        assert!((l.fray_at - 0.3).abs() < 1e-3, "{}", l.fray_at);
        assert_eq!(l.fray(), Fray::Half);
    }
}
