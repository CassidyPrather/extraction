//! A gun clicked on an empty chamber with rounds in its feed
//! (`weapons::unchambered::NeedsWorking`): its action's handle (bolt,
//! slide, charging handle, forend) glows the glove's hover green, breathing
//! a little, till it's worked: a soft halo of light, not a solid
//! ([`halo_alpha`]). An affordance answering a mistake ("this is
//! what to work"), not a warning, and never lit otherwise.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::interact::Grabbable;
use crate::weapons::Gun;
use crate::weapons::unchambered::{NeedsWorking, action_point_of};

pub struct HandleGlowPlugin;

impl Plugin for HandleGlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_glows);
    }
}

/// The glow's size (m across) and how it breathes (share, Hz): a halo
/// round the part, larger than a forend's girth, so it shows where the
/// handle is buried in wood (a pump's forend; a 3.5 cm glow was lost in
/// it at night). The light itself spans `GLOW_REACH` of it, about 9 cm.
pub const GLOW_SIZE: f32 = 0.15;
pub const GLOW_BREATH: (f32, f32) = (0.15, 1.5);

/// A glow, on its gun.
#[derive(Component)]
pub struct HandleGlow;

/// How much of the green the halo adds at its middle: light, not a solid
/// (the coordinator's ruling on the first night shot, a flat green disc;
/// stacked shells after it still read as bands). It falls off smoothly to
/// nothing at the rim ([`halo_alpha`]).
pub const GLOW_PEAK: f32 = 0.35;
/// Where the halo has faded to nothing, a share of its radius: well inside
/// the quad, so no edge shows (the coordinator's crop of the last one).
pub const GLOW_REACH: f32 = 0.6;

/// The halo's opacity `r` of the way out from its middle (0..1): the peak
/// at the middle, smoothly to nothing at the rim.
pub fn halo_alpha(r: f32) -> f32 {
    // A smoothstep down to nothing at `GLOW_REACH`: at the quad's own rim,
    // where the night's exposure showed any faint remainder as a circle,
    // there's none.
    let t = (1.0 - r / GLOW_REACH).clamp(0.0, 1.0);
    GLOW_PEAK * t * t * (3.0 - 2.0 * t)
}

fn halo_texture(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let n = 64u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let a = halo_alpha((u * u + v * v).sqrt());
            data.extend_from_slice(&[255, 255, 255, (a * 255.0).round() as u8]);
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

/// The halo is a soft disc of light turned to the eye: it's light round the
/// part, so it reads the same from any side.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn handle_glows(
    mut commands: Commands,
    marked: Query<(Entity, Option<&Gun>, &Grabbable, Option<&GlobalTransform>), With<NeedsWorking>>,
    mut glows: Query<(Entity, &ChildOf, &mut Transform), With<HandleGlow>>,
    eyes: Query<&GlobalTransform, With<Camera3d>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: Option<ResMut<Assets<Image>>>,
    time: Res<Time>,
) {
    let breath =
        1.0 + GLOW_BREATH.0 * (time.elapsed_secs() * GLOW_BREATH.1 * std::f32::consts::TAU).sin();
    let eye = eyes.iter().next().map(|g| g.translation());
    let mut have = std::collections::HashSet::new();
    for (e, parent, mut t) in &mut glows {
        let Ok((_, _, _, gun_at)) = marked.get(parent.parent()) else {
            commands.entity(e).despawn();
            continue;
        };
        have.insert(parent.parent());
        t.scale = Vec3::splat(GLOW_SIZE * breath);
        // Turned to the eye: its face (+Z) toward it, in the gun's frame.
        if let (Some(eye), Some(g)) = (eye, gun_at) {
            let at = g.transform_point(t.translation);
            let face = Transform::IDENTITY.looking_to(at - eye, Vec3::Y).rotation;
            t.rotation = g.rotation().inverse() * face;
        }
    }
    for (gun, g, grab, _) in &marked {
        if have.contains(&gun) {
            continue;
        }
        let pump =
            g.is_some_and(|g| g.firearm.def.action == extraction_core::firearm::ActionKind::Pump);
        let Some(p) = action_point_of(pump, grab) else {
            continue;
        };
        let (mesh, mat) = look
            .get_or_insert_with(|| {
                let texture = images.as_deref_mut().map(halo_texture);
                (
                    meshes.add(Rectangle::new(1.0, 1.0)),
                    materials.add(StandardMaterial {
                        // The glove's hover green (`render::props::tint_gloves`),
                        // its strength the texture's (`halo_alpha`).
                        base_color: Color::srgb(0.3, 0.8, 0.4),
                        base_color_texture: texture,
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.spawn((
            Name::new("Handle glow"),
            HandleGlow,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            NotShadowCaster,
            Transform::from_translation(p.local.translation).with_scale(Vec3::splat(GLOW_SIZE)),
            ChildOf(gun),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::{GrabPoint, GripRole};

    /// The halo is light: half the green at its middle at most, smoothly
    /// less outward, nothing at the rim.
    #[test]
    fn the_halo_falls_off_to_nothing() {
        assert!((halo_alpha(0.0) - GLOW_PEAK).abs() < 1e-6 && GLOW_PEAK <= 0.35);
        assert_eq!(halo_alpha(GLOW_REACH), 0.0, "nothing well inside the quad");
        assert_eq!(halo_alpha(1.0), 0.0);
        let steps: Vec<f32> = (0..=20).map(|i| halo_alpha(i as f32 / 20.0)).collect();
        assert!(steps.windows(2).all(|w| w[1] <= w[0]), "{steps:?}");
        assert!(halo_alpha(0.5) < GLOW_PEAK * 0.1, "faint near its reach");
    }

    /// A marked gun's handle glows, where its handle is; worked (the mark
    /// gone), the glow goes; a gun never marked has none.
    #[test]
    fn a_handle_glows_while_its_gun_wants_working() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_plugins(HandleGlowPlugin);
        let at = Vec3::new(0.03, 0.02, 0.1);
        let grab = || Grabbable {
            points: vec![
                GrabPoint::new(Transform::IDENTITY, GripRole::Primary, 0.08),
                GrabPoint::new(Transform::from_translation(at), GripRole::Handle(0), 0.07),
            ],
        };
        let gun = app.world_mut().spawn((grab(), NeedsWorking)).id();
        let other = app.world_mut().spawn(grab()).id();
        app.update();
        let glows = |app: &mut App| -> Vec<(Entity, Vec3)> {
            let mut q = app
                .world_mut()
                .query_filtered::<(&ChildOf, &Transform), With<HandleGlow>>();
            q.iter(app.world())
                .map(|(c, t)| (c.parent(), t.translation))
                .collect()
        };
        assert_eq!(glows(&mut app), vec![(gun, at)], "one glow, at the handle");
        app.update();
        assert_eq!(glows(&mut app).len(), 1, "still one");
        let _ = other;
        app.world_mut().entity_mut(gun).remove::<NeedsWorking>();
        app.update();
        assert!(glows(&mut app).is_empty(), "worked: gone");
    }
}
