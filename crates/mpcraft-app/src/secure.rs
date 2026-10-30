//! The firmware that is signed, for a board whose bootloader checks the signature.
//!
//! A board with secure boot does not run whatever is written to it: the image carries a signature,
//! the bootloader holds the public keys it trusts, and an image that is not signed by one of them is
//! not started. The signature is Ed25519.
//!
//! The layout is the one the published signer uses, read from its source rather than derived here:
//!
//! - The image inside the `.apj` carries a descriptor of eight bytes, `41 a3 e5 f2 65 69 92 07`, and
//!   the 92 bytes that begin after it are the block the signature lives in. Everything else — the
//!   bytes before the block and the bytes after it — is what is signed. The block itself is excluded,
//!   which is what lets the same image be signed again without breaking the signature.
//! - Inside the block the signature is written at 16 bytes past its start: a four-byte length of 72
//!   (the signature and its own header), an eight-byte version, 30437, and the 64 bytes of the
//!   signature, all in the byte order the board reads.
//! - The document that carries the image says `signed_firmware`, the image again compressed, its
//!   size, the room left in the flash and the SHA-512 of what was signed, in base64.
//!
//! A bootloader is not signed: it carries the keys the board trusts. The three keys every ArduPilot
//! bootloader holds are written first, and the operator's own key after them, at the same place the
//! descriptor marks.
//!
//! The private key is the operator's. It is read from the form the published signer writes —
//! `PRIVATE_KEYV1:` and the seed in base64 — or from a PKCS#8 PEM, and it is never written anywhere
//! this module chooses.

use std::path::Path;

use ed25519_dalek::pkcs8::DecodePrivateKey as _;
use ed25519_dalek::{Signer as _, SigningKey as DalekKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha512};

use crate::apj::{base64_bytes, base64_text, unpack};
use crate::error::{CommandError, CommandResult};

/// The bytes that mark the descriptor of an image that carries a signature.
const APJ_DESCRIPTOR: [u8; 8] = [0x41, 0xa3, 0xe5, 0xf2, 0x65, 0x69, 0x92, 0x07];

/// The bytes that mark the descriptor of a bootloader, whose block holds the keys.
const BOOTLOADER_DESCRIPTOR: [u8; 8] = [0x4e, 0xcf, 0x4e, 0xa5, 0xa6, 0xb6, 0xf7, 0x29];

/// The version the signature block says it is.
const SIGNATURE_VERSION: u64 = 30_437;

/// The size of the block the signature lives in, counted from the byte after the descriptor.
const SIGNATURE_BLOCK: usize = 92;

/// Where the length of the signature starts inside the block.
const BLOCK_LENGTH_AT: usize = 16;

/// The length of a public key, a private seed and a signature.
const KEY_LENGTH: usize = 32;
const SIGNATURE_LENGTH: usize = 64;

/// The three public keys every ArduPilot bootloader carries, as the published signer writes them.
const AP_PUBLIC_KEYS: [&str; 3] = [
    "WJbbpbjOz/yMB3JxnvqyTUInCQdZcStkA0qhn2ldhPI=",
    "X8jdVqxIIUmCuMSi8IhTZ40VkXW0gbRczzMtdSghqCI=",
    "snNHkX96F9A+/ISppHZrc1jjPo3jMNN+g2PToKhWSgA=",
];

/// The kind of file that is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirmwareKind {
    /// The document the manifest points at, whose image carries the signature.
    Apj,
    /// The bootloader, which carries the keys rather than a signature.
    Bootloader,
}

/// What was written when a firmware was signed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedFirmware {
    /// The file the signed form was written to.
    pub output: String,
    /// How big the file is, in bytes.
    pub bytes: u64,
    /// The SHA-512 of what was signed, in base64. Empty for a bootloader, which is not signed.
    pub sha: String,
    /// The public key the board has to trust, in base64.
    pub public_key: String,
}

/// What was written when a key was generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedKey {
    /// The file the private key was written to.
    pub private_key_path: String,
    /// The file the public key was written to, beside it.
    pub public_key_path: String,
    /// The public key itself, in base64, which is what a bootloader is told to trust.
    pub public_key: String,
}

/// An Ed25519 key, and the forms the published signer writes it in.
#[derive(Clone)]
pub struct SigningKey {
    key: DalekKey,
}

