//! A gun's state, read in the hand without a screen: what its real
//! counterpart shows, it shows ([`TELLS`], held by a test against the
//! models).
//!
//! - Cocked: an exposed hammer back (the PM, the Nagant, the flare pistol);
//!   a turn-bolt's cocking piece standing back off the bolt (the Mosin,
//!   the TOZ-8); the double's indicator pins proud over each lock; an
//!   open bolt held back (the PPSh); a crossbow's string drawn.
//! - Loaded: with the action open, the round (or the empty case) in the
//!   chamber is there to see ([`show_chambers`]); a revolver's cylinder
//!   shows its rounds; a clip shows its rounds standing in it.
//! - Empty: the slide or carrier held open on the last round where the
//!   real gun does it (the PM, the SKS, the SVD); an empty box magazine's
//!   follower between its lips; the PM magazine's round-count holes
//!   showing brass up to how full it is.
//! - How full: a magazine weighs what its rounds weigh ([`weigh_mags`]),
//!   so a full one is heavier in the hand than an empty one.
//! - Condition: wear shows as rust and bare metal (`weather::weather_looks`),
//!   the crown's nicks and the bore's soot (`wear`); a worn, fouled action
//!   drags in the hand (`Firearm::action_drag`).
//! - What it's been made into: a cut barrel, a stock off, a bolt out
//!   (`gunsmith`), a suppressor or a scope on it (`gadgets`).
//! - What's stopped it ([`look`], `stoppages`): a stovepipe's case standing
//!   in the port, a double feed's second round jammed behind the first, a
//!   slipped magazine hanging low, a hang-fire's thread of smoke from the
//!   action ([`show_jams`]).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm::{ActionKind, Firearm, Malfunction, Round};

use super::{Gun, Mag};
use crate::items::{Catalog, Item};

pub struct LegibilityPlugin;

impl Plugin for LegibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (weigh_mags, add_chambers, show_chambers, show_jams).chain(),
        );
    }
}

/// What a real gun shows of its state, by catalog id.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tells {
    /// An exposed hammer, back when cocked.
    pub hammer: bool,
    /// A turn-bolt's cocking piece, back when cocked.
    pub cocking_piece: bool,
    /// Indicator pins over its locks.
    pub indicators: bool,
    /// The slide or carrier held open on the last round.
    pub hold_open: bool,
    /// Fires from an open bolt: back is cocked.
    pub open_bolt: bool,
    /// A revolver's cylinder: its rounds seen from behind.
    pub cylinder: bool,
    /// A crossbow's string, drawn or at rest.
    pub string: bool,
}

/// What each gun's real counterpart shows.
pub const TELLS: &[(&str, Tells)] = &[
    (
        "pm_pistol",
        Tells {
            hammer: true,
            hold_open: true,
            ..NONE
        },
    ),
    (
        "nagant",
        Tells {
            hammer: true,
            cylinder: true,
            ..NONE
        },
    ),
    (
        "flare_pistol",
        Tells {
            hammer: true,
            ..NONE
        },
    ),
    (
        "mosin",
        Tells {
            cocking_piece: true,
            ..NONE
        },
    ),
    (
        "toz8",
        Tells {
            cocking_piece: true,
            ..NONE
        },
    ),
    // A hammerless boxlock: pins over its locks say which is cocked.
    (
        "toz_double",
        Tells {
            indicators: true,
            ..NONE
        },
    ),
    (
        "sks",
        Tells {
            hold_open: true,
            ..NONE
        },
    ),
    (
        "svd",
        Tells {
            hold_open: true,
            ..NONE
        },
    ),
    (
        "ppsh",
        Tells {
            open_bolt: true,
            ..NONE
        },
    ),
    // No hold-open, no indicator: you look in the port, or feel the weight.
    ("ak74", NONE),
    ("rpk74", NONE),
    ("mp133", NONE),
    (
        "crossbow",
        Tells {
            string: true,
            ..NONE
        },
    ),
];

const NONE: Tells = Tells {
    hammer: false,
    cocking_piece: false,
    indicators: false,
    hold_open: false,
    open_bolt: false,
    cylinder: false,
    string: false,
};

