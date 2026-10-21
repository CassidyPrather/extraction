//! Reading close: squeeze on a note (or the logbook, the request card) and
//! it comes up off the table or the wall into your hand, facing your eyes,
//! for you to bring to your face; open the hand and it goes back where it
//! was. Small print is read the way you'd read it, held up
//! (`docs/LEGIBILITY.md`). In flat mode, E takes it up and E puts it back.
//!
//! A panel that's also a control (the logbook, the request card) does
//! this in VR, where its push button presses it; flat mode, with no
//! buttons, still presses it.

use bevy::prelude::*;

use super::{Hand, HandAction, InteractEvent, InteractSystems};
use crate::raid::bench::ScreenText;
use crate::raid::truck::Pressable;
use crate::rig::{RigDriver, RigHand, RigHead, Side};

pub struct ReadPlugin;

impl Plugin for ReadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (take_up, hold_up)
                .chain()
                .after(super::apply_hand_actions)
                .in_set(InteractSystems::Grab),
        );
    }
}

/// A panel you can take up to read. It also needs a `Grabbable` and a
/// collider on the item layer for the hand to find it.
#[derive(Component, Default)]
pub struct Readable;

/// Being read: in this hand, and where it goes back to.
#[derive(Component)]
pub struct Reading {
    pub player: Entity,
    pub side: Side,
    home: Transform,
}

/// Whether a grab on this readable takes it up to read (rather than
/// pressing it): always in VR; in flat mode only if it isn't a control.
pub fn reads(driver: RigDriver, pressable: bool) -> bool {
    driver == RigDriver::Xr || !pressable
}

/// Where a panel held up to read sits: its bottom edge in the hand, a
/// little above the knuckles.
const HELD_ABOVE: f32 = 0.03;

fn take_up(
    mut commands: Commands,
    mut actions: MessageReader<HandAction>,
    mut events: MessageWriter<InteractEvent>,
    driver: Res<RigDriver>,
    mut panels: Query<(Entity, &mut Transform, Option<&Reading>, Has<Pressable>), With<Readable>>,
) {
    for action in actions.read() {
        match *action {
            HandAction::Grab {
                player, side, item, ..
            } => {
                let Ok((e, mut t, reading, pressable)) = panels.get_mut(item) else {
                    continue;
                };
                if !reads(*driver, pressable) {
                    continue;
                }
                match reading {
                    // Flat mode: E again puts it back.
                    Some(r) if r.player == player && r.side == side => {
                        *t = r.home;
                        commands.entity(e).remove::<Reading>();
                    }
                    Some(_) => {}
                    None => {
                        commands.entity(e).insert(Reading {
                            player,
                            side,
                            home: *t,
                        });
                        events.write(InteractEvent::Grabbed { item, side });
                    }
                }
            }
            HandAction::Release { player, side, .. } => {
                for (e, mut t, reading, _) in &mut panels {
                    if let Some(r) = reading.filter(|r| r.player == player && r.side == side) {
                        *t = r.home;
                        commands.entity(e).remove::<Reading>();
                    }
                }
            }
            _ => {}
        }
    }
}

/// Keeps each panel being read in its hand, turned to face the eyes.
fn hold_up(
    mut panels: Query<(&Reading, &ScreenText, &mut Transform, Option<&ChildOf>)>,
    hands: Query<(&RigHand, &Hand, &ChildOf)>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    parents: Query<&GlobalTransform>,
) {
    for (reading, screen, mut t, parent) in &mut panels {
        let Some(hand) = hands
            .iter()
            .find(|(rh, _, p)| rh.side == reading.side && p.parent() == reading.player)
            .map(|(_, h, _)| h.world)
        else {
            continue;
        };
        let Some(eye) = heads
            .iter()
            .find(|(_, p)| p.parent() == reading.player)
            .map(|(g, _)| g.translation())
        else {
            continue;
        };
        let at = hand.translation + Vec3::Y * (screen.size.y * 0.5 + HELD_ABOVE);
        let to_eye = (eye - at).normalize_or(Vec3::Z);
        // Its face (+Z) to the eyes, its top up.
        let world = Transform::from_translation(at).looking_to(-to_eye, Vec3::Y);
        let local = match parent.and_then(|p| parents.get(p.parent()).ok()) {
            Some(pg) => Transform::from_matrix(pg.to_matrix().inverse() * world.to_matrix()),
            None => world,
        };
        *t = local;
    }
}

