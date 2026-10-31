//! Rats in the houses. Step inside and, as often as not, there are a few
//! on the floor along the walls, sniffing about. Come near ([`SCARE`], less
//! creeping crouched) or put your torch on one ([`TORCH_REACH`]) and they
//! scurry along the wall to the nearest corner and are gone, into the
//! walls, with a squeak ([`Scurried`]). A house whose rats have run stays
//! empty for [`AWAY`] seconds; rats sitting quiet in a house mean nobody's
//! been in lately.
//!
//! A house's rats, and where they sit, are a roll of the seed and the
//! house (`robots::doors::Entrances` shells: every building with doors),
//! their floor found by a ray down inside it. Only the house you're in has
//! them.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::noise::Rng;

/// A house has rats this often.
pub const CHANCE: f32 = 0.6;
/// They run from someone this close (m)...
pub const SCARE: f32 = 4.5;
/// ...this close creeping crouched...
pub const SCARE_CREEP: f32 = 2.2;
/// ...or from a torch's beam on them from this far.
pub const TORCH_REACH: f32 = 12.0;
pub const TORCH_CONE: f32 = 0.45;
pub const SPEED: f32 = 2.6;
/// A house whose rats ran is empty for this long (s).
pub const AWAY: f32 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RatState {
    Sniff,
    /// Off to `to` (a corner), then gone.
    Run {
        to: Vec3,
    },
    Gone,
}

#[derive(Clone, Debug)]
pub struct Rat {
    pub pos: Vec3,
    pub yaw: f32,
    pub state: RatState,
    /// For the scurry's frames and the sniffing.
    pub phase: f32,
}

#[derive(Component, Clone, Debug)]
pub struct Rats {
    pub building: Entity,
    pub rats: Vec<Rat>,
    /// The house's corners at floor height, inset.
    corners: Vec<Vec3>,
}

/// Rats ran: where (for a squeak).
#[derive(Message, Clone, Copy, Debug)]
pub struct Scurried {
    pub at: Vec3,
    pub rats: u32,
}

/// Rats for a house, if it has any: along its walls on the floor.
fn rats_for(
    shell: &crate::robots::doors::Shell,
    index: usize,
    seed: u64,
    spatial: &SpatialQuery,
    always: bool,
) -> Option<Rats> {
    let mut rng = Rng::new(seed ^ 0x2A75 ^ (index as u64).wrapping_mul(0x9E37_79B9));
    if !rng.chance(CHANCE) && !always {
        return None;
    }
    let world = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    let (hx, hz) = (shell.half.x - 0.22, shell.half.y - 0.22);
    if hx < 0.8 || hz < 0.8 {
        return None;
    }
    // The floor: a ray down at the middle.
    let floor_of = |l: Vec3| {
        let from = shell.xf.transform_point(l + Vec3::Y * 1.4);
        spatial
            .cast_ray(from, Dir3::NEG_Y, 2.5, true, &world)
            .filter(|h| h.normal.y > 0.8)
            .map(|h| from + Vec3::NEG_Y * h.distance)
    };
    let mid = floor_of(Vec3::ZERO)?;
    let n = 1 + rng.below(4);
    // (The training yard's practice house always has a few:
    // `onboarding::the_rats`.)
    let n = if always { n.max(3) } else { n };
    let mut rats = Vec::new();
    for _ in 0..n * 3 {
        if rats.len() >= n {
            break;
        }
        // Along one of the four walls.
        let along = rng.range(-1.0, 1.0);
        let l = match rng.below(4) {
            0 => Vec3::new(-hx, 0.0, along * hz),
            1 => Vec3::new(hx, 0.0, along * hz),
            2 => Vec3::new(along * hx, 0.0, -hz),
            _ => Vec3::new(along * hx, 0.0, hz),
        };
        // On the floor itself (not on a table or a bed).
        let Some(p) = floor_of(l) else {
            continue;
        };
        if (p.y - mid.y).abs() > 0.12 {
            continue;
        }
        rats.push(Rat {
            pos: p,
            yaw: rng.range(0.0, std::f32::consts::TAU),
            state: RatState::Sniff,
            phase: rng.f32(),
        });
    }
    if rats.is_empty() {
        return None;
    }
    let corners = [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)]
        .iter()
        .map(|&(x, z)| shell.xf.transform_point(Vec3::new(x, 0.0, z)).with_y(mid.y))
        .collect();
    Some(Rats {
        building: shell.building,
        rats,
        corners,
    })
}

impl Rats {
    /// One tick: scared by people (feet, speed, crouched) and torches
    /// (where, which way). Returns how many started running.
    pub fn tick(&mut self, people: &[(Vec3, f32, bool)], torches: &[(Vec3, Vec3)], dt: f32) -> u32 {
        let mut ran = 0;
        let corners = self.corners.clone();
        for r in &mut self.rats {
            r.phase = (r.phase
                + dt * if matches!(r.state, RatState::Run { .. }) {
                    11.0
                } else {
                    0.7
                })
            .fract();
            match r.state {
                RatState::Sniff => {
                    let near = people.iter().any(|&(p, speed, crouched)| {
                        let reach = if crouched && speed < 1.0 {
                            SCARE_CREEP
                        } else {
                            SCARE
                        };
                        p.xz().distance(r.pos.xz()) < reach && (p.y - r.pos.y).abs() < 2.0
                    });
                    let lit = torches.iter().any(|&(at, dir)| {
                        let to = r.pos - at;
                        let d = to.length();
                        d < TORCH_REACH && dir.angle_between(to / d.max(1e-3)) < TORCH_CONE
                    });
                    if near || lit {
                        // The nearest corner, along the wall.
                        let to = corners
                            .iter()
                            .copied()
                            .min_by(|a, b| a.distance(r.pos).total_cmp(&b.distance(r.pos)))
                            .unwrap_or(r.pos);
                        r.state = RatState::Run { to };
                        ran += 1;
                    } else if (r.phase * 13.0).sin() > 0.98 {
                        r.yaw += 0.8;
                    }
                }
                RatState::Run { to } => {
                    let d = to - r.pos;
                    if d.length() < 0.1 {
                        r.state = RatState::Gone;
                        continue;
                    }
                    // Straight there: the nearest corner is along its own wall.
                    let step = d.normalize() * SPEED * dt;
                    r.pos += step.clamp_length_max(d.length());
                    r.yaw = d.z.atan2(d.x);
                }
                RatState::Gone => {}
            }
        }
        ran
    }
}