/// A magazine's mass with `rounds` in it: its own (the catalog's, empty)
/// and each round's.
pub fn mag_mass(empty: f32, rounds: u32, round: f32) -> f32 {
    empty + rounds as f32 * round
}

/// A loaded magazine weighs its rounds: heft one to know roughly how full.
fn weigh_mags(
    mut mags: Query<(&Item, &mut Mass), (With<Mag>, Changed<Item>)>,
    catalog: Res<Catalog>,
) {
    for (item, mut mass) in &mut mags {
        let def = catalog.get(item.kind());
        let round = def
            .magazine
            .as_ref()
            .and_then(|m| {
                catalog.iter().find(|(_, d)| {
                    d.rounds == 1 && d.cartridge.as_deref() == Some(m.cartridge.as_str())
                })
            })
            .map_or(0.012, |(_, d)| d.mass);
        let want = mag_mass(def.mass, item.rounds, round);
        if (mass.0 - want).abs() > 1e-4 {
            mass.0 = want;
        }
    }
}

/// A gun's chamber, drawn: the round (or the empty case) in it, seen while
/// the action's open.
#[derive(Component, Debug)]
pub struct ChamberShow {
    pub gun: Entity,
    pub chamber: usize,
}

/// Where chamber `i` is, gun space: on the bore just ahead of the ejection
/// port (a double's two side by side at the breech).
pub fn chamber_at(gun: &Gun, i: usize) -> Vec3 {
    let l = &gun.layout;
    if gun.firearm.def.barrels > 1 {
        let x = if i == 0 { -0.0128 } else { 0.0128 };
        return Vec3::new(l.muzzle.x + x, l.muzzle.y, l.load_port.z);
    }
    Vec3::new(l.muzzle.x, l.muzzle.y, l.port.z - 0.01)
}

