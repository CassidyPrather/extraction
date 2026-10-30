//! What a round does to what it hits, by what it's made of.
//!
//! Logic (headless too): every [`Hit`] becomes an [`Impact`] with an
//! [`ImpactKind`], the same classes the impact sounds use, and a round
//! crossing a water surface becomes a splash there (it doesn't go on to
//! the bed). The client turns impacts into a few short-lived bits (dust,
//! chips, splinters, sparks, spray, clods) and a mark left on the surface,
//! from pools with hard caps ([`BIT_CAP`], [`DECAL_CAP`]): a long firefight
//! costs no more draws than a short one, and every bit shares a handful of
//! meshes and materials so the renderer batches them. Bits and marks are
//! two to three times life size, so an impact reads at fighting range.

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use extraction_core::noise::Rng;
use std::collections::VecDeque;

use super::ballistics::Hit;
use crate::render::materials::Mat;
use crate::world::surfaces::Surfaces;

/// What a round struck, as far as the eye and ear care.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImpactKind {
    /// Concrete, brick, plaster, rock: dust and chips, a grey pock.
    Masonry,
    /// Planks, logs, bark: splinters, a pale-rimmed hole.
    Wood,
    /// Sheet and plate: sparks, a bright pock.
    Metal,
    /// Window glass: glittering shards.
    Glass,
    /// The ground: clods and a brown puff.
    Soil,
    /// A water surface: spray.
    Water,
    /// A robot's part (see the robot hit effects).
    Robot,
    /// A person.
    Body,
}

impl ImpactKind {
    /// Leaves a mark where it hit (static surfaces only).
    pub fn marks(self) -> bool {
        matches!(
            self,
            Self::Masonry | Self::Wood | Self::Metal | Self::Glass | Self::Soil
        )
    }
}

/// The class of a built material.
pub fn kind_of(m: Mat) -> ImpactKind {
    match m {
        Mat::Planks
        | Mat::LogWall
        | Mat::WoodFloor
        | Mat::Timber
        | Mat::Shingles
        | Mat::GunWood
        | Mat::BarkSpruce
        | Mat::BarkBirch
        | Mat::BarkPine
        | Mat::Charred
        | Mat::Sodden
        | Mat::Canvas => ImpactKind::Wood,
        Mat::RustyMetal
        | Mat::PaintedMetal
        | Mat::Corrugated
        | Mat::DarkMetal
        | Mat::RobotShell
        | Mat::GunMetal => ImpactKind::Metal,
        Mat::Glass => ImpactKind::Glass,
        Mat::Silt | Mat::Snow => ImpactKind::Soil,
        Mat::Brick
        | Mat::SootBrick
        | Mat::Plaster
        | Mat::SootPlaster
        | Mat::Concrete
        | Mat::Asphalt
        | Mat::Boulder
        | Mat::DressedStone
        | Mat::Polymer => ImpactKind::Masonry,
    }
}

/// What a built material is to a round going through it.
pub fn stuff_of(m: Mat) -> extraction_core::firearm_terminal::Stuff {
    use extraction_core::firearm_terminal::Stuff;
    match m {
        Mat::Planks | Mat::WoodFloor | Mat::Timber | Mat::Shingles | Mat::GunWood => Stuff::Planks,
        Mat::Sodden => Stuff::Planks,
        Mat::Silt | Mat::Snow => Stuff::Soil,
        Mat::Polymer => Stuff::Planks,
        // Charred timber is what's left of a log or a beam: still logs.
        Mat::LogWall | Mat::BarkSpruce | Mat::BarkBirch | Mat::BarkPine | Mat::Charred => {
            Stuff::Logs
        }
        Mat::Plaster | Mat::SootPlaster => Stuff::Plaster,
        Mat::Brick | Mat::SootBrick => Stuff::Brick,
        Mat::Concrete => Stuff::Concrete,
        Mat::Asphalt | Mat::Boulder | Mat::DressedStone => Stuff::Rock,
        Mat::RustyMetal | Mat::PaintedMetal | Mat::Corrugated => Stuff::Sheet,
        Mat::DarkMetal | Mat::GunMetal | Mat::RobotShell => Stuff::Plate,
        Mat::Glass | Mat::Canvas => Stuff::Glass,
    }
}

/// The impact class of what a round went through (its exit mark).
pub fn kind_of_stuff(s: extraction_core::firearm_terminal::Stuff) -> ImpactKind {
    use extraction_core::firearm_terminal::Stuff;
    match s {
        Stuff::Planks | Stuff::Logs => ImpactKind::Wood,
        Stuff::Plaster | Stuff::Brick | Stuff::Concrete | Stuff::Rock => ImpactKind::Masonry,
        Stuff::Sheet
        | Stuff::Plate
        | Stuff::ArmourPlate
        | Stuff::Helmet
        | Stuff::Shell
        | Stuff::Housing => ImpactKind::Metal,
        Stuff::Glass => ImpactKind::Glass,
        Stuff::Soil => ImpactKind::Soil,
    }
}

