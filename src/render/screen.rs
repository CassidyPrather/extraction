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
use bevy::render::gpu_readback::ReadbackComplete;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};

use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::render::legibility::TextSource;
use crate::render::readback::{ReadOnce, read_texture, taken};

pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Drawings>().add_systems(
            Update,
            (
                build_screens,
                update_screens,
                idle_screens,
                drop_screens,
                show_mipped,
            )
                .chain(),
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
    /// Under the words, for paper and cards: paper, not a flat sheet.
    paper: Option<Handle<Image>>,
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
    let mut root = commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(font * 0.8)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        UiTargetCamera(camera),
    ));
    // The sheet under the words: the whole panel, padding and all, drawn
    // first (as the root's own image it came out a box inset in it).
    if let Some(paper) = &view.paper {
        root.with_child((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ImageNode::new(paper.clone()).with_mode(NodeImageMode::Stretch),
        ));
    }
    let ui = root
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
    mut paper: Local<[Option<Handle<Image>>; 2]>,
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
            // Near-black ink on pale paper: lit only by the scene, paper
            // under a grey sky came out grey on grey.
            ScreenStyle::Paper => (
                Color::srgb(0.84, 0.81, 0.72),
                Color::srgb(0.07, 0.065, 0.08),
                false,
            ),
            ScreenStyle::Card => (
                Color::srgb(0.85, 0.81, 0.69),
                Color::srgb(0.06, 0.055, 0.07),
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
            // A screen's anti-glare coat: a glossy one caught the canopy
            // lamp's highlight right over the bench display's words, and a
            // rough one spread it into a wash over its black.
            perceptual_roughness: if emissive { 0.6 } else { 0.95 },
            reflectance: if emissive { 0.05 } else { 0.5 },
            ..default()
        });
        // Paper and cards each their own sheet, in their own colour.
        let sheet = match screen.style {
            ScreenStyle::Paper => Some(0),
            ScreenStyle::Card => Some(1),
            _ => None,
        };
        let mut view = ScreenView {
            paper: sheet.map(|i| {
                paper[i]
                    .get_or_insert_with(|| images.add(paper_image(bg)))
                    .clone()
            }),
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
/// moves. The mips are built off the main thread ([`Mipping`]): built on
/// it, a goal tick's few panels finishing together cost a 50-67 ms frame
/// (`docs/playtests/2026-10-28`).
fn read_back(commands: &mut Commands, panel: Entity, view: &ScreenView) {
    let (w, h) = (view.px.x as u32, view.px.y as u32);
    let (material, emissive, target) = (view.material.clone(), view.emissive, view.image.clone());
    let generation = view.generation;
    read_texture(commands, view.image.clone()).observe(
        move |mut done: On<ReadbackComplete>,
              once: Query<&ReadOnce>,
              views: Query<&ScreenView>,
              mut commands: Commands| {
            if !taken(&done, &once, &mut commands) {
                return;
            }
            if !current(&views, panel, generation) {
                return;
            }
            // Rows come padded to 256 bytes.
            let stride = (w as usize * 4).div_ceil(256) * 256;
            if done.data.len() < stride * (h as usize - 1) + w as usize * 4 {
                return;
            }
            // Taken once (above), so its bytes can be moved, not copied.
            let data = std::mem::take(&mut done.event_mut().data);
            let task =
                AsyncComputeTaskPool::get().spawn(async move { mipped(&data, w, h, stride) });
            commands.spawn(Mipping {
                task,
                panel,
                generation,
                material: material.clone(),
                emissive,
                target: target.clone(),
            });
        },
    );
}

/// Whether `panel` is still showing redraw `generation`, finished.
fn current(views: &Query<&ScreenView>, panel: Entity, generation: u32) -> bool {
    views
        .get(panel)
        .is_ok_and(|v| v.generation == generation && v.drawing.is_none())
}

/// A panel's mips being built, to be shown when they're done.
#[derive(Component)]
struct Mipping {
    task: Task<Image>,
    panel: Entity,
    generation: u32,
    material: Handle<StandardMaterial>,
    emissive: bool,
    target: Handle<Image>,
}

/// Panels whose mips are built show them: a new texture, pointed to by
/// the material (swapping an image's contents under a material doesn't
/// reach its bind group); the one it showed before goes, unless it's the
/// render target. A picture from an older redraw is dropped.
fn show_mipped(
    mut commands: Commands,
    mut mipping: Query<(Entity, &mut Mipping)>,
    views: Query<&ScreenView>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, mut m) in &mut mipping {
        let Some(image) = check_ready(&mut m.task) else {
            continue;
        };
        commands.entity(e).despawn();
        if !current(&views, m.panel, m.generation) {
            continue;
        }
        let shown = images.add(image);
        let Some(mut mat) = materials.get_mut(&m.material) else {
            continue;
        };
        let slot = if m.emissive {
            &mut mat.emissive_texture
        } else {
            &mut mat.base_color_texture
        };
        if let Some(old) = slot.replace(shown) {
            if old != m.target {
                images.remove(&old);
            }
        }
    }
}