impl std::fmt::Debug for SigningKey {
    /// Says which key it is without saying the secret: the public half is enough to tell two keys
    /// apart, and a private key in a log is a private key that leaked.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SigningKey")
            .field("public", &self.public_key_base64())
            .finish()
    }
}

impl SigningKey {
    /// The key a 32-byte seed names.
    #[must_use]
    pub fn from_seed(seed: [u8; KEY_LENGTH]) -> Self {
        Self {
            key: DalekKey::from_bytes(&seed),
        }
    }

    /// A new key, from the randomness of the machine.
    ///
    /// # Errors
    ///
    /// Returns an error when the machine cannot give randomness, which is a fault of the machine
    /// rather than of the request.
    pub fn generate() -> CommandResult<Self> {
        let mut seed = [0_u8; KEY_LENGTH];

        getrandom::getrandom(&mut seed).map_err(|error| {
            CommandError::internal(format!("the machine gave no randomness: {error}"))
        })?;

        Ok(Self::from_seed(seed))
    }

    /// Reads a key from the text of the file the operator wrote.
    ///
    /// Two forms are read: the one the published signer writes beside a PEM, `PRIVATE_KEYV1:` and
    /// the seed in base64, and a PKCS#8 PEM. A key that is neither is refused before anything is
    /// signed, because a signature made with the wrong key is a board that will not start.
    ///
    /// # Errors
    ///
    /// Returns an error when the text is not one of the two forms.
    pub fn read(text: &str) -> CommandResult<Self> {
        let trimmed = text.trim();

        if trimmed.contains("-----BEGIN PRIVATE KEY-----") {
            return DalekKey::from_pkcs8_pem(trimmed)
                .map(|key| Self { key })
                .map_err(|error| {
                    CommandError::invalid_request(format!(
                        "the private key could not be read: {error}"
                    ))
                });
        }

        let encoded = trimmed
            .strip_prefix("PRIVATE_KEYV1:")
            .ok_or_else(|| {
                CommandError::invalid_request(
                    "the key is neither PRIVATE_KEYV1 nor a PEM private key".to_owned(),
                )
            })?
            .trim();

        let seed = base64_bytes(encoded).map_err(|error| {
            CommandError::invalid_request(format!("the private key is not base64: {error}"))
        })?;

        let seed: [u8; KEY_LENGTH] = seed.try_into().map_err(|_| {
            CommandError::invalid_request(format!("the private key is not {KEY_LENGTH} bytes"))
        })?;

        Ok(Self::from_seed(seed))
    }

    /// The public key, which is what a bootloader is told to trust.
    #[must_use]
    pub fn public_key(&self) -> [u8; KEY_LENGTH] {
        self.key.verifying_key().to_bytes()
    }

    /// The public key in base64, the way the published signer writes it.
    #[must_use]
    pub fn public_key_base64(&self) -> String {
        base64_text(&self.public_key())
    }

    /// The private key in the form the published signer writes beside a PEM.
    #[must_use]
    pub fn private_key_dat(&self) -> String {
        format!("PRIVATE_KEYV1:{}", base64_text(&self.key.to_bytes()))
    }

    /// The public key in the form the published signer writes beside a PEM.
    #[must_use]
    pub fn public_key_dat(&self) -> String {
        format!("PUBLIC_KEYV1:{}", self.public_key_base64())
    }

    fn sign(&self, message: &[u8]) -> [u8; SIGNATURE_LENGTH] {
        self.key.sign(message).to_bytes()
    }
}

/// The signed form of a document, and what was signed.
#[derive(Debug)]
pub struct SignedDocument {
    /// The document, ready to be written.
    pub document: Vec<u8>,
    /// The SHA-512 of what was signed.
    pub sha: [u8; 64],
}

