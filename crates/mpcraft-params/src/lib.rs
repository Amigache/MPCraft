//! Parameter metadata and the parameter store.
//!
//! # The wire format
//!
//! A MAVLink parameter value travels in a four-byte field declared as a `float`. The
//! `param_type` says what the four bytes actually mean, and there are two ways they can be
//! filled:
//!
//! * **Byte-wise** — the parameter's own bytes are copied into the field. A sixteen-bit integer
//!   is sent as its sixteen bits, padded to four.
//! * **C-style cast** — the value is converted to a `float` first. A sixteen-bit integer of
//!   1000 travels as the float `1000.0`.
//!
//! The two are *not* interchangeable: the bits of the integer 1000 read as a float are a
//! denormal, not a number. A ground station that guesses wrong corrupts every integer
//! parameter it reads.
//!
//! # Which encoding a vehicle uses
//!
//! The vehicle says, in `AUTOPILOT_VERSION.capabilities`:
//!
//! | Capability | Meaning |
//! | --- | --- |
//! | `PARAM_ENCODE_BYTEWISE` | The values are byte-wise encoded. |
//! | `PARAM_ENCODE_C_CAST` | The values are cast to a float. |
//! | `PARAM_FLOAT` | Deprecated. The values are floats. |
//!
//! Two real autopilots need no capability bit at all:
//!
//! * **ArduPilot** uses the float encoding and ignores the `param_type` in a `PARAM_SET`. Its
//!   parameters are all floats.
//! * **PX4** uses the byte-wise encoding but does not set the bit.
//!
//! Mission Planner's rule — *"use the float encoding when the vehicle advertises
//! `PARAM_FLOAT` or is an ArduPilot, and the reported type otherwise"* — covers both. MPCraft
//! keeps that rule and adds the two current capability bits, so a vehicle that sets them is
//! understood correctly too.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::BTreeMap;

use mpcraft_mavlink::{MavAutopilot, MavParamType, MavProtocolCapability};
use serde::{Deserialize, Serialize};

pub mod metadata;
pub mod param_file;

/// The type of a parameter value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ParamType {
    /// An eight-bit unsigned integer.
    UInt8,
    /// An eight-bit signed integer.
    Int8,
    /// A sixteen-bit unsigned integer.
    UInt16,
    /// A sixteen-bit signed integer.
    Int16,
    /// A thirty-two-bit unsigned integer.
    UInt32,
    /// A thirty-two-bit signed integer.
    Int32,
    /// A sixty-four-bit unsigned integer.
    UInt64,
    /// A sixty-four-bit signed integer.
    Int64,
    /// A single-precision float.
    Real32,
    /// A double-precision float.
    Real64,
}

impl ParamType {
    /// The MAVLink wire type this corresponds to.
    #[must_use]
    pub fn as_mavlink(self) -> MavParamType {
        match self {
            Self::UInt8 => MavParamType::MAV_PARAM_TYPE_UINT8,
            Self::Int8 => MavParamType::MAV_PARAM_TYPE_INT8,
            Self::UInt16 => MavParamType::MAV_PARAM_TYPE_UINT16,
            Self::Int16 => MavParamType::MAV_PARAM_TYPE_INT16,
            Self::UInt32 => MavParamType::MAV_PARAM_TYPE_UINT32,
            Self::Int32 => MavParamType::MAV_PARAM_TYPE_INT32,
            Self::UInt64 => MavParamType::MAV_PARAM_TYPE_UINT64,
            Self::Int64 => MavParamType::MAV_PARAM_TYPE_INT64,
            Self::Real32 => MavParamType::MAV_PARAM_TYPE_REAL32,
            Self::Real64 => MavParamType::MAV_PARAM_TYPE_REAL64,
        }
    }

