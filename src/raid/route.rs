//! Which valley the truck drives to next. A card pinned on the bed's right
//! wall names it; press the card to pick another. Ring the bell at base
//! with another valley on the card and the truck drives there through the
//! night: the career is saved, and the game opens again in the new valley
//! (each valley keeps its own Zone; the stash, kit and record travel).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::truck::{FLOOR, Pressable, Truck};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::launch::LaunchOptions;
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::world::Layer;

/// The valleys there are (map id, name), and which one the card shows.
#[derive(Resource, Debug, Clone)]
pub struct Destination {
    pub maps: Vec<(String, String)>,
    pub index: usize,
}

impl Destination {
    pub fn id(&self) -> &str {
        &self.maps[self.index].0
    }

    pub fn name(&self) -> &str {
        &self.maps[self.index].1
    }
}

/// The card on the wall.
#[derive(Component)]
pub struct RouteCard;

/// Drive to another valley: save, and open the game again there.
#[derive(Message, Clone, Debug)]
pub struct Relocate {
    pub to: String,
}

pub struct RoutePlugin;

impl Plugin for RoutePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Relocate>()
            .add_systems(Startup, init_destination)
            .add_systems(Update, (spawn_card, press_card, card_text).chain());
    }
}

fn init_destination(mut commands: Commands, opts: Res<LaunchOptions>) {
    let dir = crate::asset_root_path().join("maps");
    let mut maps: Vec<(String, String)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "ron" {
                return None;
            }
            let id = path.file_stem()?.to_str()?.to_owned();
            // Practice isn't a place the truck drives to.
            if id == crate::camp::TRAINING_YARD {
                return None;
            }
            let text = std::fs::read_to_string(&path).ok()?;
            let map = extraction_core::map::MapDef::from_ron(&text).ok()?;
            Some((id, map.name))
        })
        .collect();
    maps.sort();
    if !maps.iter().any(|(id, _)| *id == opts.map) {
        maps.push((opts.map.clone(), opts.map.clone()));
    }
    let index = maps.iter().position(|(id, _)| *id == opts.map).unwrap_or(0);
    commands.insert_resource(Destination { maps, index });
}

fn spawn_card(mut commands: Commands, trucks: Query<Entity, Added<Truck>>) {
    for truck in &trucks {
        commands.spawn((
            Name::new("Route card"),
            RouteCard,
            Pressable::Destination,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(0.3, 0.2, 0.03),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.3, 0.2),
                style: ScreenStyle::Card,
            },
            // On the right canvas wall, facing in, opposite the driver's note.
            Transform::from_xyz(1.12, FLOOR + 1.55, -1.0)
                .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            ChildOf(truck),
        ));
    }
}

fn press_card(
    mut events: MessageReader<InteractEvent>,
    cards: Query<(), With<RouteCard>>,
    mut dest: ResMut<Destination>,
    mut raid: ResMut<super::Raid>,
    opts: Res<LaunchOptions>,
) {
    for ev in events.read() {
        if let InteractEvent::Pressed { item, .. } = ev {
            if cards.contains(*item)
                && dest.maps.len() > 1
                && opts.map != crate::camp::TRAINING_YARD
            {
                dest.index = (dest.index + 1) % dest.maps.len();
                let line = if dest.id() == opts.map {
                    format!("Next drive: {}. Staying put.", dest.name())
                } else {
                    format!("Next drive: {}. Ring the bell to go.", dest.name())
                };
                raid.say(line);
            }
        }
    }
}

fn card_text(
    dest: Res<Destination>,
    opts: Res<LaunchOptions>,
    mut cards: Query<&mut ScreenText, With<RouteCard>>,
    added: Query<(), Added<RouteCard>>,
) {
    if !dest.is_changed() && added.is_empty() {
        return;
    }
    // In the yard the cord goes on to the career's valley; the card can't
    // change that.
    if opts.map == crate::camp::TRAINING_YARD {
        let to = crate::persist::next_valley(&opts);
        let name = dest
            .maps
            .iter()
            .find(|(id, _)| *id == to)
            .map_or(to.clone(), |(_, n)| n.clone());
        for mut c in &mut cards {
            c.text = format!("NEXT DRIVE\n{}\n(pull the cord)", name.to_uppercase());
        }
        return;
    }
    let here = dest.id() == opts.map;
    let text = format!(
        "NEXT DRIVE\n{}\n{}",
        dest.name().to_uppercase(),
        if here {
            "(here again)"
        } else {
            "(ring the bell to go)"
        }
    );
    for mut c in &mut cards {
        c.text = text.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::body::Backpack;
    use crate::raid::truck::Stash;

    fn open(path: &std::path::Path, map: &str) -> App {
        let opts = LaunchOptions {
            robots: false,
            map: map.into(),
            save: Some(path.to_owned()),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        app
    }

    fn stash_len(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<&Backpack, With<Stash>>()
            .iter(app.world())
            .map(|b| b.contents.len())
            .sum()
    }

    #[test]
    fn the_truck_drives_to_the_other_valley() {
        let _restarts = crate::RESTART_TEST
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir()
            .join(format!("extraction-test-{}", std::process::id()))
            .join("route");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("career.ron");

        let mut a = open(&path, "marrow_creek");
        let stash = stash_len(&mut a);
        assert!(stash > 0, "a new career starts with a kit in the stash");
        {
            let mut d = a.world_mut().resource_mut::<Destination>();
            d.index = d
                .maps
                .iter()
                .position(|(id, _)| id == "kettle_pass")
                .expect("Kettle Pass is on the card");
        }
        let player = a
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(a.world())
            .next()
            .unwrap();
        a.world_mut()
            .write_message(crate::raid::DepartRequest { player });
        for _ in 0..3 {
            a.update();
        }
        assert_eq!(
            crate::take_restart().map(|r| r.map).as_deref(),
            Some("kettle_pass")
        );
        let career =
            extraction_core::save::Career::from_ron(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
        assert_eq!(career.map, "kettle_pass", "the career is headed there");
        assert!(
            career
                .zones
                .iter()
                .any(|z| z.map == "marrow_creek" && z.rolled),
            "Marrow Creek is kept as it was left"
        );

        // Morning in the new valley: its own Zone, the same stash.
        let mut b = open(&path, "kettle_pass");
        assert_eq!(stash_len(&mut b), stash, "the stash came along");
        b.world_mut().write_message(crate::persist::SaveNow);
        b.update();
        let career =
            extraction_core::save::Career::from_ron(&std::fs::read_to_string(&path).unwrap())
                .unwrap();
        assert_eq!(career.zones.len(), 2, "both valleys are in the career");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