/// A panel's picture (`Bgra8UnormSrgb`, rows `stride` bytes apart) with a
/// full mip chain, averaged in linear light so letters thin out evenly
/// with distance, and filtered anisotropically so they stay sharp at a
/// slant.
pub fn mipped(data: &[u8], w: u32, h: u32, stride: usize) -> Image {
    #[cfg(test)]
    BUILT_HERE.with(|n| n.set(n.get() + 1));
    let lin: Vec<f32> = (0..=255u8)
        .map(|v| LinearRgba::from(Srgba::rgb_u8(v, 0, 0)).red)
        .collect();
    let srgb = |l: f32| SRGB[(l.clamp(0.0, 1.0) * (SRGB_STEPS - 1) as f32 + 0.5) as usize];
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

// How many mip chains were built on this thread (the tests: none on the
// main one).
#[cfg(test)]
thread_local! {
    static BUILT_HERE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// A sheet of paper in `bg`'s colour, not a flat one (playtest 2026-10-28,
/// F4: a note read as "a solid white background"): a little darker on
/// the whole, mottled, with fibres, and darker toward its edges, as a
/// handled sheet is. Seeded, so every sheet is the same paper.
pub fn paper_image(bg: Color) -> Image {
    const N: u32 = 256;
    let base = bg.to_srgba();
    let mut rng = fastrand::Rng::with_seed(28);
    // Mottling: a coarse grid of shades, smoothly blended.
    const G: usize = 9;
    let coarse: Vec<f32> = (0..G * G).map(|_| rng.f32() * 2.0 - 1.0).collect();
    let at = |gx: usize, gy: usize| coarse[gy.min(G - 1) * G + gx.min(G - 1)];
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (u, v) = (x as f32 / N as f32, y as f32 / N as f32);
            let (fx, fy) = (u * (G - 1) as f32, v * (G - 1) as f32);
            let (ix, iy) = (fx as usize, fy as usize);
            let (tx, ty) = (fx.fract(), fy.fract());
            let mottle = at(ix, iy) * (1.0 - tx) * (1.0 - ty)
                + at(ix + 1, iy) * tx * (1.0 - ty)
                + at(ix, iy + 1) * (1.0 - tx) * ty
                + at(ix + 1, iy + 1) * tx * ty;
            let fibre = rng.f32() - 0.5;
            // Darker toward the edges: the last tenth of the sheet.
            let edge = u.min(1.0 - u).min(v.min(1.0 - v));
            let rim = (1.0 - (edge / 0.1).min(1.0)).powi(2);
            let k = 0.94 + 0.03 * mottle + 0.03 * fibre - 0.12 * rim;
            let px = |c: f32| ((c * k).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            data.extend_from_slice(&[px(base.red), px(base.green), px(base.blue), 255]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Linear light to sRGB bytes, in steps fine enough that every byte is
/// reached (the darkest sRGB steps are 1/3300 apart in linear light).
const SRGB_STEPS: usize = 8192;
static SRGB: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| {
    (0..SRGB_STEPS)
        .map(|i| {
            let l = i as f32 / (SRGB_STEPS - 1) as f32;
            (Srgba::from(LinearRgba::rgb(l, 0.0, 0.0)).red * 255.0 + 0.5) as u8
        })
        .collect()
});

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::gpu_readback::Readback;

    /// A panel waiting for its picture, `w` by `h`, and its readback.
    fn waiting(app: &mut App, w: u32, h: u32) -> (Entity, Entity, Handle<StandardMaterial>) {
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(Image::default());
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                ..default()
            });
        let view = ScreenView {
            drawing: None,
            image,
            refresh: 0,
            px: Vec2::new(w as f32, h as f32),
            font: 20.0,
            bg: Color::BLACK,
            fg: Color::WHITE,
            material: material.clone(),
            emissive: false,
            generation: 0,
            paper: None,
        };
        let before: Vec<Entity> = world
            .query_filtered::<Entity, With<Readback>>()
            .iter(world)
            .collect();
        let mut commands = world.commands();
        let panel = commands.spawn_empty().id();
        read_back(&mut commands, panel, &view);
        world.flush();
        world.entity_mut(panel).insert(view);
        let readback = world
            .query_filtered::<Entity, With<Readback>>()
            .iter(world)
            .find(|e| !before.contains(e))
            .expect("a readback");
        (panel, readback, material)
    }

    /// Paper is paper: not one flat colour but mottled and fibred, a touch
    /// darker than its colour on the whole, and darkest at its edges.
    #[test]
    fn paper_is_not_a_flat_sheet() {
        let bg = Color::srgb(0.84, 0.81, 0.72);
        let img = paper_image(bg);
        let n = img.width() as usize;
        let data = img.data.as_ref().unwrap();
        let red = |x: usize, y: usize| data[(y * n + x) * 4] as f32 / 255.0;
        let all: Vec<f32> = (0..n * n).map(|i| red(i % n, i / n)).collect();
        let mean = all.iter().sum::<f32>() / all.len() as f32;
        let sd = (all.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / all.len() as f32).sqrt();
        assert!(sd > 0.01, "flat: {sd}");
        assert!(mean < 0.84 && mean > 0.7, "{mean}");
        let rim: f32 = (0..n).map(|i| red(i, 0) + red(0, i)).sum::<f32>() / (2 * n) as f32;
        let mid: f32 = (n / 3..2 * n / 3).map(|i| red(i, n / 2)).sum::<f32>() / (n / 3) as f32;
        assert!(rim < mid - 0.05, "edges {rim} against the middle {mid}");
    }

    /// What a panel's readback hands back: rows padded to 256 bytes.
    fn picture(w: u32, h: u32) -> Vec<u8> {
        let stride = (w as usize * 4).div_ceil(256) * 256;
        (0..stride * h as usize)
            .map(|i| (i * 31 % 251) as u8)
            .collect()
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Update, show_mipped);
        app
    }

    /// A readback repeats every frame until it's packed up, and triggers in
    /// one frame aren't flushed between: its picture arriving twice is
    /// taken once (no second despawn, no second picture).
    #[test]
    fn a_picture_that_arrives_twice_is_taken_once() {
        let mut app = app();
        // A command on an entity that's gone fails the test, as it warns in
        // the game.
        app.set_error_handler(bevy::ecs::error::panic);
        let (_, readback, _) = waiting(&mut app, 64, 32);
        let before = app.world().resource::<Assets<Image>>().len();
        let warned = crate::render::readback::warnings(|| {
            let world = app.world_mut();
            for _ in 0..2 {
                world.trigger(ReadbackComplete {
                    entity: readback,
                    data: picture(64, 32),
                });
            }
            world.flush();
            let t = std::time::Instant::now();
            while app.world().resource::<Assets<Image>>().len() < before + 1 {
                assert!(t.elapsed().as_secs() < 10, "the picture never showed");
                app.update();
            }
            for _ in 0..20 {
                app.update();
            }
        });
        assert_eq!(warned, 0, "warnings (a second despawn warns)");
        let world = app.world_mut();
        assert!(world.get_entity(readback).is_err(), "packed up");
        assert_eq!(
            world.resource::<Assets<Image>>().len(),
            before + 1,
            "one picture made"
        );
    }

    /// The panels a goal tick redraws (signs, the tailgate's checklist, the
    /// wrist) finishing in the same frame cost that frame next to nothing:
    /// their mips are built off the main thread. Counted, not timed (a
    /// clock flakes on a loaded machine): built on it, six cost 80.7 ms.
    #[test]
    fn a_burst_of_redraws_costs_no_frame() {
        let mut app = app();
        let sizes = [
            (1100, 320),
            (1100, 320),
            (420, 258),
            (420, 581),
            (600, 400),
            (1100, 320),
        ];
        let waiting: Vec<_> = sizes
            .iter()
            .map(|&(w, h)| (waiting(&mut app, w, h), picture(w, h)))
            .collect();
        let world = app.world_mut();
        let done: Vec<_> = waiting
            .iter()
            .map(|((_, readback, _), data)| ReadbackComplete {
                entity: *readback,
                data: data.clone(),
            })
            .collect();
        let before = BUILT_HERE.with(|n| n.get());
        for d in done {
            world.trigger(d);
        }
        world.flush();
        let here = BUILT_HERE.with(|n| n.get()) - before;
        assert_eq!(here, 0, "mip chains built on the main thread");
        // And they all show, mipped, a few frames on.
        let shows = |app: &mut App| {
            let world = app.world();
            waiting.iter().all(|((_, _, material), _)| {
                let m = world.resource::<Assets<StandardMaterial>>();
                let images = world.resource::<Assets<Image>>();
                m.get(material)
                    .and_then(|m| m.base_color_texture.as_ref())
                    .and_then(|h| images.get(h))
                    .is_some_and(|i| i.texture_descriptor.mip_level_count > 1)
            })
        };
        let t = std::time::Instant::now();
        while !shows(&mut app) {
            assert!(t.elapsed().as_secs() < 10, "the pictures never showed");
            app.update();
        }
    }
}
