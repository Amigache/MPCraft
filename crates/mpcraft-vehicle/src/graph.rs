//! The graphs: the expressions, the fields they draw on, and the presets.
//!
//! # What this is
//!
//! A ground station is mostly graphs. Mission Planner ships a catalogue of them in
//! `graphs/mavgraphs.xml`: a named group of series, each series an expression over the
//! telemetry. This is that idea, with the expressions evaluated over MPCraft's own
//! telemetry model.
//!
//! # Why the expressions are evaluated here
//!
//! The expressions name MAVLink fields, and the interface must not know the protocol:
//! it receives typed telemetry and nothing else. So the catalogue and the evaluator live
//! in the model, and the interface receives numbers with names.
//!
//! # The catalogue
//!
//! The presets are ported from upstream's `mavgraphs.xml`, with the expressions rewritten
//! over the fields MPCraft decodes. Upstream has a hundred and eight; the ones that draw
//! on messages MPCraft does not decode yet are left out rather than shown empty, and the
//! ones MPCraft adds are marked as its own.
//!
//! # The fields
//!
//! A field is a number with a name and a unit. The units are the ones an operator reads:
//! the attitude in degrees, the velocity in metres per second, the altitude in metres.

use std::fmt;

use crate::Telemetry;

/// The name of a preset, and the group it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetName {
    /// The group, for example `Speed`.
    pub group: &'static str,
    /// The name inside the group, for example `Ground Speed`.
    pub name: &'static str,
}

impl PresetName {
    /// Creates a name from its parts.
    #[must_use]
    pub const fn new(group: &'static str, name: &'static str) -> Self {
        Self { group, name }
    }

    /// The full name, as the interface shows it.
    #[must_use]
    pub fn full(&self) -> String {
        format!("{}/{}", self.group, self.name)
    }
}

impl fmt::Display for PresetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.group, self.name)
    }
}

/// A number the graphs can draw on.
///
/// The fields are the numeric parts of the telemetry. A field that the vehicle has not
/// reported has no value, and a graph that draws on it shows a gap rather than a zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// The roll, in degrees.
    AttitudeRoll,
    /// The pitch, in degrees.
    AttitudePitch,
    /// The yaw, in degrees.
    AttitudeYaw,
    /// The ground speed, in metres per second.
    VelocityGroundSpeed,
    /// The northward speed, in metres per second.
    VelocityNorth,
    /// The eastward speed, in metres per second.
    VelocityEast,
    /// The downward speed, in metres per second.
    VelocityDown,
    /// The altitude above mean sea level, in metres.
    PositionAltitude,
    /// The latitude, in degrees.
    PositionLatitude,
    /// The longitude, in degrees.
    PositionLongitude,
    /// The battery voltage, in volts.
    BatteryVoltage,
    /// The battery current, in amperes.
    BatteryCurrent,
    /// The remaining charge, as a percentage.
    BatteryRemaining,
    /// The distance to home, in metres.
    DistanceToHome,
    /// The bearing to home, in degrees.
    BearingToHome,
    /// The number of satellites in use.
    GpsSatellites,
    /// The horizontal dilution of precision.
    GpsHdop,
    /// The item of the plan the vehicle is executing.
    MissionSequence,
    /// How many items the plan has.
    MissionTotal,
    /// Whether the vehicle is armed: one when it is, zero when it is not.
    Armed,
    /// The acceleration on the first axis, in metres per second squared.
    ImuAccelerationX,
    /// The acceleration on the second axis, in metres per second squared.
    ImuAccelerationY,
    /// The acceleration on the third axis, in metres per second squared.
    ImuAccelerationZ,
    /// The angular rate on the first axis, in degrees per second.
    ImuAngularRateX,
    /// The angular rate on the second axis, in degrees per second.
    ImuAngularRateY,
    /// The angular rate on the third axis, in degrees per second.
    ImuAngularRateZ,
    /// The magnetic field on the first axis, in gauss.
    ImuMagneticX,
    /// The magnetic field on the second axis, in gauss.
    ImuMagneticY,
    /// The magnetic field on the third axis, in gauss.
    ImuMagneticZ,
    /// The temperature of the inertial sensor, in degrees Celsius.
    ImuTemperature,
    /// The absolute pressure, in hectopascals.
    BarometerPressure,
    /// The differential pressure, in hectopascals.
    BarometerDifferentialPressure,
    /// The temperature of the barometer, in degrees Celsius.
    BarometerTemperature,
    /// The pulse width of the first servo, in microseconds.
    ServoOutput1,
    /// The pulse width of the second servo, in microseconds.
    ServoOutput2,
    /// The pulse width of the third servo, in microseconds.
    ServoOutput3,
    /// The pulse width of the fourth servo, in microseconds.
    ServoOutput4,
    /// The pulse width of the fifth servo, in microseconds.
    ServoOutput5,
    /// The pulse width of the sixth servo, in microseconds.
    ServoOutput6,
    /// The pulse width of the seventh servo, in microseconds.
    ServoOutput7,
    /// The pulse width of the eighth servo, in microseconds.
    ServoOutput8,
    /// The value of the first radio channel, in microseconds.
    RcInput1,
    /// The value of the second radio channel, in microseconds.
    RcInput2,
    /// The value of the third radio channel, in microseconds.
    RcInput3,
    /// The value of the fourth radio channel, in microseconds.
    RcInput4,
    /// The value of the fifth radio channel, in microseconds.
    RcInput5,
    /// The value of the sixth radio channel, in microseconds.
    RcInput6,
    /// The value of the seventh radio channel, in microseconds.
    RcInput7,
    /// The value of the eighth radio channel, in microseconds.
    RcInput8,
    /// The strength of the radio signal, as a percentage.
    RcSignal,
    /// The speed through the air, in metres per second.
    Airspeed,
    /// The speed over the ground, in metres per second.
    AirspeedGroundSpeed,
    /// The rate of climb, in metres per second.
    AirspeedClimb,
    /// The throttle, as a percentage.
    AirspeedThrottle,
}