    /// The type a MAVLink wire type corresponds to, if it is one MPCraft understands.
    #[must_use]
    pub fn from_mavlink(value: MavParamType) -> Option<Self> {
        Some(match value {
            MavParamType::MAV_PARAM_TYPE_UINT8 => Self::UInt8,
            MavParamType::MAV_PARAM_TYPE_INT8 => Self::Int8,
            MavParamType::MAV_PARAM_TYPE_UINT16 => Self::UInt16,
            MavParamType::MAV_PARAM_TYPE_INT16 => Self::Int16,
            MavParamType::MAV_PARAM_TYPE_UINT32 => Self::UInt32,
            MavParamType::MAV_PARAM_TYPE_INT32 => Self::Int32,
            MavParamType::MAV_PARAM_TYPE_UINT64 => Self::UInt64,
            MavParamType::MAV_PARAM_TYPE_INT64 => Self::Int64,
            MavParamType::MAV_PARAM_TYPE_REAL32 => Self::Real32,
            MavParamType::MAV_PARAM_TYPE_REAL64 => Self::Real64,
        })
    }

    /// Whether the type can be carried exactly in the four-byte wire field.
    ///
    /// A sixty-four-bit value cannot: the field is four bytes wide, so only the low half of
    /// such a value survives. MPCraft reports such a parameter but cannot exchange it
    /// exactly.
    ///
    /// ```
    /// use mpcraft_params::ParamType;
    ///
    /// assert!(ParamType::Int32.is_representable());
    /// assert!(!ParamType::Int64.is_representable());
    /// ```
    #[must_use]
    pub const fn is_representable(self) -> bool {
        !matches!(self, Self::UInt64 | Self::Int64 | Self::Real64)
    }

    /// Decodes the four wire bytes as this type's own byte layout.
    ///
    /// This is the byte-wise interpretation: the bytes are the little-endian representation of
    /// an integer, or the bits of a float.
    ///
    /// ```
    /// use mpcraft_params::ParamType;
    ///
    /// // The bytes of the integer 1000.
    /// assert_eq!(ParamType::UInt16.decode_raw(1000u32), 1000.0);
    /// ```
    #[must_use]
    pub fn decode_raw(self, wire_bits: u32) -> f64 {
        let bytes = wire_bits.to_le_bytes();

        match self {
            Self::UInt8 => f64::from(bytes[0]),
            Self::Int8 => f64::from(bytes[0] as i8),
            Self::UInt16 => f64::from(u16::from_le_bytes([bytes[0], bytes[1]])),
            Self::Int16 => f64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
            Self::UInt32 => f64::from(u32::from_le_bytes(bytes)),
            Self::Int32 => f64::from(i32::from_le_bytes(bytes)),
            // The wire field is four bytes wide, so a sixty-four-bit value only carries
            // its low half. See `is_representable`.
            Self::UInt64 => f64::from(u32::from_le_bytes(bytes)),
            Self::Int64 => f64::from(i32::from_le_bytes(bytes)),
            Self::Real32 => f64::from(f32::from_bits(wire_bits)),
            Self::Real64 => f64::from(f32::from_bits(u32::from_le_bytes(bytes))),
        }
    }

    /// Encodes a value into the four wire bytes using this type's own byte layout.
    ///
    /// ```
    /// use mpcraft_params::ParamType;
    ///
    /// assert_eq!(ParamType::UInt16.encode_raw(1000.0), 1000);
    /// assert_eq!(ParamType::Real32.encode_raw(0.5), 0.5f32.to_bits());
    /// ```
    #[must_use]
    pub fn encode_raw(self, value: f64) -> u32 {
        let bytes: [u8; 4] = match self {
            Self::UInt8 => {
                let v = value as u8;
                [v, 0, 0, 0]
            }
            Self::Int8 => {
                let v = value as i8;
                [v as u8, 0, 0, 0]
            }
            Self::UInt16 => {
                let v = value as u16;
                [v as u8, (v >> 8) as u8, 0, 0]
            }
            Self::Int16 => {
                let v = value as i16;
                [v as u8, (v >> 8) as u8, 0, 0]
            }
            Self::UInt32 => (value as u32).to_le_bytes(),
            Self::Int32 => (value as i32).to_le_bytes(),
            // The wire field is four bytes wide, so a sixty-four-bit value only carries
            // its low half. See `is_representable`.
            Self::UInt64 => (value as u64).to_le_bytes()[..4]
                .try_into()
                .unwrap_or([0, 0, 0, 0]),
            Self::Int64 => (value as i64).to_le_bytes()[..4]
                .try_into()
                .unwrap_or([0, 0, 0, 0]),
            Self::Real32 => (value as f32).to_le_bytes(),
            Self::Real64 => {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&value.to_le_bytes()[..4]);
                bytes
            }
        };

