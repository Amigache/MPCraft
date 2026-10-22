//! MAVLink 2 message signing.
//!
//! # Protocol
//!
//! A signed frame carries a 13-byte trailer:
//!
//! ```text
//! [link_id: 1][timestamp: 6][signature: 6]
//! ```
//!
//! The signature is the first 48 bits of
//! `SHA-256(secret_key || header_and_payload_and_crc || link_id || timestamp)`.
//!
//! # Timestamp
//!
//! The timestamp counts 100-microsecond intervals since 2015-01-01. It **must be strictly
//! increasing** for a given link id — a repeated timestamp produces a repeated signature, which
//! a replay detector will reject. MPCraft enforces that here rather than at the call site.

use sha2::{Digest, Sha256};

use crate::error::{ProtocolError, Result};

/// Length of a MAVLink 2 secret key, in bytes.
pub const SECRET_KEY_LEN: usize = 32;

/// A MAVLink 2 secret key.
#[derive(Clone, PartialEq, Eq)]
pub struct SigningKey {
    bytes: [u8; SECRET_KEY_LEN],
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key material, not even in a panic message.
        f.write_str("SigningKey(<redacted>)")
    }
}

impl SigningKey {
    /// Wraps 32 raw bytes as a signing key.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self { bytes }
    }

    /// Derives a signing key from a user seed, as the protocol specifies.
    ///
    /// The seed is hashed with SHA-256 and truncated to 32 bytes.
    #[must_use]
    pub fn from_seed(seed: &str) -> Self {
        let digest = Sha256::digest(seed.as_bytes());
        let mut bytes = [0u8; SECRET_KEY_LEN];
        bytes.copy_from_slice(&digest);
        Self { bytes }
    }

    /// The raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; SECRET_KEY_LEN] {
        &self.bytes
    }
}

/// The set of keys that may be used to verify incoming frames.
///
/// A peer may rotate keys, so several keys can be valid at once. Verification tries each in
/// turn and remembers which one matched.
#[derive(Debug, Default, Clone)]
pub struct SigningKeyring {
    keys: Vec<(String, SigningKey)>,
    last_match: Option<usize>,
}

impl SigningKeyring {
    /// Creates an empty keyring.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            last_match: None,
        }
    }

    /// Adds a key under a name, replacing any key with the same name.
    pub fn insert(&mut self, name: impl Into<String>, key: SigningKey) {
        let name = name.into();
        match self.keys.iter_mut().find(|(n, _)| *n == name) {
            Some(entry) => entry.1 = key,
            None => self.keys.push((name, key)),
        }
    }

    /// Removes a key by name, returning whether it was present.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.keys.len();
        self.keys.retain(|(n, _)| n != name);
        self.keys.len() != before
    }

    /// The number of keys in the ring.
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether the ring holds no keys.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The name of the key that last successfully verified a frame.
    #[must_use]
    pub fn last_match(&self) -> Option<&str> {
        self.last_match
            .and_then(|i| self.keys.get(i))
            .map(|(n, _)| n.as_str())
    }

    /// Computes the 48-bit signature of a frame.
    ///
    /// `frame_bytes` must be the frame **including** its CRC trailer but **excluding** the
    /// 13-byte signature block.
    #[must_use]
    pub fn sign(
        &self,
        key: &SigningKey,
        frame_bytes: &[u8],
        link_id: u8,
        timestamp: u64,
    ) -> [u8; 6] {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(frame_bytes);
        hasher.update([link_id]);
        hasher.update(timestamp.to_le_bytes());
        let digest = hasher.finalize();

        let mut signature = [0u8; 6];
        signature.copy_from_slice(&digest[..6]);
        signature
    }

    /// Verifies a signature against every key in the ring.
    ///
    /// On success the matching key's name is recorded and returned.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::MissingSigningKey`] when the ring is empty, and
    /// [`ProtocolError::SignatureInvalid`] when no key matches.
    pub fn verify(
        &mut self,
        frame_bytes: &[u8],
        link_id: u8,
        timestamp: u64,
        signature: &[u8; 6],
    ) -> Result<&str> {
        if self.keys.is_empty() {
            return Err(ProtocolError::MissingSigningKey);
        }

        for (index, (name, key)) in self.keys.iter().enumerate() {
            let candidate = self.sign(key, frame_bytes, link_id, timestamp);
            // Constant-time comparison: a timing side channel would leak the key byte by byte.
            if constant_time_eq(&candidate, signature) {
                self.last_match = Some(index);
                return Ok(name.as_str());
            }
        }

        Err(ProtocolError::SignatureInvalid)
    }
}