/// A gun whose chambers can be looked into gets them drawn (not a
/// revolver: its cylinder shows its own).
fn add_chambers(
    mut commands: Commands,
    guns: Query<(Entity, &Gun), Added<Gun>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut smoke: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut struck: Local<Option<Handle<StandardMaterial>>>,
) {
    // Drawn where there's drawing; kept track of headless all the same.
    if let (Some(mut meshes), Some(mut materials), None) = (meshes, materials, look.as_ref()) {
        *look = Some((
            meshes.add(Cylinder::new(0.0055, 0.03)),
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.78, 0.6, 0.3),
                metallic: 0.9,
                perceptual_roughness: 0.35,
                ..default()
            }),
        ));
        // A struck round: the dented primer's soot, the brass dulled.
        *struck = Some(materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.31, 0.17),
            metallic: 0.6,
            perceptual_roughness: 0.7,
            ..default()
        }));
        *smoke = Some((
            meshes.add(Sphere::new(0.006)),
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.75, 0.75, 0.72, 0.45),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
        ));
    }
    for (e, gun) in &guns {
        let f = &gun.firearm;
        let parts: &[Jam] = match f.def.action {
            ActionKind::Crossbow => &[],
            ActionKind::Revolver => &[Jam::Smoke, Jam::Bullet],
            _ => &[Jam::Stovepipe, Jam::Transit, Jam::Smoke, Jam::Bullet],
        };
        for &part in parts {
            let mut c = commands.spawn((
                Name::new("Jam"),
                JamShow { gun: e, part },
                Transform::IDENTITY,
                Visibility::Hidden,
                ChildOf(e),
            ));
            if let Some((mesh, mat)) = look.as_ref() {
                if part == Jam::Smoke {
                    if let Some((m, mt)) = smoke.as_ref() {
                        c.insert((Mesh3d(m.clone()), MeshMaterial3d(mt.clone())));
                    }
                } else {
                    c.insert((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
                }
            }
        }
        if f.def.action == ActionKind::Revolver || f.def.cartridge == "bolt" {
            continue;
        }
        for i in 0..f.chambers.len() {
            let mut c = commands.spawn((
                Name::new("Chamber"),
                ChamberShow { gun: e, chamber: i },
                // A case along the bore.
                Transform::from_translation(chamber_at(gun, i))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                Visibility::Hidden,
                ChildOf(e),
            ));
            if let (Some((mesh, mat)), Some(dull)) = (look.as_ref(), struck.as_ref()) {
                c.insert((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    ChamberMats {
                        live: mat.clone(),
                        struck: dull.clone(),
                    },
                ));
            }
        }
    }
}

/// A chamber's round's two looks: as it came, and struck (a dud).
#[derive(Component)]
pub struct ChamberMats {
    live: Handle<StandardMaterial>,
    struck: Handle<StandardMaterial>,
}

/// The chamber's round shows while the action's open far enough to see it;
/// an empty case darker than a live round.
pub fn show_chambers(
    guns: Query<&Gun>,
    mut shows: Query<(
        &ChamberShow,
        &mut Visibility,
        &mut Transform,
        Option<(&ChamberMats, &mut MeshMaterial3d<StandardMaterial>)>,
    )>,
) {
    for (c, mut vis, mut t, mats) in &mut shows {
        let Ok(gun) = guns.get(c.gun) else {
            continue;
        };
        let f = &gun.firearm;
        let open = f.bolt_travel > 0.3;
        let held = f.chambers.get(c.chamber).copied().flatten();
        let want = match (open, held) {
            (true, Some(_)) => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
        if *vis != want {
            *vis = want;
        }
        // A spent case sits a little proud of the chamber (the extractor
        // has it), a live round seated.
        let back = if held == Some(Round::Spent) {
            0.006
        } else {
            0.0
        };
        let at = chamber_at(gun, c.chamber) + Vec3::Z * back;
        if t.translation != at {
            t.translation = at;
        }
        if let Some((m, mut mat)) = mats {
            let want = if f.struck == Some(c.chamber) {
                &m.struck
            } else {
                &m.live
            };
            if mat.0 != *want {
                mat.0 = want.clone();
            }
        }
    }
}

/// A stoppage's part, drawn: a stovepipe's case standing in the port, a
/// double feed's second round behind the first, a hang-fire's smoke, a
/// squib's bullet stuck in the bore (seen down the muzzle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jam {
    Stovepipe,
    Transit,
    Smoke,
    Bullet,
}

#[derive(Component, Debug)]
pub struct JamShow {
    pub gun: Entity,
    pub part: Jam,
}

/// Whether `part` shows on a gun in state `f`.
pub fn jam_shows(f: &Firearm, part: Jam) -> bool {
    match part {
        Jam::Stovepipe => f.malfunction == Some(Malfunction::FailureToEject),
        Jam::Transit => f.malfunction == Some(Malfunction::DoubleFeed) && f.in_transit().is_some(),
        Jam::Smoke => f.hang.is_some(),
        Jam::Bullet => f.squib.is_some(),
    }
}

pub fn show_jams(
    guns: Query<&Gun>,
    time: Res<Time>,
    mut shows: Query<(&JamShow, &mut Visibility, &mut Transform)>,
) {
    for (j, mut vis, mut t) in &mut shows {
        let Ok(gun) = guns.get(j.gun) else {
            continue;
        };
        let want = if jam_shows(&gun.firearm, j.part) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if want == Visibility::Hidden {
            continue;
        }
        let l = &gun.layout;
        let at = match j.part {
            // Caught upright in the port by the closing bolt.
            Jam::Stovepipe => Transform::from_translation(l.port + Vec3::Y * 0.01),
            // Nosed up behind the case still in the chamber.
            Jam::Transit => {
                Transform::from_translation(chamber_at(gun, 0) + Vec3::new(0.0, -0.004, 0.028))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 - 0.35))
            }
            // A hand's breadth down the bore from the muzzle.
            Jam::Bullet => Transform::from_translation(l.muzzle + Vec3::Z * 0.1)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(1.0, 0.5, 1.0)),
            // A thread rising from the action and starting again.
            Jam::Smoke => {
                let rise = (time.elapsed_secs() * 0.7).fract();
                Transform::from_translation(l.port + Vec3::Y * (0.01 + 0.04 * rise))
                    .with_scale(Vec3::splat(1.0 + 1.5 * rise))
            }
        };
        if *t != at {
            *t = at;
        }
    }
}

