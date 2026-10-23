//! Each valley's look: the colour grade every player-view camera gets
//! (see `docs/ART_DIRECTION.md`). The grade runs inside the tonemapping
//! pass the cameras already have, so it costs nothing per pixel beyond
//! what AgX does anyway.
//!
//! `EXTRACTION_LOOK="sat=1.1,contrast=1.05,..."` overrides fields at
//! launch, for trying a look in screenshots without a rebuild.
//!
//! The eyes adapt (`adapt_eyes`): as the light fails (dusk, a room) the
//! exposure opens up, slowly, like an eye and not a camera: never all
//! the way (dusk and a dark room still read darker than day), fast back
//! the other way.

use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection};

/// A valley's grade. Neutral is all zeros and ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Stops on top of the cameras' exposure.
    pub exposure: f32,
    /// Warmer (+) or cooler (-), CIE x.
    pub temperature: f32,
    /// Magenta (+) or green (-), CIE y.
    pub tint: f32,
    /// Saturation after tonemapping (AgX desaturates on its way out).
    pub saturation: f32,
    /// Contrast round the midtones.
    pub contrast: f32,
    /// Shadows' saturation: lower keeps the murk from going muddy.
    pub shadow_saturation: f32,
    /// Lifts the blacks a little (VR: nothing too dark to read).
    pub shadow_lift: f32,
    /// Highlights' gain: under 1 holds the sky back.
    pub highlight_gain: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            shadow_saturation: 1.0,
            shadow_lift: 0.0,
            highlight_gain: 1.0,
        }
    }
}

impl Look {
    /// The look for a map, by its name.
    pub fn for_map(name: &str) -> Self {
        // AgX greys the overcast out on its way to the screen; every valley
        // gets some saturation and a little contrast back. Temperature only
        // nudges: the air's own colour is the fog's and the sky's job (a
        // grade strong enough to tint the valley read as a photo filter).
        // The blacks lift a little: under the spruce the frame went near
        // black (a hole, in a headset), and the art direction wants the
        // forest floor's texture readable.
        let base = Self {
            saturation: 1.25,
            contrast: 1.12,
            shadow_lift: 0.003,
            ..default()
        };
        let mut look = match name {
            // High, rocky and cold: cooler, a touch harder, less green.
            "Kettle Pass" => Self {
                temperature: -0.008,
                saturation: 1.15,
                contrast: 1.16,
                ..base
            },
            // Burnt: warm and drained, the ash greying everything out.
            "Cinder Works" => Self {
                temperature: 0.012,
                saturation: 1.0,
                contrast: 1.1,
                ..base
            },
            // The drained reservoir: cold, wet and blue-grey, the silt
            // draining the colour out.
            "Sallow Reservoir" => Self {
                temperature: -0.012,
                saturation: 1.05,
                contrast: 1.1,
                ..base
            },
            // High, bare and snowy: cold blue-white, a touch harder,
            // the colour bled out of everything but the rust.
            "Rime Fell" => Self {
                temperature: -0.014,
                saturation: 1.05,
                contrast: 1.14,
                ..base
            },
            // The practice ground: a shade brighter and kinder, to read.
            "Training Yard" => Self {
                exposure: 0.15,
                ..base
            },
            _ => base,
        };
        if let Ok(s) = std::env::var("EXTRACTION_LOOK") {
            look.apply_overrides(&s);
        }
        look
    }

    fn apply_overrides(&mut self, s: &str) {
        for kv in s.split(',') {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            let Ok(v) = v.trim().parse::<f32>() else {
                continue;
            };
            match k.trim() {
                "exposure" => self.exposure = v,
                "temp" => self.temperature = v,
                "tint" => self.tint = v,
                "sat" => self.saturation = v,
                "contrast" => self.contrast = v,
                "shadow_sat" => self.shadow_saturation = v,
                "lift" => self.shadow_lift = v,
                "gain" => self.highlight_gain = v,
                other => warn!("EXTRACTION_LOOK: unknown key {other}"),
            }
        }
    }