/// Every field, in the order they are stored.
pub const FIELDS: &[Field] = &[
    Field::AttitudeRoll,
    Field::AttitudePitch,
    Field::AttitudeYaw,
    Field::VelocityGroundSpeed,
    Field::VelocityNorth,
    Field::VelocityEast,
    Field::VelocityDown,
    Field::PositionAltitude,
    Field::PositionLatitude,
    Field::PositionLongitude,
    Field::BatteryVoltage,
    Field::BatteryCurrent,
    Field::BatteryRemaining,
    Field::DistanceToHome,
    Field::BearingToHome,
    Field::GpsSatellites,
    Field::GpsHdop,
    Field::MissionSequence,
    Field::MissionTotal,
    Field::Armed,
    Field::ImuAccelerationX,
    Field::ImuAccelerationY,
    Field::ImuAccelerationZ,
    Field::ImuAngularRateX,
    Field::ImuAngularRateY,
    Field::ImuAngularRateZ,
    Field::ImuMagneticX,
    Field::ImuMagneticY,
    Field::ImuMagneticZ,
    Field::ImuTemperature,
    Field::BarometerPressure,
    Field::BarometerDifferentialPressure,
    Field::BarometerTemperature,
    Field::ServoOutput1,
    Field::ServoOutput2,
    Field::ServoOutput3,
    Field::ServoOutput4,
    Field::ServoOutput5,
    Field::ServoOutput6,
    Field::ServoOutput7,
    Field::ServoOutput8,
    Field::RcInput1,
    Field::RcInput2,
    Field::RcInput3,
    Field::RcInput4,
    Field::RcInput5,
    Field::RcInput6,
    Field::RcInput7,
    Field::RcInput8,
    Field::RcSignal,
    Field::Airspeed,
    Field::AirspeedGroundSpeed,
    Field::AirspeedClimb,
    Field::AirspeedThrottle,
];

impl Field {
    /// The name the expressions use.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::AttitudeRoll => "attitude.roll",
            Self::AttitudePitch => "attitude.pitch",
            Self::AttitudeYaw => "attitude.yaw",
            Self::VelocityGroundSpeed => "velocity.groundSpeed",
            Self::VelocityNorth => "velocity.north",
            Self::VelocityEast => "velocity.east",
            Self::VelocityDown => "velocity.down",
            Self::PositionAltitude => "position.altitude",
            Self::PositionLatitude => "position.latitude",
            Self::PositionLongitude => "position.longitude",
            Self::BatteryVoltage => "battery.voltage",
            Self::BatteryCurrent => "battery.current",
            Self::BatteryRemaining => "battery.remaining",
            Self::DistanceToHome => "distanceToHome",
            Self::BearingToHome => "bearingToHome",
            Self::GpsSatellites => "gps.satellites",
            Self::GpsHdop => "gps.hdop",
            Self::MissionSequence => "mission.sequence",
            Self::MissionTotal => "mission.total",
            Self::Armed => "armed",
            Self::ImuAccelerationX => "imu.accelerationX",
            Self::ImuAccelerationY => "imu.accelerationY",
            Self::ImuAccelerationZ => "imu.accelerationZ",
            Self::ImuAngularRateX => "imu.angularRateX",
            Self::ImuAngularRateY => "imu.angularRateY",
            Self::ImuAngularRateZ => "imu.angularRateZ",
            Self::ImuMagneticX => "imu.magneticX",
            Self::ImuMagneticY => "imu.magneticY",
            Self::ImuMagneticZ => "imu.magneticZ",
            Self::ImuTemperature => "imu.temperature",
            Self::BarometerPressure => "barometer.pressure",
            Self::BarometerDifferentialPressure => "barometer.differentialPressure",
            Self::BarometerTemperature => "barometer.temperature",
            Self::ServoOutput1 => "servos.output1",
            Self::ServoOutput2 => "servos.output2",
            Self::ServoOutput3 => "servos.output3",
            Self::ServoOutput4 => "servos.output4",
            Self::ServoOutput5 => "servos.output5",
            Self::ServoOutput6 => "servos.output6",
            Self::ServoOutput7 => "servos.output7",
            Self::ServoOutput8 => "servos.output8",
            Self::RcInput1 => "rc.input1",
            Self::RcInput2 => "rc.input2",
            Self::RcInput3 => "rc.input3",
            Self::RcInput4 => "rc.input4",
            Self::RcInput5 => "rc.input5",
            Self::RcInput6 => "rc.input6",
            Self::RcInput7 => "rc.input7",
            Self::RcInput8 => "rc.input8",
            Self::RcSignal => "rc.signal",
            Self::Airspeed => "airspeed.airspeed",
            Self::AirspeedGroundSpeed => "airspeed.groundSpeed",
            Self::AirspeedClimb => "airspeed.climb",
            Self::AirspeedThrottle => "airspeed.throttle",
        }
    }

    /// The unit the value is in.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::AttitudeRoll | Self::AttitudePitch | Self::AttitudeYaw => "°",
            Self::VelocityGroundSpeed
            | Self::VelocityNorth
            | Self::VelocityEast
            | Self::VelocityDown => "m/s",
            Self::PositionAltitude | Self::DistanceToHome => "m",
            Self::PositionLatitude | Self::PositionLongitude | Self::BearingToHome => "°",
            Self::BatteryVoltage => "V",
            Self::BatteryCurrent => "A",
            Self::BatteryRemaining => "%",
            Self::GpsSatellites => "",
            Self::GpsHdop => "",
            Self::MissionSequence | Self::MissionTotal => "",
            Self::Armed => "",
            Self::ImuAccelerationX | Self::ImuAccelerationY | Self::ImuAccelerationZ => "m/s²",
            Self::ImuAngularRateX | Self::ImuAngularRateY | Self::ImuAngularRateZ => "°/s",
            Self::ImuMagneticX | Self::ImuMagneticY | Self::ImuMagneticZ => "gauss",
            Self::ImuTemperature | Self::BarometerTemperature => "°C",
            Self::BarometerPressure | Self::BarometerDifferentialPressure => "hPa",
            Self::ServoOutput1
            | Self::ServoOutput2
            | Self::ServoOutput3
            | Self::ServoOutput4
            | Self::ServoOutput5
            | Self::ServoOutput6
            | Self::ServoOutput7
            | Self::ServoOutput8 => "µs",
            Self::RcInput1
            | Self::RcInput2
            | Self::RcInput3
            | Self::RcInput4
            | Self::RcInput5
            | Self::RcInput6
            | Self::RcInput7
            | Self::RcInput8 => "µs",
            Self::RcSignal => "%",
            Self::Airspeed | Self::AirspeedGroundSpeed | Self::AirspeedClimb => "m/s",
            Self::AirspeedThrottle => "%",
        }
    }

    /// The field with the given name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        FIELDS.iter().copied().find(|field| field.name() == name)
    }

    /// The index of the field in the sample.
    #[must_use]
    pub fn index(self) -> usize {
        FIELDS.iter().position(|field| *field == self).unwrap_or(0)
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// One instant of the telemetry, as the graphs see it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// How long after the start of the history this instant is.
    pub at: std::time::Duration,
    /// The value of every field, in the order of [`FIELDS`].
    pub values: Vec<Option<f64>>,
}

