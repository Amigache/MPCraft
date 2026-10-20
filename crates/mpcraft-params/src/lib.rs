//! Parameter metadata for MPCraft.
//!
//! Parameter metadata is what turns a raw `PARAM_VALUE` into a usable editor with the right
//! widget, validation and explanation. The source of truth is Mission Planner's
//! `ParameterMetaData.xml`, which is a domain specification rather than an implementation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
