//! How often each kind of thing has been in your hand (#107, the user:
//! "Once somebody picks something up enough times, you should stop
//! automatically popping up the tooltip"). Counted per kind, kept in the
//! save; past [`QUIET_AFTER`] grabs a kind's own callout (what it is, how
//! it works, junk's tag) waits to be asked for (`render::said`).

use bevy::prelude::*;

pub use extraction_core::save::QUIET_AFTER;

/// Grabs by kind id, the local player's.
#[derive(Resource, Default, Debug, Clone)]
pub struct Handled(pub std::collections::BTreeMap<String, u32>);

impl Handled {
    /// Whether a kind's own callout still shows on its own: through its
    /// first `QUIET_AFTER` grabs (the one just made counted).
    pub fn still_told(&self, id: &str) -> bool {
        self.0.get(id).copied().unwrap_or(0) <= QUIET_AFTER
    }
}

pub struct HandledPlugin;

impl Plugin for HandledPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Handled>()
            .add_systems(Update, count_grabs);
    }
}

/// One more for each thing the local player takes in hand.
pub fn count_grabs(
    taken: Query<(&super::Item, &crate::interact::Held), Added<crate::interact::Held>>,
    locals: Query<Entity, With<crate::rig::LocalPlayer>>,
    catalog: Res<super::Catalog>,
    mut handled: ResMut<Handled>,
) {
    let me = locals.iter().next();
    for (item, held) in &taken {
        if Some(held.player) == me {
            *handled
                .0
                .entry(catalog.get(item.kind()).id.clone())
                .or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_is_told_through_its_first_grabs_then_waits() {
        let mut h = Handled::default();
        assert!(h.still_told("pm_pistol"));
        for n in 1..=QUIET_AFTER {
            h.0.insert("pm_pistol".into(), n);
            assert!(h.still_told("pm_pistol"), "grab {n}");
        }
        h.0.insert("pm_pistol".into(), QUIET_AFTER + 1);
        assert!(!h.still_told("pm_pistol"));
        assert!(h.still_told("crackers"), "per kind");
    }
}
