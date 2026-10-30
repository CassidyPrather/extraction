//! The headset between valleys. A map change ends the game and opens it
//! again in the next valley, and the new session used to wait for the
//! world before it started: on the playtest the player stood 23 s in
//! SteamVR's empty room, took it for a crash, and said so.
//!
//! Now the session starts as soon as the runtime has a headset
//! (`xr::handle_session`), and until the world is in, ready and warm
//! (`timeline::WorldReady`), every view the player sees (the eyes, a
//! mirror) draws only [`LAYER`]: the valley's name on a panel a couple of
//! metres ahead, over the sky. The world shows once, all at once. Leaving
//! for the next valley, the panel comes back with where to ([`Leaving`])
//! before the game ends, so the runtime's own room shows only while one
//! process hands over to the next.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;

use crate::raid::bench::{ScreenStyle, ScreenText};

/// The loading view's render layer: nothing else is on it.
pub const LAYER: usize = 30;

/// Seconds the leaving panel shows before the game ends.
pub const LEAVE_SHOW: f32 = 0.8;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingView>()
            .init_resource::<Leaving>()
            // Before visibility's worked out: the frame the world shows is
            // drawn whole, not with last frame's loading-layer lists.
            .add_systems(
                PostUpdate,
                (hold_views, place_card)
                    .chain()
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
    }
}

/// When the world was let into the player's views (`None`: still loading).
#[derive(Resource, Default)]
pub struct LoadingView {
    pub lifted_at: Option<f64>,
}

/// Ending this session for the next valley: shows where to, then quits.
/// Set by whatever drives on (`camp`, `persist::relocate`) in a headset
/// game; flat, they quit at once.
#[derive(Resource, Default)]
pub struct Leaving {
    to: Option<String>,
    shown: f32,
    frames: u32,
}

impl Leaving {
    pub fn go(&mut self, map: &str) {
        info!("loading view: leaving for {map}");
        self.to = Some(map.to_owned());
        self.shown = 0.0;
        self.frames = 0;
    }
}

/// A player's view's own layers, kept while it shows the loading view.
#[derive(Component)]
struct HeldLayers(Option<RenderLayers>);

/// The panel, and the words on it.
#[derive(Component)]
pub struct LoadingCard(String);

