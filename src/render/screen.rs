//! In-world text panels: any entity with a [`ScreenText`] gets a quad
//! showing its text, drawn by a UI tree rendered into a texture. The
//! camera and UI that draw it exist only while it redraws (a few frames
//! after its text changes); the texture keeps the picture. So dozens of
//! panels cost nothing while they're idle, and the renderer never carries
//! dozens of cameras. The bench display, notes on the memorial board.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::raid::bench::{ScreenStyle, ScreenText};

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Drawings>().add_systems(
            Update,
            (build_screens, update_screens, idle_screens, drop_screens).chain(),
        );
    }
}

/// Frames a new panel's camera renders (the font may still be loading),
/// and frames after its text changes (laying out, then drawing).
const FIRST_FRAMES: u8 = 30;
const REFRESH_FRAMES: u8 = 3;

/// Pixels per metre on a panel.
const PX_PER_M: f32 = 1000.0;

#[derive(Component)]
struct ScreenView {
    /// The camera and UI tree drawing the panel, while it redraws.
    drawing: Option<Drawing>,
    image: Handle<Image>,
    /// Frames left to draw; then the camera and UI go and the texture
    /// keeps its picture.
    refresh: u8,
    /// Panel pixels, the style's largest font, its colours.
    px: Vec2,
    font: f32,
    bg: Color,
    fg: Color,
}

#[derive(Clone, Copy)]
struct Drawing {
    camera: Entity,
    ui: Entity,
    text: Entity,
}

/// Panels' drawing entities, so a panel that goes away takes them along.
#[derive(Resource, Default)]
struct Drawings(std::collections::HashMap<Entity, Drawing>);

/// Spawns the camera and UI tree that draw `text` into `view`'s texture.
fn start_drawing(commands: &mut Commands, view: &ScreenView, text: &str) -> Drawing {
    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -10,
                clear_color: ClearColorConfig::Custom(view.bg),
                ..default()
            },
            RenderTarget::Image(view.image.clone().into()),
        ))
        .id();
    let font = fit_font(text, view.px, view.font);
    let mut text_e = Entity::PLACEHOLDER;
    let ui = commands
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
            text_e = ui
                .spawn((
                    Text::new(text.to_owned()),
                    TextFont {
                        font_size: FontSize::Px(font),
                        ..default()
                    },
                    TextColor(view.fg),
                ))
                .id();
        })
        .id();
    Drawing {
        camera,
        ui,
        text: text_e,
    }
}

fn stop_drawing(commands: &mut Commands, d: Drawing) {
    commands.entity(d.ui).despawn();
    commands.entity(d.camera).despawn();
}

/// Word-wraps `text` into at most `max_lines` lines of `columns`
/// characters, ending in "..." if it doesn't all fit.
pub fn wrap(text: &str, columns: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let fits = cur.is_empty() || cur.chars().count() + 1 + word.chars().count() <= columns;
        if !fits {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        // A word longer than the line is cut.
        cur.extend(word.chars().take(columns));
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = &mut lines[max_lines - 1];
        while last.chars().count() + 3 > columns {
            last.pop();
        }
        *last = format!("{}...", last.trim_end());
    }
    lines
}

