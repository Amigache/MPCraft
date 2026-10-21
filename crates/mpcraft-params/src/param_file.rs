//! The parameter file.
//!
//! Mission Planner reads and writes a vehicle's parameters as a plain text file: a line
//! per parameter, the name and the value separated by a comma, and the comments that
//! explain the file starting with a hash.
//!
//! The format is a domain specification — it is how a set of parameters travels between
//! two ground stations and between a ground station and a text editor — so it lives in
//! the parameter crate and not in the interface.
//!
//! ```
//! use mpcraft_params::{param_file, ParamType, Parameter};
//!
//! let text = "ARMING_CHECK,1\nRTL_ALT,100\n";
//!
//! let parameters = param_file::read(text).expect("the file is read");
//! assert_eq!(parameters.len(), 2);
//! assert_eq!(parameters[1].value, 100.0);
//! ```

use crate::{ParamType, Parameter, ParameterStore};

/// The failure modes of reading a parameter file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParamFileError {
    /// A line names a parameter whose value is not a number.
    #[error("the value of {name} is not a number: {value:?}")]
    NotANumber {
        /// The parameter the line names.
        name: String,
        /// What the line said.
        value: String,
    },
}

/// The parameters that are not real parameters.
///
/// They are the ones the vehicle writes for its own bookkeeping — the number of the
/// mission, the ground pressure of the last boot — and they are not part of the
/// configuration of the vehicle. Upstream leaves them out of the file for the same
/// reason.
const NOT_PARAMETERS: &[&str] = &[
    "SYSID_SW_MREV",
    "WP_TOTAL",
    "CMD_TOTAL",
    "FENCE_TOTAL",
    "SYS_NUM_RESETS",
    "ARSPD_OFFSET",
    "GND_ABS_PRESS",
    "GND_TEMP",
    "BARO1_GND_PRESS",
    "BARO2_GND_PRESS",
    "BARO3_GND_PRESS",
    "BARO_GND_TEMP",
    "CMD_INDEX",
    "LOG_LASTFILE",
    "FORMAT_VERSION",
];

/// Reads the parameters of a file.
///
/// The lines that hold no parameter — the comments, the empty ones — are skipped, and
/// so are the parameters the vehicle writes for its own bookkeeping.
///
/// # Errors
///
/// Returns an error when a line names a parameter whose value is not a number. A file
/// with a broken value is not read half way: a ground station that reads half a file
/// and writes it back has lost half of the configuration.
pub fn read(text: &str) -> Result<Vec<Parameter>, ParamFileError> {
    let mut parameters = Vec::new();

    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // The name and the value are separated by a comma, a space or a tab.
        let mut parts = line.split([',', ' ', '\t']);

        let Some(name) = parts.next().filter(|name| !name.is_empty()) else {
            continue;
        };

        let Some(value) = parts.next().filter(|value| !value.is_empty()) else {
            continue;
        };

        if NOT_PARAMETERS.contains(&name) {
            continue;
        }

        let value: f64 = value.parse().map_err(|_| ParamFileError::NotANumber {
            name: name.to_owned(),
            value: value.to_owned(),
        })?;

        // The type is not in the file: the value is read as a real number, and the
        // vehicle casts it to whatever the parameter is.
        parameters.push(Parameter::new(name, value, ParamType::Real64));
    }

    Ok(parameters)
}

/// Writes the parameters of a store as a file.
///
/// The parameters are written in the order they are held, which is the order the
/// vehicle reported them in, and the values are written the way an operator reads them:
/// the whole number when it is one, and the fraction when it is not.
#[must_use]
pub fn write(store: &ParameterStore) -> String {
    let mut text = String::from("# On board parameter file\n");

    for parameter in store.iter() {
        let value = parameter.value;

        let written = if value.fract() == 0.0 && value.abs() < 1e15 {
            format!("{}", value as i64)
        } else {
            format!("{value}")
        };

        text.push_str(&format!("{},{}\n", parameter.name, written));
    }

    text
}

