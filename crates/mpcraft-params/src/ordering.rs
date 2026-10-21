//! The order the parameters are written in.
//!
//! # What this is
//!
//! Upstream sorts the parameters before it writes them: the ones whose name ends with
//! `ENABLE` go first, and the rest follow in alphabetical order. The rule is in
//! `ListExtension.SortENABLE`, and it is there because a parameter that turns something
//! on has to be written while the thing it turns on is still as it was — the vehicle
//! takes the change at once, and a change that arrives before the rest is a change the
//! vehicle acts on with the old values around it.
//!
//! The order is the knowledge of the autopilots, so it lives here and not in the
//! interface.
//!
//! # What it is for
//!
//! Writing a parameter is one message and one answer. Writing a hundred of them is a
//! hundred of those, and the order they are written in decides whether the vehicle ends
//! up with what the operator asked for.

use std::cmp::Ordering;

/// The order the given parameters are written in.
///
/// The parameters that turn something on are written first, and the rest follow in
/// alphabetical order. The order of the list that is given does not matter.
#[must_use]
pub fn write_order(names: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = names.to_vec();

    sorted.sort_by(|left, right| {
        let left_switch = is_switch(left);
        let right_switch = is_switch(right);

        match (left_switch, right_switch) {
            (true, true) | (false, false) => left.cmp(right),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        }
    });

    sorted
}

/// Whether the parameter turns something on.
///
/// The name of such a parameter ends with `ENABLE`: `FS_GCS_ENABLE`, `AHRS_ENABLE`,
/// `COMPASS_ENABLE`. The rule is upstream's, and it is exact: a parameter whose name
/// ends with `ENABLED` is not one of them.
#[must_use]
pub fn is_switch(name: &str) -> bool {
    name.ends_with("ENABLE")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn the_switches_are_written_first() {
        let ordered = write_order(&names(&["SERIAL0_BAUD", "AHRS_ENABLE", "SERIAL0_PROTOCOL"]));

        assert_eq!(
            ordered,
            names(&["AHRS_ENABLE", "SERIAL0_BAUD", "SERIAL0_PROTOCOL"])
        );
    }

    #[test]
    fn the_others_are_in_alphabetical_order() {
        let ordered = write_order(&names(&["WP_RADIUS", "FS_BATT_ENABLE", "RTL_ALT"]));

        assert_eq!(ordered, names(&["FS_BATT_ENABLE", "RTL_ALT", "WP_RADIUS"]));
    }

    #[test]
    fn a_parameter_that_does_not_turn_anything_on_is_not_a_switch() {
        assert!(is_switch("AHRS_ENABLE"));
        assert!(is_switch("FS_GCS_ENABLE"));
        assert!(!is_switch("SERIAL0_BAUD"));
        assert!(!is_switch("SERIAL0_ENABLED"));
    }

    #[test]
    fn an_empty_list_is_written_as_it_is() {
        assert!(write_order(&[]).is_empty());
    }
}