/// Compares two byte slices without short-circuiting on the first difference.
fn constant_time_eq(a: &[u8; 6], b: &[u8; 6]) -> bool {
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Produces a strictly increasing MAVLink 2 signature timestamp.
///
/// The protocol requires the timestamp to increase for a given link id; a repeated timestamp
/// yields a repeated signature that a replay detector rejects.
#[derive(Debug, Default)]
pub struct TimestampSource {
    last: Option<u64>,
}

impl TimestampSource {
    /// Creates a timestamp source.
    #[must_use]
    pub fn new() -> Self {
        Self { last: None }
    }

    /// Returns the next timestamp, guaranteed to be strictly greater than the previous one.
    ///
    /// `now` is the current time in 100-microsecond units since 2015-01-01.
    pub fn next(&mut self, now: u64) -> u64 {
        let candidate = match self.last {
            Some(last) if now <= last => last + 1,
            _ => now,
        };
        self.last = Some(candidate);
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_derivation_is_deterministic() {
        let a = SigningKey::from_seed("shared-secret");
        let b = SigningKey::from_seed("shared-secret");
        assert_eq!(a, b);
        assert_ne!(a, SigningKey::from_seed("other-secret"));
    }

    #[test]
    fn key_debug_output_does_not_leak_material() {
        let key = SigningKey::from_seed("secret");
        let rendered = format!("{key:?}");
        assert!(!rendered.contains("secret"));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn signature_round_trips_through_the_keyring() {
        let mut keyring = SigningKeyring::new();
        keyring.insert("station-1", SigningKey::from_seed("shared-secret"));

        let frame = b"\xFD\x00\x01\x00\x00\x01\x01\x00\x00\x00";
        let key = SigningKey::from_seed("shared-secret");
        let signature = keyring.sign(&key, frame, 0, 42);

        assert_eq!(
            keyring.verify(frame, 0, 42, &signature).unwrap(),
            "station-1"
        );
    }

    #[test]
    fn a_signed_frame_carries_its_signature_and_verifies() {
        use crate::frame::{encode_v2_signed, FrameParser, MavHeader, IFLAG_SIGNED};

        let mut keyring = SigningKeyring::new();
        let key = SigningKey::from_seed("shared-secret");
        keyring.insert("station-1", key.clone());

        // A heartbeat is a message every vehicle understands.
        let message =
            crate::dialect::MavMessage::HEARTBEAT(crate::dialect::messages::HEARTBEAT_DATA {
                custom_mode: 5,
                mavtype: crate::dialect::MavType::MAV_TYPE_QUADROTOR,
                autopilot: crate::dialect::MavAutopilot::MAV_AUTOPILOT_ARDUPILOTMEGA,
                ..Default::default()
            });

        let frame = encode_v2_signed(
            &message,
            MavHeader {
                system_id: 255,
                component_id: 190,
                sequence: 7,
            },
            &keyring,
            &key,
            3,
            1234,
        );

        // The frame says that it is signed, and it carries the block of the signature.
        assert_eq!(frame[2] & IFLAG_SIGNED, IFLAG_SIGNED);
        assert_eq!(frame.len(), frame.len());

        // The frame that comes back verifies with the key it was signed with.
        let mut parser = FrameParser::new();
        let mut frames = Vec::new();
        if let Err(error) = parser.feed(&frame, &mut frames) {
            panic!("the frame is read: {error} ? {} bytes", frame.len());
        }

        let parsed = frames.first().expect("the frame is there");
        let signature = parsed.signature.expect("the frame is signed");

        assert_eq!(signature.link_id, 3);
        assert_eq!(signature.timestamp, 1234);

        let signed = &parsed.raw[..parsed.raw.len() - crate::frame::SIGNATURE_LEN];
        assert_eq!(
            keyring
                .verify(
                    signed,
                    signature.link_id,
                    signature.timestamp,
                    &signature.value
                )
                .expect("the frame was signed with the key"),
            "station-1"
        );
    }

    #[test]
    fn wrong_signature_is_rejected() {
        let mut keyring = SigningKeyring::new();
        keyring.insert("station-1", SigningKey::from_seed("shared-secret"));

        let frame = b"\xFD\x00\x01\x00\x00\x01\x01\x00\x00\x00";
        let key = SigningKey::from_seed("different-secret");
        let signature = keyring.sign(&key, frame, 0, 42);

        assert!(matches!(
            keyring.verify(frame, 0, 42, &signature),
            Err(ProtocolError::SignatureInvalid)
        ));
    }

    #[test]
    fn empty_keyring_reports_missing_key() {
        let mut keyring = SigningKeyring::new();
        let signature = [0u8; 6];
        assert!(matches!(
            keyring.verify(b"frame", 0, 0, &signature),
            Err(ProtocolError::MissingSigningKey)
        ));
    }

    #[test]
    fn timestamps_are_strictly_increasing() {
        let mut source = TimestampSource::new();
        assert_eq!(source.next(100), 100);
        assert_eq!(source.next(100), 101);
        assert_eq!(source.next(99), 102);
        assert_eq!(source.next(200), 200);
    }
}