/// Signs the image of an `.apj` and hands back the document that carries it.
///
/// The signature is made over the image without the block it lives in, which is what the published
/// signer does; the block is then written with the length, the version and the signature, and the
/// document is filled again with the image, its size, the room left in the flash and the SHA-512 of
/// what was signed.
///
/// # Errors
///
/// Returns an error when the document cannot be read, when the image inside it is not the length
/// the document says, when the descriptor the signer looks for is not there, or when the block
/// would run past the image.
pub fn sign_apj(document: &[u8], key: &SigningKey) -> CommandResult<SignedDocument> {
    let mut value: serde_json::Value = serde_json::from_slice(document).map_err(|error| {
        CommandError::invalid_request(format!(
            "the file is not the document of the firmware: {error}"
        ))
    })?;

    let declared = value
        .get("image_size")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            CommandError::invalid_request("the document says no image size".to_owned())
        })? as usize;

    let packed = value
        .get("image")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CommandError::invalid_request("the document carries no image".to_owned()))?;

    let packed = base64_bytes(packed).map_err(|error| {
        CommandError::invalid_request(format!("the image is not base64: {error}"))
    })?;

    let mut image = unpack(&packed).map_err(|error| {
        CommandError::invalid_request(format!("the image could not be unpacked: {error}"))
    })?;

    if image.len() != declared {
        return Err(CommandError::invalid_request(format!(
            "the document says {declared} bytes and the image inside it holds {}",
            image.len()
        )));
    }

    let at = find(&image, &APJ_DESCRIPTOR).ok_or_else(|| {
        CommandError::invalid_request(
            "the image carries no descriptor to sign, so it is not one this signer reads"
                .to_owned(),
        )
    })?;

    // The block begins after the descriptor, and it is the block that is left out of the message.
    let block = at + APJ_DESCRIPTOR.len();

    if image.len() < block + SIGNATURE_BLOCK {
        return Err(CommandError::invalid_request(
            "the image ends before the block the signature lives in".to_owned(),
        ));
    }

    let message = signed_region(&image, block);
    let signature = key.sign(&message);
    let sha = Sha512::digest(&message);

    let length_at = block + BLOCK_LENGTH_AT;
    image[length_at..length_at + 4].copy_from_slice(&((SIGNATURE_LENGTH + 8) as u32).to_le_bytes());
    image[length_at + 4..length_at + 12].copy_from_slice(&SIGNATURE_VERSION.to_le_bytes());
    image[length_at + 12..length_at + 12 + SIGNATURE_LENGTH].copy_from_slice(&signature);

    value["image"] = serde_json::Value::String(base64_text(&pack(&image)?));
    value["image_size"] = serde_json::Value::from(image.len());
    value["signed_firmware"] = serde_json::Value::from(true);
    value["sha"] = serde_json::Value::String(base64_text(&sha));

    if let (Some(total), Some(size)) = (
        value.get("flash_total").and_then(serde_json::Value::as_u64),
        value.get("image_size").and_then(serde_json::Value::as_u64),
    ) {
        value["flash_free"] = serde_json::Value::from(total.saturating_sub(size));
    }

    let document = serde_json::to_vec_pretty(&value).map_err(|error| {
        CommandError::internal(format!("the signed document could not be written: {error}"))
    })?;

    Ok(SignedDocument {
        document,
        sha: sha.into(),
    })
}

/// What a check of a signed document found.
///
/// A signature that does not agree is not an error of the reading: the document was read, and what it
/// says does not hold. The two are kept apart so the interface can say which one happened — a file
/// that cannot be read is a different fact from a file whose signature is wrong.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedFirmware {
    /// The number of bytes the image holds, as the document says and as it is.
    pub image_size: usize,
    /// Whether the signature the image carries was made by the key it was checked against.
    pub signature_ok: bool,
    /// Whether the SHA-512 of what was signed is the one the document carries.
    ///
    /// `None` when the document carries no number to compare with, which is not the same as a number
    /// that does not match.
    pub sha_ok: Option<bool>,
    /// The SHA-512 of what was signed, in hexadecimal, as this crate computed it.
    pub sha: String,
}

