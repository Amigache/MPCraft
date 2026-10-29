//! The places an operator marked on the map.

use serde::{Deserialize, Serialize};

/// A point of interest: a place with a name, kept between two runs.
///
/// The position is in degrees, as the map and the vehicle speak them. The name is what the
/// operator wrote; two points with the same name at the same place are one point, which is the
/// rule upstream applies when it loads its file more than once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Poi {
    /// What the operator calls the place.
    pub name: String,
    /// Where it is.
    pub latitude: f64,
    /// Where it is.
    pub longitude: f64,
}

impl Poi {
    /// Whether this point is the same as another, which is what keeps a load from piling the
    /// same marker on top of itself.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        self.name == other.name
            && self.latitude == other.latitude
            && self.longitude == other.longitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_place_is_the_same_only_with_the_same_name_and_position() {
        let place = Poi {
            name: "Gate".to_owned(),
            latitude: -35.0,
            longitude: 149.0,
        };

        assert!(place.same(&place));
        assert!(!place.same(&Poi {
            name: "Gate".to_owned(),
            latitude: -35.0,
            longitude: 149.1,
        }));
        assert!(!place.same(&Poi {
            name: "Shed".to_owned(),
            latitude: -35.0,
            longitude: 149.0,
        }));
    }
}
