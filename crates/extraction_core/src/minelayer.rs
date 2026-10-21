//! The minelayer's rules: a slow tracked crawler that sows pressure mines
//! across the ways people walk, and the mines themselves. (The machine is
//! `robots::minelayer` in the game.)
//!
//! Mines are meant to be found:
//!
//! - **A laid mine shows.** It sits proud of the ground with a ring of
//!   turned earth round it and a red light that blinks every [`BLINK`]
//!   seconds; the crawler lays them in a short line ([`field`]) across a
//!   way, so one found means more beside it.
//! - **It's a pressure plate.** Weight on it ([`TRIGGER_KG`]) clicks it,
//!   and [`FUSE`] seconds later it goes off ([`blast`]). It doesn't arm
//!   for [`ARM`] seconds after it's laid.
//! - **Disarm it** with a hand held on its prong for [`DISARM`] seconds
//!   ([`Disarm`]), or bolt cutters through its lead; or shoot it from out
//!   of its reach.
//!
//! The crawler carries [`HOPPER`] mines in a striped hopper on its back:
//! hole the hopper and they cook off together.

/// How fast it crawls (m/s), and turns (degrees a second).
pub const SPEED: f32 = 1.3;
pub const TURN: f32 = 45.0;
/// Mines in a full hopper; in a field it lays; and their spacing (m).
pub const HOPPER: u32 = 6;
pub const PER_FIELD: u32 = 4;
pub const SPACING: f32 = 1.8;
/// Seconds it stops to lay one (the chunk and hiss of the chute).
pub const LAY_TIME: f32 = 2.0;

/// The mine: seconds before it arms after it's laid, the weight that sets
/// it off (kg), and how near its centre (m).
pub const ARM: f32 = 4.0;
pub const TRIGGER_KG: f32 = 25.0;
pub const PLATE: f32 = 0.32;
/// Seconds from the click to the blast.
pub const FUSE: f32 = 0.35;
/// Its blast: harm at its heart and how far it reaches (m).
pub const BLAST: f32 = 90.0;
pub const REACH: f32 = 4.0;
/// Its red light: a blink every this many seconds, this long.
pub const BLINK: f32 = 2.0;
pub const BLINK_ON: f32 = 0.12;
/// Seconds of a hand held on its prong to disarm it.
pub const DISARM: f32 = 2.5;

/// Harm a mine's blast does at `d` metres (falls off to nothing at
/// [`REACH`]).
pub fn blast(d: f32) -> f32 {
    let f = (1.0 - d / REACH).clamp(0.0, 1.0);
    BLAST * f * f
}

/// Where a field's mines go: `n` spots across a way at `site`, the way
/// running along `along` (x/z), staggered so a careful line of steps
/// can't slip between two without looking.
pub fn field(site: [f32; 2], along: [f32; 2], n: u32) -> Vec<[f32; 2]> {
    let len = (along[0] * along[0] + along[1] * along[1]).sqrt().max(1e-4);
    let (ax, az) = (along[0] / len, along[1] / len);
    let (sx, sz) = (-az, ax);
    (0..n)
        .map(|k| {
            let across = (k as f32 - (n as f32 - 1.0) * 0.5) * SPACING;
            let stagger = if k % 2 == 0 { -0.6 } else { 0.6 };
            [
                site[0] + sx * across + ax * stagger,
                site[1] + sz * across + az * stagger,
            ]
        })
        .collect()
}

/// Whether a mine may lie at `p`: never in a doorway (`map_doors`), on a
/// road, or close in by a building, where you couldn't see it coming.
pub fn spot_ok(
    map: &crate::map::MapDef,
    hm: &crate::terrain::Heightmap,
    doors: &[crate::map_doors::Door],
    p: [f32; 2],
) -> bool {
    crate::map_doors::clear_of_doors(doors, p, 3.0)
        && crate::anomaly::clear_of_roads(hm, p, 0.5)
        && crate::anomaly::clear_of_buildings(map, p, 2.0)
}

/// A field at `site` across `along` ([`field`]), less the spots where a
/// mine may not lie ([`spot_ok`]).
pub fn site_field(
    map: &crate::map::MapDef,
    hm: &crate::terrain::Heightmap,
    site: [f32; 2],
    along: [f32; 2],
) -> Vec<[f32; 2]> {
    let doors = crate::map_doors::doors(map);
    field(site, along, PER_FIELD)
        .into_iter()
        .filter(|p| spot_ok(map, hm, &doors, *p))
        .collect()
}

