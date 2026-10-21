//! The pre-flight checklist.
//!
//! # What this is
//!
//! Before a vehicle flies, a handful of things have to be true: the GPS has a fix, there
//! are enough satellites, the telemetry link is healthy, the battery is charged, the
//! vehicle is at the right altitude, and the pilot has checked the airframe with their own
//! hands.
//!
//! Upstream keeps that list in `checklistDefault.xml`. The first six items watch a value of
//! the vehicle and compare it with a threshold; the other six are questions the pilot
//! answers by hand.
//!
//! The conditions are the vehicle's knowledge, not the interface's, so they live here: the
//! interface shows what the rules say and does not know why.
//!
//! # What the values are
//!
//! The names are the ones MPCraft uses for the values of the vehicle. `linkQuality` is the
//! share of the packets that arrived, which is what the telemetry link is worth.

use std::sync::OnceLock;

use mpcraft_params::ParameterStore;
use mpcraft_vehicle::Telemetry;
use serde::{Deserialize, Serialize};

/// The catalogue, as it is stored.
const CATALOGUE: &str = include_str!("../data/checklist.toml");

/// The condition an item checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Condition {
    /// The item is not a check: it only shows a value.
    #[default]
    None,
    /// The value is less than the threshold.
    Lt,
    /// The value is less than or equal to the threshold.
    Lte,
    /// The value is equal to the threshold.
    Eq,
    /// The value is greater than the threshold.
    Gt,
    /// The value is greater than or equal to the threshold.
    Gte,
    /// The value is different from the threshold.
    Neq,
}

impl Condition {
    /// Whether the condition holds for a value.
    #[must_use]
    pub fn holds(self, value: f64, trigger: f64) -> bool {
        match self {
            Self::None => false,
            Self::Lt => value < trigger,
            Self::Lte => value <= trigger,
            Self::Eq => value == trigger,
            Self::Gt => value > trigger,
            Self::Gte => value >= trigger,
            Self::Neq => value != trigger,
        }
    }
}

/// Where the value of an item comes from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// The type of the GPS fix: 0 no GPS, 1 no fix, 2 two dimensions, 3 three dimensions.
    GpsStatus,
    /// The number of satellites in use.
    SatCount,
    /// The share of the packets that arrived, as a percentage.
    LinkQuality,
    /// The voltage of the battery.
    BatteryVoltage,
    /// The name of the flight mode.
    Mode,
    /// The altitude of the vehicle.
    Altitude,
    /// A question the pilot answers by hand.
    Manual,
}

/// One item of the checklist.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    /// What the item is about.
    pub description: String,
    /// Where the value comes from.
    pub source: Source,
    /// The condition the value has to meet.
    ///
    /// A question for the pilot has no condition: there is nothing to measure.
    #[serde(default)]
    pub condition: Condition,
    /// The value the condition is measured against.
    #[serde(default)]
    pub trigger: f64,
    /// The text to show, with `{value}` and `{trigger}` to be replaced.
    #[serde(default)]
    pub text: String,
}

/// The catalogue, as it is stored.
#[derive(Debug, Clone, Default, Deserialize)]
struct Stored {
    #[serde(default)]
    items: Vec<ChecklistItem>,
}

/// The checklist, read once.
static CATALOGUE_INDEX: OnceLock<Vec<ChecklistItem>> = OnceLock::new();

/// The checklist of the vehicle.
///
/// ```
/// use mpcraft_app::checklist;
///
/// assert!(!checklist::items().is_empty());
/// ```
#[must_use]
pub fn items() -> &'static [ChecklistItem] {
    CATALOGUE_INDEX.get_or_init(|| {
        match toml::from_str::<Stored>(CATALOGUE) {
            Ok(stored) => stored.items,
            Err(error) => {
                // A checklist that could not be read is an empty checklist, and an empty
                // checklist says "everything is in order" when it knows nothing. It has to
                // be said in the log.
                tracing::error!("the pre-flight checklist could not be read: {error}");
                Vec::new()
            }
        }
    })
}

/// A value of the vehicle, as the checklist sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number.
    Number(f64),
    /// Something that is not a number.
    Text(String),
}

impl Value {
    /// The value as it is written in the text of the item.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Number(value) => {
                // The same two decimals upstream writes.
                let rounded = (value * 100.0).round() / 100.0;
                if (rounded - rounded.trunc()).abs() < f64::EPSILON {
                    format!("{}", rounded.trunc() as i64)
                } else {
                    format!("{rounded}")
                }
            }
            Self::Text(text) => text.clone(),
        }
    }

    /// The value as a number, when it is one.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(_) => None,
        }
    }
}

/// One line of the checklist, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistEntry {
    /// What the item is about.
    pub description: String,
    /// The text of the item, with the value in it.
    pub text: String,
    /// Whether the condition holds.
    pub passed: bool,
    /// Whether the value was known at all.
    ///
    /// A value the vehicle has not reported is **unknown**, which is not the same as a
    /// failed check: a checklist that says "no" about something nobody measured is lying.
    pub known: bool,
    /// Whether the pilot answers this one by hand.
    pub manual: bool,
}

/// The share of the packets that arrived, as a percentage.
///
/// A link that has lost nothing is a hundred; a link that has lost everything is a zero.
#[must_use]
pub fn link_quality(received: u64, lost: u64) -> f64 {
    let total = received + lost;

    if total == 0 {
        return 0.0;
    }

    (received as f64 * 100.0) / total as f64
}

