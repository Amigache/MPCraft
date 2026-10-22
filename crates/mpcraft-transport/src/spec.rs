//! Connection specifications.
//!
//! A connection target is written as a single string, so that a user can type it into a
//! dialog, store it in a profile, and paste it into a script.
//!
//! ```text
//! serial:COM3:57600
//! serial:/dev/ttyUSB0:115200|nodtr
//! udp:127.0.0.1:14550
//! udp::14550|server
//! tcp:192.168.1.10:5760
//! file:/logs/flight.tlog|rate=1.0
//! ```
//!
//! # Grammar
//!
//! ```text
//! spec      := scheme ":" target [ "|" option [ "|" option ]* ]
//! scheme    := "serial" | "udp" | "tcp" | "file"
//! option    := key [ "=" value ]
//! ```
//!
//! The `target` is scheme-specific and positional:
//!
//! | Scheme | Target | Positional extras |
//! | --- | --- | --- |
//! | `serial` | the port name | `:<baud rate>` |
//! | `udp` | `<host>:<port>` | — |
//! | `tcp` | `<host>:<port>` | — |
//! | `file` | a filesystem path | — |
//!
//! Options follow the first `|`. Using `|` rather than `:` keeps Windows paths such as
//! `C:\logs\flight.tlog` unambiguous.
//!
//! | Option | Schemes | Meaning |
//! | --- | --- | --- |
//! | `server` | `udp`, `tcp` | bind and wait for the peer instead of connecting out |
//! | `client` | `udp`, `tcp` | connect out (the default) |
//! | `dtr` / `nodtr` | `serial` | the state of the DTR line when the port is opened |
//! | `baud=<n>` | `serial` | the baud rate, overriding the positional form |
//! | `rate=<f>` | `file` | the replay rate; `1.0` is real time, `0` is as fast as possible |

use std::fmt;
use std::str::FromStr;

/// The transport a connection uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionScheme {
    /// A serial port.
    Serial,
    /// UDP.
    Udp,
    /// TCP.
    Tcp,
    /// A file being replayed.
    File,
}

impl ConnectionScheme {
    /// The scheme's textual name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::Udp => "udp",
            Self::Tcp => "tcp",
            Self::File => "file",
        }
    }
}

impl fmt::Display for ConnectionScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ConnectionScheme {
    type Err = SpecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "serial" => Ok(Self::Serial),
            "udp" => Ok(Self::Udp),
            "tcp" => Ok(Self::Tcp),
            "file" => Ok(Self::File),
            other => Err(SpecError::UnknownScheme(other.to_owned())),
        }
    }
}

/// The options a connection can carry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectionOptions {
    /// The baud rate of a serial port.
    pub baud_rate: u32,
    /// Whether the rate is found by trying the ones there are.
    ///
    /// A vehicle that is talking at a rate nobody wrote down is a vehicle that is
    /// found by trying them one after another: the port is opened at each rate in
    /// turn, and the rate at which the vehicle answers is the one it talks at.
    pub auto_baud: bool,
    /// Whether the DTR line is asserted when the port is opened.
    ///
    /// Mission Planner toggles DTR to reset boards that use it as a reset line, which is why
    /// this defaults to `false` rather than to the driver's default of `true`.
    ///
    /// The RTS line has no equivalent option: the serial driver exposes no RTS line
    /// control, and an option that silently does nothing is worse than no option at all.
    pub dtr_on_open: bool,
    /// Whether the transport waits for the peer instead of connecting out.
    pub server: bool,
    /// The replay rate of a file source; `0.0` means as fast as possible.
    pub replay_rate: f64,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            baud_rate: 57600,
            auto_baud: false,
            dtr_on_open: false,
            server: false,
            replay_rate: 0.0,
        }
    }
}

/// A parsed connection target.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionSpec {
    /// The transport to use.
    pub scheme: ConnectionScheme,
    /// The scheme-specific target: a port name, a `host:port` pair, or a file path.
    pub target: String,
    /// The options that refine the target.
    pub options: ConnectionOptions,
}

