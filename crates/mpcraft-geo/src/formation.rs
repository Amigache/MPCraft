//! The slots of a formation, and the place each slot puts a vehicle in.
//!
//! Upstream keeps one offset per vehicle (`Swarm/Formation.cs`, `Swarm/Sequence/Layout.cs`): the
//! operator says where each vehicle sits relative to the one it follows, and the ground station
//! turns those offsets into positions. An offset is in metres in the projected frame — `x` across
//! the direction of travel, `y` along it, `z` up — and it is turned by the heading of the leader
//! before it is applied, which is what upstream does in `SendCommand`.
//!
//! Two things are done differently here, on purpose:
//!
//! * Upstream invents an offset for a vehicle that has none (`Vector3(offsets.Count, 0, 0)`), so
//!   where a vehicle goes depends on how many vehicles were left out at that moment. MPCraft
//!   refuses and names the vehicle with no slot, because a formation is only safe when every
//!   member of it was given a place.
//! * Upstream takes the UTM zone from `(lng + 186) / 6`, which is not the zone the projection
//!   would pick. MPCraft projects through the zone its own projection chooses (see
//!   [`crate::projection]`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::projection::{utm_from_lat_lng, utm_to_lat_lng, Utm, UtmZoneError};
use crate::{GeoPoint, LatLng};

/// Where a vehicle sits relative to the one it follows, in metres.
///
/// `x` is across the direction of travel and `y` along it — the naming upstream's `Vector3` uses,
/// kept so that a layout written by upstream means the same thing here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Offset {
    /// Metres across the direction of travel, positive to the right.
    pub x: f64,
    /// Metres along the direction of travel, positive ahead.
    pub y: f64,
    /// Metres above the leader.
    pub z: f64,
}

impl Offset {
    /// Creates an offset.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The straight-line distance from the vehicle being followed.
    #[must_use]
    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// Which vehicle sits in which slot.
///
/// The key is the system id of the vehicle. The delays are upstream's: a formation is held for
/// `delay_end` seconds after the last vehicle arrives, and starts moving `delay_start` seconds
/// after the leader does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// The name the layout is saved under.
    pub id: String,
    /// Seconds the formation waits before it starts moving.
    pub delay_start: u32,
    /// Seconds the formation holds once every vehicle is in its slot.
    pub delay_end: u32,
    /// The slot each vehicle was given.
    pub slots: BTreeMap<u8, Offset>,
}

impl Layout {
    /// Creates an empty layout.
    #[must_use]
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            delay_start: 0,
            delay_end: 0,
            slots: BTreeMap::new(),
        }
    }

    /// Gives a vehicle its slot.
    pub fn set_slot(&mut self, system_id: u8, offset: Offset) {
        self.slots.insert(system_id, offset);
    }

    /// The slot a vehicle was given, if it was given one.
    pub fn slot(&self, system_id: u8) -> Option<Offset> {
        self.slots.get(&system_id).copied()
    }
}

/// The error a formation cannot work around.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum FormationError {
    /// A vehicle was in the formation but was never given a place in it.
    #[error("vehicle {0} has no slot in the formation")]
    MissingSlot(u8),
    /// The leader is where a UTM zone cannot be picked, which is where a metre measured in that
    /// frame cannot be turned back into a place.
    #[error("the leader cannot be projected: {0}")]
    Projection(#[from] UtmZoneError),
}

/// The place a slot puts a vehicle, with the altitude of the leader plus the height of the slot.
///
/// The rotation is upstream's: an offset facing the direction of travel is turned by the heading,
/// so a vehicle that sits ahead and to the right stays ahead and to the right however the leader
/// is pointed.
pub fn place(leader: GeoPoint, heading_deg: f64, slot: Offset) -> Result<GeoPoint, FormationError> {
    let base = utm_from_lat_lng(LatLng::new(leader.lat, leader.lng))?;

    // Upstream's rotation is applied to a clockwise compass heading (`heading = -Leader.cs.yaw`,
    // `Formation.cs` line 97), so the sine terms carry the opposite sign to a plain
    // counter-clockwise rotation. With that sign, `y` is ahead and `x` is to the right of the
    // heading however the leader is pointed; with the other sign the formation mirrors.
    let heading = heading_deg.to_radians();

    let easting = base.easting + slot.x * heading.cos() + slot.y * heading.sin();
    let northing = base.northing - slot.x * heading.sin() + slot.y * heading.cos();

    // The zone is kept rather than re-picked, so that a slot does not land in the next zone over
    // when the leader is near a zone boundary.
    let moved = utm_to_lat_lng(Utm {
        zone: base.zone,
        northern: base.northern,
        easting,
        northing,
    })?;

    Ok(GeoPoint::with_alt(
        moved.lat,
        moved.lng,
        leader.alt + slot.z,
    ))
}