        u32::from_le_bytes(bytes)
    }
}

/// How a vehicle encodes its parameter values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamEncoding {
    /// The four wire bytes are the bits of a single-precision float.
    Float,
    /// The four wire bytes are the parameter's own bytes.
    ByteWise,
}

impl ParamEncoding {
    /// Decodes a value from the wire.
    ///
    /// `reported_type` is ignored in the float encoding: the vehicle says the value is a
    /// float, whatever the type it reported.
    #[must_use]
    pub fn decode(self, reported_type: ParamType, wire_bits: u32) -> f64 {
        match self {
            Self::Float => f64::from(f32::from_bits(wire_bits)),
            Self::ByteWise => reported_type.decode_raw(wire_bits),
        }
    }

    /// Encodes a value for the wire.
    #[must_use]
    pub fn encode(self, reported_type: ParamType, value: f64) -> u32 {
        match self {
            Self::Float => (value as f32).to_bits(),
            Self::ByteWise => reported_type.encode_raw(value),
        }
    }
}

/// Works out how a vehicle encodes its parameter values.
///
/// The rules, in order:
///
/// 1. A vehicle that advertises the deprecated `PARAM_FLOAT` capability sends floats.
/// 2. An ArduPilot autopilot sends floats, whether or not it advertises the capability.
/// 3. A vehicle that advertises `PARAM_ENCODE_BYTEWISE` sends its parameters' own bytes.
/// 4. A vehicle that advertises `PARAM_ENCODE_C_CAST` sends values cast to a float.
/// 5. Otherwise the byte-wise encoding is assumed, which is what PX4 does.
///
/// ```
/// use mpcraft_params::{ParamEncoding, resolve_encoding};
/// use mpcraft_mavlink::MavAutopilot;
///
/// // An ArduPilot sends floats.
/// let encoding = resolve_encoding(None, Some(MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA));
/// assert_eq!(encoding, ParamEncoding::Float);
/// ```
#[must_use]
pub fn resolve_encoding(
    capabilities: Option<MavProtocolCapability>,
    autopilot: Option<MavAutopilot>,
) -> ParamEncoding {
    if let Some(capabilities) = capabilities {
        if advertises_param_float(&capabilities) {
            return ParamEncoding::Float;
        }
        if capabilities
            .contains(MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE)
        {
            return ParamEncoding::ByteWise;
        }
        if capabilities.contains(MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_C_CAST)
        {
            return ParamEncoding::Float;
        }
    }

    if autopilot == Some(MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA) {
        return ParamEncoding::Float;
    }

    // The protocol may still be supported without a capability bit. The byte-wise encoding
    // is what PX4 uses, and what Mission Planner assumes.
    ParamEncoding::ByteWise
}

/// Whether a vehicle advertises the deprecated `PARAM_FLOAT` capability.
///
/// The capability is deprecated in favour of `PARAM_ENCODE_C_CAST`, but vehicles in the field
/// still set it, and it means the same thing: the values are floats.
#[allow(deprecated)]
fn advertises_param_float(capabilities: &MavProtocolCapability) -> bool {
    capabilities.intersects(advertises_param_float_bits())
}

/// The bits of the deprecated `PARAM_FLOAT` capability.
#[allow(deprecated)]
fn advertises_param_float_bits() -> MavProtocolCapability {
    MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_FLOAT
}

