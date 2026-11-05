//! Probe (temporary): each gun's sight geometry from its model.

#[cfg(test)]
mod tests {
    use crate::capture::sightline::{NO_SIGHTS, first_hit};
    use bevy::prelude::*;

    #[test]
    fn probe_sight_geometry() {
        let catalog = crate::items::Catalog::load();
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let model = crate::asset_root_path().join(format!("models/guns/{}.glb", def.id));
            let Some(layout) = crate::weapons::layout::layout(&def.id) else {
                continue;
            };
            if !model.exists() || NO_SIGHTS.iter().any(|(id, _)| *id == def.id) {
                continue;
            }
            let all = crate::capture::glb::triangles(&model).unwrap();
            let tris: Vec<[Vec3; 3]> = all
                .iter()
                .filter(|t| !t.node.ends_with("_lod"))
                .map(|t| t.p)
                .collect();
            let eye = layout.sight.translation;
            let end = layout.muzzle.z - 0.05;
            let front = first_hit(&tris, eye - Vec3::Y * 0.0003, end).unwrap_or(f32::NAN);
            // Post and notch widths 1 mm under the line.
            let mut post = (f32::MAX, f32::MIN);
            let mut notch = (f32::MIN, f32::MAX);
            for i in -100..=100 {
                let x = i as f32 * 0.0002;
                let o = Vec3::new(eye.x + x, eye.y - 0.001, eye.z);
                if let Some(z) = first_hit(&tris, o, end) {
                    if (z - front).abs() < 0.015 {
                        post = (post.0.min(x), post.1.max(x));
                    } else if z > front + 0.015 {
                        if x < 0.0 {
                            notch.0 = notch.0.max(x)
                        } else {
                            notch.1 = notch.1.min(x)
                        }
                    }
                }
            }
            let (lo, hi) = layout.bounds();
            let butt_z = hi.z;
            let comb = tris
                .iter()
                .flatten()
                .filter(|p| p.z > butt_z - 0.2 && p.z < butt_z - 0.04 && p.x.abs() < 0.03)
                .map(|p| p.y)
                .fold(f32::MIN, f32::max);
            let radius = eye.z - front;
            let pw = post.1 - post.0;
            println!(
                "{:12} v{:4.0} h {:5.1}mm eye({:.3},{:.3}) front z {:.3} radius {:5.0}mm post {:4.1}mm ({:4.1}') notch {:4.1}mm comb y {:.3} (eye over comb {:5.1}mm) butt z {:.3} support {} zero {}",
                def.id,
                f.muzzle_velocity,
                (eye.y - layout.muzzle.y) * 1000.0,
                eye.y,
                eye.z,
                front,
                radius * 1000.0,
                pw * 1000.0,
                (pw / (radius + 0.0)).atan().to_degrees() * 60.0,
                (notch.1 - notch.0) * 1000.0,
                comb,
                (eye.y - comb) * 1000.0,
                butt_z,
                layout.support.is_some(),
                crate::weapons::zero::iron_zero_m(f),
            );
            let _ = lo;
        }
    }
}