/// The parameters of a store that are not the ones another store holds.
///
/// This is what a ground station shows when it compares what the vehicle has with what
/// a file says: the parameters whose value is different, and the ones that are only in
/// one of the two.
#[must_use]
pub fn differences(current: &ParameterStore, other: &ParameterStore) -> Vec<Difference> {
    let mut differences = Vec::new();

    for parameter in current.iter() {
        match other.get(&parameter.name) {
            Some(known) if known.value == parameter.value => {}
            Some(known) => differences.push(Difference {
                name: parameter.name.clone(),
                current: Some(parameter.value),
                other: Some(known.value),
            }),
            None => differences.push(Difference {
                name: parameter.name.clone(),
                current: Some(parameter.value),
                other: None,
            }),
        }
    }

    for parameter in other.iter() {
        if current.get(&parameter.name).is_none() {
            differences.push(Difference {
                name: parameter.name.clone(),
                current: None,
                other: Some(parameter.value),
            });
        }
    }

    differences.sort_by(|left, right| left.name.cmp(&right.name));

    differences
}

/// How the value of a parameter differs between two sets.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    /// The name of the parameter.
    pub name: String,
    /// The value the vehicle has, when it has one.
    pub current: Option<f64>,
    /// The value the other set has, when it has one.
    pub other: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_read() {
        let text = "# On board parameter file\nARMING_CHECK,1\nRTL_ALT,100\n";

        let parameters = read(text).expect("the file is read");

        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "ARMING_CHECK");
        assert!((parameters[0].value - 1.0).abs() < 1e-9);
        assert!((parameters[1].value - 100.0).abs() < 1e-9);
    }

    #[test]
    fn the_name_and_the_value_may_be_separated_by_anything() {
        let text = "ARMING_CHECK 1\nRTL_ALT\t100\n";

        let parameters = read(text).expect("the file is read");

        assert_eq!(parameters.len(), 2);
        assert!((parameters[1].value - 100.0).abs() < 1e-9);
    }

    #[test]
    fn the_comments_and_the_empty_lines_are_skipped() {
        let text = "# a comment\n\n   \nARMING_CHECK,1\n# another comment\n";

        let parameters = read(text).expect("the file is read");

        assert_eq!(parameters.len(), 1);
    }

    #[test]
    fn the_bookkeeping_of_the_vehicle_is_not_a_parameter() {
        let text = "WP_TOTAL,12\nARMING_CHECK,1\n";

        let parameters = read(text).expect("the file is read");

        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0].name, "ARMING_CHECK");
    }

    #[test]
    fn a_value_that_is_not_a_number_is_an_error() {
        let error = read("ARMING_CHECK,not a number\n").expect_err("the file is broken");

        assert!(matches!(error, ParamFileError::NotANumber { .. }));
    }

    #[test]
    fn a_store_is_written_and_read_back() {
        let mut store = ParameterStore::new();
        store.insert(Parameter::new("ARMING_CHECK", 1.0, ParamType::Real64));
        store.insert(Parameter::new("RTL_ALT", 100.0, ParamType::Real64));
        store.insert(Parameter::new("WP_RADIUS", 90.5, ParamType::Real64));

        let text = write(&store);
        let parameters = read(&text).expect("the file is read");

        assert_eq!(parameters.len(), 3);
        assert!((parameters[2].value - 90.5).abs() < 1e-9);
    }

    #[test]
    fn the_differences_are_the_ones_that_matter() {
        let mut current = ParameterStore::new();
        current.insert(Parameter::new("ARMING_CHECK", 1.0, ParamType::Real64));
        current.insert(Parameter::new("RTL_ALT", 100.0, ParamType::Real64));

        let mut other = ParameterStore::new();
        other.insert(Parameter::new("ARMING_CHECK", 1.0, ParamType::Real64));
        other.insert(Parameter::new("RTL_ALT", 50.0, ParamType::Real64));
        other.insert(Parameter::new("FS_BATT_ENABLE", 1.0, ParamType::Real64));

        let differences = differences(&current, &other);

        // The one that is the same is not a difference, so two are left.
        assert_eq!(differences.len(), 2);

        // The one that is the same is not a difference.
        assert!(!differences.iter().any(|d| d.name == "ARMING_CHECK"));

        let altitude = differences
            .iter()
            .find(|d| d.name == "RTL_ALT")
            .expect("the altitude differs");
        assert_eq!(altitude.current, Some(100.0));
        assert_eq!(altitude.other, Some(50.0));

        // The one only the file has.
        let battery = differences
            .iter()
            .find(|d| d.name == "FS_BATT_ENABLE")
            .expect("the file has one more");
        assert_eq!(battery.current, None);
        assert_eq!(battery.other, Some(1.0));
    }
}