/// The house you're in gets its rats; the ones you've left lose theirs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rats(
    mut commands: Commands,
    mut houses: Query<(Entity, &mut Rats)>,
    me: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    players: Query<(&GlobalTransform, &crate::player::Body), With<crate::player::Player>>,
    torches: Query<(&crate::gadgets::Device, &GlobalTransform), With<crate::gadgets::Powered>>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    spatial: SpatialQuery,
    opts: Res<crate::LaunchOptions>,
    buildings: Query<&crate::world::buildings::Building>,
    mut scurried: MessageWriter<Scurried>,
    mut emptied: Local<std::collections::HashMap<Entity, f32>>,
    time: Res<Time>,
) {
    let (Some(entrances), Ok(me)) = (entrances, me.single()) else {
        return;
    };
    let now = time.elapsed_secs();
    let at = me.translation();
    let inside = entrances.building_at(at + Vec3::Y * 0.5);
    for (e, r) in &houses {
        if Some(r.building) != inside {
            if r.rats.iter().any(|r| r.state == RatState::Gone) {
                emptied.insert(r.building, now);
            }
            commands.entity(e).despawn();
        }
    }
    if let Some(b) = inside {
        let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
        let have = houses.iter().any(|(_, r)| r.building == b);
        let fresh = emptied.get(&b).is_none_or(|t| now - t > AWAY);
        if !have && fresh && !off {
            emptied.remove(&b);
            if let Some((i, shell)) = entrances
                .shells
                .iter()
                .enumerate()
                .find(|(_, s)| s.building == b)
            {
                // The yard's practice house (not its sheds) always has them.
                let yard = opts.map == crate::camp::TRAINING_YARD
                    && buildings
                        .get(b)
                        .is_ok_and(|bl| bl.prefab == crate::onboarding::RATS_HOUSE);
                match rats_for(shell, i, opts.seed, &spatial, yard) {
                    Some(r) => {
                        commands.spawn((
                            Name::new("Rats"),
                            Transform::IDENTITY,
                            Visibility::default(),
                            r,
                        ));
                    }
                    // A house without rats (the roll is the same every
                    // time): don't look again.
                    None => {
                        emptied.insert(b, now + 1.0e6);
                    }
                }
            }
        }
    }
    let people: Vec<(Vec3, f32, bool)> = players
        .iter()
        .map(|(g, b)| {
            (
                g.translation(),
                Vec2::new(b.velocity.x, b.velocity.z).length(),
                b.height > 0.0 && b.height < crate::robots::senses::CROUCHED,
            )
        })
        .collect();
    let beams: Vec<(Vec3, Vec3)> = torches
        .iter()
        .filter(|(d, _)| d.kind == crate::gadgets::DeviceKind::Light)
        .map(|(_, g)| (g.translation(), g.forward().as_vec3()))
        .collect();
    let dt = time.delta_secs();
    for (_, mut r) in &mut houses {
        let ran = r.tick(&people, &beams, dt);
        if ran > 0 {
            let at = r.rats.iter().map(|r| r.pos).sum::<Vec3>() / r.rats.len() as f32;
            scurried.write(Scurried { at, rats: ran });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn house() -> Rats {
        let corners = vec![
            Vec3::new(-3.0, 0.0, -3.0),
            Vec3::new(3.0, 0.0, -3.0),
            Vec3::new(3.0, 0.0, 3.0),
            Vec3::new(-3.0, 0.0, 3.0),
        ];
        Rats {
            building: Entity::PLACEHOLDER,
            rats: vec![Rat {
                pos: Vec3::new(3.0, 0.0, 1.0),
                yaw: 0.0,
                state: RatState::Sniff,
                phase: 0.0,
            }],
            corners,
        }
    }

    /// They sit tight for a crouched creep at 3 m but not a walk at 4;
    /// a torch on them from 10 m sends them off; running, they reach a
    /// corner and are gone.
    #[test]
    fn rats_run_from_you_and_your_torch_to_a_corner() {
        let mut h = house();
        assert_eq!(
            h.tick(&[(Vec3::new(0.2, 0.0, 1.0), 0.5, true)], &[], 0.03),
            0
        );
        assert_eq!(
            h.tick(&[(Vec3::new(-0.8, 0.0, 1.0), 1.4, false)], &[], 0.03),
            1
        );
        let mut h = house();
        let from = Vec3::new(-7.0, 1.2, 1.0);
        let dir = (h.rats[0].pos - from).normalize();
        assert_eq!(
            h.tick(&[], &[(from, -dir)], 0.03),
            0,
            "the beam pointed away"
        );
        assert_eq!(h.tick(&[], &[(from, dir)], 0.03), 1);
        for _ in 0..200 {
            h.tick(&[], &[], 0.03);
        }
        assert_eq!(h.rats[0].state, RatState::Gone);
    }
}
