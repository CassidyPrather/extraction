//! The minelayer's fields in the valleys: where the game's minelayer
//! (`robots::minelayer`, its rules [`crate::minelayer`]) will sow its
//! mines on a map, worked out the way the game picks them, and checked
//! (`Rule::Mines`, see `docs/MAP_DESIGN.md`, "Mines").
//!
//! The game sends it out from the robots' Gate once a raid to the points
//! of interest nearest the insertion: `Poi` markers on walkable ground,
//! nearest first, the very nearest skipped, the next [`FIELDS`]. At each
//! it lays [`minelayer::site_field`] across the way it came (the last
//! step of its walk there), from a hopper of [`minelayer::HOPPER`].
//!
//! A field is a find, not a trap: no mine on a main route
//! ([`ON_ROUTE`] of the walk a player would take, between its samples),
//! none in a door's strip or a kiosk's, and the site good for a field: at
//! least half its mines survive [`minelayer::spot_ok`].

use crate::geom::{P2, dist, polyline_closest};
use crate::map::{MapDef, MarkerKind};
use crate::map_sentry::Checked;
use crate::minelayer;
use crate::nav::NavGrid;
use crate::terrain::Heightmap;

/// Fields a minelayer lays a raid (the game's `robots::minelayer::FIELDS`;
/// keep the two the same).
pub const FIELDS: usize = 2;
/// A mine this near a main route's line is on it: its plate and a boot.
pub const ON_ROUTE: f32 = minelayer::PLATE + 0.5;

/// One field as the game would lay it.
#[derive(Clone, Debug)]
pub struct MineField {
    /// The point of interest it's laid at.
    pub name: String,
    pub site: P2,
    /// The way it came (x, z).
    pub along: P2,
    /// Where the mines go, [`minelayer::spot_ok`] passed.
    pub mines: Vec<P2>,
    /// How many of them the hopper has left to lay.
    pub laid: usize,
}

/// The fields the game's minelayer lays on `map`, in order. `walk` is the
/// walkers' grid (the game's `walker::Nav`).
pub fn fields(map: &MapDef, hm: &Heightmap, walk: &NavGrid) -> Vec<MineField> {
    let gate = map
        .markers_of(&MarkerKind::RobotGate)
        .next()
        .map_or([0.0, -280.0], |m| m.at);
    let ins = map
        .markers_of(&MarkerKind::Insertion)
        .next()
        .map_or([0.0, 0.0], |m| m.at);
    let mut pois: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Poi)
        .filter(|m| walk.walkable(m.at))
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    pois.sort_by(|a, b| dist(a.1, ins).total_cmp(&dist(b.1, ins)));
    let mut here = gate;
    let mut hopper = minelayer::HOPPER as usize;
    let mut out = Vec::new();
    for (name, site) in pois.into_iter().skip(1).take(FIELDS) {
        if hopper == 0 {
            break;
        }
        let path = walk.path(here, site).unwrap_or_else(|| vec![site]);
        let along = match path.len() {
            0 | 1 => [site[0] - here[0], site[1] - here[1]],
            n => [
                path[n - 1][0] - path[n - 2][0],
                path[n - 1][1] - path[n - 2][1],
            ],
        };
        let mines = minelayer::site_field(map, hm, site, along);
        let laid = mines.len().min(hopper);
        hopper -= laid;
        // It drives on from over the last one it laid.
        here = mines[..laid].last().copied().unwrap_or(site);
        out.push(MineField {
            name: name.to_string(),
            site,
            along,
            mines,
            laid,
        });
    }
    out
}

/// The fields against the rules above. `routes` are the main routes'
/// points.
pub fn check(
    map: &MapDef,
    hm: &Heightmap,
    walk: &NavGrid,
    routes: &[(String, Vec<P2>)],
) -> Checked {
    let mut out = Checked::default();
    let doors = crate::map_doors::doors(map);
    let strips = crate::map_surge::door_strips(map);
    let fields = fields(map, hm, walk);
    let mut line = Vec::new();
    for f in &fields {
        let near = routes
            .iter()
            .map(|(_, pts)| polyline_closest(pts, f.site).0)
            .fold(f32::INFINITY, f32::min);
        line.push(format!(
            "{} ({:.0}, {:.0}) {} of {} spots, {} laid, a main route {:.0} m off",
            f.name,
            f.site[0],
            f.site[1],
            f.mines.len(),
            minelayer::PER_FIELD,
            f.laid,
            near
        ));
        if (f.mines.len() as u32) * 2 < minelayer::PER_FIELD {
            out.findings.push((
                format!(
                    "the minelayer's field at {} has room for {} of its {} mines",
                    f.name,
                    f.mines.len(),
                    minelayer::PER_FIELD
                ),
                f.site,
            ));
        }
        for m in &f.mines {
            if let Some((to, _)) = routes
                .iter()
                .find(|(_, pts)| polyline_closest(pts, *m).0 < ON_ROUTE)
            {
                out.findings.push((
                    format!(
                        "a mine of the field at {} lies on the route to {to}",
                        f.name
                    ),
                    *m,
                ));
            }
            let in_door = doors
                .iter()
                .find(|d| d.strip(*m) < minelayer::PLATE)
                .map(|d| format!("the {}'s door", map.placements[d.placement].prefab))
                .or_else(|| {
                    strips
                        .iter()
                        .find(|(_, r)| r.sdf(*m) < minelayer::PLATE)
                        .map(|(what, _)| what.clone())
                });
            if let Some(what) = in_door {
                out.findings.push((
                    format!("a mine of the field at {} lies in {what}", f.name),
                    *m,
                ));
            }
        }
    }
    out.measures.push(format!(
        "minelayer fields: {}",
        if line.is_empty() {
            "none (no points of interest past the nearest)".into()
        } else {
            line.join("; ")
        }
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field laid across a main route is found, and so is one in a
    /// doorway when the minelayer's own care is bypassed.
    #[test]
    fn a_field_on_a_route_is_found() {
        let map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        let walk = crate::map_nav::zone_grid(&map, &hm, crate::map_nav::MAX_SLOPE);
        let fs = fields(&map, &hm, &walk);
        // Only two of its points of interest stand on open ground: one field.
        assert_eq!(fs.len(), 1, "{fs:?}");
        // A route straight through the first field's middle mine.
        let m = fs[0].mines[fs[0].mines.len() / 2];
        let route = vec![[m[0] - 20.0, m[1] + 0.7], [m[0] + 20.0, m[1] + 0.7]];
        let c = check(&map, &hm, &walk, &[("Test".into(), route)]);
        assert!(
            c.findings
                .iter()
                .any(|f| f.0.contains("on the route to Test")),
            "{:?}",
            c.findings
        );
    }
}
