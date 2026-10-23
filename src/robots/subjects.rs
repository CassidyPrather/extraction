//! Machines a contract step needs: what counts as one, and keeping one
//! findable.
//!
//! A step on the card that needs a machine of a kind (a photograph of a
//! walker, of a heavy) names it ([`needs`], an exhaustive match: a new
//! step that needs a machine has to say which). Only that kind counts
//! ([`counts`]): "a walker" is a plain walker on its legs, not a crawler,
//! a hound, a grappler or the heavy. The camera asks the same question
//! (`progression::contracts::photograph`).
//!
//! While such a step is on the card and you're out, the director keeps
//! one on the surface and findable ([`keep_one_findable`]): if none that
//! counts is about within [`REACH`] of the drop, one is sent from the
//! gate to work a site within [`REACH`] of it for the rest of the raid.
//! Once a raid, and again only if that one is wrecked before the picture
//! is taken.

use bevy::prelude::*;
use extraction_core::contracts::Subject;
use extraction_core::map::MarkerKind;
use extraction_core::progression::Job as CardJob;

use super::{Breed, Job, JobKind, Robot, RobotKind};
use crate::raid::{Phase, Raid};
use crate::world::Zone;

/// One is kept within this of the drop (m)...
pub const REACH: f32 = 400.0;
/// ...looked for this often (s)...
pub const LOOK_EVERY: f32 = 5.0;
/// ...and sent to work its site this long (s).
pub const WORK_FOR: f32 = 1800.0;

/// The machine a job on the card needs, if it needs one.
pub fn needs(job: &CardJob) -> Option<Subject> {
    match job {
        CardJob::Photograph { subject, taken } => (!taken).then_some(*subject),
        CardJob::Fetch { .. }
        | CardJob::Recover { .. }
        | CardJob::Letter
        | CardJob::Ledger
        | CardJob::Scout { .. }
        | CardJob::Bring { .. }
        | CardJob::Plant { .. } => None,
    }
}

/// What a machine is, for whether it counts as a subject.
#[derive(Clone, Copy, Debug, Default)]
pub struct Shape {
    pub crawler: bool,
    pub hound: bool,
    pub grappler: bool,
    pub dormant: bool,
}

/// Whether this machine is the kind the step names.
pub fn counts(subject: Subject, robot: &Robot, breed: Option<&Breed>, shape: Shape) -> bool {
    if robot.wrecked || shape.dormant {
        return false;
    }
    let heavy = breed.is_some_and(|b| b.0 == "heavy");
    match subject {
        Subject::Heavy => heavy,
        Subject::Walker => {
            robot.kind == RobotKind::Walker
                && !heavy
                && !shape.crawler
                && !shape.hound
                && !shape.grappler
        }
    }
}

/// Where you're dropped.
pub fn drop_at(zone: &Zone) -> Option<Vec3> {
    let at = zone.map.markers_of(&MarkerKind::Insertion).next()?.at;
    Some(zone.on_ground(at[0], at[1]))
}

/// A site to work within [`REACH`] of the drop: the work site or point of
/// interest nearest halfway out, on ground a walker can stand on.
pub fn site_near(zone: &Zone, nav: &extraction_core::nav::NavGrid, drop: Vec3) -> Vec3 {
    let d = Vec2::new(drop.x, drop.z);
    let best = zone
        .map
        .markers
        .iter()
        .filter(|m| matches!(m.kind, MarkerKind::WorkSite | MarkerKind::Poi))
        .map(|m| Vec2::from(m.at))
        .filter(|p| p.distance(d) < REACH * 0.9 && p.distance(d) > 60.0)
        .filter_map(|p| nav.nearest_walkable(p.to_array()).map(Vec2::from_array))
        .min_by(|a, b| {
            (a.distance(d) - REACH * 0.5)
                .abs()
                .total_cmp(&(b.distance(d) - REACH * 0.5).abs())
        });
    let p = best.unwrap_or_else(|| {
        // Nowhere named: out toward the middle of the valley.
        let toward = (-d).normalize_or(Vec2::NEG_Y);
        let p = d + toward * REACH * 0.5;
        nav.nearest_walkable(p.to_array())
            .map_or(p, Vec2::from_array)
    });
    zone.on_ground(p.x, p.y)
}

/// The one sent this raid, if any.
#[derive(Resource, Default, Debug)]
pub struct Sent(pub Option<Entity>);

pub struct SubjectsPlugin;

impl Plugin for SubjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sent>()
            .add_systems(Update, keep_one_findable);
    }
}

