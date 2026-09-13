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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapDisplayCoordinate {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateLocation {
    pub map_id: String,
    pub map_names: game_knowledge::LocaleNames,
    pub location: WorldCoordinate,
    pub normalized: NormalizedMapCoordinate,
    pub map_display: MapDisplayCoordinate,
    pub approximate_region: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MapPointDistance {
    pub distance_meters: f64,
    pub bearing_degrees: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NearbyMapPoint {
    pub id: String,
    pub name: String,
    pub names: game_knowledge::LocaleNames,
    pub kind: MapPointKind,
    pub location: WorldCoordinate,
    pub map_display: MapDisplayCoordinate,
    pub distance_map_display: f64,
    pub distance_meters: f64,
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
    pub distance_meters: f64,
    pub bearing_degrees: f64,
    pub within_reviewed_radius: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelAnchor {
    pub id: String,
    pub name: String,
    pub names: game_knowledge::LocaleNames,
    pub kind: Option<MapPointKind>,
    pub location: WorldCoordinate,
    pub map_display: MapDisplayCoordinate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelAnchorSeed {
    pub id: String,
    pub name: String,
    pub location: WorldCoordinate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TravelRoute {
    pub destination_name: Option<String>,
    pub from: WorldCoordinate,
    pub from_map_display: MapDisplayCoordinate,
    pub to: WorldCoordinate,
    pub to_map_display: MapDisplayCoordinate,
    pub direct_distance_meters: f64,
    pub direct_distance_map_display: f64,
    pub from_nearest_fast_travel: TravelAnchor,
    pub to_nearest_fast_travel: TravelAnchor,
    pub fast_travel_distance_meters: f64,
    pub fast_travel_distance_map_display: f64,
    pub recommended_mode: String,
    pub recommended_distance_meters: f64,
    pub recommended_distance_map_display: f64,
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
        let regions = self
            .store()
            .map_regions()
            .filter(|region| region.map_id == map.id)
            .collect::<Vec<_>>();
        let reviewed = regions
            .iter()
            .filter(|region| region.boundary_is_reviewed)
            .count();
        let containing = regions
            .iter()
            .filter_map(|region| region.geometry.map(|geometry| (*region, geometry)))
            .filter(|(_, geometry)| geometry.contains(&location))
            .collect::<Vec<_>>();
        let chosen = containing
            .iter()
            .min_by(|left, right| {
                left.1
                    .volume()
                    .total_cmp(&right.1.volume())
                    .then_with(|| left.0.id.cmp(&right.0.id))
            })
            .copied();
        let mut provenances = vec![&map.provenance];
        provenances.extend(chosen.map(|(region, _)| &region.provenance));
        let mut answer = self.context().answer(
            AnswerStatus::Ok,
            Some(CoordinateLocation {
                map_id: map.id.clone(),
                map_names: map.names.clone(),
                location,
                normalized,
                map_display: world_to_map_display(location),
                approximate_region: chosen.map(|(region, _)| region.names.en.clone()),
            }),
            provenances,
            Vec::new(),
        );
        match chosen {
            Some((region, _)) => {
                let nested = containing
                    .iter()
                    .filter(|(other, _)| other.id != region.id)
                    .map(|(other, _)| other.names.en.clone())
                    .collect::<Vec<_>>();
                if !nested.is_empty() {
                    answer.uncertainty.push(format!(
                        "this coordinate also falls inside {}; the smallest reviewed region boundary was reported",
                        nested.join(", ")
                    ));
                }
            }
            None if reviewed == 0 => answer
                .uncertainty
                .push("region is unknown because reviewed boundary geometry is unavailable".into()),
            None if reviewed == regions.len() => answer
                .uncertainty
                .push("no reviewed region boundary contains this coordinate".into()),
            None => answer.uncertainty.push(format!(
                "no reviewed region boundary contains this coordinate; {} of {} regions on this map have no reviewed boundary",
                regions.len() - reviewed,
                regions.len()
            )),
        }
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
                map_display: world_to_map_display(point.location),
                distance_map_display: distance_map_display(&location, &point.location),
                distance_meters: distance_meters(&location, &point.location),
                bearing_degrees: bearing(&location, &point.location),
            })
            .collect::<Vec<_>>();
        points.sort_by(|left, right| {
            left.distance_meters
                .total_cmp(&right.distance_meters)
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
                    .ambiguous(self.ambiguous_message("Pal", &candidates))
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
            let leads = crate::describe_acquisition_leads(&pal.habitat_leads);
            let message = if leads.is_empty() {
                "this Pal has no reviewed target-build habitat coverage".to_string()
            } else {
                format!("this Pal has no fixed field habitat; habitat leads: {leads}")
            };
            return self.context().unknown(message);
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
                    distance_meters: distance_meters(&location, &zone.location),
                    bearing_degrees: bearing(&location, &zone.location),
                    within_reviewed_radius: distance <= zone.radius,
                }
            })
            .collect::<Vec<_>>();
        zones.sort_by(|left, right| {
            right
                .within_reviewed_radius
                .cmp(&left.within_reviewed_radius)
                .then_with(|| left.distance_meters.total_cmp(&right.distance_meters))
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

    pub fn plan_travel_route(
        &self,
        from: WorldCoordinate,
        to: WorldCoordinate,
        destination_name: Option<String>,
        extra_anchors: &[TravelAnchorSeed],
    ) -> GuideAnswer<TravelRoute> {
        let mut anchors = self
            .store()
            .map_points()
            .filter(|point| point.kind == MapPointKind::FastTravel)
            .map(|point| TravelAnchor {
                id: point.id.clone(),
                name: point.names.en.clone(),
                names: point.names.clone(),
                kind: Some(MapPointKind::FastTravel),
                location: point.location,
                map_display: world_to_map_display(point.location),
            })
            .collect::<Vec<_>>();
        anchors.extend(extra_anchors.iter().map(|anchor| TravelAnchor {
            id: anchor.id.clone(),
            name: anchor.name.clone(),
            names: game_knowledge::LocaleNames {
                en: anchor.name.clone(),
                zh_hans: Some(anchor.name.clone()),
            },
            kind: None,
            location: anchor.location,
            map_display: world_to_map_display(anchor.location),
        }));
        if anchors.is_empty() {
            return self
                .context()
                .unknown("no fast-travel anchors are available for routing");
        }

        let nearest_to = |origin: &WorldCoordinate| -> TravelAnchor {
            anchors
                .iter()
                .min_by(|left, right| {
                    distance_meters(origin, &left.location)
                        .total_cmp(&distance_meters(origin, &right.location))
                        .then_with(|| left.id.cmp(&right.id))
                })
                .expect("anchor list is non-empty")
                .clone()
        };
        let from_anchor = nearest_to(&from);
        let to_anchor = nearest_to(&to);
        let direct_distance_meters = distance_meters(&from, &to);
        let direct_distance_map_display = distance_map_display(&from, &to);
        let fast_travel_distance_meters = distance_meters(&from, &from_anchor.location)
            + distance_meters(&to, &to_anchor.location);
        let fast_travel_distance_map_display = distance_map_display(&from, &from_anchor.location)
            + distance_map_display(&to, &to_anchor.location);
        let recommended_mode = if fast_travel_distance_meters > direct_distance_meters {
            "direct"
        } else {
            "fast_travel"
        };
        let recommended_distance_meters = direct_distance_meters.min(fast_travel_distance_meters);
        let recommended_distance_map_display =
            direct_distance_map_display.min(fast_travel_distance_map_display);
        let route = TravelRoute {
            destination_name,
            from,
            from_map_display: world_to_map_display(from),
            to,
            to_map_display: world_to_map_display(to),
            direct_distance_meters,
            direct_distance_map_display,
            from_nearest_fast_travel: from_anchor,
            to_nearest_fast_travel: to_anchor,
            fast_travel_distance_meters,
            fast_travel_distance_map_display,
            recommended_mode: recommended_mode.to_string(),
            recommended_distance_meters,
            recommended_distance_map_display,
        };
        let provenances = [
            &route.from_nearest_fast_travel.id,
            &route.to_nearest_fast_travel.id,
        ]
        .into_iter()
        .filter_map(|id| self.store().map_point(id))
        .map(|point| &point.provenance)
        .collect::<Vec<_>>();
        let mut answer =
            self.context()
                .answer(AnswerStatus::Ok, Some(route), provenances, Vec::new());
        answer
            .uncertainty
            .push("distances are straight-line values, not terrain-verified travel paths".into());
        if extra_anchors.is_empty() {
            answer.uncertainty.push(
                "route considered reviewed fast-travel points only; player base camps were not included"
                    .into(),
            );
        }
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

fn distance_meters(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    distance(origin, target) / 100.0
}

fn distance_map_display(origin: &WorldCoordinate, target: &WorldCoordinate) -> f64 {
    let origin_display = world_to_map_display(*origin);
    let target_display = world_to_map_display(*target);
    (target_display.x - origin_display.x).hypot(target_display.y - origin_display.y)
}

pub fn map_display_to_world(display: MapDisplayCoordinate) -> WorldCoordinate {
    WorldCoordinate {
        x: -123_509.0 + display.y * 459.0,
        y: 159_622.0 + display.x * 459.0,
        z: 0.0,
    }
}

pub fn world_to_map_display(location: WorldCoordinate) -> MapDisplayCoordinate {
    MapDisplayCoordinate {
        x: (location.y - 159_622.0) / 459.0,
        y: (location.x + 123_509.0) / 459.0,
    }
}