impl Sample {
    /// Takes a sample of the telemetry.
    ///
    /// The attitude is converted to degrees here, once, so that the expressions read the
    /// way an operator reads them.
    #[must_use]
    pub fn from_telemetry(telemetry: &Telemetry, at: std::time::Duration) -> Self {
        let mut values = vec![None; FIELDS.len()];

        let mut set = |field: Field, value: Option<f64>| {
            values[field.index()] = value;
        };

        if let Some(attitude) = telemetry.attitude {
            set(Field::AttitudeRoll, Some(attitude.roll.to_degrees()));
            set(Field::AttitudePitch, Some(attitude.pitch.to_degrees()));
            set(Field::AttitudeYaw, Some(attitude.yaw.to_degrees()));
        }

        if let Some(velocity) = telemetry.velocity {
            set(Field::VelocityGroundSpeed, Some(velocity.ground_speed));
            set(Field::VelocityNorth, Some(velocity.north));
            set(Field::VelocityEast, Some(velocity.east));
            set(Field::VelocityDown, Some(velocity.down));
        }

        if let Some(position) = telemetry.position {
            set(Field::PositionAltitude, Some(position.alt));
            set(Field::PositionLatitude, Some(position.lat));
            set(Field::PositionLongitude, Some(position.lng));
        }

        if let Some(battery) = telemetry.battery {
            set(Field::BatteryVoltage, Some(battery.voltage));
            set(Field::BatteryCurrent, Some(battery.current));
            set(Field::BatteryRemaining, Some(battery.remaining));
        }

        set(Field::DistanceToHome, telemetry.distance_to_home);
        set(Field::BearingToHome, telemetry.bearing_to_home);

        if let Some(gps) = telemetry.gps {
            set(Field::GpsSatellites, Some(f64::from(gps.satellites)));
            set(Field::GpsHdop, Some(gps.hdop));
        }

        if let Some(mission) = telemetry.mission {
            set(Field::MissionSequence, Some(f64::from(mission.seq)));
            set(Field::MissionTotal, Some(f64::from(mission.total)));
        }

        set(
            Field::Armed,
            telemetry.armed.map(|armed| f64::from(u8::from(armed))),
        );

        if let Some(imu) = telemetry.imu {
            set(Field::ImuAccelerationX, Some(imu.acceleration[0]));
            set(Field::ImuAccelerationY, Some(imu.acceleration[1]));
            set(Field::ImuAccelerationZ, Some(imu.acceleration[2]));
            // The model keeps the rate in radians; the graph reads it in degrees, the way
            // an operator reads it.
            set(
                Field::ImuAngularRateX,
                Some(imu.angular_rate[0].to_degrees()),
            );
            set(
                Field::ImuAngularRateY,
                Some(imu.angular_rate[1].to_degrees()),
            );
            set(
                Field::ImuAngularRateZ,
                Some(imu.angular_rate[2].to_degrees()),
            );
            set(Field::ImuMagneticX, Some(imu.magnetic_field[0]));
            set(Field::ImuMagneticY, Some(imu.magnetic_field[1]));
            set(Field::ImuMagneticZ, Some(imu.magnetic_field[2]));
            set(Field::ImuTemperature, Some(imu.temperature));
        }

        if let Some(barometer) = telemetry.barometer {
            set(Field::BarometerPressure, Some(barometer.absolute_pressure));
            set(
                Field::BarometerDifferentialPressure,
                Some(barometer.differential_pressure),
            );
            set(Field::BarometerTemperature, Some(barometer.temperature));
        }

        if let Some(controls) = telemetry.controls {
            for (index, output) in controls.outputs.iter().enumerate() {
                set(servo_field(index), *output);
            }
            for (index, input) in controls.inputs.iter().enumerate() {
                set(rc_field(index), *input);
            }
            set(Field::RcSignal, controls.signal);
        }

        if let Some(airspeed) = telemetry.airspeed {
            set(Field::Airspeed, airspeed.airspeed);
            set(Field::AirspeedGroundSpeed, Some(airspeed.ground_speed));
            set(Field::AirspeedClimb, Some(airspeed.climb));
            set(Field::AirspeedThrottle, Some(airspeed.throttle));
        }

        Self { at, values }
    }

    /// The value of a field, if the vehicle reported it.
    #[must_use]
    pub fn get(&self, field: Field) -> Option<f64> {
        self.values.get(field.index()).copied().flatten()
    }
}

/// The field of the servo with the given place.
///
/// The servos are numbered from one, the way they are on the vehicle.
fn servo_field(index: usize) -> Field {
    match index {
        0 => Field::ServoOutput1,
        1 => Field::ServoOutput2,
        2 => Field::ServoOutput3,
        3 => Field::ServoOutput4,
        4 => Field::ServoOutput5,
        5 => Field::ServoOutput6,
        6 => Field::ServoOutput7,
        _ => Field::ServoOutput8,
    }
}

