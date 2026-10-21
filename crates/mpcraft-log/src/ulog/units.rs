//! What is known about the unit of the fields of the topics of PX4.
//!
//! # Why this is here
//!
//! A log of PX4 says the type of every value but not its unit. The topics that were
//! made when a double was too much to carry write a latitude as a whole number of
//! hundred-millionths of a degree and a height as a whole number of millimetres; the
//! topics that came later write the same values as doubles of degrees and of metres.
//!
//! The two cannot be told apart from the log alone, but they can be told apart by the
//! type: a scaled value is always a whole number, and a value of a floating type is
//! always already in its unit. The reader applies the scale of the table below only to
//! the fields of a whole number, and gives the values in the unit they belong to.

use super::ULogType;

/// What is known about the unit of one field of one topic of PX4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldUnit {
    /// The name of the topic the field belongs to.
    pub topic: &'static str,
    /// The name of the field.
    pub field: &'static str,
    /// The unit the value is in.
    pub unit: &'static str,
    /// The factor the value the log holds is multiplied by.
    pub scale: f64,
    /// What is added to the value once it is multiplied.
    pub offset: f64,
    /// Whether the factor is only applied to a field of a whole number.
    ///
    /// A field of a floating type is already in its unit, and scaling it would move
    /// it away from it.
    pub whole_only: bool,
}

/// The unit of a field of a topic, when it is known.
///
/// The factor of the entry is only applied when the field is of a whole number,
/// unless the entry says otherwise.
#[must_use]
pub fn unit_of(topic: &str, field: &str) -> Option<&'static FieldUnit> {
    UNITS
        .iter()
        .find(|unit| unit.topic == topic && unit.field == field)
}

/// The unit of a field of a topic, and the factor that goes with it.
///
/// The factor is one when the field is not of a whole number, because a value of a
/// floating type is already in its unit.
#[must_use]
pub fn scale_of(topic: &str, field: &str, kind: &ULogType) -> (Option<&'static str>, f64, f64) {
    match unit_of(topic, field) {
        Some(unit) if !unit.whole_only || kind.is_integer() => {
            (Some(unit.unit), unit.scale, unit.offset)
        }
        Some(unit) => (Some(unit.unit), 1.0, 0.0),
        None => (None, 1.0, 0.0),
    }
}

/// A position is written as a scaled whole number in the topics that were made when a
/// double was too much to carry: the latitude and the longitude in hundred-millionths
/// of a degree, and the height over the sea in millimetres.
const UNITS: &[FieldUnit] = &[
    FieldUnit {
        topic: "vehicle_gps_position",
        field: "lat",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_gps_position",
        field: "lon",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_gps_position",
        field: "alt",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_gps_position",
        field: "alt_ellipsoid",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_gps_position",
        field: "noise_per_ms",
        unit: "m/s",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "ekf_gps_position",
        field: "lat",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "ekf_gps_position",
        field: "lon",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "ekf_gps_position",
        field: "alt",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "ekf_gps_position",
        field: "alt_ellipsoid",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "gps_dump",
        field: "lat",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "gps_dump",
        field: "lon",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "gps_dump",
        field: "alt",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "home_position",
        field: "lat",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "home_position",
        field: "lon",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "home_position",
        field: "alt",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_global_position",
        field: "lat",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_global_position",
        field: "lon",
        unit: "deg",
        scale: 1e-7,
        offset: 0.0,
        whole_only: true,
    },
    FieldUnit {
        topic: "vehicle_global_position",
        field: "alt",
        unit: "m",
        scale: 1e-3,
        offset: 0.0,
        whole_only: true,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_of_a_whole_number_is_applied() {
        let (unit, scale, offset) = scale_of("vehicle_gps_position", "lat", &ULogType::Int32);

        assert_eq!(unit, Some("deg"));
        assert_eq!(scale, 1e-7);
        assert_eq!(offset, 0.0);
    }

    #[test]
    fn the_scale_of_a_double_is_not_applied() {
        // A topic that came later writes the latitude as a double of degrees, and a
        // double of degrees is not to be scaled.
        let (unit, scale, _) = scale_of("vehicle_gps_position", "lat", &ULogType::Double);

        assert_eq!(unit, Some("deg"));
        assert_eq!(scale, 1.0);
    }

    #[test]
    fn a_field_that_is_not_known_has_no_unit() {
        let (unit, scale, _) = scale_of("vehicle_gps_position", "eph", &ULogType::Float);

        assert_eq!(unit, None);
        assert_eq!(scale, 1.0);
    }
}
