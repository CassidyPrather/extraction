//! Iron sights that line up. The game zeroes a gun along a line parallel
//! to its bore at its eye point's height (`layout.sight`: `weapons::zero`,
//! `feel::front_post`), so the rear sight's floor (the notch, the rib, the
//! groove) and the front sight's top (the post, the bead) must both sit on
//! that line: a notch standing over it hides the post, and a post short of
//! it has the round go somewhere the sights don't show.
//!
//! From the eye point down the bore, through every triangle of the gun's
//! model as it rests: a line [`ABOVE`] over the eye line runs clear to the
//! muzzle (over the notch and the post), and one [`BELOW`] under it first
//! meets the front sight (nothing nearer stands that high).

use std::path::Path;

use bevy::prelude::*;

/// How far over the eye line nothing may stand, metres.
pub const ABOVE: f32 = 0.0003;
/// How far under it the front sight's top must reach.
pub const BELOW: f32 = 0.0003;
/// Guns with no sights to line up: (id, why).
pub const NO_SIGHTS: &[(&str, &str)] = &[(
    "flare_pistol",
    "a signal pistol: a groove along the shield and no front sight, lobbed, not aimed",
)];

/// The front sight is within this of the muzzle, metres (the AK's post is
/// 9 cm behind its brake; a shotgun's bead is at the muzzle).
pub const FRONT: f32 = 0.12;

/// Where a ray from `o` along -Z first meets a triangle (its z), if before
/// `z_end`.
fn first_hit(tris: &[[Vec3; 3]], o: Vec3, z_end: f32) -> Option<f32> {
    let d = Vec3::NEG_Z;
    let mut best: Option<f32> = None;
    for t in tris {
        // Moller-Trumbore.
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
        let z = o.z - dist;
        if dist > 0.0 && z > z_end && best.is_none_or(|b| z > b) {
            best = Some(z);
        }
    }
    best
}

/// What's wrong with a gun's sight line, if anything.
pub fn check(id: &str, model: &Path) -> Option<String> {
    let layout = crate::weapons::layout::layout(id)?;
    let tris: Vec<[Vec3; 3]> = super::glb::triangles(model)
        .ok()?
        .into_iter()
        .filter(|t| !t.node.ends_with("_lod"))
        .map(|t| t.p)
        .collect();
    let eye = layout.sight.translation;
    let muzzle = layout.muzzle.z;
    let end = muzzle - 0.05;
    let fmt = |z: Option<f32>| z.map_or("nothing".to_owned(), |z| format!("z {z:.3}"));
    let over = first_hit(&tris, eye + Vec3::Y * ABOVE, end);
    let under = first_hit(&tris, eye - Vec3::Y * BELOW, end);
    let mut bad = Vec::new();
    if let Some(z) = over {
        bad.push(format!(
            "something stands over the eye line (y {:.4}) at z {z:.3}",
            eye.y
        ));
    }
    match under {
        Some(z) if z <= muzzle + FRONT => {}
        _ => bad.push(format!(
            "the front sight's top is short of the eye line (y {:.4}): {:.1} mm under it meets {}",
            eye.y,
            BELOW * 1000.0,
            fmt(under)
        )),
    }
    (!bad.is_empty()).then(|| format!("{id}: {}", bad.join("; ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ray_meets_the_nearest_face_ahead() {
        let wall = |z: f32| {
            [[
                Vec3::new(-1., -1., z),
                Vec3::new(1., -1., z),
                Vec3::new(0., 1., z),
            ]]
        };
        let tris: Vec<[Vec3; 3]> = [wall(-0.5), wall(-0.2), wall(0.3)].concat();
        assert_eq!(first_hit(&tris, Vec3::ZERO, -1.0), Some(-0.2));
        assert_eq!(first_hit(&tris, Vec3::ZERO, -0.3), Some(-0.2));
        assert_eq!(first_hit(&tris, Vec3::new(0.0, 5.0, 0.0), -1.0), None);
    }

    /// Every gun's eye line runs over its rear sight onto its front sight's
    /// top (the 28 October playtest: the eye looked at the back of the
    /// PM's and AK's rear sights, under their notches).
    #[test]
    fn every_guns_eye_line_runs_over_its_rear_sight_onto_its_front() {
        let catalog = crate::items::Catalog::load();
        let mut bad = Vec::new();
        let mut seen = 0;
        for (_, def) in catalog.iter() {
            if def.firearm.is_none() {
                continue;
            }
            let model = crate::asset_root_path().join(format!("models/guns/{}.glb", def.id));
            if !model.exists()
                || crate::weapons::layout::layout(&def.id).is_none()
                || NO_SIGHTS.iter().any(|(id, _)| *id == def.id)
            {
                continue;
            }
            seen += 1;
            match check(&def.id, &model) {
                Some(f) => {
                    println!("FAIL  {f}");
                    bad.push(f);
                }
                None => println!("ok    {}", def.id),
            }
        }
        assert!(seen >= 10, "only {seen} guns");
        assert!(bad.is_empty(), "sights off their line:\n{}", bad.join("\n"));
    }
}