/// Word-wraps every line of `text` to `columns`.
pub fn wrap_lines(text: &str, columns: usize) -> String {
    text.lines()
        .flat_map(|l| {
            if l.trim().is_empty() {
                vec![String::new()]
            } else {
                wrap(l, columns, usize::MAX)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
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

/// A panel's pixels per metre, its size in pixels, and its style's
/// largest font (px).
fn panel(screen: &ScreenText) -> (f32, u32, u32, f32) {
    // Small panels (a wrist readout) still need enough pixels for text.
    let ppm = PX_PER_M.max(420.0 / screen.size.x.max(1e-3));
    let w = (screen.size.x * ppm).round().max(16.0) as u32;
    let h = (screen.size.y * ppm).round().max(16.0) as u32;
    let font = h as f32
        / match screen.style {
            ScreenStyle::Lcd => 16.0,
            ScreenStyle::Paper | ScreenStyle::Crt => 14.0,
            ScreenStyle::Card => 4.5,
        };
    (ppm, w, h, font)
}

/// How tall the panel's letters come out (the em, in metres) once its text
/// is fitted: what decides whether it can be read, and from how far.
pub fn em_metres(screen: &ScreenText) -> f32 {
    let (ppm, w, h, font) = panel(screen);
    fit_font(&screen.text, Vec2::new(w as f32, h as f32), font) / ppm
}

/// `text` wrapped to whichever line length gives it the biggest letters on
/// a panel of this size and style (a long sign wraps wide, a short one
/// narrow).
pub fn wrap_to_fit(text: &str, size: Vec2, style: ScreenStyle) -> String {
    let mut best = (0.0, text.to_string());
    for columns in 12..=60 {
        let wrapped = wrap_lines(text, columns);
        let em = em_metres(&ScreenText {
            text: wrapped.clone(),
            size,
            style,
        });
        // Only a real gain beats a narrower wrap (shorter lines read easier).
        if em > best.0 + 1e-4 {
            best = (em, wrapped);
        }
    }
    best.1
}

/// The smallest letters (em, as an angle at the eye) a headset shows
/// legibly. A Beyond 2 at the auto render scale has about 20 pixels a
/// degree; 0.6 degrees is a 12-pixel em.
pub const MIN_EM_DEG: f32 = 0.6;

/// Where a panel is read from (m): a signpost from a few steps off, your
/// wrist from your face, everything else (cards, notes, screens) from
/// about a metre.
pub fn read_from(screen: &ScreenText) -> f32 {
    if screen.style == ScreenStyle::Paper && screen.size.x >= 0.7 {
        3.0
    } else if screen.size.x < 0.1 {
        0.45
    } else {
        1.0
    }
}

fn build_screens(
    mut commands: Commands,
    screens: Query<(Entity, &ScreenText), Added<ScreenText>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drawings: ResMut<Drawings>,
) {
    for (e, screen) in &screens {
        let (_, w, h, font) = panel(screen);
        let (bg, fg, emissive) = match screen.style {
            ScreenStyle::Lcd => (
                Color::srgb(0.03, 0.025, 0.01),
                Color::srgb(1.0, 0.68, 0.22),
                true,
            ),
            ScreenStyle::Paper => (
                Color::srgb(0.78, 0.75, 0.66),
                Color::srgb(0.16, 0.15, 0.17),
                false,
            ),
            ScreenStyle::Card => (
                Color::srgb(0.8, 0.77, 0.66),
                Color::srgb(0.14, 0.13, 0.15),
                false,
            ),
            ScreenStyle::Crt => (
                Color::srgb(0.01, 0.03, 0.015),
                Color::srgb(0.35, 1.0, 0.45),
                true,
            ),
        };
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
        let mut view = ScreenView {
            drawing: None,
            image: handle.clone(),
            refresh: FIRST_FRAMES,
            px: Vec2::new(w as f32, h as f32),
            font,
            bg,
            fg,
        };
        let d = start_drawing(&mut commands, &view, &screen.text);
        view.drawing = Some(d);
        drawings.0.insert(e, d);
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
        commands.entity(e).insert(view).with_child((
            Mesh3d(meshes.add(Rectangle::new(screen.size.x, screen.size.y))),
            MeshMaterial3d(material),
            NotShadowCaster,
        ));
    }
}

fn update_screens(
    mut commands: Commands,
    mut screens: Query<(Entity, &ScreenText, &mut ScreenView), Changed<ScreenText>>,
    mut texts: Query<(&mut Text, &mut TextFont)>,
    mut drawings: ResMut<Drawings>,
) {
    for (e, screen, mut view) in &mut screens {
        match view.drawing {
            Some(d) => {
                if let Ok((mut t, mut f)) = texts.get_mut(d.text) {
                    if t.0 != screen.text {
                        view.refresh = view.refresh.max(REFRESH_FRAMES);
                        t.0 = screen.text.clone();
                        let size = FontSize::Px(fit_font(&screen.text, view.px, view.font));
                        if f.font_size != size {
                            f.font_size = size;
                        }
                    }
                }
            }
            None => {
                let d = start_drawing(&mut commands, &view, &screen.text);
                view.drawing = Some(d);
                view.refresh = REFRESH_FRAMES;
                drawings.0.insert(e, d);
            }
        }
    }
}

/// A panel that has finished drawing lets its camera and UI go: its
/// texture keeps the picture.
fn idle_screens(
    mut commands: Commands,
    mut screens: Query<(Entity, &mut ScreenView)>,
    mut drawings: ResMut<Drawings>,
) {
    for (e, mut view) in &mut screens {
        if view.drawing.is_none() {
            continue;
        }
        view.refresh = view.refresh.saturating_sub(1);
        if view.refresh == 0 {
            if let Some(d) = view.drawing.take() {
                stop_drawing(&mut commands, d);
                drawings.0.remove(&e);
            }
        }
    }
}

/// A panel that went away (despawned, or its text taken off) takes its
/// drawing along.
fn drop_screens(
    mut commands: Commands,
    mut removed: RemovedComponents<ScreenText>,
    mut drawings: ResMut<Drawings>,
) {
    for e in removed.read() {
        if let Some(d) = drawings.0.remove(&e) {
            stop_drawing(&mut commands, d);
        }
    }
}
