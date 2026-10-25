//! The vehicle registry.
//!
//! A link can carry several vehicles: the autopilot, a gimbal, a camera, a companion
//! computer. Each is identified by the pair `(system_id, component_id)`, which is what
//! Mission Planner calls `MAVList.GetID`.
//!
//! # Vehicle type detection
//!
//! The autopilot and the airframe are both reported in the heartbeat, and their combination
//! determines what the vehicle is. Mission Planner maps that combination onto a small
//! enumeration of firmware families; MPCraft keeps the same mapping because the rest of the
//! application branches on it.

use std::collections::HashMap;

use mpcraft_mavlink::{MavAutopilot, MavType};

/// The family of autopilot a vehicle belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VehicleKind {
    /// An ArduCopter-family multirotor.
    ArduCopter,
    /// An ArduPlane-family fixed wing.
    ArduPlane,
    /// An ArduRover-family ground vehicle.
    ArduRover,
    /// An ArduSub-family submersible.
    ArduSub,
    /// An ArduPilot antenna tracker.
    ArduTracker,
    /// A PX4 autopilot.
    Px4,
    /// Anything else.
    Other,
}

impl VehicleKind {
    /// A human-readable name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArduCopter => "ArduCopter",
            Self::ArduPlane => "ArduPlane",
            Self::ArduRover => "ArduRover",
            Self::ArduSub => "ArduSub",
            Self::ArduTracker => "ArduTracker",
            Self::Px4 => "PX4",
            Self::Other => "Other",
        }
    }

    /// Determines the vehicle family from the reported autopilot and airframe.
    ///
    /// ```
    /// use mpcraft_link::VehicleKind;
    /// use mpcraft_mavlink::{MavAutopilot, MavType};
    ///
    /// let kind = VehicleKind::from_heartbeat(
    ///     MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
    ///     MavType::MAV_TYPE_QUADROTOR,
    /// );
    /// assert_eq!(kind, VehicleKind::ArduCopter);
    /// ```
    #[must_use]
    pub fn from_heartbeat(autopilot: MavAutopilot, mav_type: MavType) -> Self {
        match autopilot {
            MavAutopilot::MAV_AUTOPILOT_PX4 => Self::Px4,
            MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA => match mav_type {
                MavType::MAV_TYPE_FIXED_WING
                | MavType::MAV_TYPE_FLAPPING_WING
                | MavType::MAV_TYPE_VTOL_TAILSITTER_DUOROTOR
                | MavType::MAV_TYPE_VTOL_TAILSITTER_QUADROTOR
                | MavType::MAV_TYPE_VTOL_TILTROTOR
                | MavType::MAV_TYPE_VTOL_FIXEDROTOR
                | MavType::MAV_TYPE_VTOL_TAILSITTER
                | MavType::MAV_TYPE_VTOL_TILTWING
                | MavType::MAV_TYPE_VTOL_RESERVED5 => Self::ArduPlane,
                MavType::MAV_TYPE_GROUND_ROVER | MavType::MAV_TYPE_SURFACE_BOAT => Self::ArduRover,
                MavType::MAV_TYPE_SUBMARINE => Self::ArduSub,
                MavType::MAV_TYPE_ANTENNA_TRACKER => Self::ArduTracker,
                MavType::MAV_TYPE_QUADROTOR
                | MavType::MAV_TYPE_COAXIAL
                | MavType::MAV_TYPE_HELICOPTER
                | MavType::MAV_TYPE_HEXAROTOR
                | MavType::MAV_TYPE_OCTOROTOR
                | MavType::MAV_TYPE_TRICOPTER => Self::ArduCopter,
                _ => Self::Other,
            },
            _ => Self::Other,
        }
    }
}

/// The identity of a vehicle on a link.
///
/// The type belongs to the vehicle model rather than to the link: the same pair identifies a
/// vehicle in the registry, in the telemetry, and in the log of a flight that has already
/// happened. Keeping one definition stops the two from disagreeing about what an id is.
pub use mpcraft_vehicle::VehicleId;

