//! Log parsing for MPCraft.
//!
//! Supports the formats ArduPilot and PX4 actually produce: MAVLink telemetry logs (`.tlog`),
//! DataFlash text logs (`.log`), DataFlash binary logs (`.bin`), and PX4 ULog (`.ulg`).
//!
//! Logs are large. Every parser streams; none loads a whole file into memory.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod binary;
pub mod dataflash;
pub mod tlog;
pub mod ulog;

pub use binary::{BinaryReader, BinarySample};
pub use dataflash::{DataFlashReader, MessageFormat, Sample, Value};
pub use tlog::{LogError, Record, TlogReader};
pub use ulog::{
    FieldUnit, ULog, ULogDamage, ULogDropout, ULogField, ULogFormat, ULogHeader, ULogInfo,
    ULogMessage, ULogParameter, ULogReader, ULogSample, ULogTopic, ULogType,
};
