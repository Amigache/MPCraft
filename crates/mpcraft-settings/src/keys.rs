//! The keys of the signed link.
//!
//! A link that is signed needs a secret key on each side. The key is given by the
//! operator, and the operator writes it in the settings file, where it can be read and
//! changed by hand.

use serde::{Deserialize, Serialize};

/// One key of the link.
///
/// The secret is the hexadecimal text of the thirty-two bytes of the key, which is what
/// the tool that makes keys prints. A secret that is not hexadecimal is refused when
/// the link is opened, and not before: a file that is being written by hand should say
/// what it wants to say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct SigningKeyEntry {
    /// The name the key is listed under.
    pub name: String,

    /// The secret, in hexadecimal.
    pub secret: String,
}

impl SigningKeyEntry {
    /// A key with the given name and secret.
    #[must_use]
    pub fn new(name: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            secret: secret.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_read_from_the_settings() {
        let text = r#"
            name = "station-1"
            secret = "00112233445566778899aabbccddeeff"
        "#;

        let key: SigningKeyEntry = toml::from_str(text).expect("the key is read");

        assert_eq!(key.name, "station-1");
        assert_eq!(key.secret, "00112233445566778899aabbccddeeff");
    }
}
