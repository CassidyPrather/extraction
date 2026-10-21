//! Wear you can see (client side), at the muzzle where a careful look
//! goes: the bore darkens from bright steel to soot as the gun fouls, and
//! the crown round it picks up nicks as the gun wears. Spread opens with
//! the same two numbers (`Firearm::spread_radians`), and a cleaning kit, oil
//! or the rod's strokes take the soot back out (spare parts the nicks, as
//! far as a worn gun can be brought back).

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::Gun;

/// Nicks on the crown at the worst.
pub const MAX_NICKS: u32 = 6;

/// The radius of a cartridge's bore, metres.
pub fn bore_radius(cartridge: &str) -> f32 {
    match cartridge {
        "12ga" => 0.0092,
        "5.45x39" => 0.0028,
        "9x18" => 0.0046,
        _ => 0.0039,
    }
}

/// Nicks on the crown at `condition`.
pub fn nicks(condition: f32) -> u32 {
    (((1.0 - condition) * 8.0) as u32).min(MAX_NICKS)
}

/// A gun's muzzle, as worn as it is: its bore's material and the nicks
/// shown.
#[derive(Component)]
pub struct Crown {
    gun: Entity,
    bore: Handle<StandardMaterial>,
    soot: u32,
    nicks: Vec<Entity>,
    shown: u32,
}

pub struct WearPlugin;

impl Plugin for WearPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_crowns, wear_crowns).chain());
    }
}

fn dress_crowns(
    mut commands: Commands,
    guns: Query<(Entity, &Gun), Added<Gun>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, gun) in &guns {
        let def = &gun.firearm.def;
        let r = bore_radius(&def.cartridge);
        let steel = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.55, 0.57),
            metallic: 0.9,
            perceptual_roughness: 0.35,
            ..default()
        });
        let nick_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.74, 0.7),
            metallic: 1.0,
            perceptual_roughness: 0.2,
            ..default()
        });
        let disc = meshes.add(Circle::new(r));
        let ring = meshes.add(Annulus::new(r, r + 0.0025));
        let nick = meshes.add(Cuboid::new(0.0012, 0.0012, 0.0004));
        let barrels = def.barrels.max(1);
        for b in 0..barrels {
            let mut at = gun.layout.muzzle;
            if barrels > 1 {
                at.x += if b == 0 { -0.0128 } else { 0.0128 };
            }
            // Facing out of the muzzle (-Z), a hair in front of its face.
            let face = Transform::from_translation(at + Vec3::NEG_Z * 0.0004)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
            let bore = materials.add(StandardMaterial {
                base_color: Color::srgb(0.5, 0.5, 0.52),
                metallic: 0.9,
                perceptual_roughness: 0.3,
                ..default()
            });
            let mut nicks = Vec::new();
            let crown = commands
                .spawn((
                    Name::new("Muzzle crown"),
                    face,
                    Visibility::default(),
                    ChildOf(e),
                ))
                .id();
            commands.spawn((
                Mesh3d(disc.clone()),
                MeshMaterial3d(bore.clone()),
                Transform::default(),
                NotShadowCaster,
                ChildOf(crown),
            ));
            commands.spawn((
                Mesh3d(ring.clone()),
                MeshMaterial3d(steel.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0001),
                NotShadowCaster,
                ChildOf(crown),
            ));
            for k in 0..MAX_NICKS {
                // Round the crown at odd angles, as knocks fall.
                let a = k as f32 * 2.399 + b as f32;
                let p = Vec2::from_angle(a) * (r + 0.00125);
                nicks.push(
                    commands
                        .spawn((
                            Mesh3d(nick.clone()),
                            MeshMaterial3d(nick_mat.clone()),
                            Transform::from_xyz(p.x, p.y, 0.0003)
                                .with_rotation(Quat::from_rotation_z(a)),
                            Visibility::Hidden,
                            NotShadowCaster,
                            ChildOf(crown),
                        ))
                        .id(),
                );
            }
            commands.entity(crown).insert(Crown {
                gun: e,
                bore,
                soot: u32::MAX,
                nicks,
                shown: 0,
            });
        }
    }
}

/// The bore's soot and the crown's nicks follow the gun's fouling and
/// condition.
fn wear_crowns(
    mut crowns: Query<&mut Crown>,
    guns: Query<&Gun>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    for mut c in &mut crowns {
        let Ok(gun) = guns.get(c.gun) else {
            continue;
        };
        let f = gun.firearm.fouling.clamp(0.0, 1.0);
        let soot = (f * 20.0) as u32;
        if soot != c.soot {
            c.soot = soot;
            if let Some(mut m) = materials.get_mut(&c.bore) {
                let clean = Vec3::new(0.5, 0.5, 0.52);
                let sooty = Vec3::new(0.03, 0.028, 0.025);
                let col = clean.lerp(sooty, f.powf(0.6));
                m.base_color = Color::srgb(col.x, col.y, col.z);
                m.metallic = 0.9 - 0.7 * f;
                m.perceptual_roughness = 0.3 + 0.6 * f;
            }
        }
        let n = nicks(gun.firearm.condition);
        if n != c.shown {
            c.shown = n;
            for (k, e) in c.nicks.iter().enumerate() {
                if let Ok(mut v) = vis.get_mut(*e) {
                    *v = if (k as u32) < n {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gun in good order has an unmarked crown; a worn one is nicked, to
    /// a limit.
    #[test]
    fn the_crown_nicks_as_it_wears() {
        assert_eq!(nicks(1.0), 0);
        assert_eq!(nicks(0.9), 0);
        assert!(nicks(0.5) >= 3);
        assert_eq!(nicks(0.0), MAX_NICKS);
    }
}
