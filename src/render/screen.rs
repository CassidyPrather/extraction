//! In-world text panels: any entity with a [`ScreenText`] gets a quad
//! showing its text, drawn by a UI tree rendered into a texture (one small
//! 2D camera per panel). The bench display, notes on the memorial board.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::raid::bench::{ScreenStyle, ScreenText};

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (build_screens, update_screens).chain());
    }
}

/// Pixels per metre on a panel.
const PX_PER_M: f32 = 1000.0;

#[derive(Component)]
struct ScreenView {
    text: Entity,
    /// Panel pixels and the style's largest font.
    px: Vec2,
    font: f32,
}

/// The largest font (px) up to `max` at which `text` fits a `px` panel
/// without wrapping: bounded by line count and by the longest line.
fn fit_font(text: &str, px: Vec2, max: f32) -> f32 {
    let lines = text.lines().count().max(1) as f32;
    let widest = text
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    // Rough metrics for the default font: line height ~1.2em, glyphs
    // ~0.6em wide (it is monospaced); padding is 0.8em a side.
    let by_height = px.y / (lines * 1.2 + 1.6);
    let by_width = px.x / (widest * 0.64 + 1.6);
    max.min(by_height).min(by_width).max(6.0)
}

fn build_screens(
    mut commands: Commands,
    screens: Query<(Entity, &ScreenText), Added<ScreenText>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, screen) in &screens {
        // Small panels (a wrist readout) still need enough pixels for text.
        let ppm = PX_PER_M.max(420.0 / screen.size.x.max(1e-3));
        let w = (screen.size.x * ppm).round().max(16.0) as u32;
        let h = (screen.size.y * ppm).round().max(16.0) as u32;
        let (bg, fg, emissive, font) = match screen.style {
            ScreenStyle::Lcd => (
                Color::srgb(0.03, 0.025, 0.01),
                Color::srgb(1.0, 0.68, 0.22),
                true,
                h as f32 / 16.0,
            ),
            ScreenStyle::Paper => (
                Color::srgb(0.78, 0.75, 0.66),
                Color::srgb(0.16, 0.15, 0.17),
                false,
                h as f32 / 14.0,
            ),
        };
        let max_font = font;
        let font = fit_font(&screen.text, Vec2::new(w as f32, h as f32), max_font);
        let mut image = Image::new_fill(
            Extent3d {
                width: w,
                height: h,
                ..default()
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        let camera = commands
            .spawn((
                Camera2d,
                Camera {
                    order: -10,
                    clear_color: ClearColorConfig::Custom(bg),
                    ..default()
                },
                RenderTarget::Image(handle.clone().into()),
            ))
            .id();
        let mut text = Entity::PLACEHOLDER;
        commands
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    padding: UiRect::all(px(font * 0.8)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                UiTargetCamera(camera),
            ))
            .with_children(|ui| {
                text = ui
                    .spawn((
                        Text::new(screen.text.clone()),
                        TextFont {
                            font_size: FontSize::Px(font),
                            ..default()
                        },
                        TextColor(fg),
                    ))
                    .id();
            });
        let material = materials.add(StandardMaterial {
            base_color: if emissive { Color::BLACK } else { Color::WHITE },
            base_color_texture: (!emissive).then(|| handle.clone()),
            emissive: if emissive {
                LinearRgba::rgb(2.5, 2.5, 2.5)
            } else {
                LinearRgba::BLACK
            },
            emissive_texture: emissive.then(|| handle.clone()),
            perceptual_roughness: if emissive { 0.15 } else { 0.95 },
            ..default()
        });
        commands
            .entity(e)
            .insert(ScreenView {
                text,
                px: Vec2::new(w as f32, h as f32),
                font: max_font,
            })
            .with_child((
                Mesh3d(meshes.add(Rectangle::new(screen.size.x, screen.size.y))),
                MeshMaterial3d(material),
                NotShadowCaster,
            ));
    }
}

fn update_screens(
    screens: Query<(&ScreenText, &ScreenView), Changed<ScreenText>>,
    mut texts: Query<(&mut Text, &mut TextFont)>,
) {
    for (screen, view) in &screens {
        if let Ok((mut t, mut f)) = texts.get_mut(view.text) {
            if t.0 != screen.text {
                t.0 = screen.text.clone();
                let size = FontSize::Px(fit_font(&screen.text, view.px, view.font));
                if f.font_size != size {
                    f.font_size = size;
                }
            }
        }
    }
}
