//! What the keyring says about the keys the operator wrote.
//!
//! The secret is read here, in the layer that reads it, and nowhere else: a secret that is not
//! hexadecimal is not turned into bytes and not guessed at. It is left out, and the name of the key
//! that was left out is said, because a link signed with a key that is not the one the vehicle has
//! is a link that does not work at all — and an operator who wrote a key by hand has to be told
//! which line they wrote wrong.
//!
//! The file is read as it is written, so the keys keep the order the file gave them, and the first
//! one that can be read is the one the frames are signed with. That is not a preference: it is what
//! a file with several keys means when nothing in it says which one to use.

use serde::Serialize;

/// One key as the interface reads it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningKeyView {
    /// The name the key is listed under.
    pub name: String,
    /// The secret, in hexadecimal, as the operator wrote it.
    pub secret: String,
}

/// What the keyring says about a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SigningStatus {
    /// The keys, in the order the file wrote them.
    pub keys: Vec<SigningKeyView>,
    /// The names of the secrets that could not be read as thirty-two bytes.
    pub refused: Vec<String>,
    /// The name of the key the frames are signed with, when there is one.
    pub signed_with: Option<String>,
}

/// The keyring and what it says about the file it was read from.
#[derive(Debug, Clone)]
pub struct Keyring {
    /// The keys, in the order the file wrote them.
    pub keys: Vec<SigningKeyView>,
    /// The names of the secrets that could not be read.
    pub refused: Vec<String>,
    /// The name of the first key that could be read.
    pub signed_with: Option<String>,
    /// The key the outgoing frames are signed with.
    pub signing: Option<mpcraft_link::SigningKey>,
    /// The keys the incoming frames are verified against.
    pub ring: mpcraft_link::SigningKeyring,
}

impl Keyring {
    /// What the keyring says, without the runtime part of it.
    #[must_use]
    pub fn status(&self) -> SigningStatus {
        SigningStatus {
            keys: self.keys.clone(),
            refused: self.refused.clone(),
            signed_with: self.signed_with.clone(),
        }
    }

    /// The setup a session is opened with.
    #[must_use]
    pub fn setup(&self) -> mpcraft_link::SigningSetup {
        mpcraft_link::SigningSetup {
            key: self.signing.clone(),
            keyring: self.ring.clone(),
            link_id: 1,
        }
    }
}

/// The bytes of a hexadecimal text, when it is hexadecimal.
///
/// Thirty-two bytes are written as sixty-four characters, which is what the tool that makes keys
/// prints. A text of another length is not a key, and a character that is not a digit of the
/// hexadecimal numbers is not a key either; both are said rather than repaired.
#[must_use]
pub fn to_bytes(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();

    if text.len() != 64 {
        return None;
    }

    let mut bytes = [0_u8; 32];

    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = text.get(index * 2..index * 2 + 2)?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }

    Some(bytes)
}

/// Reads the keyring from what the operator wrote.
#[must_use]
pub fn read(settings: &mpcraft_settings::Settings) -> Keyring {
    let mut keyring = Keyring {
        keys: Vec::new(),
        refused: Vec::new(),
        signed_with: None,
        signing: None,
        ring: mpcraft_link::SigningKeyring::new(),
    };

    for entry in &settings.signing_keys {
        keyring.keys.push(SigningKeyView {
            name: entry.name.clone(),
            secret: entry.secret.clone(),
        });

        let Some(bytes) = to_bytes(&entry.secret) else {
            keyring.refused.push(entry.name.clone());
            continue;
        };

        let key = mpcraft_link::SigningKey::from_bytes(bytes);
        keyring.ring.insert(entry.name.clone(), key.clone());

        // The first key that can be read is the one the frames are signed with.
        if keyring.signing.is_none() {
            keyring.signed_with = Some(entry.name.clone());
            keyring.signing = Some(key);
        }
    }

    keyring
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings file with the keys the test names.
    fn file(keys: &[(&str, &str)]) -> mpcraft_settings::Settings {
        mpcraft_settings::Settings {
            connection_profiles: Vec::new(),
            favourite_parameters: Vec::new(),
            signing_keys: keys
                .iter()
                .map(|(name, secret)| mpcraft_settings::SigningKeyEntry::new(*name, *secret))
                .collect(),
            plugins: Vec::new(),
            map_injection_folder: None,
            swarm_leader: None,
            swarm_enabled: false,
            swarm_slots: Vec::new(),
            mirrors: Vec::new(),
            pois: Vec::new(),
        }
    }

    #[test]
    fn a_secret_that_is_not_hexadecimal_is_left_out_and_said() {
        // The hand: `zz` is not a hexadecimal pair, so the whole secret cannot be read as bytes.
        // It stays in the list because the file has to say what the operator wrote, and it is said
        // as the name of the line they have to fix.
        let readable = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        assert_eq!(
            readable.len(),
            64,
            "sixty-four characters are thirty-two bytes"
        );

        let status = read(&file(&[("good", readable), ("bad", "zz")])).status();

        assert_eq!(status.keys.len(), 2, "the file keeps both lines");
        assert_eq!(status.refused, vec!["bad".to_owned()]);
        assert_eq!(status.signed_with.as_deref(), Some("good"));
    }

    #[test]
    fn a_secret_of_another_length_is_not_a_key() {
        // Thirty-two bytes are sixty-four characters. Sixty-three is a key with one character
        // missing, and it is not read rather than padded — padding would be a different key than
        // the one the vehicle has.
        let short = "0".repeat(63);

        assert!(to_bytes(&short).is_none());
        assert_eq!(
            read(&file(&[("short", short.as_str())])).status().refused,
            vec!["short".to_owned()]
        );
    }

    #[test]
    fn the_bytes_are_the_pairs_of_the_text_in_the_order_they_are_written() {
        // The hand: each byte is the pair of characters it is written as, so the pairs at the front
        // of the test secret are `00` and `11` and `22`, the pair at index ten is `aa` and the one
        // at eleven is `bb`, and the last pair is `ff`. These were written down before the
        // implementation was called with them.
        let bytes = to_bytes("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
            .expect("the text is hexadecimal");

        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[1], 0x11);
        assert_eq!(bytes[2], 0x22);
        assert_eq!(bytes[10], 0xaa);
        assert_eq!(bytes[11], 0xbb);
        assert_eq!(bytes[31], 0xff);
    }

    #[test]
    fn the_first_key_that_can_be_read_is_the_one_that_signs() {
        // The file says nothing about which key to use, so the first readable one is used, and a
        // key written before it that cannot be read does not take its place.
        let unreadable = "not hexadecimal at all";
        // Sixty-four characters: twenty and twelve, twice.
        let readable = "aabbccddee00112233445566778899aabbccddee00112233445566778899aabb";
        assert_eq!(readable.len(), 64, "the hand counts before the code does");

        let status = read(&file(&[("first", unreadable), ("second", readable)])).status();

        assert_eq!(status.refused, vec!["first".to_owned()]);
        assert_eq!(status.signed_with.as_deref(), Some("second"));
    }

    #[test]
    fn a_file_with_no_keys_is_a_link_that_is_not_signed() {
        let status = read(&mpcraft_settings::Settings::default()).status();

        assert!(status.keys.is_empty());
        assert!(status.refused.is_empty());
        assert_eq!(status.signed_with, None);
    }
}