/// A round struck something (or the water).
#[derive(Message, Clone, Copy, Debug)]
pub struct Impact {
    pub kind: ImpactKind,
    pub point: Vec3,
    /// Out of the surface.
    pub normal: Vec3,
    /// The round's velocity coming in.
    pub velocity: Vec3,
    /// What was hit; `None` for water.
    pub entity: Option<Entity>,
    /// It's a static surface that can wear a mark.
    pub decal: bool,
    /// On a robot: which part, and whether the round hurt it (a live
    /// part takes damage; a dead one just rings).
    pub part: Option<crate::robots::PartRole>,
    pub hurt: bool,
}

pub struct ImpactPlugin;

impl Plugin for ImpactPlugin {
    fn build(&self, app: &mut App) {
        // Inside the ballistics set, so robot parts are read before the
        // robots take the damage.
        app.add_message::<Impact>().add_systems(
            FixedUpdate,
            classify_hits
                .in_set(super::ballistics::BallisticsSystems)
                .after(super::ballistics::fly_projectiles),
        );
    }
}

/// Every hit, classed.
#[allow(clippy::too_many_arguments)]
fn classify_hits(
    mut hits: MessageReader<Hit>,
    mut out: MessageWriter<Impact>,
    parts: Query<&crate::robots::RobotPart>,
    trees: Query<(), With<crate::world::trees::TreeTrunks>>,
    structures: Query<(), With<crate::world::structures::Structure>>,
    items: Query<(), With<crate::items::Item>>,
    bodies: Query<(), With<crate::player::BodyCollider>>,
    built: Query<(&GlobalTransform, &Surfaces)>,
) {
    for h in hits.read() {
        let made_of = built
            .get(h.entity)
            .ok()
            .and_then(|(gt, surf)| surf.at(gt, h.point));
        let part = parts.get(h.entity).ok();
        let (kind, decal) = if bodies.contains(h.entity) {
            (ImpactKind::Body, false)
        } else if part.is_some() {
            (ImpactKind::Robot, false)
        } else if items.contains(h.entity) {
            (ImpactKind::Metal, false)
        } else if trees.contains(h.entity) {
            (ImpactKind::Wood, true)
        } else if let Some(m) = made_of {
            (kind_of(m), true)
        } else if structures.contains(h.entity) {
            (ImpactKind::Masonry, true)
        } else {
            (ImpactKind::Soil, true)
        };
        out.write(Impact {
            kind,
            point: h.point,
            normal: h.normal,
            velocity: h.velocity,
            entity: Some(h.entity),
            decal: decal && kind.marks(),
            part: part.map(|p| p.role),
            hurt: part.is_some_and(|p| p.alive()),
        });
    }
}

/// Live bits at once, all impacts together.
pub const BIT_CAP: usize = 160;
/// Marks on surfaces at once; the oldest goes first.
pub const DECAL_CAP: usize = 64;
/// How long a mark stays, seconds.
pub const DECAL_LIFE: f32 = 90.0;
/// How far a mark stands proud of its surface, metres (house rule: 4-5 mm).
pub const DECAL_PROUD: f32 = 0.005;

/// The valley's wind at head height, m/s: the direction the trees sway
/// in, as hard as the weather blows it.
pub fn wind_velocity(weather: Option<&crate::world::weather::Conditions>) -> Vec3 {
    let strength = weather.map_or(0.4, |c| c.wind());
    Vec3::new(0.8, 0.0, 0.6) * 2.5 * strength
}

/// A first-in, first-out set with a cap: pushing past it hands back the
/// oldest to remove.
#[derive(Debug, Default)]
pub struct Capped {
    cap: usize,
    q: VecDeque<Entity>,
}

impl Capped {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            q: VecDeque::with_capacity(cap),
        }
    }

    pub fn push(&mut self, e: Entity) -> Option<Entity> {
        self.q.push_back(e);
        (self.q.len() > self.cap)
            .then(|| self.q.pop_front())
            .flatten()
    }

    pub fn len(&self) -> usize {
        self.q.len()
    }

    pub fn is_empty(&self) -> bool {
        self.q.is_empty()
    }
}

/// A flying bit of an impact (client side).
#[derive(Component)]
pub struct Bit {
    pub(crate) vel: Vec3,
    pub(crate) age: f32,
    pub(crate) life: f32,
    pub(crate) size: Vec3,
    /// Grows by this share of its size over its life (dust spreads).
    pub(crate) grow: f32,
    pub(crate) gravity: f32,
    pub(crate) drag: f32,
    /// Stretched along its velocity (sparks).
    pub(crate) align: bool,
    pub(crate) spin: Vec3,
    /// Share of the wind it rides (smoke drifts, chips don't).
    pub(crate) wind: f32,
}

/// A mark left on a surface (client side).
#[derive(Component)]
pub struct Decal {
    age: f32,
    size: f32,
}