/// What makes a panel `size` across one to take up and read: the marker, a
/// grab point anywhere on it, and a thin collider for the hand to find
/// (on the item layer, colliding with nothing).
pub fn readable(size: Vec2) -> impl Bundle {
    (
        Readable,
        super::Grabbable {
            points: vec![super::GrabPoint::free()],
        },
        avian3d::prelude::Collider::cuboid(size.x, size.y, 0.01),
        avian3d::prelude::CollisionLayers::new(
            crate::world::Layer::Item,
            avian3d::prelude::LayerMask::NONE,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vr_harness::Vr;

    /// Through the headset path: squeeze on the logbook on the bed's wall,
    /// bring the hand up in front of your face and it's there, facing your
    /// eyes, and its page hasn't turned; open the hand and it's back on the
    /// wall.
    #[test]
    fn the_logbook_comes_up_to_be_read_and_goes_back() {
        let mut vr = Vr::new("training_yard");
        vr.step(5);
        let (book, home) = vr
            .app
            .world_mut()
            .query_filtered::<(Entity, &GlobalTransform), With<crate::raid::logbook::LogbookCard>>()
            .iter(vr.app.world())
            .map(|(e, g)| (e, g.translation()))
            .next()
            .expect("the logbook");
        // In the bed, facing the left wall (-X), the card at arm's length.
        let feet = home + Vec3::new(0.65, -1.66, 0.0);
        vr.stand(feet, std::f32::consts::FRAC_PI_2);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.hand(Side::Right, home + Vec3::X * 0.02, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert!(
            vr.app.world().get::<Reading>(book).is_some(),
            "the squeeze takes it up"
        );
        assert!(!vr.pressed().contains(&book), "taking it up isn't a press");
        assert_eq!(vr.hand_state(Side::Right).holding, None);
        // Up in front of the face.
        let eye = feet + Vec3::Y * 1.65;
        vr.move_hand(Side::Right, eye + Vec3::new(-0.3, -0.2, 0.0));
        vr.step(3);
        let g = *vr.app.world().get::<GlobalTransform>(book).unwrap();
        let to_eye = eye - g.translation();
        assert!(
            to_eye.length() < 0.45,
            "{:.2} m from the eyes",
            to_eye.length()
        );
        assert!(
            (g.rotation() * Vec3::Z).dot(to_eye.normalize()) > 0.95,
            "facing the eyes"
        );
        // Let go: back on the wall.
        vr.grip(Side::Right, 0.0);
        vr.step(3);
        assert!(vr.app.world().get::<Reading>(book).is_none());
        let back = vr
            .app
            .world()
            .get::<GlobalTransform>(book)
            .unwrap()
            .translation();
        assert!(
            back.distance(home) < 1e-3,
            "back at {back:.3}, was {home:.3}"
        );
    }

    /// A note lying on its own (as the client lays one on a table) comes
    /// up the same way, and isn't taken as an item.
    #[test]
    fn a_note_on_a_table_comes_up_to_be_read() {
        let mut vr = Vr::new("training_yard");
        let feet = vr.feet();
        let home = Transform::from_translation(feet + Vec3::new(0.0, 0.9, -0.45))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        let note = vr
            .app
            .world_mut()
            .spawn((
                crate::world::notes::note_panel("Gone to the pump station. Back by six."),
                crate::render::legibility::TextSource::Note,
                readable(crate::world::notes::NOTE),
                home,
            ))
            .id();
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.step(2);
        vr.hand(
            Side::Left,
            home.translation + Vec3::Y * 0.02,
            Quat::IDENTITY,
            0.0,
            0.0,
        );
        vr.step(2);
        vr.grip(Side::Left, 1.0);
        vr.step(3);
        assert!(vr.app.world().get::<Reading>(note).is_some(), "taken up");
        assert_eq!(vr.hand_state(Side::Left).holding, None);
        vr.grip(Side::Left, 0.0);
        vr.step(3);
        assert_eq!(*vr.app.world().get::<Transform>(note).unwrap(), home);
    }
}
