//! Weather: each Surge cycle brings its own, a pure function of the seed
//! and the cycle. It's gameplay as much as looks — visibility caps how far
//! robots (and you) see, and rain masks noise — so it lives here, and the
//! sky, fog, lights and rain read it (`sky::apply_weather`, `rain`).

use bevy::prelude::*;
use extraction_core::noise::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherKind {
    Drizzle,
    PeaSouper,
    Downpour,
    Clearing,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Conditions {
    pub kind: WeatherKind,
    /// Metres at which things fade out.
    pub visibility: f32,
    /// 0..=1.
    pub rain: f32,
    /// Ground-fog density at its base height (see `render::fog`).
    pub ground_fog: f32,
    /// Direct sun, lux.
    pub sun: f32,
    pub cycle: u32,
}

impl Conditions {
    pub fn for_cycle(seed: u64, cycle: u32) -> Self {
        let mut rng = Rng::new(seed ^ 0x3EA7_4E12 ^ (cycle as u64).wrapping_mul(0x9E37_79B9));
        // The first cycle is always the reference look.
        let kind = if cycle == 0 {
            WeatherKind::Drizzle
        } else {
            [
                WeatherKind::Drizzle,
                WeatherKind::Drizzle,
                WeatherKind::PeaSouper,
                WeatherKind::Downpour,
                WeatherKind::Clearing,
            ][rng.below(5)]
        };
        let (visibility, rain, ground_fog, sun) = match kind {
            WeatherKind::Drizzle => (320.0, 0.35, 0.011, 1600.0),
            WeatherKind::PeaSouper => (150.0, 0.0, 0.024, 1100.0),
            WeatherKind::Downpour => (210.0, 1.0, 0.009, 900.0),
            WeatherKind::Clearing => (520.0, 0.0, 0.006, 2600.0),
        };
        Self {
            kind,
            visibility,
            rain,
            ground_fog,
            sun,
            cycle,
        }
    }

    /// How far a robot's eyes reach, given its own sight range.
    pub fn sight(&self, own: f32) -> f32 {
        own.min(self.visibility * 0.6)
    }

    /// How hard the wind blows, 0..=1: still in thick fog, hard in a
    /// downpour.
    pub fn wind(&self) -> f32 {
        match self.kind {
            WeatherKind::Drizzle => 0.4,
            WeatherKind::PeaSouper => 0.12,
            WeatherKind::Downpour => 0.9,
            WeatherKind::Clearing => 0.55,
        }
    }

    /// Hearing scale: rain covers footsteps (not gunshots, much).
    pub fn hearing(&self) -> f32 {
        1.0 - 0.35 * self.rain
    }
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Conditions::for_cycle(1, 0))
            .add_systems(Update, follow_cycle);
    }
}

/// New Surge cycle (or a loaded save), new weather.
fn follow_cycle(
    raid: Res<crate::raid::Raid>,
    opts: Res<crate::launch::LaunchOptions>,
    mut conditions: ResMut<Conditions>,
) {
    let mut want = Conditions::for_cycle(opts.seed, raid.cycle);
    if let Some(w) = &opts.weather {
        // Dev override: borrow the first cycle that has the asked-for weather.
        let kind = match w.as_str() {
            "fog" => WeatherKind::PeaSouper,
            "downpour" => WeatherKind::Downpour,
            "clearing" => WeatherKind::Clearing,
            _ => WeatherKind::Drizzle,
        };
        if let Some(c) = (0..200)
            .map(|c| Conditions::for_cycle(opts.seed, c))
            .find(|c| c.kind == kind)
        {
            want = Conditions {
                cycle: raid.cycle,
                ..c
            };
        }
    }
    if *conditions != want {
        *conditions = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_varies_by_cycle_but_not_by_chance() {
        let a: Vec<WeatherKind> = (0..20).map(|c| Conditions::for_cycle(5, c).kind).collect();
        let b: Vec<WeatherKind> = (0..20).map(|c| Conditions::for_cycle(5, c).kind).collect();
        assert_eq!(a, b);
        assert_eq!(a[0], WeatherKind::Drizzle);
        assert!(a.iter().any(|k| *k != WeatherKind::Drizzle));
    }
}
