//! In-world text panels: any entity with a [`ScreenText`] gets a quad
//! showing its text, drawn by a UI tree rendered into a texture. The
//! camera and UI that draw it exist only while it redraws (a few frames
//! after its text changes); the texture keeps the picture. So dozens of
//! panels cost nothing while they're idle, and the renderer never carries
//! dozens of cameras. The bench display, notes on the memorial board.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::render::legibility::TextSource;

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
    /// The panel's material, and whether it shows the picture as light.
    material: Handle<StandardMaterial>,
    emissive: bool,
    /// Counts redraws: a picture read back from an older one is dropped.
    generation: u32,
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
            // The wrist's and the radio's few lines of capitals as big as
            // their screens allow.
            ScreenStyle::Lcd => 6.0,
            // Paper: a short sign's words as big as a ten-line page's.
            ScreenStyle::Paper => 10.0,
            ScreenStyle::Crt => 14.0,
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

fn build_screens(
    mut commands: Commands,
    screens: Query<(Entity, &ScreenText, Option<&TextSource>, Option<&Name>), Added<ScreenText>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drawings: ResMut<Drawings>,
) {
    for (e, screen, source, name) in &screens {
        if source.is_none() {
            // Every panel says what it is, so the legibility gauges can
            // measure it from where it's read (`render::legibility`).
            error!(
                "text panel {} has no TextSource: {:?}",
                name.map_or("(unnamed)".into(), |n| n.to_string()),
                screen.text.lines().next().unwrap_or("")
            );
        }
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
            | TextureUsages::COPY_SRC
            | TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        let material = materials.add(StandardMaterial {
            base_color: if emissive { Color::BLACK } else { Color::WHITE },
            base_color_texture: (!emissive).then(|| handle.clone()),
            emissive: if emissive {
                LinearRgba::rgb(2.5, 2.5, 2.5)
            } else {
                LinearRgba::BLACK
            },
            emissive_texture: emissive.then(|| handle.clone()),
            // Matte: a glossy screen catches a lamp's highlight right over
            // its words (the bench display under the canopy light did).
            perceptual_roughness: if emissive { 0.7 } else { 0.95 },
            ..default()
        });
        let mut view = ScreenView {
            drawing: None,
            image: handle.clone(),
            refresh: FIRST_FRAMES,
            px: Vec2::new(w as f32, h as f32),
            font,
            bg,
            fg,
            material: material.clone(),
            emissive,
            generation: 0,
        };
        let d = start_drawing(&mut commands, &view, &screen.text);
        view.drawing = Some(d);
        drawings.0.insert(e, d);
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (e, screen, mut view) in &mut screens {
        match view.drawing {
            Some(d) => {
                if let Ok((mut t, mut f)) = texts.get_mut(d.text) {
                    if t.0 != screen.text {
                        view.refresh = view.refresh.max(REFRESH_FRAMES);
                        view.generation += 1;
                        show_target(&mut materials, &mut images, &view);
                        t.0 = screen.text.clone();
                        let size = FontSize::Px(fit_font(&screen.text, view.px, view.font));
                        if f.font_size != size {
                            f.font_size = size;
                        }
                    }
                }
            }
            None => {
                view.generation += 1;
                show_target(&mut materials, &mut images, &view);
                let d = start_drawing(&mut commands, &view, &screen.text);
                view.drawing = Some(d);
                view.refresh = REFRESH_FRAMES;
                drawings.0.insert(e, d);
            }
        }
    }
}

/// While a panel redraws, its material shows the render target itself.
fn show_target(
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    view: &ScreenView,
) {
    let Some(mut m) = materials.get_mut(&view.material) else {
        return;
    };
    let slot = if view.emissive {
        &mut m.emissive_texture
    } else {
        &mut m.base_color_texture
    };
    if slot.as_ref() != Some(&view.image) {
        if let Some(old) = slot.replace(view.image.clone()) {
            images.remove(&old);
        }
    }
}

/// A panel that has finished drawing lets its camera and UI go, and its
/// picture is read back to be shown with mipmaps.
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
                read_back(&mut commands, e, &view);
            }
        }
    }
}

