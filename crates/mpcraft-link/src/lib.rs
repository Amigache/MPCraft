//! Session layer for MPCraft.
//!
//! # The concurrency model
//!
//! Mission Planner has **two** consumers competing for the same socket: a background
//! `SerialReader` loop and every blocking request/response transaction. They are arbitrated by
//! a cooperative boolean flag (`giveComport`) and a semaphore. A transaction that forgets the
//! flag deadlocks the entire link.
//!
//! MPCraft replaces this with an actor model:
//!
//! * **One reader task per session** owns the byte stream, parses frames, and publishes them on
//!   a `tokio::broadcast` channel.
//! * **Transactions** register a matcher and await a dedicated `oneshot` responder. They must
//!   not rely on broadcast buffering, which is lossy under load.
//! * **Telemetry aggregation** runs in its own task and publishes an `ArcSwap` snapshot.
//!
//! Concurrent transactions are therefore safe by construction, and a slow transaction cannot
//! starve telemetry.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
