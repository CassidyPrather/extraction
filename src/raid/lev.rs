//! The driver in the truck: Lev, behind the cab's rear window
//! (`docs/LORE.md`, "The driver"). What he gives you he passes back
//! through a hatch in the bed's front wall: the hatch slides open, his hand
//! comes through with the thing, sets it on the ledge under the hatch, and
//! goes back; when he's handed everything over the hatch shuts. Anything
//! that's a gift from him comes this way ([`HandOver`]).
//!
//! And he notices what you bring home: the drive-home line changes with an
//! artifact, a robot's eye or cell, a lot of food (`raid::lines`).

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::VecDeque;

use super::truck::{FLOOR, Truck};
use super::{Phase, Raid};
use crate::anomaly::artifacts::CarriedBy;
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::rig::LocalPlayer;
use crate::world::Layer;

pub struct LevPlugin;

impl Plugin for LevPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HandOver>()
            .init_resource::<Handovers>()
            .add_systems(Update, (fit_hatch, hand_over, homecoming).chain());
    }
}

/// Lev hands an item back through the hatch (one per item).
#[derive(Message, Clone, Debug)]
pub struct HandOver {
    pub id: String,
    pub condition: f32,
}

/// The hatch in the bed's front wall, truck space: its centre (on the
/// wall) and size; between the memorial board and the medicine cabinet, at
/// the height of the cab's rear window.
pub const HATCH_AT: Vec3 = Vec3::new(0.23, FLOOR + 0.85, -1.79);
pub const HATCH: Vec2 = Vec2::new(0.30, 0.26);
/// The ledge under it: the centre of its top.
pub const LEDGE_AT: Vec3 = Vec3::new(0.23, FLOOR + 0.70, -1.72);
/// The ledge's collider (the model's timber is 0.34 wide; things go in
/// its middle, clear of the cabinet).
pub const LEDGE: Vec3 = Vec3::new(0.26, 0.025, 0.14);

/// How long each move takes, seconds.
const OPEN: f32 = 0.6;
const REACH: f32 = 1.0;
const WITHDRAW: f32 = 0.7;
const CLOSE: f32 = 0.6;

/// The hatch panel: how far open, 0 to 1.
#[derive(Component, Default, Debug)]
pub struct CabHatch {
    pub open: f32,
}

/// His hand (posed by the client).
#[derive(Component)]
pub struct LevHand;

/// The ledge under the hatch.
#[derive(Component)]
pub struct HatchLedge;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
enum Stage {
    #[default]
    Idle,
    Opening,
    Reaching,
    Withdrawing,
    Closing,
}

/// What he's still to hand over, and where he is with it.
#[derive(Resource, Default)]
pub struct Handovers {
    queue: VecDeque<HandOver>,
    stage: Stage,
    t: f32,
    /// The item in his hand.
    item: Option<Entity>,
    /// Things set down so far (they go side by side).
    placed: usize,
    /// Where the one in his hand goes.
    target: Vec3,
    /// Every item that came through, for the tests: (id, entity).
    pub handed: Vec<(String, Entity)>,
}

impl Handovers {
    pub fn busy(&self) -> bool {
        self.stage != Stage::Idle || !self.queue.is_empty()
    }
}

/// Where the `n`th thing goes, truck space: small things side by side on
/// the ledge; a long gun laid on the floor below the hatch, along the bed.
pub fn slot(n: usize, long: bool) -> Vec3 {
    if long {
        return Vec3::new(0.35, FLOOR + 0.05, -1.2) + Vec3::X * 0.08 * (n % 3) as f32;
    }
    let x = [0.0, -0.08, 0.08, -0.04, 0.04][n % 5];
    LEDGE_AT + Vec3::new(x, LEDGE.y * 0.5 + 0.04, 0.0)
}

/// His hand's path, truck space, by how far along the reach (0 in the cab,
/// 1 over the ledge).
pub fn hand_at(s: f32, slot: Vec3) -> Vec3 {
    let start = HATCH_AT - Vec3::Z * 0.45;
    let mid = HATCH_AT + Vec3::Z * 0.08;
    let end = slot + Vec3::Y * 0.06;
    if s < 0.5 {
        start.lerp(mid, s * 2.0)
    } else {
        mid.lerp(end, (s - 0.5) * 2.0)
    }
}