    pub fn grading(&self) -> ColorGrading {
        // Bevy's own contrast pivots at 0.5 in linear light, before the
        // tonemapper: this scene lives well under that, so it crushed
        // everything to black. Contrast here is a power curve round
        // mid-grey instead, built from the CDL's gain and gamma:
        // 0.18 * (c / 0.18)^k = (c * gain)^k with gain = 0.18^((1 - k) / k).
        let k = self.contrast.max(0.1);
        let section = |saturation: f32, lift: f32| ColorGradingSection {
            saturation,
            gamma: 1.0 / k,
            gain: 0.18f32.powf((1.0 - k) / k),
            lift,
            ..default()
        };
        let mut highlights = section(1.0, 0.0);
        highlights.gain *= self.highlight_gain;
        ColorGrading {
            global: ColorGradingGlobal {
                exposure: self.exposure,
                temperature: self.temperature,
                tint: self.tint,
                post_saturation: self.saturation,
                ..default()
            },
            shadows: section(self.shadow_saturation, self.shadow_lift),
            midtones: section(1.0, 0.0),
            highlights,
        }
    }
}

/// How far a room's walls cut the sky's light (`dim_indoors`).
pub use crate::world::weather::INTERIOR_LIGHT;

/// How much of a light deficit the eyes make up (in stops, as a share).
const ADAPT_SHARE: f32 = 0.6;
/// The most they open up, in stops.
const ADAPT_MAX: f32 = 3.0;
/// Opening up to the dark is slow; closing down to the light is quick (s).
/// Into the dark, the first stop comes in a few seconds (out of a lit room
/// into the night you start to see again), the rest over a minute or so.
const ADAPT_TO_DARK: f32 = 6.0;
const ADAPT_TO_DARK_LATE: f32 = 20.0;
const ADAPT_TO_LIGHT: f32 = 2.5;
/// What a day outside lights the eye with (lux): the scale of `light`.
const DAY_LUX: f32 = 2500.0;
/// How long a lamp's light at the eye takes to count (s): walking past one
/// mustn't pump the exposure.
const LAMP_EASE: f32 = 1.5;

/// The eyes' current adaptation, in stops over the valley's grade.
#[derive(Resource, Default, Debug)]
pub struct Adaptation {
    pub stops: f32,
    /// How far indoors the eye is, eased (0 outside, 1 in a room).
    pub indoors: f32,
    /// The grade it's added to (set with the weather).
    pub base: f32,
    /// The lamps' light at the eye, eased (against a day's).
    pub lamps: f32,
    /// The grade's shadow lift (set with the weather), before `Below`
    /// takes it away.
    pub base_lift: f32,
    settled: bool,
}

/// How far below ground the eye is, 0 (the surface) to 1 (deep in the
/// workings), published by `world::underground::go_dark`. Down there the
/// dark is true black: the grade's shadow lift goes (an unlit room stays
/// black, not grey) and the eyes open a stop less (a torch-lit wall
/// doesn't glare); what you see is what the torch, a lamp, a flare lights.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct Below(pub f32);

/// How many stops less the eyes open at full depth (`Below`).
const BELOW_ADAPT_CUT: f32 = 1.0;

impl Adaptation {
    /// What the eyes would settle to, at this light (1 = a day outside).
    pub fn target(light: f32) -> f32 {
        (-light.max(1e-3).log2() * ADAPT_SHARE).clamp(0.0, ADAPT_MAX)
    }

    /// `target`, this far below ground (`Below`).
    pub fn target_below(light: f32, below: f32) -> f32 {
        Self::target(light).min(ADAPT_MAX - BELOW_ADAPT_CUT * below.clamp(0.0, 1.0))
    }

    /// Takes the eyes straight to their adapted state next frame, as if
    /// they'd been where the camera is for a while (the legibility gauge
    /// reads each panel with adapted eyes).
    pub fn resettle(&mut self) {
        self.settled = false;
    }

