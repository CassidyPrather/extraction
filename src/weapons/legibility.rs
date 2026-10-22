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

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm::{ActionKind, Round};

use super::{Gun, Mag};
use crate::items::{Catalog, Item};

pub struct LegibilityPlugin;

impl Plugin for LegibilityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (weigh_mags, add_chambers, show_chambers).chain());
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
    }
    for (e, gun) in &guns {
        let f = &gun.firearm;
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
            if let Some((mesh, mat)) = look.as_ref() {
                c.insert((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
            }
        }
    }
}

/// The chamber's round shows while the action's open far enough to see it;
/// an empty case darker than a live round.
pub fn show_chambers(
    guns: Query<&Gun>,
    mut shows: Query<(&ChamberShow, &mut Visibility, &mut Transform)>,
) {
    for (c, mut vis, mut t) in &mut shows {
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