#[derive(Resource)]
pub(crate) struct FxAssets {
    pub(crate) cube: Handle<Mesh>,
    pub(crate) ball: Handle<Mesh>,
    pub(crate) disc: Handle<Mesh>,
    pub(crate) dust: Handle<StandardMaterial>,
    pub(crate) soil_dust: Handle<StandardMaterial>,
    pub(crate) chip: Handle<StandardMaterial>,
    pub(crate) splinter: Handle<StandardMaterial>,
    pub(crate) spark: Handle<StandardMaterial>,
    pub(crate) shard: Handle<StandardMaterial>,
    pub(crate) clod: Handle<StandardMaterial>,
    pub(crate) spray: Handle<StandardMaterial>,
    pub(crate) hole: Handle<StandardMaterial>,
    pub(crate) pale_rim: Handle<StandardMaterial>,
    pub(crate) bright_pock: Handle<StandardMaterial>,
    pub(crate) soil_mark: Handle<StandardMaterial>,
    pub(crate) flake: Handle<StandardMaterial>,
    pub(crate) fizz: Handle<StandardMaterial>,
    pub(crate) oil: Handle<StandardMaterial>,
    pub(crate) gas: Handle<StandardMaterial>,
    pub(crate) cone: Handle<Mesh>,
    pub(crate) flash: Handle<StandardMaterial>,
    pub(crate) smoke: Handle<StandardMaterial>,
}

#[derive(Resource)]
pub(crate) struct FxPool {
    pub(crate) bits: Capped,
    pub(crate) decals: Capped,
    pub(crate) rng: Rng,
}

pub struct ImpactFxPlugin;

impl Plugin for ImpactFxPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FxPool {
            bits: Capped::new(BIT_CAP),
            decals: Capped::new(DECAL_CAP),
            rng: Rng::new(0x1A9C7),
        })
        .add_systems(Startup, fx_assets)
        .add_systems(Update, (spawn_fx, fly_bits, age_decals).chain());
    }
}

fn fx_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let flash = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.8, 0.45),
        emissive: LinearRgba::rgb(1.0, 0.75, 0.4) * 12.0,
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    let mut m = |r: f32, g: f32, b: f32, a: f32, glow: f32, rough: f32, metal: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgba(r, g, b, a),
            emissive: LinearRgba::rgb(r, g, b) * glow,
            perceptual_roughness: rough,
            metallic: metal,
            alpha_mode: if a < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            unlit: glow > 0.0,
            ..default()
        })
    };
    let assets = FxAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        ball: meshes.add(Sphere::new(0.5).mesh().ico(2).unwrap()),
        disc: meshes.add(Circle::new(0.5).mesh().resolution(10)),
        // Dust is unlit: a lit, fogged sphere reads as a dark ball.
        dust: m(0.6, 0.59, 0.56, 0.32, 0.001, 1.0, 0.0),
        soil_dust: m(0.38, 0.35, 0.3, 0.32, 0.001, 1.0, 0.0),
        chip: m(0.45, 0.44, 0.42, 1.0, 0.0, 0.9, 0.0),
        splinter: m(0.78, 0.66, 0.46, 1.0, 0.0, 0.8, 0.0),
        spark: m(1.0, 0.72, 0.3, 1.0, 6.0, 1.0, 0.0),
        shard: m(0.85, 0.92, 0.95, 0.55, 0.0, 0.05, 0.0),
        clod: m(0.27, 0.21, 0.15, 1.0, 0.0, 1.0, 0.0),
        spray: m(0.85, 0.9, 0.95, 0.6, 0.0, 0.1, 0.0),
        hole: m(0.06, 0.055, 0.05, 1.0, 0.0, 1.0, 0.0),
        pale_rim: m(0.8, 0.7, 0.52, 1.0, 0.0, 0.9, 0.0),
        bright_pock: m(0.78, 0.78, 0.76, 1.0, 0.0, 0.3, 1.0),
        soil_mark: m(0.16, 0.12, 0.09, 1.0, 0.0, 1.0, 0.0),
        flake: m(0.3, 0.32, 0.3, 1.0, 0.0, 0.5, 0.8),
        fizz: m(0.55, 0.85, 1.0, 1.0, 8.0, 1.0, 0.0),
        oil: m(0.05, 0.04, 0.03, 1.0, 0.0, 0.08, 0.0),
        gas: m(0.8, 0.82, 0.8, 0.3, 0.001, 1.0, 0.0),
        cone: meshes.add(
            Cone::new(0.5, 1.0)
                .mesh()
                .resolution(8)
                .build()
                .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ),
        flash,
        smoke: m(0.72, 0.72, 0.7, 0.22, 0.001, 1.0, 0.0),
    };
    commands.insert_resource(assets);
}

