//! The graphs, as the interface sees them.
//!
//! The expressions are evaluated over the telemetry the model keeps, and the interface
//! receives numbers with names. It never sees a MAVLink field.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mpcraft_vehicle::graph::{self, History};

use crate::error::{CommandError, CommandResult};

/// The history the graphs are drawn from.
///
/// The history is shared: the shell fills it from the state the link publishes, and the
/// commands read it.
#[derive(Clone)]
pub struct GraphHistory {
    history: Arc<Mutex<History>>,
}

impl Default for GraphHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphHistory {
    /// Creates an empty history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: Arc::new(Mutex::new(History::default())),
        }
    }

    /// Records a sample of the telemetry.
    pub fn record(&self, telemetry: &mpcraft_vehicle::Telemetry) {
        if let Ok(mut history) = self.history.lock() {
            history.push_telemetry(telemetry);
        }
    }

    /// Empties the history.
    pub fn clear(&self) {
        if let Ok(mut history) = self.history.lock() {
            history.clear();
        }
    }

    /// How many samples the history holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.history.lock().map_or(0, |history| history.len())
    }

    /// Whether the history holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The presets the user can choose.
    #[must_use]
    pub fn presets(&self) -> Vec<PresetInfo> {
        graph::catalogue()
            .into_iter()
            .map(|preset| PresetInfo {
                name: preset.preset.name.full(),
                group: preset.preset.name.group.to_owned(),
                label: preset.preset.name.name.to_owned(),
                description: preset.preset.description.to_owned(),
                upstream: preset.preset.upstream,
                series: preset
                    .series
                    .iter()
                    .map(|series| SeriesInfo {
                        name: series.name.to_owned(),
                        unit: series.unit.to_owned(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// The values of a preset over the history.
    ///
    /// The `since` argument is how long back to look, in seconds. A live graph only wants
    /// the recent past, and asking for the whole history every second would send
    /// megabytes over the wire for nothing.
    ///
    /// # Errors
    ///
    /// Returns an error when there is no preset with that name.
    pub fn series(&self, name: &str, since: Option<Duration>) -> CommandResult<SeriesData> {
        let Some(preset) = graph::preset_named(name) else {
            return Err(CommandError::invalid_request(format!(
                "there is no graph called {name:?}"
            )));
        };

        let compiled = graph::catalogue()
            .into_iter()
            .find(|candidate| candidate.preset.name.full() == name);

        let Some(compiled) = compiled else {
            return Err(CommandError::invalid_request(format!(
                "the graph {name:?} could not be read"
            )));
        };

        let history = self
            .history
            .lock()
            .map_err(|_| CommandError::internal("the history of the graphs is poisoned"))?;

        let series = compiled
            .series
            .iter()
            .map(|series| {
                let values = history.series(&series.expression);

                // The window is applied to the values, not to the samples: the instants are
                // the same for every series of one sample, so the series stay aligned.
                let values: Vec<_> = match since {
                    Some(window) => {
                        // The window is measured back from the newest sample: a graph
                        // shows the recent past, not the beginning of the history.
                        let oldest = history.newest().saturating_sub(window);
                        values.into_iter().filter(|(at, _)| *at >= oldest).collect()
                    }
                    None => values,
                };

                SeriesPoints {
                    name: series.name.to_owned(),
                    unit: series.unit.to_owned(),
                    x: values.iter().map(|(at, _)| at.as_secs_f64()).collect(),
                    y: values.iter().map(|(_, value)| *value).collect(),
                }
            })
            .collect();

        Ok(SeriesData {
            name: preset.name.full(),
            description: preset.description.to_owned(),
            series,
        })
    }
}

/// A graph the user can choose.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    /// The full name, as the interface shows it.
    pub name: String,
    /// The group it belongs to.
    pub group: String,
    /// The name inside the group.
    pub label: String,
    /// What the graph shows.
    pub description: String,
    /// Whether the graph comes from upstream's catalogue.
    pub upstream: bool,
    /// The series it draws.
    pub series: Vec<SeriesInfo>,
}

/// One series of a graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesInfo {
    /// The name of the series.
    pub name: String,
    /// The unit the values are in.
    pub unit: String,
}

/// The values of a graph over the history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesData {
    /// The full name of the graph.
    pub name: String,
    /// What the graph shows.
    pub description: String,
    /// The series, each with its values.
    pub series: Vec<SeriesPoints>,
}

/// The values of one series.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesPoints {
    /// The name of the series.
    pub name: String,
    /// The unit the values are in.
    pub unit: String,
    /// How long after the start of the history each value is, in seconds.
    pub x: Vec<f64>,
    /// The values themselves.
    pub y: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mpcraft_vehicle::{Attitude, Telemetry};

    #[test]
    fn the_catalogue_is_offered_with_its_series() {
        let history = GraphHistory::new();
        let presets = history.presets();

        assert!(!presets.is_empty());

        for preset in &presets {
            assert!(!preset.series.is_empty());
            assert!(preset.name.contains('/'));
        }
    }

    #[test]
    fn a_graph_that_does_not_exist_is_an_error() {
        let history = GraphHistory::new();
        let error = history.series("Nothing/Here", None).unwrap_err();

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn a_graph_is_empty_until_there_is_telemetry() {
        let history = GraphHistory::new();
        let data = history.series("Speed/Ground Speed", None).unwrap();

        assert!(data.series[0].x.is_empty());
        assert!(data.series[0].y.is_empty());
    }

    #[test]
    fn a_graph_draws_the_telemetry_that_was_recorded() {
        let history = GraphHistory::new();

        let telemetry = Telemetry {
            attitude: Some(Attitude {
                roll: 0.0,
                pitch: 0.0,
                yaw: 0.0,
            }),
            velocity: Some(mpcraft_vehicle::Velocity {
                north: 4.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 4.0,
            }),
            ..Telemetry::default()
        };

        history.record(&telemetry);

        let data = history.series("Speed/Ground Speed", None).unwrap();
        assert_eq!(data.series[0].y.len(), 1);
        assert!((data.series[0].y[0] - 4.0).abs() < 1e-9);
    }

    #[test]
    fn the_window_limits_what_the_graph_returns() {
        let history = GraphHistory::new();

        let telemetry = Telemetry {
            velocity: Some(mpcraft_vehicle::Velocity {
                north: 4.0,
                east: 0.0,
                down: 0.0,
                ground_speed: 4.0,
            }),
            ..Telemetry::default()
        };

        for _ in 0..5 {
            history.record(&telemetry);
            std::thread::sleep(std::time::Duration::from_millis(1100));
        }

        // The whole history has five samples.
        let everything = history.series("Speed/Ground Speed", None).unwrap();
        assert_eq!(everything.series[0].y.len(), 5);

        // The last two seconds hold only the most recent ones.
        let window = history
            .series(
                "Speed/Ground Speed",
                Some(std::time::Duration::from_secs(2)),
            )
            .unwrap();
        assert!(
            window.series[0].y.len() < everything.series[0].y.len(),
            "the window did not shorten the series"
        );
    }
}