impl ConnectionSpec {
    /// Parses a connection specification.
    ///
    /// # Errors
    ///
    /// Returns a [`SpecError`] describing exactly what was wrong. The parser never panics and
    /// never guesses: an unrecognised option is an error rather than a silently ignored token.
    pub fn parse(input: &str) -> Result<Self, SpecError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(SpecError::Empty);
        }

        let (scheme_text, remainder) = input.split_once(':').ok_or(SpecError::MissingSeparator)?;
        let scheme = ConnectionScheme::from_str(scheme_text)?;

        let mut parts = remainder.split('|');
        let target = parts.next().unwrap_or_default().trim().to_owned();
        if target.is_empty() {
            return Err(SpecError::MissingTarget);
        }

        let mut options = ConnectionOptions::default();

        // The scheme decides how the positional parts are read.
        let target = match scheme {
            ConnectionScheme::Serial => {
                let mut parts = target.split(':');
                let port = parts.next().unwrap_or_default().to_owned();

                if let Some(baud) = parts.next() {
                    options.baud_rate = baud
                        .parse()
                        .map_err(|_| SpecError::InvalidBaudRate(baud.to_owned()))?;
                }

                port
            }
            ConnectionScheme::Udp | ConnectionScheme::Tcp => {
                let mut parts = target.split(':');
                let host = parts.next().unwrap_or_default();
                let port_text = parts.next().unwrap_or_default().trim();

                if port_text.is_empty() {
                    return Err(SpecError::MissingPort);
                }

                let port: u16 = port_text
                    .parse()
                    .map_err(|_| SpecError::InvalidPort(port_text.to_owned()))?;
                if port == 0 {
                    return Err(SpecError::InvalidPort(port_text.to_owned()));
                }

                format!("{host}:{port}")
            }
            ConnectionScheme::File => {
                // The whole target is the path; it may contain colons.
                target
            }
        };

        for option in parts {
            if option.is_empty() {
                continue;
            }
            let (key, value) = option
                .split_once('=')
                .map_or((option, None), |(k, v)| (k, Some(v)));

            match (scheme, key.to_ascii_lowercase().as_str()) {
                (ConnectionScheme::Udp | ConnectionScheme::Tcp, "server") => {
                    options.server = true;
                }
                (ConnectionScheme::Udp | ConnectionScheme::Tcp, "client") => {
                    options.server = false;
                }
                (ConnectionScheme::Serial, "dtr") => options.dtr_on_open = true,
                (ConnectionScheme::Serial, "nodtr") => options.dtr_on_open = false,
                (ConnectionScheme::Serial, "baud") => {
                    let text = value.ok_or(SpecError::MissingValue("baud".to_owned()))?;

                    // The rate can be left to the program: it opens the port at each
                    // rate in turn until the vehicle answers.
                    if text.eq_ignore_ascii_case("auto") {
                        options.auto_baud = true;
                        continue;
                    }

                    options.baud_rate = text
                        .parse()
                        .map_err(|_| SpecError::InvalidBaudRate(text.to_owned()))?;
                }
                (ConnectionScheme::File, "rate") => {
                    let text = value.ok_or(SpecError::MissingValue("rate".to_owned()))?;
                    let rate: f64 = text
                        .parse()
                        .map_err(|_| SpecError::InvalidRate(text.to_owned()))?;
                    if rate < 0.0 {
                        return Err(SpecError::InvalidRate(text.to_owned()));
                    }
                    options.replay_rate = rate;
                }
                (_, other) => {
                    return Err(SpecError::UnknownOption {
                        scheme,
                        option: other.to_owned(),
                    });
                }
            }
        }

        Ok(Self {
            scheme,
            target,
            options,
        })
    }

    /// The host of a network target, if the scheme has one.
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        match self.scheme {
            ConnectionScheme::Udp | ConnectionScheme::Tcp => {
                self.target.split_once(':').map(|(host, _)| host)
            }
            _ => None,
        }
    }

    /// The port of a network target, if the scheme has one.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        match self.scheme {
            ConnectionScheme::Udp | ConnectionScheme::Tcp => self
                .target
                .split_once(':')
                .and_then(|(_, port)| port.parse().ok()),
            _ => None,
        }
    }

    /// The serial port name, if the scheme is serial.
    #[must_use]
    pub fn port_name(&self) -> Option<&str> {
        match self.scheme {
            ConnectionScheme::Serial => Some(self.target.as_str()),
            _ => None,
        }
    }
}