/// The field of the radio channel with the given place.
///
/// The channels are numbered from one, the way they are on the transmitter.
fn rc_field(index: usize) -> Field {
    match index {
        0 => Field::RcInput1,
        1 => Field::RcInput2,
        2 => Field::RcInput3,
        3 => Field::RcInput4,
        4 => Field::RcInput5,
        5 => Field::RcInput6,
        6 => Field::RcInput7,
        _ => Field::RcInput8,
    }
}

/// A function the expressions can call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    /// The angle in degrees.
    Degrees,
    /// The angle in radians.
    Radians,
    /// The square root.
    Sqrt,
    /// The absolute value.
    Abs,
    /// The smallest of the arguments.
    Min,
    /// The largest of the arguments.
    Max,
    /// The largest whole number that is not greater than the argument.
    Floor,
    /// The smallest whole number that is not less than the argument.
    Ceil,
    /// The nearest whole number.
    Round,
    /// The sign of the argument: one, zero or minus one.
    Sign,
}

impl Function {
    /// The function with the given name, if there is one.
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "degrees" => Some(Self::Degrees),
            "radians" => Some(Self::Radians),
            "sqrt" => Some(Self::Sqrt),
            "abs" => Some(Self::Abs),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "floor" => Some(Self::Floor),
            "ceil" => Some(Self::Ceil),
            "round" => Some(Self::Round),
            "sign" => Some(Self::Sign),
            _ => None,
        }
    }

    /// Applies the function.
    fn apply(self, arguments: &[f64]) -> Option<f64> {
        match self {
            Self::Degrees => arguments.first().map(|value| value.to_degrees()),
            Self::Radians => arguments.first().map(|value| value.to_radians()),
            Self::Sqrt => arguments.first().map(|value| value.sqrt()),
            Self::Abs => arguments.first().map(|value| value.abs()),
            Self::Min => arguments.iter().copied().reduce(f64::min),
            Self::Max => arguments.iter().copied().reduce(f64::max),
            Self::Floor => arguments.first().map(|value| value.floor()),
            Self::Ceil => arguments.first().map(|value| value.ceil()),
            Self::Round => arguments.first().map(|value| value.round()),
            Self::Sign => arguments.first().map(|value| {
                if *value > 0.0 {
                    1.0
                } else if *value < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }),
        }
    }
}

/// An expression over the telemetry.
///
/// The grammar is the one upstream's graphs use: the arithmetic operators, the power, and
/// a handful of functions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A constant.
    Number(f64),
    /// A field of the telemetry.
    Field(Field),
    /// The negation of an expression.
    Negate(Box<Expression>),
    /// The sum of two expressions.
    Add(Box<Expression>, Box<Expression>),
    /// The difference of two expressions.
    Subtract(Box<Expression>, Box<Expression>),
    /// The product of two expressions.
    Multiply(Box<Expression>, Box<Expression>),
    /// The quotient of two expressions.
    Divide(Box<Expression>, Box<Expression>),
    /// One expression raised to the power of another.
    Power(Box<Expression>, Box<Expression>),
    /// A call of a function.
    Call(Function, Vec<Expression>),
}

impl Expression {
    /// Evaluates the expression over a sample.
    ///
    /// Returns `None` when the expression needs a field the vehicle has not reported: a
    /// graph that draws on an unknown value shows a gap, not a zero.
    #[must_use]
    pub fn evaluate(&self, sample: &Sample) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Field(field) => sample.get(*field),
            Self::Negate(inner) => inner.evaluate(sample).map(|value| -value),
            Self::Add(left, right) => Some(left.evaluate(sample)? + right.evaluate(sample)?),
            Self::Subtract(left, right) => Some(left.evaluate(sample)? - right.evaluate(sample)?),
            Self::Multiply(left, right) => Some(left.evaluate(sample)? * right.evaluate(sample)?),
            Self::Divide(left, right) => {
                let divisor = right.evaluate(sample)?;
                if divisor == 0.0 {
                    return None;
                }
                Some(left.evaluate(sample)? / divisor)
            }
            Self::Power(left, right) => {
                let base = left.evaluate(sample)?;
                let exponent = right.evaluate(sample)?;
                Some(base.powf(exponent))
            }
            Self::Call(function, arguments) => {
                let values: Option<Vec<f64>> = arguments
                    .iter()
                    .map(|argument| argument.evaluate(sample))
                    .collect();
                function.apply(&values?)
            }
        }
    }
}

/// A failure in an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// The expression could not be read.
    Syntax {
        /// What went wrong.
        reason: String,
    },
    /// The expression names a field that does not exist.
    UnknownField {
        /// The name that was used.
        name: String,
    },
    /// The expression calls a function that does not exist.
    UnknownFunction {
        /// The name that was used.
        name: String,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { reason } => write!(f, "the expression cannot be read: {reason}"),
            Self::UnknownField { name } => write!(f, "there is no field called {name:?}"),
            Self::UnknownFunction { name } => write!(f, "there is no function called {name:?}"),
        }
    }
}

impl std::error::Error for GraphError {}

/// A token of an expression.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Operator(char),
    Open,
    Close,
    Comma,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(value) => write!(f, "{value}"),
            Self::Name(name) => write!(f, "{name}"),
            Self::Operator(operator) => write!(f, "{operator}"),
            Self::Open => write!(f, "("),
            Self::Close => write!(f, ")"),
            Self::Comma => write!(f, ","),
        }
    }
}

/// Splits an expression into its tokens.
fn tokenize(text: &str) -> Result<Vec<Token>, GraphError> {
    let mut tokens = Vec::new();
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];

        if character.is_whitespace() {
            index += 1;
            continue;
        }

        if character.is_ascii_digit() || (character == '.' && index + 1 < characters.len()) {
            let start = index;

            while index < characters.len()
                && (characters[index].is_ascii_digit() || characters[index] == '.')
            {
                index += 1;
            }

            let literal: String = characters[start..index].iter().collect();
            let value = literal.parse::<f64>().map_err(|_| GraphError::Syntax {
                reason: format!("{literal:?} is not a number"),
            })?;

            tokens.push(Token::Number(value));
            continue;
        }

        if character.is_alphabetic() || character == '_' {
            let start = index;

            while index < characters.len()
                && (characters[index].is_alphanumeric()
                    || characters[index] == '_'
                    || characters[index] == '.')
            {
                index += 1;
            }

            let name: String = characters[start..index].iter().collect();
            tokens.push(Token::Name(name));
            continue;
        }

        match character {
            '+' | '-' | '/' | '^' => {
                tokens.push(Token::Operator(character));
                index += 1;
            }
            '*' => {
                // The power is written with two stars, and it is a different operator.
                if characters.get(index + 1) == Some(&'*') {
                    tokens.push(Token::Operator('^'));
                    index += 2;
                } else {
                    tokens.push(Token::Operator('*'));
                    index += 1;
                }
            }
            '(' => {
                tokens.push(Token::Open);
                index += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                index += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                index += 1;
            }
            other => {
                return Err(GraphError::Syntax {
                    reason: format!("{other:?} is not part of an expression"),
                });
            }
        }
    }

    Ok(tokens)
}