/// "Marrow Creek" from "marrow_creek".
pub fn map_title(map: &str) -> String {
    map.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[allow(clippy::too_many_arguments)]
fn hold_views(
    mut commands: Commands,
    world: crate::render::timeline::WorldReady,
    zone: Option<Res<crate::world::Zone>>,
    mut view: ResMut<LoadingView>,
    mut leaving: ResMut<Leaving>,
    time: Res<Time<Real>>,
    views: Query<
        (Entity, Option<&RenderLayers>, Has<HeldLayers>),
        Or<(With<XrCamera>, With<super::xr::Mirror>)>,
    >,
    held: Query<(Entity, &HeldLayers)>,
    cards: Query<(Entity, &LoadingCard)>,
    mut exit: MessageWriter<AppExit>,
) {
    let words = match &leaving.to {
        Some(to) => Some(format!("On to {}", map_title(to))),
        None if view.lifted_at.is_none() && !world.ready() => Some(
            zone.as_ref()
                .map_or("Loading".to_string(), |z| z.map.name.clone()),
        ),
        None => None,
    };
    if let Some(words) = words {
        for (e, layers, kept) in &views {
            if !kept {
                info!("loading view: held on {e} (\"{words}\")");
                commands
                    .entity(e)
                    .insert((HeldLayers(layers.cloned()), RenderLayers::layer(LAYER)));
            }
        }
        if !cards.iter().any(|(_, c)| c.0 == words) {
            for (e, _) in &cards {
                commands.entity(e).try_despawn();
            }
            commands.spawn((
                Name::new("Loading view"),
                LoadingCard(words.clone()),
                ScreenText {
                    text: words,
                    size: Vec2::new(1.8, 0.45),
                    style: ScreenStyle::Lcd,
                },
                crate::render::legibility::TextSource::Loading,
                RenderLayers::layer(LAYER),
                Transform::from_xyz(0.0, -1000.0, 0.0),
                Visibility::Visible,
            ));
        }
        if leaving.to.is_some() {
            leaving.shown += time.delta_secs();
            leaving.frames += 1;
            if leaving.shown >= LEAVE_SHOW && leaving.frames >= 3 {
                exit.write(AppExit::Success);
            }
        }
        return;
    }
    // The world's in: every view gets its own layers back, at once.
    for (e, kept) in &held {
        let mut e = commands.entity(e);
        e.remove::<HeldLayers>();
        match &kept.0 {
            Some(layers) => e.insert(layers.clone()),
            None => e.remove::<RenderLayers>(),
        };
    }
    for (e, _) in &cards {
        commands.entity(e).try_despawn();
    }
    if view.lifted_at.is_none() {
        let at = crate::render::timeline::since_start();
        view.lifted_at = Some(at);
        info!("loading view: lifted at {at:.1} s, the world in");
    }
}

/// The panel two metres ahead of the eyes at their height, facing them,
/// and on the loading layer with the quad it's drawn on. It stays put
/// while the head turns a little, and comes round when it turns away.
fn place_card(
    mut commands: Commands,
    eyes: Query<(&XrCamera, &GlobalTransform)>,
    mut cards: Query<(&mut Transform, Option<&Children>), With<LoadingCard>>,
    layered: Query<(), With<RenderLayers>>,
) {
    let Some((_, eye)) = eyes.iter().min_by_key(|(c, _)| c.0) else {
        return;
    };
    let pos = eye.translation();
    // Where the eyes look, within 25 degrees of level.
    let look = eye.forward().as_vec3();
    let level = Vec3::new(look.x, 0.0, look.z)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let pitch = look.y.clamp(-1.0, 1.0).asin().clamp(-0.44, 0.44);
    let ahead = level * pitch.cos() + Vec3::Y * pitch.sin();
    for (mut t, children) in &mut cards {
        let to_card = t.translation - pos;
        let off = to_card
            .try_normalize()
            .map_or(f32::MAX, |d| d.angle_between(ahead));
        if off > 35f32.to_radians() || !(1.0..4.0).contains(&to_card.length()) {
            // The quad faces +Z: turned so that +Z looks back at the eye.
            *t = Transform::from_translation(pos + ahead * 2.0).looking_to(ahead, Vec3::Y);
        }
        for c in children.into_iter().flatten() {
            if !layered.contains(*c) {
                commands.entity(*c).insert(RenderLayers::layer(LAYER));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(LoadingPlugin)
            .add_message::<AppExit>();
        app
    }

    fn eye(app: &mut App, n: u32) -> Entity {
        app.world_mut()
            .spawn((XrCamera(n), Transform::from_xyz(0.0, 1.7, 0.0)))
            .id()
    }

    fn layers(app: &App, e: Entity) -> Option<RenderLayers> {
        app.world().get::<RenderLayers>(e).cloned()
    }

    fn card(app: &mut App) -> Option<String> {
        let w = app.world_mut();
        w.query::<&LoadingCard>()
            .iter(w)
            .next()
            .map(|c| c.0.clone())
    }

    /// While the world loads the eyes draw only the loading view, the
    /// panel on it; once it's in, they draw the world, and the panel's gone.
    #[test]
    fn the_eyes_see_the_loading_view_until_the_world_is_in() {
        let mut app = app();
        let building = app
            .world_mut()
            .spawn(crate::world::buildings::PropsPending(0))
            .id();
        let (a, b) = (eye(&mut app, 0), eye(&mut app, 1));
        for _ in 0..3 {
            app.update();
            for e in [a, b] {
                assert_eq!(layers(&app, e), Some(RenderLayers::layer(LAYER)));
            }
            assert_eq!(card(&mut app).as_deref(), Some("Loading"));
        }
        app.world_mut().despawn(building);
        app.world_mut()
            .insert_resource(crate::world::trees::ForestBuilt);
        app.update();
        app.update();
        for e in [a, b] {
            assert_eq!(layers(&app, e), None, "the eye's own layers");
        }
        assert_eq!(card(&mut app), None);
        assert!(app.world().resource::<LoadingView>().lifted_at.is_some());
        // A building waiting on its props later doesn't bring it back.
        app.world_mut()
            .spawn(crate::world::buildings::PropsPending(0));
        app.update();
        assert_eq!(layers(&app, a), None);
    }

    /// Leaving, the panel says where to, and the game ends after it's shown.
    #[test]
    fn leaving_shows_where_to_then_quits() {
        let mut app = app();
        app.world_mut()
            .insert_resource(crate::world::trees::ForestBuilt);
        let a = eye(&mut app, 0);
        app.update();
        app.update();
        assert_eq!(layers(&app, a), None);
        app.world_mut().resource_mut::<Leaving>().go("marrow_creek");
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(100),
        ));
        let quit = |app: &mut App| {
            app.world_mut()
                .resource_mut::<Messages<AppExit>>()
                .drain()
                .count()
                > 0
        };
        let mut at = None;
        for i in 0..30 {
            app.update();
            if i == 2 {
                assert_eq!(layers(&app, a), Some(RenderLayers::layer(LAYER)));
                assert_eq!(card(&mut app).as_deref(), Some("On to Marrow Creek"));
            }
            if quit(&mut app) {
                at = Some(i);
                break;
            }
        }
        let at = at.expect("the game ends");
        assert!(at >= 7, "not before the panel's shown (frame {at})");
    }

    #[test]
    fn titles() {
        assert_eq!(map_title("marrow_creek"), "Marrow Creek");
        assert_eq!(map_title("training_yard"), "Training Yard");
    }
}