/// The state of one vehicle on the link.
#[derive(Debug, Clone)]
pub struct Vehicle {
    /// The identity of the vehicle.
    pub id: VehicleId,
    /// The family the vehicle belongs to, once its heartbeat has been seen.
    pub kind: Option<VehicleKind>,
    /// The vehicle type the vehicle reported.
    pub mav_type: Option<MavType>,
    /// The autopilot the vehicle reported.
    pub autopilot: Option<MavAutopilot>,
    /// The custom mode value the vehicle reported.
    pub custom_mode: Option<u32>,
    /// Whether the vehicle is the one the session selected.
    pub primary: bool,
}

impl Vehicle {
    /// Creates a vehicle record.
    #[must_use]
    pub fn new(id: VehicleId) -> Self {
        Self {
            id,
            kind: None,
            mav_type: None,
            autopilot: None,
            custom_mode: None,
            primary: false,
        }
    }
}

/// The vehicles seen on a link.
#[derive(Debug, Default)]
pub struct VehicleRegistry {
    vehicles: HashMap<VehicleId, Vehicle>,
}

impl VehicleRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a vehicle, creating it if it is new.
    pub fn observe(&mut self, id: VehicleId) -> &mut Vehicle {
        self.vehicles.entry(id).or_insert_with(|| Vehicle::new(id))
    }

    /// The number of vehicles seen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vehicles.len()
    }

    /// Whether no vehicle has been seen.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vehicles.is_empty()
    }

    /// The vehicles seen, ordered by their identity.
    #[must_use]
    pub fn vehicles(&self) -> Vec<&Vehicle> {
        let mut out: Vec<&Vehicle> = self.vehicles.values().collect();
        out.sort_by_key(|v| (v.id.system_id, v.id.component_id));
        out
    }

    /// The vehicle with the given identity.
    #[must_use]
    pub fn get(&self, id: VehicleId) -> Option<&Vehicle> {
        self.vehicles.get(&id)
    }

    /// The vehicle with the given identity, mutably.
    pub fn get_mut(&mut self, id: VehicleId) -> Option<&mut Vehicle> {
        self.vehicles.get_mut(&id)
    }

    /// The primary vehicle — the one the session selected.
    #[must_use]
    pub fn primary(&self) -> Option<&Vehicle> {
        self.vehicles.values().find(|v| v.primary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ardupilot_vehicles_are_classified_by_airframe() {
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                MavType::MAV_TYPE_QUADROTOR
            ),
            VehicleKind::ArduCopter
        );
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                MavType::MAV_TYPE_FIXED_WING
            ),
            VehicleKind::ArduPlane
        );
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                MavType::MAV_TYPE_GROUND_ROVER
            ),
            VehicleKind::ArduRover
        );
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                MavType::MAV_TYPE_SUBMARINE
            ),
            VehicleKind::ArduSub
        );
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                MavType::MAV_TYPE_ANTENNA_TRACKER
            ),
            VehicleKind::ArduTracker
        );
    }

    #[test]
    fn px4_is_recognised() {
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_PX4,
                MavType::MAV_TYPE_QUADROTOR
            ),
            VehicleKind::Px4
        );
    }

    #[test]
    fn an_unknown_autopilot_is_other() {
        assert_eq!(
            VehicleKind::from_heartbeat(
                MavAutopilot::MAV_AUTOPILOT_GENERIC,
                MavType::MAV_TYPE_QUADROTOR
            ),
            VehicleKind::Other
        );
    }

    #[test]
    fn the_registry_keeps_one_entry_per_vehicle() {
        let mut registry = VehicleRegistry::new();
        assert!(registry.is_empty());

        registry.observe(VehicleId::new(1, 1));
        registry.observe(VehicleId::new(1, 1));
        registry.observe(VehicleId::new(1, 154));

        assert_eq!(registry.len(), 2);
        assert!(registry.get(VehicleId::new(1, 154)).is_some());
    }

    #[test]
    fn the_primary_vehicle_is_the_one_that_was_selected() {
        let mut registry = VehicleRegistry::new();

        registry.observe(VehicleId::new(1, 1)).primary = true;
        registry.observe(VehicleId::new(1, 154));

        assert_eq!(registry.primary().map(|v| v.id), Some(VehicleId::new(1, 1)));
    }

    #[test]
    fn vehicle_ids_display_as_system_and_component() {
        assert_eq!(VehicleId::new(1, 1).to_string(), "1/1");
    }
}