/// What a look at a gun shows of whatever's stopped it: the bolt, the
/// port (and, its action shut, what a press check finds in the chamber),
/// the magazine, the hammer or cocking piece where there's one to see, a
/// revolver's gate, a crossbow's string, smoke. All of it drawn: the bolt
/// and hammer by `visuals::animate_parts`, the chamber by
/// [`show_chambers`], the rest by [`show_jams`] and `reload::hang_mags`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Look {
    pub bolt: &'static str,
    pub port: Vec<&'static str>,
    pub mag: &'static str,
    pub cocked: Option<bool>,
    pub gate_open: bool,
    pub string_drawn: bool,
    pub smoke: bool,
    /// The selector on safe (every gun with one shows where it is).
    pub safe: bool,
}

pub fn look(id: &str, f: &Firearm) -> Look {
    let tells = TELLS.iter().find(|(g, _)| *g == id).map(|(_, t)| *t);
    let t = f.bolt_travel;
    let bolt = if f.def.action == ActionKind::BreakAction {
        if t >= 0.5 { "broken open" } else { "shut" }
    } else if f.locked_back || t >= 0.95 {
        "back"
    } else if t > 0.3 {
        "part open"
    } else if t > 0.05 {
        "on a case"
    } else {
        "home"
    };
    let mut port = Vec::new();
    if jam_shows(f, Jam::Stovepipe) {
        port.push("a case standing in the port");
    }
    if jam_shows(f, Jam::Transit) {
        port.push("a round jammed behind");
    }
    if jam_shows(f, Jam::Bullet) {
        port.push("a bullet stuck in the bore");
    }
    let c = if f.def.action == ActionKind::Revolver {
        f.cylinder
    } else {
        0
    };
    port.push(match f.chambers.get(c).copied().flatten() {
        Some(Round::Live) if f.struck == Some(c) => "a dented primer in the chamber",
        Some(Round::Live) => "a round in the chamber",
        Some(Round::Spent) => "a case in the chamber",
        None => "an empty chamber",
    });
    let mag = match &f.magazine {
        None => "none",
        Some(_) if f.mag_unseated => "hanging low",
        Some(m) if m.rounds == 0 => "home, its follower up",
        Some(_) => "home",
    };
    let cocked = tells
        .filter(|t| t.hammer || t.cocking_piece || t.indicators || t.open_bolt)
        .map(|_| f.hammers_cocked.iter().any(|h| *h));
    Look {
        bolt,
        port,
        mag,
        cocked,
        gate_open: f.gate_open,
        string_drawn: f.def.action == ActionKind::Crossbow && f.locked_back,
        smoke: jam_shows(f, Jam::Smoke),
        safe: f.fire_mode() == extraction_core::firearm::FireMode::Safe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model's top-level node names.
    fn nodes(dir: &str, id: &str) -> Vec<String> {
        let path = crate::asset_root_path().join(format!("models/{dir}/{id}.glb"));
        let b = std::fs::read(&path).unwrap_or_else(|_| panic!("{path:?}"));
        let n = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
        let j: serde_json::Value = serde_json::from_slice(&b[20..20 + n]).unwrap();
        j["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|n| n["name"].as_str().map(str::to_owned))
            .collect()
    }

    /// Every gun in the catalog is in the table, and shows what its real
    /// counterpart shows: its model has the moving part for each tell, and
    /// its mechanism holds open or fires from an open bolt as the real one
    /// does. Every detachable box magazine shows its follower when empty,
    /// and the PM's its brass through its round-count holes.
    #[test]
    fn every_gun_shows_what_the_real_one_does() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let id = def.id.as_str();
            let Some((_, t)) = TELLS.iter().find(|(g, _)| *g == id) else {
                bad.push(format!("{id}: not in the table"));
                continue;
            };
            let names = nodes("guns", id);
            let has = |n: &str| names.iter().any(|m| m.starts_with(n));
            let mut need = |ok: bool, what: &str| {
                if !ok {
                    bad.push(format!("{id}: {what}"));
                }
            };
            need(!t.hammer || has("hammer"), "no hammer to see cocked");
            need(
                !t.cocking_piece || has("cocking_piece"),
                "no cocking piece to see cocked",
            );
            if t.indicators {
                for i in 0..f.barrels {
                    need(has(&format!("indicator_{i}")), "no cocking indicator");
                }
            }
            need(t.cylinder == has("cylinder_round_"), "cylinder rounds");
            need(!t.string || has("string_drawn"), "no drawn string");
            need(
                f.last_round_hold_open == t.hold_open,
                "holds open where the real one doesn't, or not where it does",
            );
            need(f.open_bolt == t.open_bolt, "open bolt");
        }
        for (_, def) in catalog.iter() {
            let Some(m) = def.magazine.as_ref() else {
                continue;
            };
            if m.stripper || def.id == "ppsh_drum" {
                // Clips show their rounds standing in them; the drum is
                // shut (you heft it).
                continue;
            }
            let names = nodes("mags", &def.id);
            if !names.iter().any(|n| n == "static_follower") {
                bad.push(format!("{}: no follower to see when empty", def.id));
            }
            if def.id == "pm_mag" {
                for n in [2, 4, 6, 8] {
                    if !names.iter().any(|m| *m == format!("round_{n}")) {
                        bad.push(format!("pm_mag: no brass behind hole {n}"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A full magazine is heavier than an empty one by its rounds.
    #[test]
    fn a_full_magazine_weighs_its_rounds() {
        let full = mag_mass(0.23, 30, 0.011);
        assert!((full - 0.56).abs() < 1e-4, "{full}");
        assert_eq!(mag_mass(0.23, 0, 0.011), 0.23);
    }

    /// A press check from the hand: a loaded AK's charging handle drawn
    /// back a little way shows the round in the chamber; let go, the bolt
    /// closes over it and it's hidden. Unloaded, the same draw shows nothing.
    #[test]
    fn a_press_check_shows_the_round_in_the_chamber() {
        use crate::interact::GripRole;
        use crate::rig::Side;
        use crate::weapons::feel::vr_tests::{glide_side, gun_mut, hold, load};
        use extraction_core::firearm::FireMode;
        let check = |loaded: bool| -> (bool, bool) {
            let mut r = hold("ak74", false, false);
            if loaded {
                load(&mut r, "ak545", 30, FireMode::Semi);
            }
            r.vr.step(5);
            let shown = |r: &mut crate::weapons::feel::vr_tests::Range| {
                let mut q = r.vr.app.world_mut().query::<(&ChamberShow, &Visibility)>();
                q.iter(r.vr.app.world())
                    .any(|(c, v)| c.gun == r.gun && *v != Visibility::Hidden)
            };
            let handle = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
            let back =
                r.vr.app
                    .world()
                    .get::<GlobalTransform>(r.gun)
                    .unwrap()
                    .back()
                    .as_vec3();
            r.vr.hand(Side::Left, handle, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(30);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            assert!(
                r.vr.hand_state(Side::Left).handle.is_some(),
                "on the handle"
            );
            glide_side(&mut r, Side::Left, handle, handle + back * 0.06, 15);
            r.vr.step(3);
            let open = shown(&mut r);
            assert!(gun_mut(&mut r).firearm.bolt_travel > 0.3, "drawn back");
            r.vr.trigger(Side::Left, 0.0);
            r.vr.step(20);
            (open, shown(&mut r))
        };
        assert_eq!(check(true), (true, false), "loaded: seen open, hidden shut");
        assert_eq!(check(false), (false, false), "empty: nothing to see");
    }

    /// Hefted, a full AK magazine is heavier than an empty one by its
    /// rounds.
    #[test]
    fn a_full_magazine_is_heavier_in_the_hand() {
        use crate::rig::Side;
        use crate::weapons::feel::vr_tests::{hold, take_in};
        let mut r = hold("ak74", false, false);
        let near = r.feet + Vec3::new(-0.3, 1.1, -0.3);
        let mag = take_in(&mut r, Side::Left, "ak_mag", near);
        r.vr.step(3);
        let mass = |r: &mut crate::weapons::feel::vr_tests::Range| {
            r.vr.app.world().get::<Mass>(mag).unwrap().0
        };
        r.vr.app.world_mut().get_mut::<Item>(mag).unwrap().rounds = 30;
        r.vr.step(3);
        let full = mass(&mut r);
        r.vr.app.world_mut().get_mut::<Item>(mag).unwrap().rounds = 0;
        r.vr.step(3);
        let empty = mass(&mut r);
        println!("AK magazine: full {full:.2} kg, empty {empty:.2} kg");
        assert!(full > empty + 0.3, "{full} vs {empty}");
    }
}