/// The hatch, the ledge and his hand go in when the truck does.
fn fit_hatch(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Cab hatch"),
            CabHatch::default(),
            Transform::from_translation(HATCH_AT),
            Visibility::default(),
            ChildOf(truck),
        ));
        commands.spawn((
            Name::new("Hatch ledge"),
            HatchLedge,
            Collider::cuboid(LEDGE.x, LEDGE.y, LEDGE.z),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
            Transform::from_translation(LEDGE_AT - Vec3::Y * LEDGE.y * 0.5),
            Visibility::default(),
            ChildOf(truck),
        ));
        commands.spawn((
            Name::new("Lev's hand"),
            LevHand,
            Transform::from_translation(HATCH_AT - Vec3::Z * 0.45),
            Visibility::Hidden,
            ChildOf(truck),
        ));
    }
}

/// Opens the hatch, passes each thing through, shuts it.
#[allow(clippy::too_many_arguments)]
fn hand_over(
    mut commands: Commands,
    mut asked: MessageReader<HandOver>,
    mut state: ResMut<Handovers>,
    time: Res<Time>,
    catalog: Res<Catalog>,
    trucks: Query<&GlobalTransform, With<Truck>>,
    mut hatches: Query<&mut CabHatch>,
    mut hands: Query<(&mut Transform, &mut Visibility), With<LevHand>>,
    mut items: Query<&mut Transform, (With<Item>, Without<LevHand>)>,
) {
    state.queue.extend(asked.read().cloned());
    let Ok(truck) = trucks.single() else {
        return;
    };
    let dt = time.delta_secs();
    let s = &mut *state;
    s.t += dt;
    let mut open = None;
    match s.stage {
        Stage::Idle => {
            if !s.queue.is_empty() {
                s.stage = Stage::Opening;
                s.t = 0.0;
                s.placed = 0;
            }
        }
        Stage::Opening => {
            open = Some((s.t / OPEN).min(1.0));
            if s.t >= OPEN {
                s.stage = Stage::Reaching;
                s.t = 0.0;
            }
        }
        Stage::Reaching => {
            // The thing is in his hand from the start of the reach.
            if s.item.is_none()
                && let Some(next) = s.queue.pop_front()
            {
                if let Some(kind) = catalog.kind(&next.id) {
                    let long =
                        catalog.get(kind).category == extraction_core::items::Category::Firearm;
                    s.target = slot(s.placed, long);
                    let e = spawn_item(
                        &mut commands,
                        &catalog,
                        SpawnItem {
                            kind,
                            transform: Transform::from_translation(
                                truck.transform_point(hand_at(0.0, s.target)),
                            ),
                            condition: next.condition,
                        },
                    );
                    commands
                        .entity(e)
                        .insert((RigidBodyDisabled, ColliderDisabled));
                    s.item = Some(e);
                    s.handed.push((next.id.clone(), e));
                } else {
                    warn!("Lev has no {} to hand over", next.id);
                }
            }
            let k = (s.t / REACH).min(1.0);
            let at = truck.transform_point(hand_at(k, s.target));
            if let Some(e) = s.item
                && let Ok(mut t) = items.get_mut(e)
            {
                t.translation = at - truck.rotation() * Vec3::Y * 0.03;
                t.rotation = truck.rotation();
            }
            if s.t >= REACH {
                // Set down on the ledge, and let go.
                if let Some(e) = s.item.take() {
                    if let Ok(mut t) = items.get_mut(e) {
                        t.translation = truck.transform_point(s.target);
                    }
                    commands
                        .entity(e)
                        .remove::<(RigidBodyDisabled, ColliderDisabled)>();
                    s.placed += 1;
                }
                s.stage = Stage::Withdrawing;
                s.t = 0.0;
            }
        }
        Stage::Withdrawing => {
            if s.t >= WITHDRAW {
                s.t = 0.0;
                s.stage = if s.queue.is_empty() {
                    Stage::Closing
                } else {
                    Stage::Reaching
                };
            }
        }
        Stage::Closing => {
            open = Some(1.0 - (s.t / CLOSE).min(1.0));
            if s.t >= CLOSE {
                s.stage = Stage::Idle;
                s.t = 0.0;
            }
        }
    }
    if let Some(o) = open {
        for mut h in &mut hatches {
            h.open = o;
        }
    }
    // The hand: out while reaching and withdrawing.
    let (local, shown) = match s.stage {
        Stage::Reaching => (hand_at((s.t / REACH).min(1.0), s.target), true),
        Stage::Withdrawing => (hand_at(1.0 - (s.t / WITHDRAW).min(1.0), s.target), true),
        _ => (hand_at(0.0, s.target), false),
    };
    for (mut t, mut v) in &mut hands {
        t.translation = local;
        let want = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
}

/// What came home with you, as far as Lev's concerned.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Cargo {
    pub artifact: bool,
    pub eye: bool,
    pub cell: bool,
    pub food: usize,
}