    /// One step toward `target` over `dt` seconds.
    pub fn step(&mut self, target: f32, dt: f32) {
        let tau = if target <= self.stops {
            ADAPT_TO_LIGHT
        } else if self.stops < 1.0 {
            ADAPT_TO_DARK
        } else {
            ADAPT_TO_DARK_LATE
        };
        self.stops += (target - self.stops) * (1.0 - (-dt / tau).exp());
    }
}

pub fn adapt_eyes(
    conditions: Res<crate::world::weather::Conditions>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    eyes: Query<(&GlobalTransform, &Camera), (With<Camera3d>, Without<super::OffscreenCamera>)>,
    mut grades: Query<&mut ColorGrading>,
    lamps: Query<(&PointLight, &GlobalTransform)>,
    spots: Query<(&SpotLight, &GlobalTransform)>,
    below: Option<Res<Below>>,
    mut adapt: ResMut<Adaptation>,
    time: Res<Time>,
) {
    let below = below.map_or(0.0, |b| b.0.clamp(0.0, 1.0));
    let eye = eyes
        .iter()
        .filter(|(_, c)| c.is_active)
        .map(|(g, _)| g.translation())
        .next();
    let indoors = match (eye, &entrances) {
        (Some(p), Some(e)) => e.building_at(p).is_some(),
        _ => false,
    };
    // Through a doorway the room's dark comes on over a step or two.
    let dt = time.delta_secs();
    let want = if indoors { 1.0 } else { 0.0 };
    adapt.indoors += (want - adapt.indoors) * (1.0 - (-dt / 0.4).exp());
    // Lamps near the eye light it too: by a lantern at night the eyes
    // don't open as far, and the dark past its pool is darker for it.
    // A torch in your hand lights what's in front of you: the walls it
    // finds light your eyes back (a share of its own light, near).
    let at_eye = eye.map_or(0.0, |p| lamp_light(p, &lamps) + torch_light(p, &spots));
    adapt.lamps += (at_eye - adapt.lamps) * (1.0 - (-dt / LAMP_EASE).exp());
    let light = conditions.light * (1.0 - adapt.indoors * (1.0 - INTERIOR_LIGHT)) * (1.0 - below)
        + adapt.lamps;
    let target = Adaptation::target_below(light, below);
    // A new raid starts with the eyes already used to its light.
    if !adapt.settled || conditions.is_changed() {
        adapt.stops = target;
        adapt.settled = true;
    } else {
        adapt.step(target, dt);
    }
    let want = adapt.base + adapt.stops;
    let lift = adapt.base_lift * (1.0 - below);
    for mut g in &mut grades {
        if (g.global.exposure - want).abs() > 1e-4 {
            g.global.exposure = want;
        }
        if (g.shadows.lift - lift).abs() > 1e-6 {
            g.shadows.lift = lift;
        }
    }
}

/// The light a held torch throws back at the eye, against a day's: a
/// share of its light, if it's within arm's reach (a spot light carried
/// by someone). The walls it finds are what you see by.
pub fn torch_light<'a>(
    p: Vec3,
    spots: impl IntoIterator<Item = (&'a SpotLight, &'a GlobalTransform)>,
) -> f32 {
    let mut lux = 0.0;
    for (l, gt) in spots {
        if l.intensity <= 0.0 || gt.translation().distance_squared(p) > 1.5 * 1.5 {
            continue;
        }
        // The walls it lights, a couple of metres off: its lumens into
        // its cone (2 pi (1 - cos angle) steradians) over 4 m^2, and the
        // share of what you see that they fill.
        let cone = std::f32::consts::TAU * (1.0 - l.outer_angle.cos()).max(0.05);
        lux += l.intensity / cone / 4.0 * 0.3;
    }
    (lux / DAY_LUX).min(0.2)
}

