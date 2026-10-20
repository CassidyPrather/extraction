//! The ballistics range: every gun fired at a panel of every material,
//! through the real projectile path, against a table of what should
//! happen (it stops, it goes through, it glances off). Built high above
//! the valley, out of everything's way, with a witness plate behind each
//! panel: a hit on the witness is a round that got through.
//!
//! `run_range` drives it in a headless app; the scenario `ballistics_range`
//! and a test both use it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm_terminal::Stuff;

use super::Gun;
use super::ballistics::{Hit, Ricochet, Shot};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::render::materials::Mat;
use crate::world::Layer;
use crate::world::surfaces::Surfaces;

/// What should happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Stop,
    Through,
    /// Most rounds at a graze glance off.
    Glance,
}

/// The guns on the range (catalog ids).
pub const GUNS: [&str; 5] = ["pm_pistol", "ak74", "mosin", "sks", "toz_double"];

/// The panels: a material and its thickness, as buildings use them.
pub const PANELS: [(Mat, f32); 9] = [
    (Mat::Planks, 0.1),
    (Mat::LogWall, 0.25),
    (Mat::Plaster, 0.15),
    (Mat::Brick, 0.25),
    (Mat::Concrete, 0.2),
    (Mat::Boulder, 0.5),
    (Mat::Corrugated, 0.02),
    (Mat::RobotShell, 0.03),
    (Mat::Glass, 0.01),
];

/// Head on: does `gun` get through `mat`?
pub fn expected(gun: &str, mat: Mat) -> Expect {
    use Expect::*;
    let rifle = matches!(gun, "ak74" | "mosin" | "sks");
    match super::impacts::stuff_of(mat) {
        Stuff::Glass | Stuff::Sheet => Through,
        Stuff::Planks | Stuff::Plaster if rifle => Through,
        Stuff::Logs if gun == "mosin" => Through,
        _ => Stop,
    }
}

/// At a graze: hard faces turn rifle rounds.
pub fn expected_graze(mat: Mat) -> Option<Expect> {
    super::impacts::stuff_of(mat)
        .ricochets()
        .then_some(Expect::Glance)
}

/// The line of fire: head on into a panel facing +Z, or a graze, 0.03
/// off its face.
fn line(graze: bool) -> Vec3 {
    if graze {
        Vec3::new(-1.0, 0.0, -0.03).normalize()
    } else {
        Vec3::NEG_Z
    }
}

/// Where the range is built: well above any valley.
const ALTITUDE: f32 = 900.0;
/// Panels this far apart, so no round strays into a neighbour.
const SPACING: f32 = 6.0;

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Witness {
    cell: usize,
}

/// One trial: a gun, a panel, head on or at a graze.
#[derive(Clone, Debug)]
pub struct Cell {
    pub gun: &'static str,
    pub mat: Mat,
    pub graze: bool,
    pub want: Expect,
    pub shots: u32,
    pub through: u32,
    pub glanced: u32,
}

impl Cell {
    pub fn got(&self) -> Expect {
        match self.want {
            Expect::Glance if self.glanced * 2 > self.shots => Expect::Glance,
            _ if self.through > 0 && self.through * 2 >= self.shots => Expect::Through,
            _ if self.glanced * 2 > self.shots => Expect::Glance,
            _ => Expect::Stop,
        }
    }
}

