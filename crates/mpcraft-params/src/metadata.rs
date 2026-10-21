//! The metadata of the parameters.
//!
//! # What this is
//!
//! Every parameter of every autopilot has a description, a unit, a range it takes and,
//! sometimes, a list of the values it can be given. Upstream keeps that in
//! `ParameterFactMetaData.xml`: it is the knowledge of the autopilots, not of the
//! interface.
//!
//! The catalogue here is that file, converted once into a compact form the crate carries,
//! so that the application does not have to carry a parser for a two-megabyte XML file to
//! draw a list of parameters. The conversion is made by
//! `scripts/convert-parameter-metadata.py`.
//!
//! # What it is for
//!
//! A parameter editor that shows a name and a number is a table of numbers. The
//! description, the unit and the range are what turn it into something an operator can
//! work with: they say what the number means and what it is allowed to be.
//!
//! # The parameters that are not in it
//!
//! The catalogue knows the parameters of the autopilots it was written for. A parameter
//! that is not in it is not an error: it simply has no description, and the interface
//! shows the name and the value alone.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The catalogue, as it is stored.
const CATALOGUE: &str = include_str!("../data/parameters.toml");

/// One parameter of the catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterMeta {
    /// The name of the parameter.
    pub name: String,
    /// The group it belongs to, for example `UAVCAN Motor Parameters`.
    #[serde(default)]
    pub group: String,
    /// The name the parameter is shown with.
    #[serde(default)]
    pub display_name: String,
    /// The type of the parameter.
    #[serde(default)]
    pub kind: String,
    /// The value the parameter has by default.
    #[serde(default)]
    pub default: String,
    /// What the parameter does, in a line.
    #[serde(default)]
    pub description: String,
    /// What the parameter does, at length.
    #[serde(default)]
    pub long_description: String,
    /// The unit the value is in.
    #[serde(default)]
    pub unit: String,
    /// The smallest value the parameter takes.
    #[serde(default)]
    pub minimum: String,
    /// The largest value the parameter takes.
    #[serde(default)]
    pub maximum: String,
    /// The range, as the documentation gives it: the two ends separated by a space.
    #[serde(default)]
    pub range: String,
    /// How much the value changes by when it is nudged.
    #[serde(default)]
    pub increment: String,
    /// Whether a change to the parameter needs a restart to take effect.
    #[serde(default)]
    pub reboot_required: String,
    /// Whether the parameter is one an operator touches often.
    #[serde(default)]
    pub user: String,
    /// The values the parameter can be given, when it is a choice.
    #[serde(default)]
    pub values: Vec<ParamChoice>,
    /// The flags of the mask, when the parameter is one.
    #[serde(default)]
    pub bitmask: Vec<ParamFlag>,
    /// The vehicle families the parameter belongs to.
    #[serde(default)]
    pub vehicles: Vec<String>,
}

/// One of the values a parameter can be given, when it is a choice.
///
/// A choice is a value and the name it is called: the number the vehicle stores and the
/// words the operator reads. The two belong together, because the words alone say
/// nothing about what has to be written to the vehicle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamChoice {
    /// The value the parameter takes when this choice is made.
    pub code: f64,
    /// What the choice is called.
    pub text: String,
}

impl ParameterMeta {
    /// Whether a change to the parameter needs a restart to take effect.
    ///
    /// The catalogue says so in words, and the words are "true" when it does.
    #[must_use]
    pub fn needs_reboot(&self) -> bool {
        self.reboot_required.eq_ignore_ascii_case("true")
    }
}

/// One of the flags of a mask.
///
/// A parameter that is a mask holds several switches in one number: each flag is a
/// part of it, and the flag is either raised or not. The part it is is the mask of the
/// flag, and the documentation writes the place it occupies in the number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamFlag {
    /// The mask of the flag: the part of the number that is raised when it is on.
    pub mask: u32,
    /// What the flag is called.
    pub text: String,
}

/// The catalogue, as it is stored.
#[derive(Debug, Clone, Default, Deserialize)]
struct Stored {
    #[serde(default)]
    parameters: Vec<ParameterMeta>,
}

/// The metadata of every parameter the catalogue knows.
static CATALOGUE_INDEX: OnceLock<HashMap<String, ParameterMeta>> = OnceLock::new();

/// The metadata of a parameter, if the catalogue knows it.
///
/// ```
/// use mpcraft_params::metadata;
///
/// let meta = metadata::of("RTL_ALT").expect("the catalogue knows it");
/// assert!(!meta.description.is_empty());
/// assert!(metadata::of("NOT_A_PARAMETER").is_none());
/// ```
#[must_use]
pub fn of(name: &str) -> Option<&'static ParameterMeta> {
    catalogue().get(name)
}

/// The groups of the catalogue, in alphabetical order.
///
/// The groups are the subjects the parameters belong to — *Geofence*, *EKF2* — and the
/// parameters that the documentation knows but the facts file does not belong to none.
#[must_use]
pub fn groups() -> Vec<&'static str> {
    let index = catalogue();

    let mut names: Vec<&str> = index
        .values()
        .map(|meta| meta.group.as_str())
        .filter(|group| !group.is_empty())
        .collect();

    names.sort_unstable();
    names.dedup();

    names
}

/// The vehicle families the catalogue knows.
#[must_use]
pub fn vehicles() -> Vec<&'static str> {
    let index = catalogue();

    let mut names: Vec<&str> = index
        .values()
        .flat_map(|meta| meta.vehicles.iter().map(|vehicle| vehicle.as_str()))
        .collect();

    names.sort_unstable();
    names.dedup();

    names
}

/// The catalogue, read once.
fn catalogue() -> &'static HashMap<String, ParameterMeta> {
    CATALOGUE_INDEX.get_or_init(|| {
        let stored: Stored = toml::from_str(CATALOGUE).unwrap_or_default();

        stored
            .parameters
            .into_iter()
            .map(|meta| (meta.name.clone(), meta))
            .collect()
    })
}

/// How many parameters the catalogue holds.
#[must_use]
pub fn len() -> usize {
    catalogue().len()
}

/// Whether the catalogue holds nothing.
#[must_use]
pub fn is_empty() -> bool {
    len() == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_knows_the_parameters_of_the_autopilots() {
        assert!(len() > 4000, "the catalogue holds {} parameters", len());
    }

    #[test]
    fn a_parameter_has_a_description_and_a_range() {
        let meta = of("RTL_ALT").expect("the catalogue knows it");

        assert!(!meta.description.is_empty());
        assert!(!meta.display_name.is_empty());
        assert!(!meta.range.is_empty());
        assert!(!meta.vehicles.is_empty());
    }

    #[test]
    fn a_parameter_knows_its_type_and_its_values() {
        let meta = of("GF_ACTION").expect("the catalogue knows it");

        assert_eq!(meta.kind, "int32");
        assert!(!meta.minimum.is_empty());
        assert!(!meta.maximum.is_empty());
    }

    #[test]
    fn a_parameter_the_catalogue_does_not_know_has_no_metadata() {
        assert!(of("NOT_A_PARAMETER").is_none());
    }

    #[test]
    fn the_groups_are_the_ones_of_the_autopilot() {
        let groups = groups();

        assert!(!groups.is_empty());
        assert!(groups.iter().all(|group| !group.is_empty()));
    }

    #[test]
    fn the_vehicles_are_the_families_of_the_autopilots() {
        let vehicles = vehicles();

        assert!(vehicles.contains(&"ArduCopter2"));
        assert!(vehicles.contains(&"ArduPlane"));
    }
}