/// Checks the checklist against the vehicle.
///
/// The parameters are only used by the items that watch one; the checklist of the vehicle
/// that MPCraft ships watches none, but the mechanism is there for the ones an operator
/// adds.
#[must_use]
pub fn evaluate(
    items: &[ChecklistItem],
    telemetry: &Telemetry,
    parameters: &ParameterStore,
    quality: f64,
) -> Vec<ChecklistEntry> {
    items
        .iter()
        .map(|item| {
            if item.source == Source::Manual {
                return ChecklistEntry {
                    description: item.description.clone(),
                    text: item.description.clone(),
                    passed: false,
                    known: false,
                    manual: true,
                };
            }

            let value = value_of(item, telemetry, parameters, quality);

            let known = value.is_some();
            let passed = value
                .as_ref()
                .and_then(Value::as_number)
                .is_some_and(|number| item.condition.holds(number, item.trigger));

            let text = value.as_ref().map_or_else(String::new, Value::display);

            let text = item
                .text
                .replace("{value}", &text)
                .replace("{trigger}", &format!("{}", item.trigger));

            ChecklistEntry {
                description: item.description.clone(),
                text,
                passed,
                known,
                manual: false,
            }
        })
        .collect()
}

/// The value an item watches.
fn value_of(
    item: &ChecklistItem,
    telemetry: &Telemetry,
    parameters: &ParameterStore,
    quality: f64,
) -> Option<Value> {
    match item.source {
        Source::GpsStatus => telemetry
            .gps
            .map(|gps| Value::Number(f64::from(gps.fix_type as u8))),
        Source::SatCount => telemetry
            .gps
            .map(|gps| Value::Number(f64::from(gps.satellites))),
        Source::LinkQuality => Some(Value::Number(quality)),
        Source::BatteryVoltage => telemetry
            .battery
            .as_ref()
            .map(|battery| Value::Number(battery.voltage)),
        Source::Mode => telemetry.mode.as_ref().map(|mode| {
            Value::Text(
                mode.name
                    .clone()
                    .unwrap_or_else(|| format!("mode {}", mode.custom_mode)),
            )
        }),
        Source::Altitude => telemetry
            .position
            .as_ref()
            .map(|position| Value::Number(position.alt)),
        Source::Manual => None,
    }
    .or_else(|| {
        // A value the vehicle has not reported may be a parameter.
        parameters
            .get(&item.description)
            .map(|parameter| Value::Number(parameter.value))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_mavlink::MavGpsFixType;
    use mpcraft_vehicle::{Battery, GpsStatus};

    fn telemetry() -> Telemetry {
        Telemetry {
            gps: Some(GpsStatus {
                fix_type: MavGpsFixType::GPS_FIX_TYPE_3D_FIX,
                satellites: 9,
                hdop: 1.1,
            }),
            battery: Some(Battery {
                voltage: 25.2,
                current: -4.0,
                remaining: 87.0,
            }),
            ..Telemetry::default()
        }
    }

    #[test]
    fn the_catalogue_is_read() {
        let items = items();

        assert!(!items.is_empty());
        assert!(items.iter().any(|item| item.source == Source::Manual));
    }

    #[test]
    fn a_check_that_holds_is_reported_as_passing() {
        let entries = evaluate(items(), &telemetry(), &ParameterStore::new(), 100.0);

        let gps = entries
            .iter()
            .find(|entry| entry.description == "Veryify GPS")
            .expect("the GPS check is in the catalogue");

        assert!(gps.known);
        assert!(gps.passed);
        assert!(gps.text.contains('3'));
    }

    #[test]
    fn a_check_that_fails_is_reported_as_failing() {
        // A link that has lost a third of its packets is not a good link.
        let entries = evaluate(items(), &telemetry(), &ParameterStore::new(), 60.0);

        let link = entries
            .iter()
            .find(|entry| entry.description == "Telemetry Signal")
            .expect("the link check is in the catalogue");

        assert!(link.known);
        assert!(!link.passed);
    }

    #[test]
    fn a_value_nobody_measured_is_unknown_and_not_a_failure() {
        // No GPS: the check is unknown, and it does not say the check failed.
        let entries = evaluate(
            items(),
            &Telemetry::default(),
            &ParameterStore::new(),
            100.0,
        );

        let gps = entries
            .iter()
            .find(|entry| entry.description == "Veryify GPS")
            .expect("the GPS check is in the catalogue");

        assert!(!gps.known);
        assert!(!gps.passed);
    }

    #[test]
    fn a_question_for_the_pilot_is_marked_as_such() {
        let entries = evaluate(items(), &telemetry(), &ParameterStore::new(), 100.0);

        let manual = entries
            .iter()
            .find(|entry| entry.description == "Tail and wings secured?")
            .expect("the question is in the catalogue");

        assert!(manual.manual);
        assert!(!manual.known);
    }

    #[test]
    fn the_conditions_are_the_usual_ones() {
        assert!(Condition::Gte.holds(3.0, 3.0));
        assert!(Condition::Gt.holds(3.1, 3.0));
        assert!(!Condition::Gt.holds(3.0, 3.0));
        assert!(Condition::Lt.holds(4.9, 5.0));
        assert!(Condition::Neq.holds(1.0, 2.0));
        assert!(!Condition::None.holds(1.0, 2.0));
    }

    #[test]
    fn the_link_quality_is_the_share_of_the_packets_that_arrived() {
        assert!((link_quality(100, 0) - 100.0).abs() < 1e-9);
        assert!((link_quality(50, 50) - 50.0).abs() < 1e-9);
        assert!((link_quality(0, 0) - 0.0).abs() < 1e-9);
    }
}
