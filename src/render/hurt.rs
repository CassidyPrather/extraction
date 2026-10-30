//! Being hit is unmistakable: the edges of the view darken red for a
//! moment, most on the side the hit came from, more for a harder one
//! (`docs/playtests/2026-10-28`, F1: "some kind of vignette when you're
//! taking damage would let me know when I'm being shot"). Alongside the
//! hands' shake (`rig::haptics::hurt_haptics`), never instead of it.
//!
//! Comfort first: never opaque ([`CAP`] of the setting), no flash over
//! the middle of the view, and held, not strobed, under sustained fire
//! (a hit within [`HOLD`] of the last raises the edges and holds them;
//! they only fade once the hits stop). Harm with no direction (a fall,
//! an anomaly, the Surge) darkens every edge alike; bleeding throbs
//! faintly every [`THROB_EVERY`] s, so a wound that only the wrist would
//! report is seen wherever you look. `Comfort::hurt_vignette` sets how
//! strong (0 is off). Headset and flat alike.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::health::{Health, Hurt};
use crate::rig::comfort::Comfort;
use crate::rig::{LocalPlayer, RigHead};

pub struct HurtVignettePlugin;

impl Plugin for HurtVignettePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_hurt_vignette, hurt_vignette).chain());
    }
}

/// The edges' darkest at full strength (alpha).
pub const CAP: f32 = 0.7;
/// How long the edges hold after a hit before they fade (s).
pub const HOLD: f32 = 0.3;
/// How fast they fade after that (time constant, s): gone in about 0.6 s.
pub const FADE: f32 = 0.2;
/// How often a bleeding wound throbs (s), and how strongly (of full).
pub const THROB_EVERY: f32 = 1.5;
pub const THROB: f32 = 0.25;
/// The faintest hit still shows this much (of full), and a hit of this
/// share of the player's health shows at full.
pub const LEAST: f32 = 0.25;
pub const FULL_AT: f32 = 0.25;

/// The edges, in head space: top (a hit from ahead), right, bottom (from
/// behind), left.
const SIDES: [Vec2; 4] = [Vec2::Y, Vec2::X, Vec2::NEG_Y, Vec2::NEG_X];

/// How strong a hit of `amount` shows, of full, on a player with `max` hp.
pub fn severity(amount: f32, max: f32) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }
    (amount / (max.max(1.0) * FULL_AT)).clamp(LEAST, 1.0)
}

/// Each edge's share of a hit from `from` (a direction in head space,
/// -Z ahead): every edge some, the side it came from most.
pub fn side_weights(from: Vec3) -> [f32; 4] {
    // Ahead maps to the top edge, behind to the bottom.
    let d = Vec2::new(from.x, -from.z).normalize_or_zero();
    if d == Vec2::ZERO {
        return [1.0; 4];
    }
    SIDES.map(|s| 0.35 + 0.65 * d.dot(s).max(0.0))
}

/// The edges' levels (0..1 of full), held after each hit, then fading.
#[derive(Component, Default)]
pub struct HurtVignette {
    pub levels: [f32; 4],
    /// Seconds since the edges were last raised.
    pub since: f32,
    /// Seconds to the next throb while bleeding.
    throb: f32,
    materials: Vec<Handle<StandardMaterial>>,
}

impl HurtVignette {
    /// Raises the edges to at least `by` (each edge its own share).
    pub fn hit(&mut self, by: [f32; 4]) {
        for (l, b) in self.levels.iter_mut().zip(by) {
            *l = l.max(b).min(1.0);
        }
        self.since = 0.0;
    }

    /// Time passes: held for [`HOLD`] after the last raise, then fading.
    pub fn tick(&mut self, dt: f32) {
        self.since += dt;
        if self.since > HOLD {
            let k = (-dt / FADE).exp();
            for l in &mut self.levels {
                *l *= k;
                if *l < 0.01 {
                    *l = 0.0;
                }
            }
        }
    }
}

