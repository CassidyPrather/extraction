//! The sight picture, gun by gun: what the eye sees down iron sights, and
//! that the round goes where it says.
//!
//! - **The line** is the sights as built (`layout::irons`): over a notch's
//!   shoulders to the post's top (the picture is the post's top level with
//!   the shoulders, centred in the notch), or, on a bead gun, the eye just
//!   over the breech to the bead's top, so the whole bead sits on the rib.
//!   The round is zeroed to that line (`zero::iron_up`), the cheek lines up
//!   on it (`feel::sight_fix`), and bots aim down it.
//! - **The cheek weld** ([`weld_eye`]): a stocked gun shouldered puts the
//!   eye [`WELD`] ahead of the butt, on the line; a pistol's eye is at arm's
//!   length behind it ([`ARMS_EYE`]). The sights line up there without
//!   fiddling (`feel::at_sights` reaches back to the butt).
//! - **Reading the post**: at the weld it's at least [`POST_PIXELS`] of a
//!   headset's pixels wide ([`PIXEL`]); on a rifle zeroed at 100 m, no
//!   wider than a man's torso ([`TORSO`]) there, so he still shows beside it.
//!   None of these guns had a dot on its post, so none has one here; by day
//!   and by dusk the dark post reads against the target, by night against
//!   what the torch lights (`docs/shots/weapon-feel/sights_*`).
//!
//! The tests hold each gun's line to its model.

use bevy::prelude::*;

use super::Gun;

/// Where a cheek weld puts a rifleman's eye: this far ahead of the end of
/// the butt, metres (the face on the comb, a hand behind the receiver).
pub const WELD: f32 = 0.18;
/// A pistol's eye (or a gun's with its stock off), behind its eye point at
/// arm's length, metres.
pub const ARMS_EYE: f32 = 0.6;
/// A headset's pixel, radians (a 20 pixels-a-degree panel).
pub const PIXEL: f32 = std::f32::consts::PI / 180.0 / 20.0;
/// The fewest pixels wide a post reads at.
pub const POST_PIXELS: f32 = 2.0;
/// A man's torso, metres: at 100 m a rifle's post may cover no more.
pub const TORSO: f32 = 0.5;