/// One kind of bit in an impact's recipe.
pub(crate) struct Spray {
    pub(crate) n: usize,
    pub(crate) mesh: fn(&FxAssets) -> Handle<Mesh>,
    pub(crate) mat: fn(&FxAssets) -> Handle<StandardMaterial>,
    pub(crate) size: Vec3,
    /// Speed out of the surface, and of scatter, m/s.
    pub(crate) out: f32,
    pub(crate) scatter: f32,
    /// Share of the round's own direction it keeps (sparks skate on).
    pub(crate) carry: f32,
    pub(crate) life: f32,
    pub(crate) grow: f32,
    pub(crate) gravity: f32,
    pub(crate) drag: f32,
    pub(crate) align: bool,
    pub(crate) spin: f32,
    pub(crate) wind: f32,
}

pub(crate) const SPRAY: Spray = Spray {
    n: 0,
    mesh: |a| a.cube.clone(),
    mat: |a| a.chip.clone(),
    size: Vec3::splat(0.01),
    out: 1.0,
    scatter: 1.0,
    carry: 0.0,
    life: 0.6,
    grow: 0.0,
    gravity: 9.8,
    drag: 0.0,
    align: false,
    spin: 0.0,
    wind: 0.0,
};

/// A robot hit, by the part and whether it hurt: what it hurt shows
/// (fragments off armour, a fizzing short off a sensor, oil from a joint,
/// gas from a cell); a round off a dead part only glances.
fn robot_recipe(part: Option<crate::robots::PartRole>, hurt: bool) -> Vec<Spray> {
    use crate::robots::PartRole;
    let sparks = |n| Spray {
        n,
        mat: |a| a.spark.clone(),
        size: Vec3::new(0.005, 0.005, 0.1),
        out: 3.0,
        scatter: 3.5,
        carry: 4.0,
        life: 0.22,
        align: true,
        ..SPRAY
    };
    // Off a dead part, or a plate taking it for what's behind: it only
    // glances (Robot bodies' `damage::strike` reads it as a shrug).
    if !hurt || matches!(part, Some(PartRole::Armour(_))) {
        return vec![Spray {
            carry: 6.0,
            out: 1.0,
            ..sparks(4)
        }];
    }
    match part {
        Some(PartRole::Sensor) => vec![
            Spray {
                n: 12,
                mat: |a| a.fizz.clone(),
                size: Vec3::new(0.004, 0.004, 0.06),
                out: 1.0,
                scatter: 1.6,
                life: 0.55,
                gravity: 2.0,
                drag: 1.5,
                align: true,
                ..SPRAY
            },
            Spray {
                n: 3,
                mat: |a| a.shard.clone(),
                size: Vec3::new(0.025, 0.003, 0.02),
                out: 1.5,
                scatter: 1.0,
                life: 0.8,
                spin: 18.0,
                ..SPRAY
            },
        ],
        Some(PartRole::Leg(_) | PartRole::Rotor(_)) => vec![
            sparks(5),
            Spray {
                n: 8,
                mesh: |a| a.ball.clone(),
                mat: |a| a.oil.clone(),
                size: Vec3::splat(0.022),
                out: 2.0,
                scatter: 1.2,
                life: 0.9,
                ..SPRAY
            },
        ],
        Some(PartRole::GasCell(_)) => vec![
            sparks(3),
            Spray {
                n: 3,
                mesh: |a| a.ball.clone(),
                mat: |a| a.gas.clone(),
                size: Vec3::splat(0.1),
                out: 1.5,
                scatter: 0.5,
                life: 1.4,
                grow: 3.0,
                gravity: -0.5,
                drag: 1.5,
                wind: 0.8,
                ..SPRAY
            },
        ],
        // The core, a weapon, the shell: sparks and a shower of fragments.
        _ => vec![
            sparks(10),
            Spray {
                n: 6,
                mat: |a| a.flake.clone(),
                size: Vec3::new(0.022, 0.004, 0.02),
                out: 2.5,
                scatter: 2.0,
                life: 0.9,
                spin: 22.0,
                ..SPRAY
            },
        ],
    }
}