/// A parameter and its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The parameter name, at most sixteen characters.
    pub name: String,
    /// The value.
    pub value: f64,
    /// The type the vehicle reported for the parameter.
    pub reported_type: ParamType,
}

impl Parameter {
    /// Creates a parameter.
    #[must_use]
    pub fn new(name: impl Into<String>, value: f64, reported_type: ParamType) -> Self {
        Self {
            name: name.into(),
            value,
            reported_type,
        }
    }
}

/// The parameters of a vehicle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterStore {
    parameters: BTreeMap<String, Parameter>,
    total_reported: u16,
}

impl ParameterStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a parameter.
    pub fn insert(&mut self, parameter: Parameter) {
        self.parameters.insert(parameter.name.clone(), parameter);
    }

    /// The parameter with the given name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Parameter> {
        self.parameters.get(name)
    }

    /// The parameter with the given name, mutably.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Parameter> {
        self.parameters.get_mut(name)
    }

    /// The number of parameters held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Whether the store holds no parameters.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// The parameters, ordered by name.
    pub fn iter(&self) -> impl Iterator<Item = &Parameter> {
        self.parameters.values()
    }

    /// The total number of parameters the vehicle reported.
    ///
    /// This is the count from the `param_count` field. It can exceed the number received when
    /// some are missing.
    #[must_use]
    pub fn total_reported(&self) -> u16 {
        self.total_reported
    }

    /// Records the total the vehicle reported.
    pub fn set_total_reported(&mut self, total: u16) {
        self.total_reported = total;
    }

    /// Whether every reported parameter has been received.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.total_reported != 0 && self.parameters.len() >= usize::from(self.total_reported)
    }

    /// The number of reported parameters that have not been received yet.
    #[must_use]
    pub fn missing_count(&self) -> usize {
        usize::from(self.total_reported).saturating_sub(self.parameters.len())
    }
}

