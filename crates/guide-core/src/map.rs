use game_knowledge::{MapPointKind, PalSpawnPlacementKind, WorldCoordinate};
use serde::{Deserialize, Serialize};

use crate::{AnswerStatus, GuideAnswer, GuideEngine};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedMapCoordinate {
    pub x: f64,
    pub y: f64,
    pub pixel_x: i64,
    pub pixel_y: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateLocation {
    pub map_id: String,
    pub map_names: game_knowledge::LocaleNames,
    pub location: WorldCoordinate,
    pub normalized: NormalizedMapCoordinate,
    pub approximate_region: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapPointDistance {
    pub distance: f64,
    pub bearing_degrees: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearbyMapPoint {
    pub id: String,
    pub name: String,
    pub names: game_knowledge::LocaleNames,
    pub kind: MapPointKind,
    pub location: WorldCoordinate,
    pub distance: f64,
    pub bearing_degrees: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearbyHabitatZone {
    pub zone_id: String,
    pub pal_id: String,
    pub placement_kind: PalSpawnPlacementKind,
    pub location: WorldCoordinate,
    pub radius: f64,
    pub level_min: u32,
    pub level_max: u32,
    pub distance: f64,
    pub bearing_degrees: f64,
    pub within_reviewed_radius: bool,
}

impl GuideEngine {
    pub fn locate_coordinate(&self, location: WorldCoordinate) -> GuideAnswer<CoordinateLocation> {
        let Some(map) = self
            .store()
            .maps()
            .filter(|map| {
                location.x >= map.bounds.min_x
                    && location.x <= map.bounds.max_x
                    && location.y >= map.bounds.min_y
                    && location.y <= map.bounds.max_y
            })
            .min_by_key(|map| (map.priority, map.id.clone()))
        else {
            return self
                .context()
                .unknown("coordinate is outside all reviewed map bounds");
        };
        let width = map.bounds.max_x - map.bounds.min_x;
        let height = map.bounds.max_y - map.bounds.min_y;
        let normalized = NormalizedMapCoordinate {
            x: (location.x - map.bounds.min_x) / width,
            y: (location.y - map.bounds.min_y) / height,
            pixel_x: ((location.x - map.bounds.min_x) / width * f64::from(map.logical_size)).round()
                as i64,
            pixel_y: ((location.y - map.bounds.min_y) / height * f64::from(map.logical_size))
                .round() as i64,
        };
        let mut answer = self.context().answer(
            AnswerStatus::Ok,
            Some(CoordinateLocation {
                map_id: map.id.clone(),
                map_names: map.names.clone(),
                location,
                normalized,
                approximate_region: None,
            }),
            vec![&map.provenance],
            Vec::new(),
        );
        answer
            .uncertainty
            .push("region is unknown because reviewed boundary geometry is unavailable".into());
        answer
    }

    pub fn find_nearby_map_points(
        &self,
        location: WorldCoordinate,
        kind: Option<MapPointKind>,
        limit: usize,
    ) -> GuideAnswer<Vec<NearbyMapPoint>> {
        let mut points = self
            .store()
            .map_points()
            .filter(|point| kind.is_none_or(|filter| point.kind == filter))
            .map(|point| NearbyMapPoint {
                id: point.id.clone(),
                name: point.names.en.clone(),
                names: point.names.clone(),
                kind: point.kind,
                location: point.location,
                distance: distance(&location, &point.location),
                bearing_degrees: bearing(&location, &point.location),
            })
            .collect::<Vec<_>>();
        points.sort_by(|left, right| {
            left.distance
                .total_cmp(&right.distance)
                .then_with(|| left.id.cmp(&right.id))
        });
        points.truncate(limit);
        let provenances = points
            .iter()
            .filter_map(|point| self.store().map_point(&point.id))
            .map(|point| &point.provenance)
            .collect::<Vec<_>>();
        let mut answer =
            self.context()
                .answer(AnswerStatus::Ok, Some(points), provenances, Vec::new());
        answer.uncertainty.push(
            "distances and bearings are straight-line values, not terrain-verified travel paths"
                .into(),
        );
        answer
    }

    pub fn find_pal_spawn_zones(
        &self,
        pal_query: &str,
        location: WorldCoordinate,
        limit: usize,
    ) -> GuideAnswer<Vec<NearbyHabitatZone>> {
        let resolution = match self.resolve(pal_query, Some(crate::EntityKind::Pal)) {
            crate::resolver::Resolution::Unique(resolution) => resolution,
            crate::resolver::Resolution::Ambiguous(candidates) => {
                return self
                    .context()
                    .ambiguous(crate::lookup::ambiguous_message("Pal", &candidates))
            }
            crate::resolver::Resolution::Unknown => {
                return self
                    .context()
                    .unknown("unknown Pal; no reviewed record matches")
            }
        };
        let Some(pal) = self.store().pal(&resolution.id) else {
            return self
                .context()
                .unknown("resolved Pal is absent from the store");
        };
        if pal.habitat_ids.is_empty() {
            return self
                .context()
                .unknown("this Pal has no reviewed target-build habitat coverage");
        }
        let mut zones = pal
            .habitat_ids
            .iter()
            .filter_map(|zone_id| self.store().pal_habitat_zone(zone_id))
            .map(|zone| {
                let distance = distance(&location, &zone.location);
                NearbyHabitatZone {
                    zone_id: zone.id.clone(),
                    pal_id: zone.pal_id.clone(),
                    placement_kind: zone.placement_kind,
                    location: zone.location,
                    radius: zone.radius,
                    level_min: zone.level_min,
                    level_max: zone.level_max,
                    distance,
                    bearing_degrees: bearing(&location, &zone.location),
                    within_reviewed_radius: distance <= zone.radius,
                }
            })
            .collect::<Vec<_>>();
        zones.sort_by(|left, right| {
            right
                .within_reviewed_radius
                .cmp(&left.within_reviewed_radius)
                .then_with(|| left.distance.total_cmp(&right.distance))
                .then_with(|| left.zone_id.cmp(&right.zone_id))
        });
        zones.truncate(limit);
        let provenances = zones
            .iter()
            .filter_map(|zone| self.store().pal_habitat_zone(&zone.zone_id))
            .map(|zone| &zone.provenance)
            .chain(std::iter::once(&pal.provenance))
            .collect::<Vec<_>>();
        let mut answer =
            self.context()
                .answer(AnswerStatus::Ok, Some(zones), provenances, Vec::new());
        answer.uncertainty.push(
            "zones are spawner volumes; actual presence still depends on game spawn state".into(),
        );
        answer
    }
}

fn bearing(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    let degrees = (target.x - origin.x)
        .atan2(target.y - origin.y)
        .to_degrees();
    (degrees + 360.0) % 360.0
}

fn distance(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    (target.x - origin.x).hypot(target.y - origin.y)
}