fn recipe(kind: ImpactKind) -> Vec<Spray> {
    let puff = |mat: fn(&FxAssets) -> Handle<StandardMaterial>, n| Spray {
        n,
        mesh: |a| a.ball.clone(),
        mat,
        size: Vec3::splat(0.08),
        out: 0.9,
        scatter: 0.4,
        life: 1.0,
        grow: 2.2,
        gravity: -0.25,
        drag: 2.5,
        wind: 0.7,
        ..SPRAY
    };
    match kind {
        ImpactKind::Masonry => vec![
            puff(|a| a.dust.clone(), 4),
            Spray {
                n: 6,
                size: Vec3::splat(0.022),
                out: 2.5,
                scatter: 1.5,
                life: 0.7,
                spin: 20.0,
                ..SPRAY
            },
        ],
        ImpactKind::Wood => vec![
            puff(|a| a.soil_dust.clone(), 1),
            Spray {
                n: 7,
                mat: |a| a.splinter.clone(),
                size: Vec3::new(0.008, 0.008, 0.07),
                out: 3.0,
                scatter: 2.0,
                life: 0.9,
                spin: 25.0,
                ..SPRAY
            },
        ],
        ImpactKind::Metal | ImpactKind::Robot => vec![Spray {
            n: 10,
            mat: |a| a.spark.clone(),
            size: Vec3::new(0.005, 0.005, 0.1),
            out: 3.0,
            scatter: 3.5,
            carry: 4.0,
            life: 0.22,
            align: true,
            ..SPRAY
        }],
        ImpactKind::Glass => vec![Spray {
            n: 9,
            mat: |a| a.shard.clone(),
            size: Vec3::new(0.035, 0.003, 0.025),
            out: 1.5,
            scatter: 1.5,
            carry: 1.5,
            life: 0.9,
            spin: 18.0,
            ..SPRAY
        }],
        ImpactKind::Soil => vec![
            puff(|a| a.soil_dust.clone(), 3),
            Spray {
                n: 6,
                mat: |a| a.clod.clone(),
                size: Vec3::splat(0.03),
                out: 3.0,
                scatter: 1.2,
                life: 0.8,
                spin: 10.0,
                ..SPRAY
            },
        ],
        ImpactKind::Water => vec![Spray {
            n: 14,
            mesh: |a| a.ball.clone(),
            mat: |a| a.spray.clone(),
            size: Vec3::splat(0.03),
            out: 4.0,
            scatter: 1.0,
            life: 0.75,
            ..SPRAY
        }],
        ImpactKind::Body => Vec::new(),
    }
}

/// What a round throws out of the far side of what it went through, on
/// along its line (`n` is the way it went): steel spalls a cone of hot
/// flakes, boards blow out long splinters, masonry a cone of dust and
/// grit, glass a fan of shards.
fn spall(kind: ImpactKind) -> Vec<Spray> {
    let cone = |mat: fn(&FxAssets) -> Handle<StandardMaterial>, n| Spray {
        n,
        mesh: |a| a.ball.clone(),
        mat,
        size: Vec3::splat(0.07),
        out: 3.5,
        scatter: 0.9,
        life: 1.1,
        grow: 2.6,
        gravity: -0.2,
        drag: 3.5,
        wind: 0.7,
        ..SPRAY
    };
    match kind {
        ImpactKind::Metal | ImpactKind::Robot => vec![
            Spray {
                n: 16,
                mat: |a| a.spark.clone(),
                size: Vec3::new(0.004, 0.004, 0.09),
                out: 7.0,
                scatter: 2.2,
                life: 0.25,
                align: true,
                ..SPRAY
            },
            Spray {
                n: 5,
                mat: |a| a.flake.clone(),
                size: Vec3::new(0.014, 0.003, 0.012),
                out: 4.5,
                scatter: 1.8,
                life: 0.8,
                spin: 30.0,
                ..SPRAY
            },
        ],
        ImpactKind::Wood => vec![
            cone(|a| a.soil_dust.clone(), 2),
            Spray {
                n: 10,
                mat: |a| a.splinter.clone(),
                size: Vec3::new(0.01, 0.01, 0.11),
                out: 5.0,
                scatter: 1.6,
                life: 1.0,
                spin: 30.0,
                ..SPRAY
            },
        ],
        ImpactKind::Masonry => vec![
            cone(|a| a.dust.clone(), 6),
            Spray {
                n: 9,
                size: Vec3::splat(0.026),
                out: 4.0,
                scatter: 1.4,
                life: 0.8,
                spin: 20.0,
                ..SPRAY
            },
        ],
        ImpactKind::Glass => vec![Spray {
            n: 12,
            mat: |a| a.shard.clone(),
            size: Vec3::new(0.03, 0.003, 0.022),
            out: 3.5,
            scatter: 1.2,
            life: 0.9,
            spin: 18.0,
            ..SPRAY
        }],
        k => recipe(k),
    }
}

/// An impact on the far side of what the round went through: its surface
/// faces along the round.
pub fn is_exit(imp: &Impact) -> bool {
    imp.entity.is_some() && imp.normal.dot(imp.velocity) > 0.0
}

/// Throws one spray's bits from `at`: out along `n`, carried along
/// `glance`.
pub(crate) fn spray_bits(
    commands: &mut Commands,
    pool: &mut FxPool,
    a: &FxAssets,
    s: &Spray,
    at: Vec3,
    n: Vec3,
    glance: Vec3,
) {
    for _ in 0..s.n {
        let rand = Vec3::new(
            pool.rng.range(-1.0, 1.0),
            pool.rng.range(-1.0, 1.0),
            pool.rng.range(-1.0, 1.0),
        );
        let vel = n * s.out * pool.rng.range(0.6, 1.2)
            + rand * s.scatter
            + glance * s.carry * pool.rng.range(0.5, 1.0);
        let rot = if s.align {
            Quat::from_rotation_arc(Vec3::Z, vel.normalize_or(Vec3::Z))
        } else {
            Quat::IDENTITY
        };
        let e = commands
            .spawn((
                Bit {
                    vel,
                    age: 0.0,
                    life: s.life * pool.rng.range(0.7, 1.2),
                    size: s.size,
                    grow: s.grow,
                    gravity: s.gravity,
                    drag: s.drag,
                    align: s.align,
                    spin: rand * s.spin,
                    wind: s.wind,
                },
                Mesh3d((s.mesh)(a)),
                MeshMaterial3d((s.mat)(a)),
                Transform::from_translation(at)
                    .with_rotation(rot)
                    .with_scale(s.size),
                NotShadowCaster,
                NotShadowReceiver,
            ))
            .id();
        if let Some(old) = pool.bits.push(e) {
            commands.entity(old).try_despawn();
        }
    }
}