/// The point lights' light at `p`, against a day's: each one's lumens
/// spread over a sphere (cd), over the distance squared, within its range.
pub fn lamp_light<'a>(
    p: Vec3,
    lamps: impl IntoIterator<Item = (&'a PointLight, &'a GlobalTransform)>,
) -> f32 {
    let mut lux = 0.0;
    for (l, gt) in lamps {
        let d2 = gt.translation().distance_squared(p);
        if d2 > l.range * l.range || l.intensity <= 0.0 {
            continue;
        }
        let c = l.color.to_linear();
        let y = 0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue;
        lux += l.intensity / (4.0 * std::f32::consts::PI) / d2.max(0.25) * y;
    }
    // A lamp in your face is still not the day.
    (lux / DAY_LUX).min(0.5)
}

/// What a thing indoors is seen by, with eyes used to the room, against a
/// day outside (the sky's light on it, the eyes' adaptation on top). Flat:
/// the renderer's sky light indoors is the one dimmed ambient
/// (`dim_indoors`) wherever in whichever building, and a loot item, its
/// own entity, takes none of the building's baked occlusion; only a patch
/// of direct sun, or a lamp, adds to it. Left out: those, and a valley's
/// grade (the training yard's is a shade brighter).
pub fn indoor_seen(conditions: &crate::world::weather::Conditions) -> f32 {
    let sky = crate::world::sky::sky_dim(conditions) * conditions.light * INTERIOR_LIGHT;
    sky * Adaptation::target(conditions.light * INTERIOR_LIGHT).exp2()
}

/// The least [`indoor_seen`] that lets you tell a thing you came for
/// from the floor at arm's length without a torch: three stops under a
/// day outside. The night is under it; the torch is for that.
pub const LOOT_SEEN_FLOOR: f32 = 1.0 / 8.0;

/// The ambient before `dim_indoors` this frame, and what it wrote, so
/// `undim` gives back exactly what was there (if nothing changed it since);
/// the same for each camera's fog colours (RGB: the alpha carries the
/// wetness, `world::sky::carry_wetness`).
#[derive(Resource, Default)]
pub struct Dimmed(Option<(f32, f32)>, Vec<(Entity, [Vec3; 2], [Vec3; 2])>);

fn rgb(c: Color) -> Vec3 {
    let l = c.to_linear();
    Vec3::new(l.red, l.green, l.blue)
}

fn with_rgb(c: Color, v: Vec3) -> Color {
    Color::LinearRgba(LinearRgba::new(v.x, v.y, v.z, c.alpha()))
}

/// Indoors, the room's walls cut the sky's light: the ambient (the sky's
/// share of every surface's light; lamps and torches aren't it) drops to
/// `INTERIOR_LIGHT` while the eye is inside a building, so a room at dusk
/// is dark until a light is on. Applied last thing before the frame is
/// drawn and taken off again first thing in the next (`undim`), so the
/// sky and the underground, which set the ambient from what they last
/// wrote, never see it. The fog's light goes with it: the air in a room
/// is lit by the room, and fogged at the sky's brightness it laid a grey
/// veil over the dark end of every room. The outdoors through a doorway
/// dims with it; the sky, unlit, doesn't.
pub fn dim_indoors(
    adapt: Res<Adaptation>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fogs: Query<(Entity, &mut DistanceFog)>,
    mut dimmed: ResMut<Dimmed>,
) {
    let k = 1.0 - adapt.indoors * (1.0 - INTERIOR_LIGHT);
    dimmed.0 = (k < 0.999).then(|| {
        let before = ambient.brightness;
        ambient.brightness *= k;
        (before, ambient.brightness)
    });
    dimmed.1.clear();
    if k >= 0.999 {
        return;
    }
    for (e, mut fog) in &mut fogs {
        let before = [rgb(fog.color), rgb(fog.directional_light_color)];
        let wrote = [before[0] * k, before[1] * k];
        fog.color = with_rgb(fog.color, wrote[0]);
        fog.directional_light_color = with_rgb(fog.directional_light_color, wrote[1]);
        dimmed.1.push((e, before, wrote));
    }
}

