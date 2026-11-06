//! Retroreflective tape (the `Reflector` model material: a snow pole's
//! bands): it sends a light back the way it came, so to someone holding a
//! torch, a band in its beam shines out of the dark where paint is lost.
//! A standard material can't do that, so each reflector's mesh is lit for
//! the eye's own torch: the torch's light reaching it (inside the beam's
//! cone, falling with distance squared) picks one of a few shared copies
//! of the material, each glowing more (`grade::ToTheEye`, so it holds its
//! colour through the eye's adaptation). By day, or with no torch on it,
//! it's pale grey tape.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use crate::models::ModelMaterials;

pub struct ReflectorsPlugin;

impl Plugin for ReflectorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (make_steps, find, shine).chain());
    }
}

/// How many glowing steps (besides dark).
pub const STEPS: usize = 6;
/// The tape's glow at its brightest (linear, before the eye's exposure):
/// a lantern's core is (6, 3.2, 1); white tape a little cool.
const GLOW: LinearRgba = LinearRgba::rgb(5.5, 5.5, 5.0);
/// How far off a band in the beam's middle shines at its brightest (m):
/// nearer, as bright; further, falling with distance squared.
pub const FULL_AT: f32 = 25.0;
/// How near the eye a spot light must be to be the eye's own torch (m).
const HELD: f32 = 1.5;

/// The tape's shared copies: dark, then each step brighter.
#[derive(Resource)]
struct Steps {
    base: AssetId<StandardMaterial>,
    steps: Vec<Handle<StandardMaterial>>,
}

/// A mesh drawn in tape, and the step it's on.
#[derive(Component)]
pub struct Reflector(usize);

fn make_steps(
    mut commands: Commands,
    model_mats: Option<Res<ModelMaterials>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    steps: Option<Res<Steps>>,
) {
    if steps.is_some() {
        return;
    }
    let Some(base) = model_mats.and_then(|m| m.cached("Reflector").cloned()) else {
        return;
    };
    let Some(m) = materials.get(&base).cloned() else {
        return;
    };
    let mut all = vec![base.clone()];
    for k in 1..=STEPS {
        let h = materials.add(m.clone());
        commands.spawn((
            Name::new(format!("Reflector glow {k}")),
            crate::render::grade::ToTheEye {
                material: h.clone(),
                emissive: GLOW * level(k),
            },
        ));
        all.push(h);
    }
    commands.insert_resource(Steps {
        base: base.id(),
        steps: all,
    });
}

/// Step `k`'s share of the full glow.
pub fn level(k: usize) -> f32 {
    k as f32 / STEPS as f32
}

/// Meshes come in drawn in the tape: they're reflectors.
fn find(
    mut commands: Commands,
    steps: Option<Res<Steps>>,
    added: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
) {
    let Some(steps) = steps else {
        return;
    };
    for (e, m) in &added {
        if m.0.id() == steps.base {
            commands.entity(e).insert(Reflector(0));
        }
    }
}

/// The step for a band lit `share` of the torch's full reach (1 at
/// [`FULL_AT`] in the beam's middle).
pub fn step_for(share: f32) -> usize {
    if share <= 0.02 {
        return 0;
    }
    ((share.min(1.0).sqrt() * STEPS as f32).ceil() as usize).clamp(1, STEPS)
}

/// How much of the torch reaches a band at `p`: the beam's cone and the
/// distance (1 in its middle at [`FULL_AT`]).
pub fn reach(torch: &GlobalTransform, spot: &SpotLight, p: Vec3) -> f32 {
    let d = p - torch.translation();
    let dist = d.length();
    if dist < 0.01 || dist > spot.range {
        return 0.0;
    }
    let cos = d.dot(*torch.forward()) / dist;
    let (outer, inner) = (spot.outer_angle.cos(), spot.inner_angle.cos());
    let t = ((cos - outer) / (inner - outer).max(1e-4)).clamp(0.0, 1.0);
    let cone = t * t * (3.0 - 2.0 * t);
    cone * (FULL_AT / dist).powi(2)
}

fn shine(
    steps: Option<Res<Steps>>,
    eyes: crate::render::puff::Eyes,
    spots: Query<(&SpotLight, &GlobalTransform, &InheritedVisibility)>,
    mut tape: Query<(
        &mut Reflector,
        &GlobalTransform,
        Option<&Aabb>,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(steps) = steps else {
        return;
    };
    let eye = crate::render::puff::eye(&eyes);
    // The eye's own torch, if it's on.
    let torch = eye.and_then(|eye| {
        spots
            .iter()
            .filter(|(s, g, v)| {
                v.get() && s.intensity > 0.0 && g.translation().distance(eye) < HELD
            })
            .min_by(|a, b| {
                a.1.translation()
                    .distance(eye)
                    .total_cmp(&b.1.translation().distance(eye))
            })
    });
    for (mut r, g, aabb, mut mat) in &mut tape {
        let k = match torch {
            Some((spot, tg, _)) => {
                let p = g.transform_point(aabb.map_or(Vec3::ZERO, |a| Vec3::from(a.center)));
                step_for(reach(tg, spot, p))
            }
            None => 0,
        };
        if r.0 != k {
            r.0 = k;
            mat.0 = steps.steps[k].clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In the beam a band shines more the nearer it is, and full at
    /// [`FULL_AT`]; out of it, it's dark.
    #[test]
    fn a_band_shines_in_the_beam_and_not_out_of_it() {
        let spot = crate::gadgets::visuals::torch_beam();
        let torch = GlobalTransform::from(Transform::IDENTITY);
        let ahead = |d: f32| step_for(reach(&torch, &spot, Vec3::NEG_Z * d));
        assert_eq!(ahead(FULL_AT), STEPS);
        assert_eq!(ahead(15.0), STEPS);
        assert!(ahead(40.0) > 0 && ahead(40.0) < STEPS);
        assert!(ahead(40.0) <= ahead(30.0));
        // Off to the side of the beam, or behind.
        assert_eq!(
            step_for(reach(&torch, &spot, Vec3::new(20.0, 0.0, -5.0))),
            0
        );
        assert_eq!(step_for(reach(&torch, &spot, Vec3::Z * 10.0)), 0);
        // Past the torch's range.
        assert_eq!(ahead(spot.range + 1.0), 0);
    }
}