/// Whether its light is on, `t` seconds after it was laid (each mine's
/// `phase` puts them out of step).
pub fn lit(t: f32, phase: f32) -> bool {
    (t + phase).rem_euclid(BLINK) < BLINK_ON
}

/// A mine's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mine {
    /// Seconds since it was laid.
    pub age: f32,
    /// Seconds since it clicked (None: not stepped on).
    pub clicked: Option<f32>,
}

/// What a mine does this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MineAct {
    Quiet,
    /// Stepped on: the click.
    Click,
    Blast,
}

impl Mine {
    /// One tick: `weight` the most mass (kg) on its plate now.
    pub fn step(&mut self, weight: f32, dt: f32) -> MineAct {
        self.age += dt;
        if let Some(c) = self.clicked.as_mut() {
            *c += dt;
            return if *c >= FUSE {
                MineAct::Blast
            } else {
                MineAct::Quiet
            };
        }
        if self.age >= ARM && weight >= TRIGGER_KG {
            self.clicked = Some(0.0);
            return MineAct::Click;
        }
        MineAct::Quiet
    }
}

/// A hand on a mine's prong: held [`DISARM`] seconds, it's safe; let go
/// and it starts over.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Disarm {
    pub held: f32,
}

impl Disarm {
    /// One tick: whether a hand is on it. True once it's disarmed.
    pub fn step(&mut self, holding: bool, dt: f32) -> bool {
        if holding {
            self.held += dt;
        } else {
            self.held = 0.0;
        }
        self.held >= DISARM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A light step doesn't set it off; a person does, after it's armed,
    /// with a click and then the blast; a fresh mine is safe.
    #[test]
    fn a_step_clicks_then_it_blows() {
        let dt = 0.05;
        let mut m = Mine::default();
        // Fresh: a person can stand on it.
        for _ in 0..((ARM / dt) as usize - 2) {
            assert_eq!(m.step(80.0, dt), MineAct::Quiet);
        }
        let mut m = Mine {
            age: ARM,
            clicked: None,
        };
        // A fox doesn't.
        assert_eq!(m.step(8.0, dt), MineAct::Quiet);
        assert_eq!(m.step(80.0, dt), MineAct::Click);
        // Stepping off doesn't save you.
        let mut t = 0.0;
        loop {
            t += dt;
            if m.step(0.0, dt) == MineAct::Blast {
                break;
            }
            assert!(t < 1.0);
        }
        assert!((t - FUSE).abs() < dt * 1.5, "{t}");
    }

    #[test]
    fn its_blast_falls_off() {
        assert_eq!(blast(0.0), BLAST);
        assert!(blast(1.0) > 40.0, "a step away is a bad wound");
        assert!(blast(2.5) < 15.0);
        assert_eq!(blast(REACH), 0.0);
        assert_eq!(blast(10.0), 0.0);
    }

    /// A field lies across the way, spaced so you can see a gap but not
    /// walk a straight line through without stepping round.
    #[test]
    fn a_field_lies_across_the_way() {
        let f = field([10.0, 20.0], [0.0, -1.0], PER_FIELD);
        assert_eq!(f.len(), PER_FIELD as usize);
        let xs: Vec<f32> = f.iter().map(|p| p[0]).collect();
        let span = xs.iter().cloned().fold(f32::MIN, f32::max)
            - xs.iter().cloned().fold(f32::MAX, f32::min);
        assert!((span - SPACING * (PER_FIELD - 1) as f32).abs() < 1e-3);
        for w in f.windows(2) {
            assert!((w[0][1] - w[1][1]).abs() > 1.0, "staggered");
        }
    }

    #[test]
    fn a_held_hand_disarms_it() {
        let dt = 0.1;
        let mut d = Disarm::default();
        for _ in 0..20 {
            assert!(!d.step(true, dt));
        }
        d.step(false, dt);
        assert_eq!(d.held, 0.0, "let go and it starts over");
        let mut done = false;
        for _ in 0..((DISARM / dt) as usize + 1) {
            done = d.step(true, dt);
        }
        assert!(done);
        // Its light blinks, short and seldom.
        let on = (0..2000).filter(|k| lit(*k as f32 * 0.01, 0.3)).count();
        assert!(on > 80 && on < 160, "{on}");
    }
}