/// While the card needs a machine and you're out: one that counts within
/// reach of the drop, or one sent.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn keep_one_findable(
    mut commands: Commands,
    raid: Option<Res<Raid>>,
    progression: Option<Res<crate::progression::Progression>>,
    director: Option<Res<super::director::Director>>,
    zone: Res<Zone>,
    nav: Option<Res<super::walker::Nav>>,
    catalog: Option<Res<crate::items::Catalog>>,
    robots: Query<(
        Entity,
        &Robot,
        &GlobalTransform,
        Option<&Breed>,
        (
            Has<super::indoor::Crawler>,
            Has<super::hound::Hound>,
            Has<super::grappler::Grappler>,
            Has<super::indoor::Dormant>,
        ),
    )>,
    mut sent: ResMut<Sent>,
    time: Res<Time>,
    mut next: Local<f32>,
    mut last: Local<Option<Phase>>,
) {
    let (Some(raid), Some(progression), Some(nav)) = (raid, progression, nav) else {
        return;
    };
    let before = last.replace(raid.phase.clone());
    if matches!(before, Some(Phase::AtBase)) && raid.phase == Phase::Out {
        sent.0 = None;
    }
    let now = time.elapsed_secs();
    if now < *next || raid.phase != Phase::Out || raid.recalled {
        return;
    }
    *next = now + LOOK_EVERY;
    if !director.as_ref().is_some_and(|d| d.enabled) {
        return;
    }
    let Some(subject) = progression.0.job.as_ref().and_then(needs) else {
        return;
    };
    let Some(drop) = drop_at(&zone) else {
        return;
    };
    let count = |e: Entity| {
        robots.get(e).ok().filter(|(_, r, _, b, (c, h, g, d))| {
            counts(
                subject,
                r,
                *b,
                Shape {
                    crawler: *c,
                    hound: *h,
                    grappler: *g,
                    dormant: *d,
                },
            )
        })
    };
    // The one already sent, still going: it's on its way.
    if sent.0.is_some_and(|e| count(e).is_some()) {
        return;
    }
    let near = robots.iter().any(|(e, _, g, ..)| {
        count(e).is_some()
            && Vec2::new(g.translation().x - drop.x, g.translation().z - drop.z).length() < REACH
    });
    if near {
        return;
    }
    let site = site_near(&zone, &nav.0, drop);
    let exit = super::director::gate(&zone);
    let job = Job {
        kind: JobKind::Watch,
        site,
        duration: WORK_FOR,
        exit,
    };
    let e = match subject {
        Subject::Walker => super::walker::spawn_walker(&mut commands, &zone, exit, job),
        Subject::Heavy => {
            let Some(catalog) = catalog else {
                return;
            };
            super::heavy::spawn_heavy(&mut commands, &zone, &catalog, exit, job)
        }
    };
    commands
        .entity(e)
        .insert(super::Brain::Transit { to: site });
    info!(
        "subjects: {} sent to work by the drop, at {site:.0}",
        subject.name()
    );
    sent.0 = Some(e);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The card names a kind and only that kind counts: a crawler, a
    /// hound, a grappler and the heavy aren't "a walker"; a dormant one
    /// isn't findable.
    #[test]
    fn only_the_named_kind_counts() {
        let walker = Robot {
            kind: RobotKind::Walker,
            wrecked: false,
        };
        let plain = Shape::default();
        assert!(counts(Subject::Walker, &walker, None, plain));
        for shape in [
            Shape {
                crawler: true,
                ..plain
            },
            Shape {
                hound: true,
                ..plain
            },
            Shape {
                grappler: true,
                ..plain
            },
            Shape {
                dormant: true,
                ..plain
            },
        ] {
            assert!(!counts(Subject::Walker, &walker, None, shape), "{shape:?}");
        }
        let heavy = Breed("heavy");
        assert!(!counts(Subject::Walker, &walker, Some(&heavy), plain));
        assert!(counts(Subject::Heavy, &walker, Some(&heavy), plain));
        assert!(!counts(Subject::Heavy, &walker, None, plain));
    }

    /// Minutes in, at most, before one is findable.
    const FINDABLE_IN: f32 = 4.0;

    /// In every valley, with a photograph of each kind on the card, one
    /// that counts is about within REACH of the drop before FINDABLE_IN
    /// minutes are out.
    #[test]
    fn in_every_valley_the_subject_is_findable_early() {
        let maps = [
            "marrow_creek",
            "cinder_works",
            "kettle_pass",
            "rime_fell",
            "sallow_reservoir",
        ];
        let mut late = vec![];
        for map in maps {
            for subject in [Subject::Walker, Subject::Heavy] {
                let secs = findable_after(map, subject);
                eprintln!("{map} {}: {secs:?} s", subject.name());
                if secs.is_none_or(|s| s > FINDABLE_IN * 60.0) {
                    late.push((map, subject.name(), secs));
                }
            }
        }
        assert!(late.is_empty(), "not findable in time: {late:?}");
    }

    /// Seconds out before one that counts is within REACH of the drop.
    fn findable_after(map: &str, subject: Subject) -> Option<f32> {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: true,
                map: map.into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .job = Some(CardJob::Photograph {
            subject,
            taken: false,
        });
        app.world_mut()
            .resource_mut::<super::super::director::Director>()
            .enabled = true;
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let zone = app.world().resource::<Zone>().clone();
        let drop = drop_at(&zone)?;
        let hz = 90.0;
        for k in 0..(FINDABLE_IN * 60.0 * hz) as usize {
            app.update();
            if k % 90 != 0 {
                continue;
            }
            if app.world().resource::<Raid>().phase != Phase::Out {
                app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
            }
            let found = app
                .world_mut()
                .query::<(
                    &Robot,
                    &GlobalTransform,
                    Option<&Breed>,
                    (
                        Has<super::super::indoor::Crawler>,
                        Has<super::super::hound::Hound>,
                        Has<super::super::grappler::Grappler>,
                        Has<super::super::indoor::Dormant>,
                    ),
                )>()
                .iter(app.world())
                .any(|(r, g, b, (c, h, gr, d))| {
                    let p = g.translation();
                    counts(
                        subject,
                        r,
                        b,
                        Shape {
                            crawler: c,
                            hound: h,
                            grappler: gr,
                            dormant: d,
                        },
                    ) && Vec2::new(p.x - drop.x, p.z - drop.z).length() < REACH
                });
            if found {
                return Some(k as f32 / hz);
            }
        }
        None
    }
}