/// Reads a panel's finished picture back and shows it with a full mip
/// chain and anisotropic filtering instead of the bare render target: a
/// texture with more texels than the eye has pixels for it (a sign seen
/// from up the track, a note at a slant) otherwise sparkles as the head
/// moves.
fn read_back(commands: &mut Commands, panel: Entity, view: &ScreenView) {
    let (w, h) = (view.px.x as u32, view.px.y as u32);
    let (material, emissive, target) = (view.material.clone(), view.emissive, view.image.clone());
    let generation = view.generation;
    commands
        .spawn(Readback::texture(view.image.clone()))
        .observe(
            move |done: On<ReadbackComplete>,
                  mut images: ResMut<Assets<Image>>,
                  mut materials: ResMut<Assets<StandardMaterial>>,
                  views: Query<&ScreenView>,
                  mut commands: Commands| {
                commands.entity(done.entity).despawn();
                let current = views
                    .get(panel)
                    .is_ok_and(|v| v.generation == generation && v.drawing.is_none());
                if !current {
                    return;
                }
                // Rows come padded to 256 bytes.
                let stride = (w as usize * 4).div_ceil(256) * 256;
                if done.data.len() < stride * (h as usize - 1) + w as usize * 4 {
                    return;
                }
                let shown = images.add(mipped(&done.data, w, h, stride));
                // A new texture, pointed to by the material (swapping an
                // image's contents under a material doesn't reach its bind
                // group); the one it showed before goes, unless it's the
                // render target.
                let Some(mut m) = materials.get_mut(&material) else {
                    return;
                };
                let slot = if emissive {
                    &mut m.emissive_texture
                } else {
                    &mut m.base_color_texture
                };
                if let Some(old) = slot.replace(shown) {
                    if old != target {
                        images.remove(&old);
                    }
                }
            },
        );
}

/// A panel's picture (`Bgra8UnormSrgb`, rows `stride` bytes apart) with a
/// full mip chain, averaged in linear light so letters thin out evenly
/// with distance, and filtered anisotropically so they stay sharp at a
/// slant.
pub fn mipped(data: &[u8], w: u32, h: u32, stride: usize) -> Image {
    let lin: Vec<f32> = (0..=255u8)
        .map(|v| LinearRgba::from(Srgba::rgb_u8(v, 0, 0)).red)
        .collect();
    let srgb = |l: f32| (Srgba::from(LinearRgba::rgb(l, 0.0, 0.0)).red * 255.0 + 0.5) as u8;
    let (w, h) = (w as usize, h as usize);
    let mut out = Vec::with_capacity(w * h * 6);
    let mut level: Vec<f32> = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &data[y * stride..y * stride + w * 4];
        out.extend_from_slice(row);
        for (i, &v) in row.iter().enumerate() {
            level.push(if i % 4 == 3 {
                v as f32 / 255.0
            } else {
                lin[v as usize]
            });
        }
    }
    let (mut lw, mut lh) = (w, h);
    let mut levels = 1;
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let mut next = vec![0.0f32; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(lw - 1);
                        let sy = (y * 2 + dy).min(lh - 1);
                        sum += level[(sy * lw + sx) * 4 + c];
                    }
                    next[(y * nw + x) * 4 + c] = sum * 0.25;
                }
            }
        }
        out.extend(next.iter().enumerate().map(|(i, &v)| {
            if i % 4 == 3 {
                (v * 255.0 + 0.5) as u8
            } else {
                srgb(v)
            }
        }));
        level = next;
        (lw, lh) = (nw, nh);
        levels += 1;
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width: w as u32,
            height: h as u32,
            ..default()
        },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(out);
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(16);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
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