/// Reads an expression.
struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    /// The token that comes next.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    /// Takes the token that comes next.
    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    /// Reads a sum.
    fn expression(&mut self) -> Result<Expression, GraphError> {
        let mut left = self.term()?;

        while matches!(self.peek(), Some(Token::Operator('+' | '-'))) {
            let Some(Token::Operator(operator)) = self.next() else {
                unreachable!("the operator was checked");
            };

            let right = self.term()?;

            left = match operator {
                '+' => Expression::Add(Box::new(left), Box::new(right)),
                '-' => Expression::Subtract(Box::new(left), Box::new(right)),
                _ => unreachable!("the operator was checked"),
            };
        }

        Ok(left)
    }

    /// Reads a product.
    fn term(&mut self) -> Result<Expression, GraphError> {
        let mut left = self.unary()?;

        while matches!(self.peek(), Some(Token::Operator('*' | '/'))) {
            let Some(Token::Operator(operator)) = self.next() else {
                unreachable!("the operator was checked");
            };

            let right = self.unary()?;

            left = match operator {
                '*' => Expression::Multiply(Box::new(left), Box::new(right)),
                '/' => Expression::Divide(Box::new(left), Box::new(right)),
                _ => unreachable!("the operator was checked"),
            };
        }

        Ok(left)
    }

    /// Reads a factor, which may be raised to a power.
    fn unary(&mut self) -> Result<Expression, GraphError> {
        if let Some(Token::Operator(operator)) = self.peek() {
            let operator = *operator;
            self.next();

            if operator == '-' {
                return Ok(Expression::Negate(Box::new(self.unary()?)));
            }

            if operator != '+' {
                return Err(GraphError::Syntax {
                    reason: format!("{operator} cannot start an expression"),
                });
            }
        }

        let base = self.primary()?;

        // The power binds tighter than the product, and it is right-associative.
        if let Some(Token::Operator('^')) = self.peek() {
            self.next();
            let exponent = self.unary()?;

            return Ok(Expression::Power(Box::new(base), Box::new(exponent)));
        }

        Ok(base)
    }

    /// Reads a number, a field, a call or a parenthesised expression.
    fn primary(&mut self) -> Result<Expression, GraphError> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expression::Number(value)),

            Some(Token::Name(name)) => {
                if let Some(Token::Open) = self.peek() {
                    self.next();

                    let function = Function::from_name(&name)
                        .ok_or(GraphError::UnknownFunction { name: name.clone() })?;

                    let mut arguments = Vec::new();

                    if !matches!(self.peek(), Some(Token::Close)) {
                        loop {
                            arguments.push(self.expression()?);

                            match self.next() {
                                Some(Token::Comma) => continue,
                                Some(Token::Close) => break,
                                _ => {
                                    return Err(GraphError::Syntax {
                                        reason: format!("the call of {name} is not closed"),
                                    });
                                }
                            }
                        }
                    } else {
                        self.next();
                    }

                    return Ok(Expression::Call(function, arguments));
                }

                let field = Field::from_name(&name).ok_or(GraphError::UnknownField { name })?;

                Ok(Expression::Field(field))
            }

            Some(Token::Open) => {
                let inner = self.expression()?;

                match self.next() {
                    Some(Token::Close) => Ok(inner),
                    _ => Err(GraphError::Syntax {
                        reason: "the parenthesis is not closed".to_owned(),
                    }),
                }
            }

            Some(token) => Err(GraphError::Syntax {
                reason: format!("{token} cannot start an expression"),
            }),

            None => Err(GraphError::Syntax {
                reason: "the expression ends too soon".to_owned(),
            }),
        }
    }
}

/// Reads an expression.
///
/// ```
/// use mpcraft_vehicle::graph::{parse, Expression};
///
/// let expression = parse("degrees(attitude.roll) + 1").unwrap();
/// assert!(matches!(expression, Expression::Add(_, _)));
/// ```
///
/// # Errors
///
/// Returns an error when the expression cannot be read, or when it names a field or a
/// function that does not exist.
pub fn parse(text: &str) -> Result<Expression, GraphError> {
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens,
        position: 0,
    };

    let expression = parser.expression()?;

    if parser.position != parser.tokens.len() {
        return Err(GraphError::Syntax {
            reason: format!("{:?} is left over", parser.tokens[parser.position]),
        });
    }

    Ok(expression)
}

/// One series of a graph: a name, an expression and a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Series {
    /// The name of the series, as the legend shows it.
    pub name: &'static str,
    /// The expression, as it is written in the catalogue.
    pub expression: &'static str,
    /// The unit the values are in.
    pub unit: &'static str,
}

/// A graph the user can choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    /// The name of the preset, and the group it belongs to.
    pub name: PresetName,
    /// What the graph shows.
    pub description: &'static str,
    /// The series it draws.
    pub series: &'static [Series],
    /// Whether the preset comes from upstream's catalogue.
    ///
    /// The ones that do are ported from `mavgraphs.xml`; the ones that do not are
    /// MPCraft's own, for the values upstream does not graph.
    pub upstream: bool,
}