impl fmt::Display for ConnectionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.scheme, self.target)?;

        if self.scheme == ConnectionScheme::Serial {
            write!(f, ":{}", self.options.baud_rate)?;
            if self.options.dtr_on_open {
                write!(f, "|dtr")?;
            }
        } else if self.scheme == ConnectionScheme::File {
            if self.options.replay_rate > 0.0 {
                write!(f, "|rate={}", self.options.replay_rate)?;
            }
        } else if self.options.server {
            write!(f, "|server")?;
        }

        Ok(())
    }
}

impl FromStr for ConnectionSpec {
    type Err = SpecError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// The reasons a connection specification can be rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SpecError {
    /// The specification was empty.
    #[error("the connection specification is empty")]
    Empty,

    /// The specification had no `scheme:target` separator.
    #[error("the connection specification has no ':' separator")]
    MissingSeparator,

    /// The scheme was not recognised.
    #[error("unknown connection scheme: {0}")]
    UnknownScheme(String),

    /// The target was empty.
    #[error("the connection specification has no target")]
    MissingTarget,

    /// The target has no port.
    #[error("the connection target has no port")]
    MissingPort,

    /// The port was not a valid port number.
    #[error("invalid port: {0}")]
    InvalidPort(String),

    /// The baud rate was not a valid number.
    #[error("invalid baud rate: {0}")]
    InvalidBaudRate(String),

    /// The replay rate was not a valid number.
    #[error("invalid replay rate: {0}")]
    InvalidRate(String),

    /// An option that requires a value did not have one.
    #[error("the option {0} requires a value")]
    MissingValue(String),