fn spawn_fx(
    mut commands: Commands,
    mut impacts: MessageReader<Impact>,
    mut glances: MessageReader<super::ballistics::Ricochet>,
    assets: Option<Res<FxAssets>>,
    mut pool: ResMut<FxPool>,
) {
    let Some(a) = assets else {
        impacts.clear();
        glances.clear();
        return;
    };
    let pool = &mut *pool;
    // A glance: a streak of sparks skating off along the round's new line.
    for g in glances.read() {
        let out = g.velocity.normalize_or(g.normal);
        let streak = Spray {
            n: 6,
            mat: |a| a.spark.clone(),
            size: Vec3::new(0.005, 0.005, 0.12),
            out: 1.0,
            scatter: 1.5,
            carry: 8.0,
            life: 0.18,
            align: true,
            ..SPRAY
        };
        spray_bits(&mut commands, pool, &a, &streak, g.point, g.normal, out);
    }
    for imp in impacts.read() {
        let n = imp.normal.normalize_or(Vec3::Y);
        let dir = imp.velocity.normalize_or_zero();
        // Off the surface, the way a round glances.
        let glance = (dir - 2.0 * dir.dot(n) * n).normalize_or(n);
        let sprays = if is_exit(imp) {
            spall(imp.kind)
        } else if imp.kind == ImpactKind::Robot {
            robot_recipe(imp.part, imp.hurt)
        } else {
            recipe(imp.kind)
        };
        for s in sprays {
            spray_bits(&mut commands, pool, &a, &s, imp.point + n * 0.01, n, glance);
        }
        if imp.decal {
            // The mark, and on wood and metal a smaller layer on top of it
            // (a dark hole in a fresh pale rim; a dark centre in a bright
            // pock), 4 mm prouder so the two never fight.
            let (size, mat, top) = match imp.kind {
                ImpactKind::Masonry => (0.06, a.hole.clone(), None),
                ImpactKind::Wood => (0.045, a.pale_rim.clone(), Some((0.45, a.hole.clone()))),
                ImpactKind::Metal => (0.035, a.bright_pock.clone(), Some((0.4, a.hole.clone()))),
                ImpactKind::Glass => (0.05, a.shard.clone(), None),
                _ => (0.09, a.soil_mark.clone(), None),
            };
            let twist = pool.rng.range(0.0, std::f32::consts::TAU);
            let rot = Quat::from_rotation_arc(Vec3::Z, n) * Quat::from_rotation_z(twist);
            let e = commands
                .spawn((
                    Decal { age: 0.0, size },
                    Mesh3d(a.disc.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(imp.point + n * DECAL_PROUD)
                        .with_rotation(rot)
                        .with_scale(Vec3::splat(size)),
                    NotShadowCaster,
                ))
                .id();
            if let Some((r, top)) = top {
                commands.spawn((
                    Mesh3d(a.disc.clone()),
                    MeshMaterial3d(top),
                    Transform::from_xyz(0.0, 0.0, 0.004 / size).with_scale(Vec3::new(r, r, 1.0)),
                    NotShadowCaster,
                    ChildOf(e),
                ));
            }
            if let Some(old) = pool.decals.push(e) {
                commands.entity(old).try_despawn();
            }
        }
    }
}

fn fly_bits(
    mut commands: Commands,
    mut bits: Query<(Entity, &mut Bit, &mut Transform)>,
    time: Res<Time>,
    weather: Option<Res<crate::world::weather::Conditions>>,
) {
    let dt = time.delta_secs();
    let wind = wind_velocity(weather.as_deref());
    for (e, mut b, mut t) in &mut bits {
        b.age += dt;
        if b.age >= b.life {
            commands.entity(e).despawn();
            continue;
        }
        let g = b.gravity;
        let drag = b.drag;
        b.vel += Vec3::NEG_Y * g * dt;
        b.vel *= 1.0 - (drag * dt).min(1.0);
        t.translation += (b.vel + wind * b.wind) * dt;
        let k = b.age / b.life;
        // Dust spreads; everything shrinks away at the end of its life.
        let s = b.size * (1.0 + b.grow * k) * (1.0 - k * k * k);
        t.scale = s.max(Vec3::splat(1e-4));
        if b.align {
            if let Ok(d) = Dir3::new(b.vel) {
                t.rotation = Quat::from_rotation_arc(Vec3::Z, *d);
            }
        } else if b.spin != Vec3::ZERO {
            let w = b.spin * dt;
            t.rotation = Quat::from_scaled_axis(w) * t.rotation;
        }
    }
}

fn age_decals(
    mut commands: Commands,
    mut decals: Query<(Entity, &mut Decal, &mut Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut d, mut t) in &mut decals {
        d.age += dt;
        if d.age >= DECAL_LIFE {
            commands.entity(e).despawn();
            continue;
        }
        // The last few seconds, it fades by shrinking.
        let left = ((DECAL_LIFE - d.age) / 5.0).min(1.0);
        let s = d.size * left;
        if (t.scale.x - s).abs() > 1e-5 {
            t.scale = Vec3::new(s, s, d.size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Out of the far side, a round throws more, and harder, on along its
    /// line than it splashes back off the face it went in by.
    #[test]
    fn spall_is_thrown_on_harder_than_the_entry_splash() {
        let push = |v: Vec<Spray>| v.iter().map(|s| s.n as f32 * s.out).sum::<f32>();
        for k in [
            ImpactKind::Metal,
            ImpactKind::Wood,
            ImpactKind::Masonry,
            ImpactKind::Glass,
        ] {
            assert!(push(spall(k)) > 1.5 * push(recipe(k)), "{k:?}");
        }
    }

    /// A round fired down into the creek splashes at its surface (it used
    /// to fly on through to the bed and kick up dirt).
    #[test]
    fn a_round_into_water_splashes_at_the_surface() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        #[derive(Resource, Default)]
        struct Seen(Vec<Impact>);
        app.init_resource::<Seen>().add_systems(
            FixedUpdate,
            (|mut r: MessageReader<Impact>, mut s: ResMut<Seen>| s.0.extend(r.read().copied()))
                .after(super::super::ballistics::BallisticsSystems),
        );
        app.finish();
        app.cleanup();
        app.update();
        let (at, level) = {
            let hm = &app.world().resource::<crate::world::Zone>().hm;
            (0..hm.n * hm.n)
                .find_map(|k| {
                    let (i, j) = (k % hm.n, k / hm.n);
                    let w = hm.water[hm.idx(i, j)];
                    let p = hm.pos(i, j);
                    let inside = (8..hm.n - 8).contains(&i) && (8..hm.n - 8).contains(&j);
                    (inside && !w.is_nan() && w - hm.height([p[0], p[1]]) > 0.3).then_some((p, w))
                })
                .expect("deep water on the default map")
        };
        let gun = app.world_mut().spawn_empty().id();
        app.world_mut()
            .write_message(super::super::ballistics::Shot {
                gun,
                shooter: None,
                origin: Vec3::new(at[0], level + 3.0, at[1]),
                direction: Vec3::NEG_Y,
                speed: 300.0,
                damage: 10.0,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: 1,
            });
        for _ in 0..10 {
            app.update();
        }
        let seen = &app.world().resource::<Seen>().0;
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert_eq!(seen[0].kind, ImpactKind::Water, "{seen:?}");
        assert!(
            (seen[0].point.y - level).abs() < 0.05,
            "{seen:?} at {level}"
        );
    }

    #[test]
    fn every_material_has_an_impact() {
        // The masonry, wood and metal the buildings are made of read as
        // what they are; window glass shatters.
        assert_eq!(kind_of(Mat::Brick), ImpactKind::Masonry);
        assert_eq!(kind_of(Mat::Concrete), ImpactKind::Masonry);
        assert_eq!(kind_of(Mat::Plaster), ImpactKind::Masonry);
        assert_eq!(kind_of(Mat::LogWall), ImpactKind::Wood);
        assert_eq!(kind_of(Mat::BarkPine), ImpactKind::Wood);
        assert_eq!(kind_of(Mat::Corrugated), ImpactKind::Metal);
        assert_eq!(kind_of(Mat::RustyMetal), ImpactKind::Metal);
        assert_eq!(kind_of(Mat::Glass), ImpactKind::Glass);
        // Every class but people's has bits to show.
        for k in [
            ImpactKind::Masonry,
            ImpactKind::Wood,
            ImpactKind::Metal,
            ImpactKind::Glass,
            ImpactKind::Soil,
            ImpactKind::Water,
            ImpactKind::Robot,
        ] {
            assert!(recipe(k).iter().map(|s| s.n).sum::<usize>() > 0, "{k:?}");
        }
    }

    /// A round that hurts a robot shows it, differently by part; one off a
    /// dead part only glances (a few sparks, nothing torn off).
    #[test]
    fn robot_hits_read_by_part_and_harm() {
        use crate::robots::PartRole;
        let n = |v: Vec<Spray>| v.iter().map(|s| s.n).sum::<usize>();
        let glance = n(robot_recipe(Some(PartRole::Core), false));
        for p in [
            PartRole::Core,
            PartRole::Sensor,
            PartRole::Weapon,
            PartRole::Leg(0),
            PartRole::Rotor(0),
            PartRole::GasCell(0),
        ] {
            assert!(n(robot_recipe(Some(p), true)) > glance, "{p:?}");
        }
        // Each hurt part has something of its own beyond sparks.
        let mats = |p| {
            robot_recipe(Some(p), true)
                .iter()
                .map(|s| s.mat as usize)
                .collect::<std::collections::HashSet<_>>()
        };
        assert_ne!(mats(PartRole::Sensor), mats(PartRole::Core));
        assert_ne!(mats(PartRole::Leg(0)), mats(PartRole::Core));
        assert_ne!(mats(PartRole::GasCell(0)), mats(PartRole::Leg(0)));
        // A plate taking a round only glances: it's the shell's job.
        assert_eq!(n(robot_recipe(Some(PartRole::Armour(0)), true)), glance);
    }

    /// However long the firefight, the pools never hold more than their
    /// caps (each live bit and mark is a draw).
    #[test]
    fn the_pools_are_capped() {
        let mut bits = Capped::new(BIT_CAP);
        let mut evicted = 0;
        for i in 0..2000u32 {
            if bits.push(Entity::from_raw_u32(i + 1).unwrap()).is_some() {
                evicted += 1;
            }
            assert!(bits.len() <= BIT_CAP);
        }
        assert_eq!(evicted, 2000 - BIT_CAP);
        // The worst single impact fits many times over.
        let most = [
            ImpactKind::Masonry,
            ImpactKind::Wood,
            ImpactKind::Metal,
            ImpactKind::Water,
        ]
        .iter()
        .map(|k| recipe(*k).iter().map(|s| s.n).sum::<usize>())
        .max()
        .unwrap();
        assert!(most * 8 <= BIT_CAP, "{most} bits an impact");
        assert!(BIT_CAP + 2 * DECAL_CAP < crate::render::budget::BUDGET.draws / 4);
    }
}

#[cfg(test)]
mod plate_tests {
    use super::*;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;
    use extraction_core::firearm::FireMode;

    #[derive(Resource, Default)]
    struct Seen(Vec<Impact>);

    fn see(mut impacts: MessageReader<Impact>, mut seen: ResMut<Seen>) {
        seen.0.extend(impacts.read().copied());
    }

    /// Every range target in the yard stops a pistol round on its steel
    /// plate, with a metal impact (the spark, the ring) and a mark, near
    /// where the gun was pointed (the first playtest shot straight
    /// through them, and couldn't tell a hit).
    #[test]
    fn every_range_plate_stops_and_marks_a_round() {
        let mut r = hold("pm_pistol", false, false);
        r.vr.app.init_resource::<Seen>().add_systems(Update, see);
        load(&mut r, "pm", 8, FireMode::Semi);
        let plates: Vec<(Entity, Vec3)> = {
            let mut q =
                r.vr.app
                    .world_mut()
                    .query::<(Entity, &crate::world::buildings::Building, &GlobalTransform)>();
            q.iter(r.vr.app.world())
                .filter(|(_, b, _)| b.prefab == "target")
                .map(|(e, _, gt)| (e, gt.transform_point(Vec3::new(0.0, 1.2, 0.08))))
                .collect()
        };
        assert!(!plates.is_empty(), "the yard has its range");
        let mut bad = Vec::new();
        for (i, (_, plate)) in plates.iter().enumerate() {
            // Stand off 6 m in front of it and point the gun at its middle.
            let gt =
                r.vr.app
                    .world()
                    .get::<GlobalTransform>(r.gun)
                    .unwrap()
                    .compute_transform();
            let hand = r.vr.hand_state(Side::Right).world;
            let rel = hand.rotation.inverse() * gt.rotation;
            let normal = {
                let w = r.vr.app.world();
                let b = plates[i].0;
                w.get::<GlobalTransform>(b).unwrap().rotation() * Vec3::Z
            };
            let from = *plate + normal * 6.0;
            let want = Transform::from_translation(from)
                .looking_at(*plate, Vec3::Y)
                .rotation;
            let grip_off = gt.translation - hand.translation;
            r.vr.hand(Side::Right, from - grip_off, want * rel.inverse(), 1.0, 0.0);
            r.vr.step(10);
            r.vr.app.world_mut().resource_mut::<Seen>().0.clear();
            if fire(&mut r, 1, true).is_empty() {
                bad.push(format!("target {i}: no shot"));
                continue;
            }
            r.vr.step(20);
            let seen = std::mem::take(&mut r.vr.app.world_mut().resource_mut::<Seen>().0);
            let on_plate: Vec<_> = seen
                .iter()
                .filter(|im| im.point.distance(*plate) < 0.3)
                .collect();
            if !on_plate
                .iter()
                .any(|im| im.kind == ImpactKind::Metal && im.decal)
            {
                bad.push(format!(
                    "target {i}: no marked metal hit on the plate ({seen:?})"
                ));
            }
            if seen
                .iter()
                .any(|im| (im.point - *plate).dot(-normal) > 0.05)
            {
                bad.push(format!("target {i}: a round went on through"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