/// One edge's ring: clear in the middle, dark toward the rim, and only on
/// its own side (a lobe round `side`, the four together a full ring).
fn edge_texture(images: &mut Assets<Image>, side: Vec2) -> Handle<Image> {
    let n = 128u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            // Image rows run down; head space's up is +Y.
            let v = 1.0 - (y as f32 + 0.5) / n as f32 * 2.0;
            let p = Vec2::new(u, v);
            let r = p.length();
            let ring = ((r - 0.34) / 0.36).clamp(0.0, 1.0);
            let ring = ring * ring * (3.0 - 2.0 * ring);
            let lobe = p.normalize_or_zero().dot(side).max(0.0).powf(1.5);
            let a = ring * lobe;
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

/// The dark red of it (not a flash: blood, at the edge of sight).
fn tint(alpha: f32) -> Color {
    Color::srgba(0.32, 0.0, 0.0, alpha)
}

fn add_hurt_vignette(
    mut commands: Commands,
    heads: Query<(Entity, &ChildOf), Added<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (head, parent) in &heads {
        if !locals.contains(parent.parent()) {
            continue;
        }
        let quad = meshes.add(Rectangle::new(0.5, 0.5));
        let mut vignette = HurtVignette::default();
        for side in SIDES {
            vignette.materials.push(materials.add(StandardMaterial {
                base_color: tint(0.0),
                base_color_texture: Some(edge_texture(&mut images, side)),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                fog_enabled: false,
                ..default()
            }));
        }
        let edges = vignette.materials.clone();
        // Between the blink (-0.065) and the stick's ring (-0.07).
        commands.entity(head).with_children(|c| {
            c.spawn((
                Name::new("Hurt edges"),
                vignette,
                Transform::from_xyz(0.0, 0.0, -0.068),
                Visibility::Hidden,
            ))
            .with_children(|c| {
                for m in edges {
                    c.spawn((
                        Mesh3d(quad.clone()),
                        MeshMaterial3d(m),
                        Transform::default(),
                        NotShadowCaster,
                    ));
                }
            });
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn hurt_vignette(
    mut vignettes: Query<(&mut HurtVignette, &mut Visibility, &GlobalTransform)>,
    players: Query<&Health, With<LocalPlayer>>,
    locals: Query<Entity, With<LocalPlayer>>,
    mut hurts: MessageReader<Hurt>,
    comfort: Option<Res<Comfort>>,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<(f32, f32)>>,
) {
    let strength = comfort.map_or(0.6, |c| c.hurt_vignette);
    let dt = time.delta_secs();
    let me = locals.iter().next();
    let hurts: Vec<Hurt> = hurts
        .read()
        .filter(|h| Some(h.player) == me)
        .cloned()
        .collect();
    let health = players.iter().next();
    for (mut v, mut vis, head) in &mut vignettes {
        v.tick(dt);
        let into_head = head.rotation().inverse();
        for h in &hurts {
            let max = health.map_or(100.0, |h| h.max);
            let s = severity(h.amount, max);
            let w = side_weights(into_head * (h.from - head.translation()));
            v.hit(w.map(|w| w * s));
        }
        if let Some(h) = health {
            // Harm that came with no direction (a fall, an anomaly, the
            // Surge): `since_hit` restarts and hp drops, with no Hurt.
            if let Some((was_hp, was_since)) = *last {
                let new_hit = h.since_hit < was_since || (h.since_hit < 0.1 && h.hp < was_hp);
                if new_hit && hurts.is_empty() && h.hp < was_hp {
                    let s = severity(was_hp - h.hp, h.max);
                    v.hit([s; 4]);
                }
            }
            *last = Some((h.hp, h.since_hit));
            // A wound that bleeds throbs, so it's seen without the wrist.
            if h.bleeding > 0.0 && !h.down {
                v.throb -= dt;
                if v.throb <= 0.0 {
                    v.throb = THROB_EVERY;
                    v.hit([THROB; 4]);
                }
            } else {
                v.throb = 0.0;
            }
        }
        let any = v.levels.iter().any(|l| *l > 0.0) && strength > 0.0;
        let want = if any {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        if !any {
            continue;
        }
        for (m, l) in v.materials.iter().zip(v.levels) {
            let a = (l * CAP * strength).min(CAP);
            if let Some(mut mat) = materials.get_mut(m) {
                mat.base_color = tint(a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hit from the right darkens the right edge most, every edge some;
    /// from ahead, the top; from behind, the bottom.
    #[test]
    fn a_hit_shows_on_the_side_it_came_from() {
        let [top, right, bottom, left] = side_weights(Vec3::X);
        assert!(right > top && right > left && right > bottom);
        assert!(left > 0.0 && top > 0.0);
        let [top, _, bottom, _] = side_weights(Vec3::NEG_Z);
        assert!(top > bottom);
        let [top, _, bottom, _] = side_weights(Vec3::Z);
        assert!(bottom > top);
        assert_eq!(side_weights(Vec3::Y), [1.0; 4], "straight above: all");
    }

    /// The faintest hit still shows; a quarter of your health is full;
    /// more is no more.
    #[test]
    fn a_harder_hit_shows_more_up_to_full() {
        assert_eq!(severity(0.0, 100.0), 0.0);
        assert_eq!(severity(1.0, 100.0), LEAST);
        assert!(severity(15.0, 100.0) > severity(8.0, 100.0));
        assert_eq!(severity(25.0, 100.0), 1.0);
        assert_eq!(severity(90.0, 100.0), 1.0);
    }

    /// Sustained fire holds the edges dark, never strobing; once it stops
    /// they fade to nothing within a second.
    #[test]
    fn under_fire_the_edges_hold_then_fade() {
        let mut v = HurtVignette::default();
        let dt = 1.0 / 90.0;
        let mut low = 1.0f32;
        for frame in 0..180 {
            // A hit every 0.2 s for two seconds.
            if frame % 18 == 0 {
                v.hit([0.6; 4]);
            }
            v.tick(dt);
            if frame > 0 {
                low = low.min(v.levels[0]);
            }
        }
        assert!(low > 0.55, "dipped to {low} between hits");
        for _ in 0..90 {
            v.tick(dt);
        }
        assert_eq!(v.levels, [0.0; 4], "still showing a second on");
    }

    /// However many hits land at once, the edges never go past the cap.
    #[test]
    fn a_storm_of_hits_never_blacks_out() {
        let mut v = HurtVignette::default();
        for _ in 0..50 {
            v.hit([severity(100.0, 100.0); 4]);
        }
        assert!(v.levels.iter().all(|l| *l <= 1.0));
        assert!(CAP < 0.75);
    }
}