/// The catalogue of the presets MPCraft can draw.
///
/// The ones marked as upstream's are ported from Mission Planner's `mavgraphs.xml`, with
/// the expressions rewritten over the fields MPCraft decodes. Upstream has a hundred and
/// eight; the rest draw on messages MPCraft does not decode yet, and they are left out
/// rather than shown empty.
pub static PRESETS: &[Preset] = &[
    Preset {
        name: PresetName::new("Speed", "Ground Speed"),
        description: "This shows the ground speed of the vehicle",
        series: &[Series {
            name: "Ground speed",
            expression: "velocity.groundSpeed",
            unit: "m/s",
        }],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Attitude", "Roll and Pitch"),
        description: "Roll and Pitch",
        series: &[
            Series {
                name: "Roll",
                expression: "attitude.roll",
                unit: "°",
            },
            Series {
                name: "Pitch",
                expression: "attitude.pitch",
                unit: "°",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Attitude", "Circular Angle"),
        description: "The angle the vehicle is tilted by, from the vertical",
        series: &[Series {
            name: "Circular angle",
            expression: "sqrt(attitude.roll**2 + attitude.pitch**2)",
            unit: "°",
        }],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Attitude", "Yaw"),
        description: "The heading of the vehicle",
        series: &[Series {
            name: "Yaw",
            expression: "attitude.yaw",
            unit: "°",
        }],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Position", "Altitude vs Distance to Home"),
        description: "How high the vehicle is, and how far it is from home",
        series: &[
            Series {
                name: "Altitude",
                expression: "position.altitude",
                unit: "m",
            },
            Series {
                name: "Distance to home",
                expression: "distanceToHome",
                unit: "m",
            },
        ],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Speed", "Vertical Speed"),
        description: "How fast the vehicle is climbing or descending",
        series: &[Series {
            name: "Vertical speed",
            expression: "velocity.down",
            unit: "m/s",
        }],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Navigation", "Bearing to Home"),
        description: "The direction of home, and the heading of the vehicle",
        series: &[
            Series {
                name: "Bearing to home",
                expression: "bearingToHome",
                unit: "°",
            },
            Series {
                name: "Yaw",
                expression: "attitude.yaw",
                unit: "°",
            },
        ],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Battery", "Voltage and Charge"),
        description: "The state of the battery",
        series: &[
            Series {
                name: "Voltage",
                expression: "battery.voltage",
                unit: "V",
            },
            Series {
                name: "Remaining",
                expression: "battery.remaining",
                unit: "%",
            },
        ],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Position", "Positioning"),
        description: "The state of the global positioning",
        series: &[
            Series {
                name: "Satellites",
                expression: "gps.satellites",
                unit: "",
            },
            Series {
                name: "HDOP",
                expression: "gps.hdop",
                unit: "",
            },
        ],
        upstream: false,
    },
    Preset {
        name: PresetName::new("Speed", "Ground vs Air Speed"),
        description: "This shows the ground speed of the vehicle versus its air speed",
        series: &[
            Series {
                name: "Ground speed",
                expression: "airspeed.groundSpeed",
                unit: "m/s",
            },
            Series {
                name: "Air speed",
                expression: "airspeed.airspeed",
                unit: "m/s",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Sensors/Accelerometer", "Accelerometers"),
        description: "Accelerometer Output",
        series: &[
            Series {
                name: "X",
                expression: "imu.accelerationX",
                unit: "m/s²",
            },
            Series {
                name: "Y",
                expression: "imu.accelerationY",
                unit: "m/s²",
            },
            Series {
                name: "Z",
                expression: "imu.accelerationZ",
                unit: "m/s²",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Sensors/Gyroscope", "Gyros"),
        description: "Gyroscope Output",
        series: &[
            Series {
                name: "X",
                expression: "imu.angularRateX",
                unit: "°/s",
            },
            Series {
                name: "Y",
                expression: "imu.angularRateY",
                unit: "°/s",
            },
            Series {
                name: "Z",
                expression: "imu.angularRateZ",
                unit: "°/s",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Sensors/Compass", "Compass"),
        description: "Primary Compass",
        series: &[
            Series {
                name: "X",
                expression: "imu.magneticX",
                unit: "gauss",
            },
            Series {
                name: "Y",
                expression: "imu.magneticY",
                unit: "gauss",
            },
            Series {
                name: "Z",
                expression: "imu.magneticZ",
                unit: "gauss",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Sensors/Barometer", "Barometer"),
        description: "Barometer",
        series: &[Series {
            name: "Temperature",
            expression: "barometer.temperature",
            unit: "°C",
        }],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Sensors/Barometer", "Barometric Pressure"),
        description: "Barometric Pressure",
        series: &[Series {
            name: "Pressure",
            expression: "barometer.pressure",
            unit: "hPa",
        }],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Servos", "Servos 1-4"),
        description: "First 4 servo outputs",
        series: &[
            Series {
                name: "Servo 1",
                expression: "servos.output1",
                unit: "µs",
            },
            Series {
                name: "Servo 2",
                expression: "servos.output2",
                unit: "µs",
            },
            Series {
                name: "Servo 3",
                expression: "servos.output3",
                unit: "µs",
            },
            Series {
                name: "Servo 4",
                expression: "servos.output4",
                unit: "µs",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("Servos", "Servos 1-8"),
        description: "First 8 servo outputs",
        series: &[
            Series {
                name: "Servo 1",
                expression: "servos.output1",
                unit: "µs",
            },
            Series {
                name: "Servo 2",
                expression: "servos.output2",
                unit: "µs",
            },
            Series {
                name: "Servo 3",
                expression: "servos.output3",
                unit: "µs",
            },
            Series {
                name: "Servo 4",
                expression: "servos.output4",
                unit: "µs",
            },
            Series {
                name: "Servo 5",
                expression: "servos.output5",
                unit: "µs",
            },
            Series {
                name: "Servo 6",
                expression: "servos.output6",
                unit: "µs",
            },
            Series {
                name: "Servo 7",
                expression: "servos.output7",
                unit: "µs",
            },
            Series {
                name: "Servo 8",
                expression: "servos.output8",
                unit: "µs",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("RC", "RC Input 1-4"),
        description: "First 4 RC inputs",
        series: &[
            Series {
                name: "Channel 1",
                expression: "rc.input1",
                unit: "µs",
            },
            Series {
                name: "Channel 2",
                expression: "rc.input2",
                unit: "µs",
            },
            Series {
                name: "Channel 3",
                expression: "rc.input3",
                unit: "µs",
            },
            Series {
                name: "Channel 4",
                expression: "rc.input4",
                unit: "µs",
            },
        ],
        upstream: true,
    },
    Preset {
        name: PresetName::new("RC", "RC Input 1-8"),
        description: "First 8 RC inputs",
        series: &[
            Series {
                name: "Channel 1",
                expression: "rc.input1",
                unit: "µs",
            },
            Series {
                name: "Channel 2",
                expression: "rc.input2",
                unit: "µs",
            },
            Series {
                name: "Channel 3",
                expression: "rc.input3",
                unit: "µs",
            },
            Series {
                name: "Channel 4",
                expression: "rc.input4",
                unit: "µs",
            },
            Series {
                name: "Channel 5",
                expression: "rc.input5",
                unit: "µs",
            },
            Series {
                name: "Channel 6",
                expression: "rc.input6",
                unit: "µs",
            },
            Series {
                name: "Channel 7",
                expression: "rc.input7",
                unit: "µs",
            },
            Series {
                name: "Channel 8",
                expression: "rc.input8",
                unit: "µs",
            },
        ],
        upstream: true,
    },
];

/// A series whose expression has been read.
#[derive(Debug, Clone)]
pub struct CompiledSeries {
    /// The name of the series.
    pub name: &'static str,
    /// The unit the values are in.
    pub unit: &'static str,
    /// The expression.
    pub expression: Expression,
}

/// A preset whose expressions have been read.
#[derive(Debug, Clone)]
pub struct CompiledPreset {
    /// The preset.
    pub preset: &'static Preset,
    /// The series, ready to evaluate.
    pub series: Vec<CompiledSeries>,
}

/// The catalogue with every expression read.
///
/// The expressions are read once, when they are first needed: a graph that is redrawn
/// twenty times a second must not parse its expressions twenty times a second.
pub fn catalogue() -> Vec<CompiledPreset> {
    use std::sync::OnceLock;

    static CATALOGUE: OnceLock<Vec<CompiledPreset>> = OnceLock::new();

    CATALOGUE
        .get_or_init(|| {
            PRESETS
                .iter()
                .map(|preset| CompiledPreset {
                    preset,
                    series: preset
                        .series
                        .iter()
                        .filter_map(|series| {
                            parse(series.expression)
                                .ok()
                                .map(|expression| CompiledSeries {
                                    name: series.name,
                                    unit: series.unit,
                                    expression,
                                })
                        })
                        .collect(),
                })
                .collect()
        })
        .clone()
}

/// The preset with the given full name, if there is one.
#[must_use]
pub fn preset_named(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|preset| preset.name.full() == name)
}

/// The history of the telemetry the graphs are drawn from.
///
/// The history is bounded twice over: by the number of samples and by how long they are
/// kept, because a graph of a long flight must not grow without limit.
#[derive(Debug, Clone)]
pub struct History {
    samples: std::collections::VecDeque<Sample>,
    /// The most samples that are kept.
    capacity: usize,
    /// How long the samples are kept.
    span: std::time::Duration,
    /// When the history started, so that the samples know where they are in time.
    start: std::time::Instant,
}

impl Default for History {
    fn default() -> Self {
        // The state is published five times a second, so fifteen hundred samples are five
        // minutes — which is the span the history keeps.
        Self::new(1500, std::time::Duration::from_secs(300))
    }
}

impl History {
    /// Creates a history that keeps the given number of samples over the given span.
    #[must_use]
    pub fn new(capacity: usize, span: std::time::Duration) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            span,
            start: std::time::Instant::now(),
        }
    }

    /// Adds a sample of the telemetry, timed from the start of the history.
    pub fn push_telemetry(&mut self, telemetry: &Telemetry) {
        let at = self.start.elapsed();
        self.push(Sample::from_telemetry(telemetry, at));
    }

    /// Adds a sample, and drops the ones that are too old.
    pub fn push(&mut self, sample: Sample) {
        self.samples.push_back(sample);

        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }

        if let Some(newest) = self.samples.back().map(|sample| sample.at) {
            let oldest = newest.saturating_sub(self.span);

            while self
                .samples
                .front()
                .is_some_and(|sample| sample.at < oldest)
            {
                self.samples.pop_front();
            }
        }
    }

    /// How many samples the history holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the history holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Empties the history.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// How long after the start of the history the oldest sample is.
    #[must_use]
    pub fn oldest(&self) -> std::time::Duration {
        self.samples
            .front()
            .map_or(std::time::Duration::ZERO, |sample| sample.at)
    }

    /// How long after the start of the history the newest sample is.
    #[must_use]
    pub fn newest(&self) -> std::time::Duration {
        self.samples
            .back()
            .map_or(std::time::Duration::ZERO, |sample| sample.at)
    }

    /// The values of an expression over the history, oldest first.
    ///
    /// The samples where the expression has no value are skipped, so a gap in the data is
    /// a gap in the line rather than a drop to zero.
    #[must_use]
    pub fn series(&self, expression: &Expression) -> Vec<(std::time::Duration, f64)> {
        self.samples
            .iter()
            .filter_map(|sample| expression.evaluate(sample).map(|value| (sample.at, value)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attitude, GeoPoint, Velocity};

    fn sample() -> Sample {
        let telemetry = Telemetry {
            attitude: Some(Attitude {
                roll: std::f64::consts::FRAC_PI_2,
                pitch: 0.0,
                yaw: 0.0,
            }),
            velocity: Some(Velocity {
                north: 3.0,
                east: 4.0,
                down: 0.0,
                ground_speed: 5.0,
            }),
            position: Some(GeoPoint {
                lat: -35.0,
                lng: 149.0,
                alt: 584.0,
            }),
            ..Telemetry::default()
        };

        Sample::from_telemetry(&telemetry, std::time::Duration::from_secs(1))
    }

    #[test]
    fn a_field_is_read_from_the_sample() {
        let expression = parse("velocity.groundSpeed").unwrap();
        assert_eq!(expression.evaluate(&sample()), Some(5.0));
    }

    #[test]
    fn the_arithmetic_is_the_usual_one() {
        assert_eq!(parse("1 + 2 * 3").unwrap().evaluate(&sample()), Some(7.0));
        assert_eq!(parse("(1 + 2) * 3").unwrap().evaluate(&sample()), Some(9.0));
        assert_eq!(parse("2 ** 3").unwrap().evaluate(&sample()), Some(8.0));
        assert_eq!(parse("-2 + 1").unwrap().evaluate(&sample()), Some(-1.0));
        assert_eq!(parse("10 / 4").unwrap().evaluate(&sample()), Some(2.5));
    }

    #[test]
    fn a_division_by_zero_has_no_value() {
        assert_eq!(parse("1 / 0").unwrap().evaluate(&sample()), None);
    }

    #[test]
    fn the_functions_are_the_usual_ones() {
        assert_eq!(
            parse("sqrt(attitude.roll**2 + attitude.pitch**2)")
                .unwrap()
                .evaluate(&sample()),
            Some(90.0)
        );
        assert_eq!(parse("abs(0 - 3)").unwrap().evaluate(&sample()), Some(3.0));
        assert_eq!(parse("min(1, 2)").unwrap().evaluate(&sample()), Some(1.0));
        assert_eq!(parse("max(1, 2)").unwrap().evaluate(&sample()), Some(2.0));
    }

    #[test]
    fn the_attitude_is_in_degrees() {
        let expression = parse("attitude.roll").unwrap();
        let value = expression.evaluate(&sample()).unwrap();
        assert!((value - 90.0).abs() < 1e-9);
    }

    #[test]
    fn a_field_the_vehicle_has_not_reported_has_no_value() {
        let telemetry = Telemetry::default();
        let empty = Sample::from_telemetry(&telemetry, std::time::Duration::ZERO);

        assert_eq!(
            parse("velocity.groundSpeed").unwrap().evaluate(&empty),
            None
        );
        // And the arithmetic over it has no value either.
        assert_eq!(
            parse("velocity.groundSpeed + 1").unwrap().evaluate(&empty),
            None
        );
    }

    #[test]
    fn an_unknown_field_is_an_error() {
        let error = parse("velocity.hyperspeed").unwrap_err();
        assert!(matches!(error, GraphError::UnknownField { .. }));
    }

    #[test]
    fn an_unknown_function_is_an_error() {
        let error = parse("sqrt(3) + nonsense(1)").unwrap_err();
        assert!(matches!(error, GraphError::UnknownFunction { .. }));
    }

    #[test]
    fn a_broken_expression_is_an_error_and_not_a_panic() {
        for text in ["1 +", "(1 + 2", "1 2", "** 3", ""] {
            assert!(parse(text).is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn the_fields_are_named_after_what_they_are() {
        assert_eq!(Field::from_name("attitude.roll"), Some(Field::AttitudeRoll));
        assert_eq!(Field::from_name("nothing"), None);
        assert_eq!(Field::AttitudeRoll.unit(), "°");
        assert_eq!(Field::VelocityGroundSpeed.unit(), "m/s");
    }

    #[test]
    fn the_catalogue_is_read_once_and_every_expression_is_valid() {
        let catalogue = catalogue();

        assert_eq!(catalogue.len(), PRESETS.len());

        for preset in &catalogue {
            assert!(
                !preset.series.is_empty(),
                "the preset {} has no series",
                preset.preset.name
            );
        }

        // Every expression of the catalogue parses, and the catalogue holds all of them.
        for preset in PRESETS {
            for series in preset.series {
                assert!(
                    parse(series.expression).is_ok(),
                    "the expression of {} does not parse",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn the_presets_that_come_from_upstream_say_so() {
        let upstream: Vec<String> = PRESETS
            .iter()
            .filter(|preset| preset.upstream)
            .map(|preset| preset.name.full())
            .collect();

        assert_eq!(
            upstream,
            vec![
                "Speed/Ground Speed",
                "Attitude/Roll and Pitch",
                "Attitude/Circular Angle",
                "Speed/Ground vs Air Speed",
                "Sensors/Accelerometer/Accelerometers",
                "Sensors/Gyroscope/Gyros",
                "Sensors/Compass/Compass",
                "Sensors/Barometer/Barometer",
                "Sensors/Barometer/Barometric Pressure",
                "Servos/Servos 1-4",
                "Servos/Servos 1-8",
                "RC/RC Input 1-4",
                "RC/RC Input 1-8",
            ]
        );
    }

    #[test]
    fn a_preset_is_found_by_its_full_name() {
        assert!(preset_named("Speed/Ground Speed").is_some());
        assert!(preset_named("Speed/Nothing").is_none());
    }

    #[test]
    fn the_history_keeps_the_samples_it_can() {
        let mut history = History::new(3, std::time::Duration::from_secs(60));

        for second in 0..10 {
            history.push(Sample {
                at: std::time::Duration::from_secs(second),
                values: vec![Some(second as f64); FIELDS.len()],
            });
        }

        assert_eq!(history.len(), 3);
    }

    #[test]
    fn the_history_drops_the_samples_that_are_too_old() {
        let mut history = History::new(100, std::time::Duration::from_secs(5));

        for second in 0..20 {
            history.push(Sample {
                at: std::time::Duration::from_secs(second),
                values: vec![Some(1.0); FIELDS.len()],
            });
        }

        // Only the samples of the last five seconds are kept.
        assert!(
            history.len() <= 6,
            "the history kept {} samples",
            history.len()
        );
    }

    #[test]
    fn the_history_draws_a_series_over_time() {
        let mut history = History::new(100, std::time::Duration::from_secs(60));

        for second in 0..5 {
            history.push(Sample {
                at: std::time::Duration::from_secs(second),
                values: vec![Some(second as f64); FIELDS.len()],
            });
        }

        let expression = parse("velocity.groundSpeed").unwrap();
        let series = history.series(&expression);

        assert_eq!(series.len(), 5);
        assert_eq!(series[0].1, 0.0);
        assert_eq!(series[4].1, 4.0);
    }
}