pub fn undim(
    mut ambient: ResMut<GlobalAmbientLight>,
    mut fogs: Query<&mut DistanceFog>,
    mut dimmed: ResMut<Dimmed>,
) {
    if let Some((before, wrote)) = dimmed.0.take() {
        if ambient.brightness == wrote {
            ambient.brightness = before;
        }
    }
    for (e, before, wrote) in dimmed.1.drain(..) {
        let Ok(mut fog) = fogs.get_mut(e) else {
            continue;
        };
        if rgb(fog.color).abs_diff_eq(wrote[0], 1e-6) {
            fog.color = with_rgb(fog.color, before[0]);
        }
        if rgb(fog.directional_light_color).abs_diff_eq(wrote[1], 1e-6) {
            fog.directional_light_color = with_rgb(fog.directional_light_color, before[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// By day, dawn and dusk in any weather, a storm's included, what you
    /// came for indoors is lit enough to see without a torch; on a night
    /// under cloud it isn't. Flat over every room (`indoor_seen`), so this holds every
    /// loot spot in every prefab at once.
    #[test]
    fn loot_indoors_is_seen_by_day_not_at_night() {
        use crate::world::weather::{Conditions, Hour, WeatherKind};
        let kinds = [
            WeatherKind::Drizzle,
            WeatherKind::PeaSouper,
            WeatherKind::Downpour,
            WeatherKind::Clearing,
        ];
        let mut worst = (f32::INFINITY, String::new());
        for kind in kinds {
            for hour in [Hour::Dawn, Hour::Day, Hour::Dusk] {
                let c = Conditions::compose(kind, hour, 1);
                for c in [c, c.stormy()] {
                    let seen = indoor_seen(&c);
                    println!("{kind:?} {hour:?} storm {}: {seen:.3}", c.storm);
                    if seen < worst.0 {
                        worst = (seen, format!("{kind:?} {hour:?} storm {}", c.storm));
                    }
                }
            }
        }
        assert!(
            worst.0 >= LOOT_SEEN_FLOOR,
            "indoors, {}: seen at {:.3} of a day, under {LOOT_SEEN_FLOOR}",
            worst.1,
            worst.0
        );
        // A day indoors is darker than outside, and brighter than dusk.
        let day = indoor_seen(&Conditions::compose(WeatherKind::Drizzle, Hour::Day, 1));
        let dusk = indoor_seen(&Conditions::compose(WeatherKind::Drizzle, Hour::Dusk, 1));
        assert!(day < 1.0 && dusk < day, "{day} {dusk}");
        // Under cloud the night is dark indoors. (A clear night under a
        // full moon isn't: the eyes open all the way to it.)
        for kind in &kinds[..3] {
            let night = indoor_seen(&Conditions::compose_with_moon(*kind, Hour::Night, 1, 0.5));
            assert!(night < LOOT_SEEN_FLOOR, "{kind:?} night: {night:.3}");
        }
    }

    #[test]
    fn indoors_the_fog_dims_with_the_ambient_and_comes_back() {
        let mut app = App::new();
        app.insert_resource(Adaptation {
            indoors: 1.0,
            ..default()
        })
        .insert_resource(GlobalAmbientLight {
            brightness: 100.0,
            ..default()
        })
        .init_resource::<Dimmed>()
        .add_systems(Update, (undim, dim_indoors).chain());
        let wet = 0.7;
        let cam = app
            .world_mut()
            .spawn(DistanceFog {
                color: Color::linear_rgba(0.4, 0.5, 0.6, wet),
                directional_light_color: Color::linear_rgba(1.0, 0.9, 0.8, 0.5),
                ..default()
            })
            .id();
        for _ in 0..3 {
            app.update();
        }
        let fog = app.world().get::<DistanceFog>(cam).unwrap();
        let k = INTERIOR_LIGHT;
        assert!(rgb(fog.color).abs_diff_eq(Vec3::new(0.4, 0.5, 0.6) * k, 1e-5));
        assert!(rgb(fog.directional_light_color).abs_diff_eq(Vec3::new(1.0, 0.9, 0.8) * k, 1e-5));
        // The wetness on the alpha rides through.
        assert!((fog.color.alpha() - wet).abs() < 1e-6);
        // Outside again: all of it back.
        app.world_mut().resource_mut::<Adaptation>().indoors = 0.0;
        app.update();
        let fog = app.world().get::<DistanceFog>(cam).unwrap();
        assert!(rgb(fog.color).abs_diff_eq(Vec3::new(0.4, 0.5, 0.6), 1e-5));
        assert_eq!(
            app.world().resource::<GlobalAmbientLight>().brightness,
            100.0
        );
    }

    #[test]
    fn below_ground_the_eyes_open_less_and_a_torch_lights_them() {
        // Deep in the workings: the most they open is a stop short.
        assert_eq!(
            Adaptation::target_below(0.0, 1.0),
            ADAPT_MAX - BELOW_ADAPT_CUT
        );
        assert_eq!(Adaptation::target_below(0.0, 0.0), ADAPT_MAX);
        // A torch in hand lights the eye (off the walls it finds); one
        // across the room doesn't.
        let torch = SpotLight {
            intensity: 1000.0,
            outer_angle: 0.4,
            ..default()
        };
        let near = GlobalTransform::from_translation(Vec3::new(0.3, -0.3, -0.3));
        let far = GlobalTransform::from_translation(Vec3::new(8.0, 0.0, 0.0));
        let lit = torch_light(Vec3::ZERO, [(&torch, &near)]);
        assert!(lit > 0.02 && lit <= 0.2, "{lit}");
        assert_eq!(torch_light(Vec3::ZERO, [(&torch, &far)]), 0.0);
        // It keeps the eyes about half a stop from opening all the way.
        assert!(Adaptation::target(lit) < Adaptation::target(0.0) - 0.4);
    }

    #[test]
    fn eyes_adapt_part_way_slowly_into_the_dark_and_fast_out() {
        // A day outside needs nothing; dusk and a dark room open up, but
        // never all the way.
        assert_eq!(Adaptation::target(1.0), 0.0);
        let dusk = Adaptation::target(0.36);
        assert!(dusk > 0.5 && dusk < -(0.36f32.log2()));
        assert!(Adaptation::target(0.0) <= ADAPT_MAX);
        // Into a dark room: after two seconds, well short of adapted...
        let mut a = Adaptation::default();
        let room = Adaptation::target(INTERIOR_LIGHT);
        for _ in 0..20 {
            a.step(room, 0.1);
        }
        assert!(a.stops < room * 0.35, "{} of {room}", a.stops);
        // ...and back out into the day, most of the way in two.
        for _ in 0..20 {
            a.step(0.0, 0.1);
        }
        let before = a.stops;
        let mut b = Adaptation {
            stops: room,
            ..default()
        };
        for _ in 0..20 {
            b.step(0.0, 0.1);
        }
        assert!(b.stops < room * 0.5 && before < room * 0.35);
    }

    #[test]
    fn out_of_a_lit_room_into_the_night_you_see_again_in_seconds() {
        let night = Adaptation::target(0.02);
        let mut a = Adaptation::default();
        for _ in 0..40 {
            a.step(night, 0.1);
        }
        // Four seconds in: some way, not all of it.
        assert!(
            a.stops > 0.4 && a.stops < night * 0.5,
            "{} of {night}",
            a.stops
        );
        for _ in 0..600 {
            a.step(night, 0.1);
        }
        assert!(a.stops > night * 0.9);
    }

    #[test]
    fn a_lantern_lights_the_eye_by_its_distance() {
        let lamp = PointLight {
            color: Color::WHITE,
            intensity: 9_000.0,
            range: 7.0,
            ..default()
        };
        let at = GlobalTransform::from_translation(Vec3::ZERO);
        let near = lamp_light(Vec3::new(1.5, 0.0, 0.0), [(&lamp, &at)]);
        let far = lamp_light(Vec3::new(5.0, 0.0, 0.0), [(&lamp, &at)]);
        let out = lamp_light(Vec3::new(8.0, 0.0, 0.0), [(&lamp, &at)]);
        // Close to, more than a full moon's sky (0.08); out of range, none.
        assert!(
            near > 0.08 && far < near * 0.2 && out == 0.0,
            "{near} {far}"
        );
    }
}