/// Sorts what someone carries into what he'd remark on.
pub fn cargo(ids: impl Iterator<Item = (String, extraction_core::items::Category)>) -> Cargo {
    use extraction_core::items::Category;
    let mut c = Cargo::default();
    for (id, cat) in ids {
        match (id.as_str(), cat) {
            (_, Category::Artifact) => c.artifact = true,
            ("sensor_eye", _) => c.eye = true,
            ("robot_cell", _) => c.cell = true,
            (_, Category::Food) => c.food += 1,
            _ => {}
        }
    }
    c
}

/// On the drive home he says a line: his story when it's time, else
/// something about what you've brought, else the weather.
fn homecoming(
    mut raid: ResMut<Raid>,
    mut was: Local<Option<Phase>>,
    players: Query<Entity, With<LocalPlayer>>,
    items: Query<(&Item, &CarriedBy)>,
    catalog: Res<Catalog>,
) {
    let before = was.replace(raid.phase.clone());
    let home = matches!(before, Some(Phase::Extracting { .. })) && raid.phase == Phase::AtBase;
    if !home {
        return;
    }
    let Some(player) = players.iter().next() else {
        return;
    };
    let carried = cargo(items.iter().filter(|(_, by)| by.0 == player).map(|(i, _)| {
        let def = catalog.get(i.kind());
        (def.id.clone(), def.category)
    }));
    let line = super::lines::coming_home_with(raid.extractions, raid.recalled, carried);
    raid.say(line);
}

/// Client: the hatch's sliding panel, his hand and sleeve (the opening
/// and the ledge are in the truck model).
pub struct LevVisualsPlugin;

impl Plugin for LevVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_hatch, slide_hatch));
    }
}

use crate::render::world_props::{Blockout, SwapIn};

/// The panel's depth, truck space, in the runners (`art/vehicles/truck.py`).
const PANEL_Z: f32 = -1.8255;

/// The panel slides sideways to open.
#[derive(Component)]
struct HatchPanel;

fn dress_hatch(
    mut commands: Commands,
    hatches: Query<Entity, Added<CabHatch>>,
    hands: Query<Entity, Added<LevHand>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mat = |c: Color, rough: f32| {
        materials.add(StandardMaterial {
            base_color: c,
            perceptual_roughness: rough,
            ..default()
        })
    };
    for e in &hatches {
        // The panel over the opening (the hole, its boot and the cab
        // behind are the truck model's).
        // Shut, the panel sits in the truck model's runners, just behind
        // the opening's face. Boxes until its model (`cab_hatch_panel`).
        commands
            .spawn((
                HatchPanel,
                SwapIn::one("cab_hatch_panel"),
                Transform::from_xyz(0.0, 0.0, PANEL_Z - HATCH_AT.z),
                Visibility::default(),
                ChildOf(e),
            ))
            .with_child((
                Mesh3d(meshes.add(Cuboid::new(HATCH.x + 0.02, HATCH.y + 0.02, 0.015))),
                MeshMaterial3d(mat(Color::srgb(0.27, 0.30, 0.22), 0.7)),
                Transform::IDENTITY,
                Blockout,
            ));
    }
    for e in &hands {
        // A bare, weathered hand, palm up, fingers curled round what it
        // carries (until the Hands workstream's posed one), on the end of
        // a grey wool sleeve, reaching in (+z).
        let skin = mat(Color::srgb(0.55, 0.42, 0.34), 0.75);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.085, 0.025, 0.09))),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(0.0, -0.02, -0.02),
            ChildOf(e),
        ));
        for (i, x) in [-0.03f32, -0.01, 0.01, 0.03].into_iter().enumerate() {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(
                    0.018,
                    0.018,
                    0.06 - 0.006 * (i as f32 - 1.5).abs(),
                ))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(x, 0.0, 0.045).with_rotation(Quat::from_rotation_x(0.7)),
                ChildOf(e),
            ));
        }
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.02, 0.02, 0.05))),
            MeshMaterial3d(skin),
            Transform::from_xyz(-0.05, 0.0, 0.0).with_rotation(Quat::from_rotation_y(0.6)),
            ChildOf(e),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.045, 0.4))),
            MeshMaterial3d(mat(Color::srgb(0.36, 0.36, 0.34), 1.0)),
            Transform::from_xyz(0.0, 0.0, -0.26)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ChildOf(e),
        ));
    }
}