/// Orders parameter names so that the `*_ENABLE` parameters are written last.
///
/// ArduPilot refuses to change some parameters while the feature they enable is active. Writing
/// the enable parameter first would therefore be rejected, or would leave the vehicle in a state
/// the operator did not ask for.
pub fn write_order(names: &mut [String]) {
    names.sort_by(|a, b| {
        let a_enable = a.ends_with("_ENABLE");
        let b_enable = b.ends_with("_ENABLE");
        // The enable parameters sort last: `false` before `true`.
        a_enable.cmp(&b_enable).then_with(|| a.cmp(b))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_integer_is_read_from_its_own_bytes() {
        // The bytes of the integer 1000.
        assert_eq!(ParamType::UInt16.decode_raw(1000u32), 1000.0);
        assert_eq!(ParamType::Int16.decode_raw(1000u32), 1000.0);
    }

    #[test]
    fn a_float_is_read_from_the_float_bits() {
        assert_eq!(ParamType::Real32.decode_raw(0.5f32.to_bits()), 0.5);
    }

    #[test]
    fn the_two_encodings_are_not_interchangeable() {
        // The bits of the integer 1000 read as a float are a denormal, not a number.
        let as_integer = ParamEncoding::ByteWise.decode(ParamType::UInt16, 1000);
        let as_float = ParamEncoding::Float.decode(ParamType::UInt16, 1000);

        assert_eq!(as_integer, 1000.0);
        assert!(as_float < 1e-40);
        assert_ne!(as_integer, as_float);
    }

    #[test]
    fn a_float_encoding_ignores_the_reported_type() {
        // ArduPilot reports the real type but sends a float.
        let bits = 1000.0f32.to_bits();
        assert_eq!(ParamEncoding::Float.decode(ParamType::UInt16, bits), 1000.0);
    }

    #[test]
    fn the_c_cast_encoding_sends_the_integer_as_a_float() {
        let bits = ParamEncoding::Float.encode(ParamType::UInt16, 1000.0);
        assert_eq!(bits, 1000.0f32.to_bits());
    }

    #[test]
    fn encoding_and_decoding_round_trip() {
        for (param_type, value) in [
            (ParamType::UInt8, 200.0),
            (ParamType::Int8, -100.0),
            (ParamType::UInt16, 60000.0),
            (ParamType::Int16, -30000.0),
            (ParamType::UInt32, 4_000_000_000.0),
            (ParamType::Int32, -2_000_000_000.0),
            (ParamType::Real32, 0.25),
        ] {
            let bits = ParamEncoding::ByteWise.encode(param_type, value);
            assert!(
                (ParamEncoding::ByteWise.decode(param_type, bits) - value).abs() < 0.001,
                "{param_type:?} did not round trip byte-wise"
            );

            let bits = ParamEncoding::Float.encode(param_type, value);
            assert!(
                (ParamEncoding::Float.decode(param_type, bits) - value).abs() < 0.001,
                "{param_type:?} did not round trip as a float"
            );
        }
    }

    #[test]
    fn the_mavlink_types_map_both_ways() {
        for param_type in [
            ParamType::UInt8,
            ParamType::Int8,
            ParamType::UInt16,
            ParamType::Int16,
            ParamType::UInt32,
            ParamType::Int32,
            ParamType::UInt64,
            ParamType::Int64,
            ParamType::Real32,
            ParamType::Real64,
        ] {
            let wire = param_type.as_mavlink();
            assert_eq!(ParamType::from_mavlink(wire), Some(param_type));
        }
    }

    #[test]
    fn ardupilot_sends_floats() {
        let encoding = resolve_encoding(None, Some(MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA));
        assert_eq!(encoding, ParamEncoding::Float);
    }

    #[test]
    fn the_param_float_capability_means_floats() {
        // The capability is deprecated, but vehicles in the field still set it.
        let capabilities = MavProtocolCapability::empty() | advertises_param_float_bits();
        let encoding = resolve_encoding(Some(capabilities), None);
        assert_eq!(encoding, ParamEncoding::Float);
    }

    #[test]
    fn the_bytewise_capability_means_byte_wise() {
        let capabilities = MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE;
        let encoding = resolve_encoding(Some(capabilities), None);
        assert_eq!(encoding, ParamEncoding::ByteWise);
    }

    #[test]
    fn the_c_cast_capability_means_floats() {
        let capabilities = MavProtocolCapability::MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_C_CAST;
        let encoding = resolve_encoding(Some(capabilities), None);
        assert_eq!(encoding, ParamEncoding::Float);
    }

    #[test]
    fn a_vehicle_without_capabilities_is_assumed_byte_wise() {
        // This is what PX4 does, and what Mission Planner assumes.
        let encoding = resolve_encoding(None, None);
        assert_eq!(encoding, ParamEncoding::ByteWise);
    }

    #[test]
    fn the_store_tracks_what_the_vehicle_reported() {
        let mut store = ParameterStore::new();
        assert!(store.is_empty());

        store.set_total_reported(3);
        store.insert(Parameter::new("ARMING_CHECK", 1.0, ParamType::Int8));
        store.insert(Parameter::new("RTL_ALT", 100.0, ParamType::Real32));

        assert_eq!(store.len(), 2);
        assert_eq!(store.total_reported(), 3);
        assert!(!store.is_complete());
        assert_eq!(store.missing_count(), 1);
    }

    #[test]
    fn the_enable_parameters_are_written_last() {
        let mut names = vec![
            "FS_BATT_ENABLE".to_owned(),
            "ARMING_CHECK".to_owned(),
            "FS_THR_ENABLE".to_owned(),
            "RTL_ALT".to_owned(),
        ];

        write_order(&mut names);

        assert_eq!(names[0], "ARMING_CHECK");
        assert_eq!(names[1], "RTL_ALT");
        assert_eq!(names[2], "FS_BATT_ENABLE");
        assert_eq!(names[3], "FS_THR_ENABLE");
    }
}