/// Checks the signature of a signed `.apj` against a public key.
///
/// The image is unpacked from the document, the block is found the way the signer finds it, and what
/// the block says is checked: its length, the version the signature is written with, and the
/// signature itself over everything outside the block. The SHA-512 the document carries is compared
/// with the one computed here, so a number that was edited after signing is a second thing that does
/// not hold rather than being taken on trust.
///
/// # Errors
///
/// Returns an error when the document cannot be read, when the image is not the length the document
/// says, when there is no descriptor, or when the block is not the shape this format gives it.
pub fn verify_apj(
    document: &[u8],
    public_key: &[u8; KEY_LENGTH],
) -> CommandResult<VerifiedFirmware> {
    use ed25519_dalek::{Signature, VerifyingKey};

    let value: serde_json::Value = serde_json::from_slice(document).map_err(|error| {
        CommandError::invalid_request(format!(
            "the file is not the document of the firmware: {error}"
        ))
    })?;

    let declared = value
        .get("image_size")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            CommandError::invalid_request("the document says no image size".to_owned())
        })? as usize;

    let packed = value
        .get("image")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| CommandError::invalid_request("the document carries no image".to_owned()))?;

    let packed = base64_bytes(packed).map_err(|error| {
        CommandError::invalid_request(format!("the image is not base64: {error}"))
    })?;

    let image = unpack(&packed).map_err(|error| {
        CommandError::invalid_request(format!("the image could not be unpacked: {error}"))
    })?;

    if image.len() != declared {
        return Err(CommandError::invalid_request(format!(
            "the document says {declared} bytes and the image inside it holds {}",
            image.len()
        )));
    }

    let at = find(&image, &APJ_DESCRIPTOR).ok_or_else(|| {
        CommandError::invalid_request(
            "the image carries no descriptor, so it is not one this reader knows".to_owned(),
        )
    })?;

    let block = at + APJ_DESCRIPTOR.len();

    if image.len() < block + SIGNATURE_BLOCK {
        return Err(CommandError::invalid_request(
            "the image ends before the block the signature lives in".to_owned(),
        ));
    }

    let length_at = block + BLOCK_LENGTH_AT;
    let length = u32::from_le_bytes(
        image[length_at..length_at + 4]
            .try_into()
            .map_err(|_| CommandError::internal("the length of the block could not be read"))?,
    ) as usize;
    let version = u64::from_le_bytes(
        image[length_at + 4..length_at + 12]
            .try_into()
            .map_err(|_| CommandError::internal("the version of the block could not be read"))?,
    );

    if length != SIGNATURE_LENGTH + 8 || version != SIGNATURE_VERSION {
        return Err(CommandError::invalid_request(format!(
            "the signature block is not one this format gives: it says {length} bytes of version {version}"
        )));
    }

    let signature_at = length_at + 12;
    let signature: [u8; SIGNATURE_LENGTH] = image[signature_at..signature_at + SIGNATURE_LENGTH]
        .try_into()
        .map_err(|_| CommandError::internal("the signature could not be read"))?;

    let message = signed_region(&image, block);
    let sha = Sha512::digest(&message);

    let signature_ok = VerifyingKey::from_bytes(public_key)
        .map(|key| {
            key.verify_strict(&message, &Signature::from_bytes(&signature))
                .is_ok()
        })
        .unwrap_or(false);

    let sha_ok = value
        .get("sha")
        .and_then(serde_json::Value::as_str)
        .and_then(|said| base64_bytes(said).ok())
        .map(|said| said == sha.as_slice());

    Ok(VerifiedFirmware {
        image_size: image.len(),
        signature_ok,
        sha_ok,
        sha: sha.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}

/// Reads a public key from the form the published signer writes beside a key.
///
/// # Errors
///
/// Returns an error when the text is not that form, or when the key inside it is not thirty-two
/// bytes.
pub fn read_public_key(text: &str) -> CommandResult<[u8; KEY_LENGTH]> {
    let text = text.trim();
    let packed = text
        .strip_prefix("PUBLIC_KEYV1:")
        .ok_or_else(|| {
            CommandError::invalid_request(
                "a public key is written as PUBLIC_KEYV1: and the key in base64".to_owned(),
            )
        })?
        .trim();

    let bytes = base64_bytes(packed).map_err(|error| {
        CommandError::invalid_request(format!("the public key is not base64: {error}"))
    })?;

    bytes.try_into().map_err(|_| {
        CommandError::invalid_request(format!(
            "a public key is {KEY_LENGTH} bytes and this one is {}",
            packed.len()
        ))
    })
}

/// Writes the keys a bootloader trusts into its block.
///
/// The three keys every ArduPilot bootloader carries are written first, and the operator's own key
/// after them. Nothing is signed: a bootloader is what checks signatures, so what it needs is the
/// list of keys to check with.
///
/// # Errors
///
/// Returns an error when the descriptor is not there, or when the block would run past the file.
pub fn sign_bootloader(bin: &[u8], key: &SigningKey) -> CommandResult<Vec<u8>> {
    let at = find(bin, &BOOTLOADER_DESCRIPTOR).ok_or_else(|| {
        CommandError::invalid_request(
            "the file carries no descriptor of a bootloader, so it is not one this signer reads"
                .to_owned(),
        )
    })?;

    let block = at + BOOTLOADER_DESCRIPTOR.len();
    let needed = (AP_PUBLIC_KEYS.len() + 1) * KEY_LENGTH;

    if bin.len() < block + needed {
        return Err(CommandError::invalid_request(
            "the bootloader ends before the block the keys live in".to_owned(),
        ));
    }

    let mut out = bin.to_vec();
    let mut cursor = block;

    for encoded in AP_PUBLIC_KEYS {
        let bytes = base64_bytes(encoded).map_err(|error| {
            CommandError::internal(format!("a published key is not base64: {error}"))
        })?;
        out[cursor..cursor + KEY_LENGTH].copy_from_slice(&bytes);
        cursor += KEY_LENGTH;
    }

    out[cursor..cursor + KEY_LENGTH].copy_from_slice(&key.public_key());

    Ok(out)
}

/// Signs a file and writes the signed form to another, reading the key from a file.
///
/// # Errors
///
/// Returns an error when the key, the input or the output cannot be read or written, and the errors
/// of the two signers above.
pub fn sign_file(
    kind: FirmwareKind,
    input: &Path,
    output: &Path,
    key_file: &Path,
) -> CommandResult<SignedFirmware> {
    let key = SigningKey::read(&read_text(key_file, "the private key")?)?;
    let bytes = std::fs::read(input).map_err(|error| {
        CommandError::internal(format!(
            "the file {} could not be read: {error}",
            input.display()
        ))
    })?;

    let (written, sha) = match kind {
        FirmwareKind::Apj => {
            let signed = sign_apj(&bytes, &key)?;
            (signed.document, base64_text(&signed.sha))
        }
        FirmwareKind::Bootloader => (sign_bootloader(&bytes, &key)?, String::new()),
    };

    std::fs::write(output, &written).map_err(|error| {
        CommandError::internal(format!(
            "the file {} could not be written: {error}",
            output.display()
        ))
    })?;

    Ok(SignedFirmware {
        output: output.display().to_string(),
        bytes: written.len() as u64,
        sha,
        public_key: key.public_key_base64(),
    })
}

/// Generates a key and writes it where the operator asked, with the public key beside it.
///
/// The private key is written in the form the published signer writes, so the same file can be read
/// by both. Nothing here writes a PEM: that form is read, not produced.
///
/// # Errors
///
/// Returns an error when the machine has no randomness, or when either file cannot be written.
pub fn generate_key_files(private_key_path: &Path) -> CommandResult<GeneratedKey> {
    let key = SigningKey::generate()?;

    let stem = private_key_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("key");
    let public_key_path = private_key_path.with_file_name(format!("{stem}_public_key.dat"));

    std::fs::write(private_key_path, format!("{}\n", key.private_key_dat())).map_err(|error| {
        CommandError::internal(format!(
            "the private key could not be written to {}: {error}",
            private_key_path.display()
        ))
    })?;

    std::fs::write(&public_key_path, format!("{}\n", key.public_key_dat())).map_err(|error| {
        CommandError::internal(format!(
            "the public key could not be written to {}: {error}",
            public_key_path.display()
        ))
    })?;

    Ok(GeneratedKey {
        private_key_path: private_key_path.display().to_string(),
        public_key_path: public_key_path.display().to_string(),
        public_key: key.public_key_base64(),
    })
}

/// Everything that is signed: the bytes before the block, and the bytes after it.
fn signed_region(image: &[u8], block: usize) -> Vec<u8> {
    let mut message = Vec::with_capacity(image.len() - SIGNATURE_BLOCK);

    message.extend_from_slice(&image[..block]);
    message.extend_from_slice(&image[block + SIGNATURE_BLOCK..]);

    message
}

/// Where the descriptor is in a file, if it is there.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Packs the image the way the document carries it.
fn pack(bytes: &[u8]) -> CommandResult<Vec<u8>> {
    use std::io::Write as _;

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());

    encoder.write_all(bytes).map_err(|error| {
        CommandError::internal(format!("the image could not be packed: {error}"))
    })?;

    encoder
        .finish()
        .map_err(|error| CommandError::internal(format!("the image could not be packed: {error}")))
}