fn slide_hatch(
    hatches: Query<(&CabHatch, &Children), Changed<CabHatch>>,
    mut panels: Query<&mut Transform, With<HatchPanel>>,
) {
    for (h, children) in &hatches {
        for c in children.iter() {
            if let Ok(mut t) = panels.get_mut(c) {
                t.translation.x = -(HATCH.x + 0.02) * h.open;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// He notices the right thing, and his story comes first.
    #[test]
    fn he_notices_what_comes_home() {
        use extraction_core::items::Category;
        let c = |v: &[(&str, Category)]| cargo(v.iter().map(|(i, k)| (i.to_string(), *k)));
        assert!(c(&[("art_hearth", Category::Artifact)]).artifact);
        assert!(c(&[("sensor_eye", Category::RobotPart)]).eye);
        assert_eq!(c(&[("canned_stew", Category::Food); 6]).food, 6);
        let lines = super::super::lines::CARGO_ARTIFACT;
        let art = Cargo {
            artifact: true,
            ..default()
        };
        // An odd homecoming with nothing of his own to say: the stone.
        assert!(lines.contains(&super::super::lines::coming_home_with(4, false, art)));
        // His story beat still comes first.
        assert_eq!(
            super::super::lines::coming_home_with(3, false, art),
            super::super::lines::coming_home(3, false)
        );
    }

    /// Every gift Progression's fittings give, and Hana's photograph, come
    /// through the hatch: his hand carries each to the ledge.
    #[test]
    fn every_gift_arrives_through_the_window() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let mut gifts: Vec<(String, f32)> = Vec::new();
        for u in extraction_core::progression::Upgrade::ALL {
            for (id, n, condition) in crate::progression::gifts(u) {
                for _ in 0..*n {
                    gifts.push((id.to_string(), *condition));
                }
            }
        }
        for b in crate::world::story::BEATS {
            if let Some(id) = b.gift {
                gifts.push((id.to_string(), 0.6));
            }
        }
        assert!(!gifts.is_empty());
        for (id, condition) in &gifts {
            app.world_mut().write_message(HandOver {
                id: id.clone(),
                condition: *condition,
            });
        }
        let mut seen_hand = false;
        for _ in 0..(gifts.len() * 200 + 400) {
            app.update();
            let shown = app
                .world_mut()
                .query_filtered::<&Visibility, With<LevHand>>()
                .iter(app.world())
                .any(|v| *v != Visibility::Hidden);
            seen_hand |= shown;
            if !app.world().resource::<Handovers>().busy() {
                break;
            }
        }
        assert!(seen_hand, "his hand came through");
        let truck = *app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<Truck>>()
            .single(app.world())
            .unwrap();
        let handed = app.world().resource::<Handovers>().handed.clone();
        assert_eq!(
            handed.iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(),
            gifts.iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(),
            "each in turn"
        );
        for (id, e) in handed {
            let at = app.world().get::<GlobalTransform>(e).unwrap().translation();
            let local = truck.affine().inverse().transform_point3(at);
            // Under the hatch: on its ledge, or on the floor in front of it
            // (never in the stash crate).
            assert!(
                (local - HATCH_AT).length() < 1.1 && local.y < HATCH_AT.y && local.z < -0.9,
                "{id} below the hatch, not at {local:?}"
            );
            assert!(
                app.world().get::<RigidBodyDisabled>(e).is_none(),
                "{id} let go"
            );
        }
        let open = app
            .world_mut()
            .query::<&CabHatch>()
            .iter(app.world())
            .next()
            .unwrap()
            .open;
        assert_eq!(open, 0.0, "and the hatch shut");
    }

    /// Home with a stone on you (on a homecoming with no story of his):
    /// he says so on the drive.
    #[test]
    fn home_with_a_stone_he_says_so() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .single(app.world())
            .unwrap();
        let catalog = app.world().resource::<Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let stone = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_item(
                &mut c,
                &catalog,
                SpawnItem {
                    kind: catalog.kind("art_hearth").unwrap(),
                    transform: Transform::IDENTITY,
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        // In the pack on your back.
        let bag = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .expect("a pack");
        app.world_mut().entity_mut(stone).insert((
            crate::interact::body::InBackpack { bag },
            ChildOf(bag),
            Transform::IDENTITY,
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(bag)
            .unwrap()
            .contents
            .push(stone);
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.extractions = 4;
            raid.phase = Phase::Extracting { left: 0.5 };
        }
        for _ in 0..120 {
            app.update();
        }
        let raid = app.world().resource::<Raid>();
        assert_eq!(raid.phase, Phase::AtBase, "home");
        assert!(
            raid.log
                .iter()
                .any(|(l, _)| super::super::lines::CARGO_ARTIFACT.contains(&l.as_str())),
            "{:?}",
            raid.log
        );
    }
}