/// The place each vehicle of the formation is sent to.
///
/// Membership is kept apart from the slots, as it is upstream: a vehicle can be in the formation
/// without having been given a place in it. That is an error naming the vehicle, not a position
/// worked out from how many vehicles were missing, which is what upstream's fallback does.
pub fn places(
    leader: GeoPoint,
    heading_deg: f64,
    layout: &Layout,
    members: &[u8],
) -> Result<Vec<(u8, GeoPoint)>, FormationError> {
    members
        .iter()
        .map(|&system_id| {
            let slot = layout
                .slot(system_id)
                .ok_or(FormationError::MissingSlot(system_id))?;
            Ok((system_id, place(leader, heading_deg, slot)?))
        })
        .collect()
}

/// Wraps an angle to the range -180° to 180°, which is what a heading error has to be read in.
#[must_use]
pub const fn wrap_180(degrees: f64) -> f64 {
    if degrees > 180.0 {
        return degrees - 360.0;
    }

    if degrees < -180.0 {
        return degrees + 360.0;
    }

    degrees
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: GeoPoint = GeoPoint::with_alt(-35.2809, 149.1300, 100.0);

    // A leader on the central meridian of its zone, where the grid directions are true
    // directions: the convergence angle is zero there, so a metre along the grid northing is a
    // metre due north and not merely a metre along the grid.
    const ON_MERIDIAN: GeoPoint = GeoPoint::with_alt(-35.2809, 147.0, 100.0);

    #[test]
    fn a_slot_ahead_of_a_leader_heading_north_is_a_metre_north() {
        let place_in = place(ON_MERIDIAN, 0.0, Offset::new(0.0, 10.0, 0.0)).expect("in the band");

        assert!(
            place_in.lat > ON_MERIDIAN.lat,
            "northing does not point north"
        );

        let (distance, bearing) = crate::great_circle(ON_MERIDIAN, place_in);

        assert!((distance - 10.0).abs() < 0.1, "distance was {distance}");
        assert!(
            bearing.abs() < 0.1 || (bearing - 360.0).abs() < 0.1,
            "bearing was {bearing}"
        );
    }

    #[test]
    fn the_same_slot_turns_with_the_heading() {
        // The slot that is ahead when the leader points north is ahead to the west when it points
        // west: `y` is along the direction of travel, so turning the frame turns the slot with it.
        let ahead_when_north = place(ON_MERIDIAN, 0.0, Offset::new(0.0, 10.0, 0.0)).expect("band");
        let ahead_when_west = place(ON_MERIDIAN, 270.0, Offset::new(0.0, 10.0, 0.0)).expect("band");

        let (turned_distance, turned_bearing) = crate::great_circle(ON_MERIDIAN, ahead_when_west);

        assert!(
            (turned_distance - 10.0).abs() < 0.1,
            "distance was {turned_distance}"
        );
        assert!(
            (turned_bearing - 270.0).abs() < 0.5,
            "bearing was {turned_bearing}"
        );

        // And the two are not the same place, which is what a rotation has to do.
        assert_ne!(ahead_when_north, ahead_when_west);
    }

    #[test]
    fn a_slot_to_the_right_is_measured_across_the_direction_of_travel() {
        let right_of_a_northern_leader =
            place(ON_MERIDIAN, 0.0, Offset::new(5.0, 0.0, 0.0)).expect("band");

        let (distance, bearing) = crate::great_circle(ON_MERIDIAN, right_of_a_northern_leader);

        assert!((distance - 5.0).abs() < 0.1, "distance was {distance}");
        assert!((bearing - 90.0).abs() < 0.5, "bearing was {bearing}");
    }

    #[test]
    fn a_grid_slot_is_not_a_true_bearing_away_from_the_central_meridian() {
        // Off the central meridian the grid and the earth disagree by the convergence angle, and
        // this is recorded rather than papered over: a slot measured east on the grid reads as a
        // true bearing about 1.2 degrees off at Canberra. Upstream has the same behaviour; a
        // formation held by true bearings would have to correct for it, and upstream does not.
        let right_of_canberra = place(HOME, 0.0, Offset::new(5.0, 0.0, 0.0)).expect("band");

        let bearing = crate::bearing_deg(HOME, right_of_canberra);

        assert!(
            (90.0 - bearing).abs() > 1.0 && (90.0 - bearing).abs() < 1.5,
            "convergence was {} degrees",
            90.0 - bearing
        );
    }

    #[test]
    fn the_height_of_a_slot_is_the_height_of_the_leader_plus_it() {
        let place_in = place(HOME, 45.0, Offset::new(0.0, 0.0, 20.0)).expect("band");

        assert_eq!(place_in.alt, 120.0);
    }

    #[test]
    fn a_slot_far_enough_away_keeps_the_distance_it_was_given() {
        // The projection is a plane and the earth is not, so the two agree only as long as the
        // formation is small. A hundred metres is well inside that, which is what a formation is.
        let far = place(HOME, 135.0, Offset::new(30.0, 90.0, 0.0)).expect("band");
        let distance = crate::distance_m(HOME, far);

        assert!(
            (distance - (30.0f64.powi(2) + 90.0f64.powi(2)).sqrt()).abs() < 0.2,
            "{distance}"
        );
    }

    #[test]
    fn a_vehicle_with_no_slot_is_named_rather_than_put_somewhere_by_default() {
        let mut layout = Layout::new("line");
        layout.set_slot(1, Offset::new(0.0, 10.0, 0.0));

        // Upstream would have sent vehicle 7 to `x = number of offsets so far`, which is what its
        // fallback does. Here the vehicle is named and nothing is placed.
        assert_eq!(
            places(HOME, 0.0, &layout, &[1, 7]),
            Err(FormationError::MissingSlot(7))
        );

        // And it is the first missing vehicle seen that gets named, not the last.
        assert_eq!(
            places(HOME, 0.0, &layout, &[7, 1]),
            Err(FormationError::MissingSlot(7))
        );

        // With a slot it is placed, and placement follows membership rather than the slots alone:
        // a vehicle given a slot but not in the formation is not sent anywhere.
        layout.set_slot(7, Offset::new(0.0, 20.0, 0.0));
        assert_eq!(
            places(HOME, 0.0, &layout, &[1, 7]),
            Ok(vec![
                (
                    1,
                    place(HOME, 0.0, Offset::new(0.0, 10.0, 0.0)).expect("band")
                ),
                (
                    7,
                    place(HOME, 0.0, Offset::new(0.0, 20.0, 0.0)).expect("band")
                ),
            ])
        );
        assert!(places(HOME, 0.0, &layout, &[])
            .expect("none of them is in the formation")
            .is_empty());

        let empty = Layout::new("empty");
        assert_eq!(
            places(HOME, 80.0, &empty, &[42]),
            Err(FormationError::MissingSlot(42))
        );
    }

    #[test]
    fn a_leader_outside_the_utm_band_is_refused() {
        let pole = GeoPoint::new(88.0, 149.0);

        assert!(matches!(
            place(pole, 0.0, Offset::new(1.0, 0.0, 0.0)),
            Err(FormationError::Projection(_))
        ));
    }

    #[test]
    fn a_heading_error_is_read_in_the_range_a_heading_error_can_be() {
        assert_eq!(wrap_180(190.0), -170.0);
        assert_eq!(wrap_180(-190.0), 170.0);
        assert_eq!(wrap_180(170.0), 170.0);
        assert_eq!(wrap_180(crate::bearing_deg(HOME, HOME) + 360.0), 0.0);
    }
}