    /// The option is not valid for the scheme.
    #[error("the option {option} is not valid for a {scheme} connection")]
    UnknownOption {
        /// The scheme the option was used with.
        scheme: ConnectionScheme,
        /// The offending option.
        option: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_serial_connection() {
        let spec = ConnectionSpec::parse("serial:COM3:57600").unwrap();
        assert_eq!(spec.scheme, ConnectionScheme::Serial);
        assert_eq!(spec.port_name(), Some("COM3"));
        assert_eq!(spec.options.baud_rate, 57600);
    }

    #[test]
    fn parses_a_posix_serial_port() {
        let spec = ConnectionSpec::parse("serial:/dev/ttyUSB0:115200").unwrap();
        assert_eq!(spec.port_name(), Some("/dev/ttyUSB0"));
        assert_eq!(spec.options.baud_rate, 115200);
    }

    #[test]
    fn parses_a_udp_connection() {
        let spec = ConnectionSpec::parse("udp:127.0.0.1:14550").unwrap();
        assert_eq!(spec.scheme, ConnectionScheme::Udp);
        assert_eq!(spec.host(), Some("127.0.0.1"));
        assert_eq!(spec.port(), Some(14550));
        assert!(!spec.options.server);
    }

    #[test]
    fn parses_a_udp_server_connection() {
        let spec = ConnectionSpec::parse("udp::14550|server").unwrap();
        assert_eq!(spec.host(), Some(""));
        assert_eq!(spec.port(), Some(14550));
        assert!(spec.options.server);
    }

    #[test]
    fn parses_a_tcp_connection() {
        let spec = ConnectionSpec::parse("tcp:192.168.1.10:5760").unwrap();
        assert_eq!(spec.scheme, ConnectionScheme::Tcp);
        assert_eq!(spec.host(), Some("192.168.1.10"));
        assert_eq!(spec.port(), Some(5760));
    }

    #[test]
    fn parses_a_windows_file_path() {
        // The colon in the drive letter must not be mistaken for a separator.
        let spec = ConnectionSpec::parse("file:C:\\logs\\flight.tlog|rate=2.0").unwrap();
        assert_eq!(spec.scheme, ConnectionScheme::File);
        assert_eq!(spec.target, "C:\\logs\\flight.tlog");
        assert!((spec.options.replay_rate - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parses_a_posix_file_path() {
        let spec = ConnectionSpec::parse("file:/logs/flight.tlog").unwrap();
        assert_eq!(spec.target, "/logs/flight.tlog");
        assert!((spec.options.replay_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dtr_defaults_to_low_and_can_be_overridden() {
        let spec = ConnectionSpec::parse("serial:COM3:57600").unwrap();
        assert!(!spec.options.dtr_on_open);

        let spec = ConnectionSpec::parse("serial:COM3:57600|dtr").unwrap();
        assert!(spec.options.dtr_on_open);

        let spec = ConnectionSpec::parse("serial:COM3:57600|nodtr").unwrap();
        assert!(!spec.options.dtr_on_open);
    }

    #[test]
    fn the_baud_rate_can_be_given_as_an_option() {
        let spec = ConnectionSpec::parse("serial:COM3|baud=115200").unwrap();
        assert_eq!(spec.options.baud_rate, 115200);
    }

    #[test]
    fn the_baud_rate_can_be_left_to_the_program() {
        let spec = ConnectionSpec::parse("serial:COM3|baud=auto").unwrap();
        assert!(spec.options.auto_baud);
        assert_eq!(spec.target, "COM3");

        let spec = ConnectionSpec::parse("serial:COM3|baud=AUTO").unwrap();
        assert!(spec.options.auto_baud);
    }

    #[test]
    fn the_rate_is_not_left_to_the_program_by_default() {
        let spec = ConnectionSpec::parse("serial:COM3:57600").unwrap();
        assert!(!spec.options.auto_baud);
    }

    #[test]
    fn rejects_an_unknown_scheme() {
        assert_eq!(
            ConnectionSpec::parse("carrier-pigeon:COM3"),
            Err(SpecError::UnknownScheme("carrier-pigeon".to_owned()))
        );
    }

    #[test]
    fn rejects_a_missing_target() {
        assert_eq!(ConnectionSpec::parse("udp:"), Err(SpecError::MissingTarget));
    }

    #[test]
    fn rejects_an_invalid_port() {
        assert!(matches!(
            ConnectionSpec::parse("udp:127.0.0.1:notaport"),
            Err(SpecError::InvalidPort(_))
        ));
    }

    #[test]
    fn rejects_a_missing_port() {
        assert_eq!(
            ConnectionSpec::parse("udp:notaport"),
            Err(SpecError::MissingPort)
        );
    }

    #[test]
    fn rejects_an_unknown_option() {
        assert!(matches!(
            ConnectionSpec::parse("udp:127.0.0.1:14550|frobnicate"),
            Err(SpecError::UnknownOption { .. })
        ));
    }

    #[test]
    fn rejects_an_option_from_the_wrong_scheme() {
        assert!(matches!(
            ConnectionSpec::parse("udp:127.0.0.1:14550|nodtr"),
            Err(SpecError::UnknownOption { .. })
        ));
    }

    #[test]
    fn rejects_an_empty_specification() {
        assert_eq!(ConnectionSpec::parse("   "), Err(SpecError::Empty));
    }

    #[test]
    fn round_trips_through_its_display_form() {
        for input in [
            "serial:COM3:57600",
            "udp:127.0.0.1:14550",
            "tcp:192.168.1.10:5760",
        ] {
            let spec = ConnectionSpec::parse(input).unwrap();
            let rendered = spec.to_string();
            let reparsed = ConnectionSpec::parse(&rendered).unwrap();
            assert_eq!(spec, reparsed, "{input} did not round trip");
        }
    }
}