/// Reads a text file, naming what it was for when it cannot be read.
fn read_text(path: &Path, what: &str) -> CommandResult<String> {
    std::fs::read_to_string(path).map_err(|error| {
        CommandError::internal(format!(
            "{what} at {} could not be read: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::pkcs8::EncodePrivateKey as _;
    use ed25519_dalek::Verifier as _;

    /// The first seed and public key of the Ed25519 specification, and the signature it gives for
    /// an empty message. The values are the ones RFC 8032 writes down.
    const RFC_SEED: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];

    const RFC_PUBLIC: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";

    const RFC_SIGNATURE: &str = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

    /// The image the two signers below were given, written down here so the golden numbers are
    /// over bytes that were not built by the signer. The descriptor sits at offset 16, the signed
    /// region is the 16 bytes before it and the 23 bytes after the 92-byte block.
    fn fixture_image() -> Vec<u8> {
        let mut image = Vec::new();

        image.extend((0_u8..16).collect::<Vec<u8>>());
        image.extend(APJ_DESCRIPTOR);
        image.extend([0_u8; 84]);
        image.extend(b"MPCraft secure firmware payload");

        image
    }

    fn fixture_key() -> SigningKey {
        SigningKey::from_seed(RFC_SEED)
    }

    #[test]
    fn the_key_is_the_one_the_specification_writes_down() {
        // The seed, the public key and the signature of an empty message are the values RFC 8032
        // publishes, so the interpretation of the seed — hashed and clamped rather than used as the
        // scalar — is checked by something that is not this crate.
        let key = SigningKey::from_seed(RFC_SEED);

        assert_eq!(hex(&key.public_key()), RFC_PUBLIC);
        assert_eq!(hex(&key.sign(b"")), RFC_SIGNATURE);
    }

    #[test]
    fn a_document_the_signer_made_is_the_one_the_reader_accepts() {
        let image = fixture_image();
        let key = fixture_key();

        let document = format!(
            r#"{{"board_id": 9, "image_size": {}, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let signed = sign_apj(document.as_bytes(), &key).expect("the document is signed");
        let checked =
            verify_apj(&signed.document, &key.public_key()).expect("the document is read");

        assert!(
            checked.signature_ok,
            "the signature is the one this key made"
        );
        assert_eq!(
            checked.sha_ok,
            Some(true),
            "the number the document carries is the one computed"
        );
        assert_eq!(checked.image_size, image.len());
        assert_eq!(
            checked.sha,
            signed
                .sha
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
    }

    #[test]
    fn a_signature_made_by_another_key_is_not_accepted() {
        let image = fixture_image();
        let document = format!(
            r#"{{"image_size": {}, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let signed = sign_apj(document.as_bytes(), &fixture_key()).expect("the document is signed");

        let other = SigningKey::from_seed([7_u8; 32]);
        let checked =
            verify_apj(&signed.document, &other.public_key()).expect("the document is read");

        assert!(!checked.signature_ok, "another key did not sign this");
    }

    #[test]
    fn an_image_changed_after_signing_is_not_accepted() {
        let image = fixture_image();
        let document = format!(
            r#"{{"image_size": {}, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let key = fixture_key();
        let signed = sign_apj(document.as_bytes(), &key).expect("the document is signed");

        // One byte of what was signed is changed, and the document is written again with the image
        // as it now is: the signature no longer covers the bytes it says it does.
        let mut value: serde_json::Value =
            serde_json::from_slice(&signed.document).expect("the signed form is a document");
        let mut changed = unpack(
            &base64_bytes(value["image"].as_str().expect("the image is a string"))
                .expect("the image is base64"),
        )
        .expect("the image is unpacked");

        let last = changed.len() - 1;
        changed[last] ^= 0xFF;
        let repacked = pack(&changed).expect("the image is packed again");
        value["image"] = serde_json::Value::String(base64_text(&repacked));

        let rewritten = serde_json::to_vec(&value).expect("the document is written");
        let checked = verify_apj(&rewritten, &key.public_key()).expect("the document is read");

        assert!(
            !checked.signature_ok,
            "the bytes are not the ones that were signed"
        );
    }

    #[test]
    fn a_number_edited_after_signing_is_a_second_thing_that_does_not_hold() {
        let image = fixture_image();
        let document = format!(
            r#"{{"image_size": {}, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let key = fixture_key();
        let signed = sign_apj(document.as_bytes(), &key).expect("the document is signed");

        let mut value: serde_json::Value =
            serde_json::from_slice(&signed.document).expect("the signed form is a document");
        value["sha"] = serde_json::Value::String(base64_text(&[0_u8; 64]));

        let rewritten = serde_json::to_vec(&value).expect("the document is written");
        let checked = verify_apj(&rewritten, &key.public_key()).expect("the document is read");

        // The signature still holds — it is over the image — and the number does not.
        assert!(checked.signature_ok);
        assert_eq!(checked.sha_ok, Some(false));
    }

    #[test]
    fn a_public_key_is_read_from_the_form_the_signer_writes() {
        let key = fixture_key();
        let read = read_public_key(&key.public_key_dat()).expect("the key is read");

        assert_eq!(read, key.public_key());
        assert_eq!(hex(&read), RFC_PUBLIC);

        assert!(read_public_key("not a key").is_err());
        assert!(read_public_key("PUBLIC_KEYV1:AAAA").is_err());
    }

    #[test]
    fn the_two_forms_the_published_signer_writes_are_read() {
        let key = fixture_key();
        let dat = key.private_key_dat();

        assert!(dat.starts_with("PRIVATE_KEYV1:"));
        assert_eq!(
            SigningKey::read(&dat)
                .expect("the form is read")
                .public_key(),
            key.public_key()
        );

        // The PEM form is the other half of what the published signer writes. The bytes are the
        // PKCS#8 encoding of the key, wrapped the way the format wraps them; the parser that reads
        // them back is not this module, so a key that survives the round trip is a fact about its
        // reader rather than the seed being copied through.
        let der = key
            .key
            .to_pkcs8_der()
            .expect("the key is written as PKCS#8");
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            wrap(&base64_text(der.as_bytes()), 64)
        );

        assert_eq!(
            SigningKey::read(&pem)
                .expect("the PEM is read")
                .public_key(),
            key.public_key()
        );
    }

    #[test]
    fn a_text_that_is_not_a_key_is_refused_before_anything_is_signed() {
        let error = SigningKey::read("not a key").expect_err("the text is not a key");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);

        let error = SigningKey::read("PRIVATE_KEYV1:AAAA").expect_err("the seed is not 32 bytes");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn the_signature_is_made_over_the_image_without_its_block() {
        // The numbers below were written by a second signer — the Ed25519 of the platform's own
        // cryptography, over the same bytes built here — so what they assert is the layout this
        // module reads: which bytes are signed, and where the block and its numbers go.
        let image = fixture_image();
        let key = fixture_key();
        let message = {
            let mut message = image[..24].to_vec();
            message.extend_from_slice(&image[24 + 92..]);
            message
        };

        assert_eq!(hex(&key.sign(&message)), "5a829f48bf949e1f6b44f990a24b6d91c0ca5e4e39062b0da8f7e98155cfb52885cc65fb153a8412a015a020f1384500722cc1ed717013ca8a1073a04a49100e");

        let sha = Sha512::digest(&message);

        assert_eq!(hex(&sha), "8e74e8f7abeb4f23dd8d0e53b094f46dadb830b110b35652330492bac1f4e5ca65e1be89a6a9c84e409e7132ee14915dfef8148fc9e38fba6cc6aef82a561c95");
    }

    #[test]
    fn a_signed_document_carries_the_signature_and_says_it_is_signed() {
        let image = fixture_image();
        let key = fixture_key();

        let document = format!(
            r#"{{"board_id": 9, "image_size": {}, "flash_total": 2048, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let written = sign_apj(document.as_bytes(), &key).expect("the document is signed");
        let value: serde_json::Value =
            serde_json::from_slice(&written.document).expect("the signed form is a document");

        assert_eq!(value["signed_firmware"], serde_json::Value::from(true));
        assert_eq!(value["image_size"], serde_json::Value::from(image.len()));
        assert_eq!(
            value["flash_free"],
            serde_json::Value::from(2048 - image.len())
        );
        assert!(value["sha"].as_str().is_some_and(|sha| !sha.is_empty()));

        let packed = base64_bytes(value["image"].as_str().expect("the image is there"))
            .expect("the image is base64");
        let signed = unpack(&packed).expect("the image unpacks");

        // The block says its length and its version at the place the published signer writes them.
        assert_eq!(
            u32::from_le_bytes(signed[40..44].try_into().expect("four bytes")),
            72
        );
        assert_eq!(
            u64::from_le_bytes(signed[44..52].try_into().expect("eight bytes")),
            SIGNATURE_VERSION
        );

        // And the signature is over the image from which the block was left out, which is what a
        // bootloader recomputes. The verifying key is the public half of the key that signed it.
        let message = signed_region(&signed, 24);
        let signature = ed25519_dalek::Signature::from_bytes(
            signed[52..116]
                .try_into()
                .expect("the signature is 64 bytes"),
        );

        key.key
            .verifying_key()
            .verify(&message, &signature)
            .expect("the signature verifies");
    }

    #[test]
    fn a_bootloader_carries_the_published_keys_and_the_operators_own() {
        let mut bin = Vec::new();

        bin.extend(b"bootloader head".repeat(2));
        bin.extend(BOOTLOADER_DESCRIPTOR);
        bin.extend([0_u8; 200]);
        bin.extend(b"tail");

        let key = fixture_key();
        let signed = sign_bootloader(&bin, &key).expect("the bootloader is signed");
        let at = find(&bin, &BOOTLOADER_DESCRIPTOR).expect("the fixture carries the descriptor")
            + BOOTLOADER_DESCRIPTOR.len();

        assert_eq!(signed.len(), bin.len());
        assert_eq!(&signed[..at], &bin[..at]);

        let mut cursor = at;

        for encoded in AP_PUBLIC_KEYS {
            let published = base64_bytes(encoded).expect("the published key is base64");
            assert_eq!(&signed[cursor..cursor + 32], published.as_slice());
            cursor += 32;
        }

        assert_eq!(&signed[cursor..cursor + 32], key.public_key());
        assert_eq!(&signed[at + 128..], &bin[at + 128..]);
    }

    #[test]
    fn a_file_without_the_descriptor_is_refused_before_it_is_written() {
        let key = fixture_key();

        let error = sign_bootloader(b"not a bootloader", &key).expect_err("no descriptor");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);

        // The image is the same one, with the descriptor replaced after it was packed: the signer
        // has to look for it in the unpacked image rather than in the text of the document.
        let mut image = fixture_image();
        image[16..24].fill(0);

        let document = format!(
            r#"{{"image_size": {}, "image": "{}"}}"#,
            image.len(),
            base64_text(&pack(&image).expect("the image is packed"))
        );

        let error =
            sign_apj(document.as_bytes(), &key).expect_err("the image carries no descriptor");
        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);
    }

    #[test]
    fn a_file_is_signed_from_the_key_beside_it() {
        let folder = std::env::temp_dir().join(format!("mpcraft-secure-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("the folder is made");

        let key_file = folder.join("operator.dat");
        let generated = generate_key_files(&key_file).expect("the key is written");

        assert!(key_file.exists());
        assert!(std::path::Path::new(&generated.public_key_path).exists());

        let mut bin = Vec::new();
        bin.extend(b"head");
        bin.extend(BOOTLOADER_DESCRIPTOR);
        bin.extend([0_u8; 160]);

        let input = folder.join("bootloader.bin");
        let output = folder.join("bootloader-signed.bin");
        std::fs::write(&input, &bin).expect("the input is written");

        let signed = sign_file(FirmwareKind::Bootloader, &input, &output, &key_file)
            .expect("the file is signed");

        assert_eq!(signed.bytes, bin.len() as u64);
        assert!(signed.sha.is_empty(), "a bootloader is not signed");
        assert_eq!(signed.public_key, generated.public_key);
        assert_eq!(
            std::fs::read(&output).expect("the output is there").len(),
            bin.len()
        );

        std::fs::remove_dir_all(&folder).ok();
    }

    fn wrap(text: &str, width: usize) -> String {
        let mut out = String::new();

        for (index, character) in text.chars().enumerate() {
            if index > 0 && index % width == 0 {
                out.push('\n');
            }

            out.push(character);
        }

        out
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