/// A gun's eye at the cheek weld, in gun space: on its sight line, [`WELD`]
/// ahead of the butt, or [`ARMS_EYE`] behind its eye point with no stock.
pub fn weld_eye(gun: &Gun) -> Vec3 {
    let s = gun.layout.sight;
    let d = s.rotation * Vec3::NEG_Z;
    let z = match super::feel::butt(gun) {
        Some(b) => b.z - WELD,
        None => s.translation.z + ARMS_EYE,
    };
    s.translation + d * ((z - s.translation.z) / d.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::sightline::{NO_SIGHTS, first_hit, sight_frame};

    /// Read down a bead: the whole bead shows over what's nearer.
    const BEADS: &[&str] = &["toz_double", "mp133"];
    /// Not a notch read flush: (id, why).
    const OPEN_V: &[(&str, &str)] = &[(
        "crossbow",
        "a wide V on the latch plate, read with the post down in the V's bottom",
    )];
    /// How near the line the notch's shoulders and the post's top must sit,
    /// metres.
    const FLUSH: f32 = 0.0003;
    /// How much of a bead must show over the breech, metres.
    const BEAD_SHOWN: f32 = 0.003;

    /// Every gun with iron sights, as the game would hold it.
    fn guns() -> Vec<(String, Gun, std::path::PathBuf)> {
        let catalog = crate::items::Catalog::load();
        let mut out = Vec::new();
        for (_, def) in catalog.iter() {
            let (Some(f), Some(layout)) = (
                def.firearm.as_ref(),
                crate::weapons::layout::layout(&def.id),
            ) else {
                continue;
            };
            let model = crate::asset_root_path().join(format!("models/guns/{}.glb", def.id));
            if !model.exists() || NO_SIGHTS.iter().any(|(id, _)| *id == def.id) {
                continue;
            }
            let gun = Gun {
                firearm: extraction_core::firearm::Firearm::new(f.clone(), 1.0),
                layout,
                magwell: None,
                trigger: 0.0,
                pulled: false,
                catch: false,
                dice: extraction_core::noise::Rng::new(1),
                mass: None,
            };
            out.push((def.id.clone(), gun, model));
        }
        assert!(out.len() >= 10, "only {} guns", out.len());
        out
    }

    /// Where a ray from `o` along `d` first meets a triangle, how far.
    fn cast(tris: &[[Vec3; 3]], o: Vec3, d: Vec3) -> Option<f32> {
        let mut best: Option<f32> = None;
        for t in tris {
            let e1 = t[1] - t[0];
            let e2 = t[2] - t[0];
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / det;
            let s = o - t[0];
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = d.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let dist = e2.dot(q) * inv;
            if dist > 0.0 && best.is_none_or(|b| dist < b) {
                best = Some(dist);
            }
        }
        best
    }

    /// The front sight's z in the sight frame, and the model there.
    fn front_z(tris: &[[Vec3; 3]], gun: &Gun) -> f32 {
        let end = gun.layout.muzzle.z - gun.layout.sight.translation.z - 0.05;
        first_hit(tris, Vec3::NEG_Y * 0.0003, end).expect("a front sight")
    }

    /// Scanning down from over the line at `x`, rays down the line from
    /// `from` (sight frame z): the first height (metres, over the line)
    /// whose ray meets the model where `want` says.
    fn top(tris: &[[Vec3; 3]], x: f32, from: f32, end: f32, want: &dyn Fn(f32) -> bool) -> f32 {
        (0..140)
            .map(|k| 0.0008 - k as f32 * 0.00005)
            .find(|&y| first_hit(tris, Vec3::new(x, y, from), end).is_some_and(|z| want(z)))
            .unwrap_or(f32::NAN)
    }

    /// The notch's half width, metres: the clear run about the line, 1 mm
    /// under it, to the rear sight.
    fn notch_half(tris: &[[Vec3; 3]], front: f32, end: f32) -> f32 {
        let rear = |x: f32| {
            first_hit(tris, Vec3::new(x, -0.001, 0.05), end).is_some_and(|z| z > front + 0.03)
        };
        let mut a = 0.0;
        while a < 0.015 && !rear(a) && !rear(-a) {
            a += 0.0001;
        }
        a
    }

    /// Every gun's sight picture is the one its sights are read by: a
    /// notch's shoulders level with the post's top on the line (the line
    /// once ran along the notch's floor, so a post held level with the
    /// shoulders, as sights are read, threw the round 4 mrad high: 44 cm at
    /// 100 m from an AK), and a bead gun's whole bead over its breech from
    /// the cheek weld (half of it hid behind the IZh-43's).
    #[test]
    fn every_guns_sight_picture_is_read_as_built() {
        let mut bad = Vec::new();
        for (id, gun, model) in guns() {
            if OPEN_V.iter().any(|(v, _)| *v == id) {
                continue;
            }
            let tris = sight_frame(&gun.layout, &model).unwrap();
            let front = front_z(&tris, &gun);
            let end = front - 0.05;
            let post = top(&tris, 0.0, front + 0.02, end, &|z| {
                (z - front).abs() < 0.015
            });
            if BEADS.contains(&id.as_str()) {
                // From the weld, down to BEAD_SHOWN under the bead's top,
                // the eye meets the bead first.
                let eye = gun
                    .layout
                    .sight
                    .compute_affine()
                    .inverse()
                    .transform_point3(weld_eye(&gun));
                let hidden = (0..=30)
                    .map(|k| post - k as f32 * BEAD_SHOWN / 30.0)
                    .find(|&y| {
                        let at = Vec3::new(0.0, y, front);
                        cast(&tris, eye, (at - eye).normalize())
                            .is_some_and(|t| (eye + (at - eye).normalize() * t).z > front + 0.015)
                    });
                println!(
                    "{id:12} bead top {:+.2} mm; hidden from {}",
                    post * 1000.0,
                    hidden.map_or("nowhere".into(), |y| format!(
                        "{:.2} mm under it",
                        (post - y) * 1000.0
                    ))
                );
                if let Some(y) = hidden {
                    bad.push(format!(
                        "{id}: the breech hides the bead {:.1} mm under its top",
                        (post - y) * 1000.0
                    ));
                }
                continue;
            }
            let half = notch_half(&tris, front, end);
            let shoulder =
                |s: f32| top(&tris, s * (half + 0.001), 0.05, end, &|z| z > front + 0.03);
            let (l, r) = (shoulder(-1.0), shoulder(1.0));
            println!(
                "{id:12} notch {:.1} mm, shoulders {:+.2} {:+.2} mm, post {:+.2} mm over the line",
                half * 2000.0,
                l * 1000.0,
                r * 1000.0,
                post * 1000.0
            );
            for (what, y) in [("left shoulder", l), ("right shoulder", r)] {
                if !(y.abs() <= FLUSH) {
                    bad.push(format!(
                        "{id}: the {what} {:+.2} mm off the line",
                        y * 1000.0
                    ));
                }
            }
            if !(post.abs() <= FLUSH) {
                bad.push(format!(
                    "{id}: the post's top {:+.2} mm off the line",
                    post * 1000.0
                ));
            }
        }
        assert!(bad.is_empty(), "sight pictures:\n{}", bad.join("\n"));
    }

    /// Every gun's post reads at the cheek weld: two of a headset's pixels
    /// wide at least, and on a rifle zeroed at 100 m no wider than a man
    /// there, so he shows beside it.
    #[test]
    fn every_guns_post_reads_at_the_weld() {
        let mut bad = Vec::new();
        for (id, gun, model) in guns() {
            let tris = sight_frame(&gun.layout, &model).unwrap();
            let front = front_z(&tris, &gun);
            let end = front - 0.05;
            let post = top(&tris, 0.0, front + 0.02, end, &|z| {
                (z - front).abs() < 0.015
            });
            // Its width half a millimetre under its top (what's read
            // against the target), from just behind it.
            let on = |x: f32| {
                first_hit(&tris, Vec3::new(x, post - 0.0005, front + 0.02), end)
                    .is_some_and(|z| (z - front).abs() < 0.015)
            };
            let (mut a, mut b) = (0.0f32, 0.0f32);
            while a < 0.01 && on(-a - 0.0001) {
                a += 0.0001;
            }
            while b < 0.01 && on(b + 0.0001) {
                b += 0.0001;
            }
            let width = a + b + 0.0001;
            let eye = gun
                .layout
                .sight
                .compute_affine()
                .inverse()
                .transform_point3(weld_eye(&gun));
            let angle = width / (eye.z - front);
            let zero = crate::weapons::zero::iron_zero_m(&gun.firearm.def);
            println!(
                "{id:12} post {:.1} mm at {:.0} mm: {:.1}' ({:.1} px); covers {:.0} cm at 100 m",
                width * 1000.0,
                (eye.z - front) * 1000.0,
                angle.to_degrees() * 60.0,
                angle / PIXEL,
                angle * 100.0 * 100.0
            );
            if angle < POST_PIXELS * PIXEL {
                bad.push(format!("{id}: the post is {:.1} px wide", angle / PIXEL));
            }
            if zero >= 100.0 && angle * 100.0 > TORSO {
                bad.push(format!(
                    "{id}: the post covers {:.0} cm at 100 m",
                    angle * 10000.0
                ));
            }
        }
        assert!(bad.is_empty(), "posts:\n{}", bad.join("\n"));
    }

    /// Every stocked gun shouldered, the eye at a cheek weld (a little high
    /// and to the side of the line, as a real cheek is), comes to its
    /// sights without fiddling: the eye, the rear sight and the post in
    /// line, the post gone soft (the shotguns' welds, 30 cm behind their
    /// eye points, were once too far back to count).
    #[test]
    fn every_stocked_gun_lines_up_at_the_cheek_weld() {
        use crate::weapons::feel::Handling;
        let mut bad = Vec::new();
        for (id, gun, _) in guns() {
            if super::super::feel::butt(&gun).is_none() || gun.layout.support.is_none() {
                continue;
            }
            let mut r = crate::weapons::feel::vr_tests::hold(&id, true, true);
            r.vr.step(10);
            let weld = weld_eye(&gun) + Vec3::new(0.004, 0.008, 0.0);
            let root =
                r.vr.app
                    .world()
                    .get::<Transform>(r.vr.player)
                    .unwrap()
                    .translation;
            for _ in 0..3 {
                // Head down onto the stock where the hands hold it: the weld
                // on the gun as it would lie without the turn the sights
                // have already taken (about the front post).
                let gt = *r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
                let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
                let front = super::super::feel::front_post(g);
                let fix = r.vr.app.world().get::<Handling>(r.gun).unwrap().sight_fix;
                let bare = front + fix.inverse() * (weld - front);
                r.vr.head(gt.transform_point(bare) - root);
                r.vr.step(30);
            }
            let gt = *r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            let g = r.vr.app.world().get::<Gun>(r.gun).unwrap();
            let eye =
                r.vr.app
                    .world()
                    .get::<crate::rig::RigInput>(r.vr.player)
                    .unwrap()
                    .head
                    .translation
                    + root;
            let rear = gt.transform_point(g.layout.sight.translation);
            let front = gt.transform_point(super::super::feel::front_post(g));
            let d = (rear - front).normalize();
            let v = eye - front;
            let miss = (v - d * v.dot(d)).length();
            let at_eye = r.vr.app.world().get::<Handling>(r.gun).unwrap().at_eye;
            println!(
                "{id:12} eye {:.1} mm off the line, at the eye {at_eye:.2}",
                miss * 1000.0
            );
            if miss > 0.002 || at_eye < 0.95 {
                bad.push(format!(
                    "{id}: the eye {:.1} mm off its sights' line, at the eye {at_eye:.2}",
                    miss * 1000.0
                ));
            }
        }
        assert!(bad.is_empty(), "at the cheek:\n{}", bad.join("\n"));
    }
}