/// Builds the range in `app`'s world and fires every trial; returns the
/// cells with what happened.
pub fn run_range(app: &mut App) -> Vec<Cell> {
    let catalog = app.world().resource::<Catalog>().clone();
    let mut cells = Vec::new();
    let mut guns = Vec::new();
    for (g, id) in GUNS.iter().enumerate() {
        let kind = catalog.kind(id).expect("a gun on the range");
        let gun = {
            let mut commands = app.world_mut().commands();
            spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_xyz(-50.0, ALTITUDE - 40.0, g as f32 * 3.0),
                    condition: 1.0,
                },
            )
        };
        guns.push((*id, gun));
        for (mat, _) in PANELS.iter() {
            cells.push(Cell {
                gun: id,
                mat: *mat,
                graze: false,
                want: expected(id, *mat),
                shots: 0,
                through: 0,
                glanced: 0,
            });
            if let (Some(want), true) = (expected_graze(*mat), *id == "ak74" || *id == "mosin") {
                cells.push(Cell {
                    gun: id,
                    mat: *mat,
                    graze: true,
                    want,
                    shots: 0,
                    through: 0,
                    glanced: 0,
                });
            }
        }
    }
    app.world_mut().flush();
    app.update();
    for (i, cell) in cells.iter().enumerate() {
        let thick = PANELS.iter().find(|(m, _)| *m == cell.mat).unwrap().1;
        let at = Vec3::new(i as f32 * SPACING, ALTITUDE, 0.0);
        // The panel faces +Z; the round comes at it head on or at a graze.
        let half = Vec3::new(1.0, 1.0, thick * 0.5);
        let dir = line(cell.graze);
        app.world_mut().spawn((
            Panel,
            Surfaces(vec![(half, Transform::IDENTITY, cell.mat)]),
            Transform::from_translation(at),
            RigidBody::Static,
            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        // The witness, on along the round's line: it catches what went
        // through; a glance leaves the other way.
        app.world_mut().spawn((
            Witness { cell: i },
            Transform::from_translation(at + dir * 2.5).looking_to(dir, Vec3::Y),
            RigidBody::Static,
            Collider::cuboid(1.2, 1.2, 0.05),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    app.update();
    #[derive(Resource, Default)]
    struct Seen {
        witness: Vec<usize>,
        glances: Vec<Vec3>,
    }
    app.init_resource::<Seen>().add_systems(
        FixedUpdate,
        (|mut hits: MessageReader<Hit>,
          mut glances: MessageReader<Ricochet>,
          witnesses: Query<&Witness>,
          mut seen: ResMut<Seen>| {
            for h in hits.read() {
                if let Ok(w) = witnesses.get(h.entity) {
                    seen.witness.push(w.cell);
                }
            }
            seen.glances.extend(glances.read().map(|g| g.point));
        })
        .after(super::ballistics::BallisticsSystems),
    );
    // Fire each trial in turn and let its rounds settle.
    for i in 0..cells.len() {
        let gun_id = cells[i].gun;
        let gun = guns.iter().find(|(id, _)| *id == gun_id).unwrap().1;
        let def = app
            .world()
            .get::<Gun>(gun)
            .expect("the range's gun")
            .firearm
            .def
            .clone();
        let at = Vec3::new(i as f32 * SPACING, ALTITUDE, 0.0);
        let shots = if cells[i].graze { 8 } else { 3 };
        for k in 0..shots {
            // At the middle of the panel's face, from 6 m back along the line.
            let thick = PANELS.iter().find(|(m, _)| *m == cells[i].mat).unwrap().1;
            let dir = line(cells[i].graze);
            let face = at + Vec3::new(0.0, 0.1 * k as f32 - 0.3, thick * 0.5);
            let origin = face - dir * 6.0;
            let before = {
                let s = app.world().resource::<Seen>();
                (
                    s.witness.iter().filter(|c| **c == i).count(),
                    s.glances.len(),
                )
            };
            app.world_mut().write_message(Shot {
                gun,
                shooter: None,
                origin,
                direction: dir,
                speed: def.muzzle_velocity,
                damage: def.damage,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: (i as u64) << 8 | k as u64,
            });
            for _ in 0..6 {
                app.update();
            }
            let s = app.world().resource::<Seen>();
            let through = s.witness.iter().filter(|c| **c == i).count() > before.0;
            let glanced = s.glances.len() > before.1;
            cells[i].shots += 1;
            cells[i].through += u32::from(through && !glanced);
            cells[i].glanced += u32::from(glanced);
        }
    }
    cells
}

/// The cells that didn't go as the table says, for a report.
pub fn failures(cells: &[Cell]) -> Vec<String> {
    cells
        .iter()
        .filter(|c| c.got() != c.want)
        .map(|c| {
            format!(
                "{} {} {:?}: wanted {:?}, got {:?} ({} shots, {} through, {} glanced)",
                c.gun,
                if c.graze { "grazing" } else { "head on" },
                c.mat,
                c.want,
                c.got(),
                c.shots,
                c.through,
                c.glanced
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every gun at every material, head on, and the rifles at a graze off
    /// the hard ones: the pass/stop/glance matrix holds through the real
    /// projectiles.
    #[test]
    fn the_range_matrix_holds() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let cells = run_range(&mut app);
        assert!(cells.len() >= GUNS.len() * PANELS.len());
        let bad = failures(&cells);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
