//! `EXTRACTION_PREPASS=1`: a depth prepass on every player view (both
//! eyes, the flat camera, the mirror, and the warm-up shelf's proxy so its
//! pipelines are warmed too). A headset experiment, off by default.
//!
//! Without it, a pixel behind stacked foliage cards is shaded once for
//! every card over it. Marrow Creek's spruce view is 10 layers a pixel,
//! 7.5 of them cards (`render::overdraw`). With it, each card's alpha is
//! tested once in a depth-only pass, and the main pass shades only what's
//! in front. That costs a second geometry pass an eye, so whether it pays
//! is a GPU question: the software renderer here can't answer it honestly.
//! See docs/PERFORMANCE.md, "Depth prepass (headset experiment)".

use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;

pub struct PrepassPlugin;

/// Whether this run asked for the prepass (`EXTRACTION_PREPASS=1`), read
/// once: the materials whose vertex shaders move their geometry give the
/// prepass (and so the sun's shadows) their own shaders only then
/// ([`shader`]), so without it nothing is drawn differently.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("EXTRACTION_PREPASS").is_ok_and(|v| v == "1"))
}

/// A moving material's prepass shader: its own file when the prepass is
/// on (cards swayed and cut, impostors turned, cover blown, as drawn, so
/// the depth matches the picture), Bevy's otherwise.
pub fn shader(path: &'static str) -> bevy::shader::ShaderRef {
    if on() {
        path.into()
    } else {
        bevy::shader::ShaderRef::Default
    }
}

impl Plugin for PrepassPlugin {
    fn build(&self, app: &mut App) {
        let on = on();
        if on {
            info!("render: depth prepass on every player view (EXTRACTION_PREPASS=1)");
        }
        app.insert_resource(Prepass(on))
            .add_systems(PostUpdate, give_prepass);
    }
}

/// Whether player views get a depth prepass (`EXTRACTION_PREPASS`).
#[derive(Resource, Clone, Copy)]
pub struct Prepass(pub bool);

/// On a camera with the player's look (`render::camera_look`).
#[derive(Component, Default, Clone, Copy)]
pub struct PlayerLook;

fn give_prepass(
    mut commands: Commands,
    on: Res<Prepass>,
    views: Query<Entity, (Added<PlayerLook>, Without<DepthPrepass>)>,
) {
    if !on.0 {
        return;
    }
    for e in &views {
        commands.entity(e).insert(DepthPrepass);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(on: bool) -> bool {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(PrepassPlugin);
        app.insert_resource(Prepass(on));
        let weather = crate::world::sky::Weather::default();
        let e = app
            .world_mut()
            .spawn((Camera3d::default(), super::super::camera_look(&weather)))
            .id();
        let other = app.world_mut().spawn(Camera3d::default()).id();
        app.update();
        assert!(
            app.world().get::<DepthPrepass>(other).is_none(),
            "an offscreen camera"
        );
        app.world().get::<DepthPrepass>(e).is_some()
    }

    /// Off unless asked for; asked for, every view with the player's look
    /// has it, and no other camera does.
    #[test]
    fn the_prepass_is_an_experiment() {
        assert!(!view(false));
        assert!(view(true));
    }
}
