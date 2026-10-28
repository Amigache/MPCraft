//! A device in DFU mode, read as the published rules of the protocol say they are read.
//!
//! The numbers here are not written from memory: each one is what a published implementation puts on the
//! wire, and where two implementations disagree the disagreement is kept rather than closed by hand.
//!
//! - The six class requests (0x00 detach, 0x01 download, 0x02 upload, 0x03 get status, 0x04 clear status,
//!   0x05 get state, 0x06 abort) are the same in three readings that were not built from each other:
//!   cherryusb's `class/dfu/usb_dfu.h`, micropython's `tools/pydfu.py` and ArduPilot's `Tools/renode/dfu.py`.
//! - The states (0 to 10) and the statuses (0x00 to 0x0F) are the dictionary of the specification, read
//!   through the header the first of those three carries; a number the dictionary does not give is shown as
//!   the number rather than as a word invented for it, which is the rule the DroneCAN sheet follows.
//! - A block number is not an index of a buffer: block 0 carries a command, blocks 2 and above carry data
//!   at `address + (block - 2) * transfer_size`, and block 1 is nothing. That is what ArduPilot's peer does
//!   and what pydfu sends, and an upload on a block below 2 is refused by the device rather than answered.
//! - The answer to GETSTATUS is six bytes: the status, three for how long to wait, the state, and the
//!   number of a string the device can offer.
//!
//! The checksum is where two readings disagree, and the disagreement is recorded rather than papered over.
//! Three published implementations write the same formula — `0xFFFFFFFF & -zlib.crc32(data) - 1`, which is
//! the reflected accumulator without its final inversion — and the wording of the specification gives a
//! different number for the same bytes (polynomial 0x04C11DB7, initial 0xFFFFFFFF, final inversion). The
//! rule this module carries is the one the three agree on; the other is asserted beside it so that a green
//! does not read as an agreement that was never worked out.

use mpcraft_transport::device::{Device, DeviceResult, FunctionalDescriptor};

use crate::error::{CommandError, CommandResult};

/// The class requests, as the dictionary names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// 0x00 — let go of the interface.
    Detach,
    /// 0x01 — something going to the device.
    Download,
    /// 0x02 — something coming back from it.
    Upload,
    /// 0x03 — what the last operation said.
    GetStatus,
    /// 0x04 — take the status away.
    ClearStatus,
    /// 0x05 — what state the device is in.
    GetState,
    /// 0x06 — stop what is being done.
    Abort,
}

impl Request {
    /// The number the request carries.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Detach => 0x00,
            Self::Download => 0x01,
            Self::Upload => 0x02,
            Self::GetStatus => 0x03,
            Self::ClearStatus => 0x04,
            Self::GetState => 0x05,
            Self::Abort => 0x06,
        }
    }

    /// Reads the number as the dictionary gives it. A peer that answers a request has to read what the
    /// host sent, which is why this direction exists beside `as_byte`; a number the dictionary does not
    /// give is nothing rather than a guess.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Detach),
            0x01 => Some(Self::Download),
            0x02 => Some(Self::Upload),
            0x03 => Some(Self::GetStatus),
            0x04 => Some(Self::ClearStatus),
            0x05 => Some(Self::GetState),
            0x06 => Some(Self::Abort),
            _ => None,
        }
    }
}

/// The state the device says it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 0 — the application is running.
    AppIdle,
    /// 1 — the device was asked to let go.
    AppDetach,
    /// 2 — nothing is being done.
    DfuIdle,
    /// 3 — something was written and the device has not worked on it yet.
    DownloadSync,
    /// 4 — the device is working on what was written.
    DownloadBusy,
    /// 5 — the writing is done and the device is waiting for the next thing.
    DownloadIdle,
    /// 6 — the device is applying what was written.
    ManifestSync,
    /// 7 — the device says what it applied.
    Manifest,
    /// 8 — the device will start again by itself.
    ManifestWaitReset,
    /// 9 — something is being read back.
    UploadIdle,
    /// 10 — the device refused something.
    DfuError,
}

impl State {
    /// Reads a number the dictionary gives, or answers with nothing for one it does not.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::AppIdle),
            1 => Some(Self::AppDetach),
            2 => Some(Self::DfuIdle),
            3 => Some(Self::DownloadSync),
            4 => Some(Self::DownloadBusy),
            5 => Some(Self::DownloadIdle),
            6 => Some(Self::ManifestSync),
            7 => Some(Self::Manifest),
            8 => Some(Self::ManifestWaitReset),
            9 => Some(Self::UploadIdle),
            10 => Some(Self::DfuError),
            _ => None,
        }
    }

    /// The word the dictionary gives this state.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AppIdle => "APP_IDLE",
            Self::AppDetach => "APP_DETACH",
            Self::DfuIdle => "DFU_IDLE",
            Self::DownloadSync => "DFU_DNLOAD_SYNC",
            Self::DownloadBusy => "DFU_DNLOAD_BUSY",
            Self::DownloadIdle => "DFU_DNLOAD_IDLE",
            Self::ManifestSync => "DFU_MANIFEST_SYNC",
            Self::Manifest => "DFU_MANIFEST",
            Self::ManifestWaitReset => "DFU_MANIFEST_WAIT_RESET",
            Self::UploadIdle => "DFU_UPLOAD_IDLE",
            Self::DfuError => "DFU_ERROR",
        }
    }
}

/// What the last operation said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 0x00 — it went well.
    Ok,
    /// 0x01 — the target refused.
    ErrTarget,
    /// 0x04 — the erase did not go well.
    ErrErase,
    /// 0x08 — the address is not one the device has.
    ErrAddress,
    /// A number the dictionary does not name, kept as the number.
    Other(u8),
}

impl Status {
    /// Reads a number the dictionary gives.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Ok,
            0x01 => Self::ErrTarget,
            0x04 => Self::ErrErase,
            0x08 => Self::ErrAddress,
            other => Self::Other(other),
        }
    }

    /// The number the dictionary gives this status, which is what a peer writes in the answer. It is kept
    /// as a method rather than a cast because the enum carries a value with data — `Other` holds a number
    /// the dictionary does not name — and a cast would have to invent one for it.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Ok => 0x00,
            Self::ErrTarget => 0x01,
            Self::ErrErase => 0x04,
            Self::ErrAddress => 0x08,
            Self::Other(number) => number,
        }
    }

    /// The word the dictionary gives this status, or nothing for a number it does not name.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::Ok => Some("OK"),
            Self::ErrTarget => Some("ERR_TARGET"),
            Self::ErrErase => Some("ERR_ERASE"),
            Self::ErrAddress => Some("ERR_ADDRESS"),
            Self::Other(_) => None,
        }
    }
}

/// What the answer to GETSTATUS carries: the status, how long the device asks to wait, the state, and the
/// number of a string it can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer {
    /// What the last operation said.
    pub status: Status,
    /// How long the device says to wait before asking again, in milliseconds.
    pub poll_ms: u32,
    /// The state the device says it is in.
    pub state: State,
    /// The string the device offers beside the answer, when it offers one.
    pub string_index: u8,
}

impl Answer {
    /// Reads the six bytes the request answers with.
    ///
    /// # Errors
    ///
    /// Returns an error when the answer is not six bytes, or when the state is a number the dictionary
    /// does not give.
    pub fn read(bytes: &[u8]) -> CommandResult<Self> {
        if bytes.len() != 6 {
            return Err(CommandError::invalid_request(format!(
                "the answer is {} bytes rather than the 6 the request asks for",
                bytes.len()
            )));
        }

        let state_byte = bytes[4];
        let state = State::from_byte(state_byte).ok_or_else(|| {
            CommandError::invalid_request(format!(
                "the state is {state_byte}, which the dictionary does not name"
            ))
        })?;

        Ok(Self {
            status: Status::from_byte(bytes[0]),
            // Tres bytes, leidos en el orden little-endian que la lectura publicada escribe:
            // `struct.pack("B3sBI", status, poll, state, iString)` — el primer byte despues del estado es el
            // menos significativo.
            poll_ms: u32::from_le_bytes([bytes[1], bytes[2], bytes[3], 0]),
            state,
            string_index: bytes[5],
        })
    }
}

/// The commands that ride on block 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// 0x21 — say where the next thing goes.
    SetAddressPointer,
    /// 0x41 — erase.
    Erase,
    /// 0x92 — take the read protection away.
    ReadUnprotect,
}

impl Command {
    /// The byte the command carries.
    #[must_use]
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::SetAddressPointer => 0x21,
            Self::Erase => 0x41,
            Self::ReadUnprotect => 0x92,
        }
    }
}

/// One part of the memory the interface says it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sector {
    /// Where the sector starts.
    pub address: u32,
    /// How big it is.
    pub size: u32,
    /// What the letter of the string says can be done to it, or nothing when the string says nothing.
    pub permission: Option<Permission>,
}

impl Sector {
    /// Whether this sector can be erased: a sector whose permission was not said is not an erasable one,
    /// because "not said" is not a permission.
    #[must_use]
    pub fn erasable(&self) -> bool {
        self.permission
            .is_some_and(|permission| permission.erasable)
    }
}

/// What a letter of the string says can be done to a sector. The three words are the dictionary dfu-util
/// and fwupd both write for the same seven letters — `a` reads, `b` erases, `c` reads and erases, `d`
/// writes, `e` reads and writes, `f` erases and writes, `g` does all three — so the two published readers
/// agree on the meaning of a letter even where they disagree on whether a token may carry one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permission {
    /// The letter itself, kept as what the string wrote.
    pub letter: char,
    /// It can be read.
    pub readable: bool,
    /// It can be erased.
    pub erasable: bool,
    /// It can be written.
    pub writable: bool,
}

impl Permission {
    /// Reads the letter the string writes.
    const fn from_letter(letter: char) -> Option<Self> {
        let (readable, erasable, writable) = match letter {
            'a' => (true, false, false),
            'b' => (false, true, false),
            'c' => (true, true, false),
            'd' => (false, false, true),
            'e' => (true, false, true),
            'f' => (false, true, true),
            'g' => (true, true, true),
            _ => return None,
        };

        Some(Self {
            letter,
            readable,
            erasable,
            writable,
        })
    }
}

/// The memory the interface describes in the string it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The name the string gives this memory.
    pub name: String,
    /// Where it starts.
    pub base: u32,
    /// The sectors, in the order the string lists them.
    pub sectors: Vec<Sector>,
}

impl Layout {
    /// Reads the string as the published rule reads it.
    ///
    /// Two spellings are published and both are read: `000016*1024`, which counts the size in bytes, and
    /// `04*016Kg`, which counts it in units of a thousand and puts a permission letter after it — the second
    /// is what ArduPilot's own writer produces for this family. The two cover the same ground for 1024K, but
    /// they do not make the same sectors: the two published readers that agree on a token with no unit letter
    /// (dfu-util's `src/dfuse_mem.c` and pydfu's `get_memory_layout`) read its number as the size in bytes
    /// and multiply only when the letter is there, which distributes the spec form into 24 runs instead of 12.
    /// That comparison, and fwupd's refusal of the same token, are what the test named below carries.
    ///
    /// # Errors
    ///
    /// Returns an error when the string is not the shape the rule says: no `@`, no address, no sectors, a
    /// token with no `*`, or an address that cannot be read as a number.
    pub fn read(text: &str) -> CommandResult<Self> {
        let rest = text
            .strip_prefix('@')
            .ok_or_else(|| CommandError::invalid_request("the string does not start with @"))?;

        let (name, remainder) = rest
            .split_once('/')
            .ok_or_else(|| CommandError::invalid_request("the string has no name beside the @"))?;

        let (address_text, sector_text) = remainder.split_once('/').ok_or_else(|| {
            CommandError::invalid_request("the string has an address and no sectors")
        })?;

        let address = parse_address(address_text).ok_or_else(|| {
            CommandError::invalid_request(format!(
                "the address {address_text} is not one the string can say"
            ))
        })?;

        if sector_text.is_empty() {
            return Err(CommandError::invalid_request("the string names no sectors"));
        }

        let mut sectors = Vec::new();
        let mut place = address;

        for token in sector_text.split(',') {
            let (count_part, size_part) = token.split_once('*').ok_or_else(|| {
                CommandError::invalid_request(format!("the token {token} has no *"))
            })?;

            let count: u32 = count_part.parse().map_err(|_| {
                CommandError::invalid_request(format!("{count_part} is not a count"))
            })?;

            if count == 0 {
                return Err(CommandError::invalid_request(format!(
                    "the token {token} counts no sector"
                )));
            }

            // El numero se lee hasta el primer caracter que no es digito, y lo que sigue es una letra de unidad
            // si la hay. La lectura publicada de dfu-util (`src/dfuse_mem.c`, el `switch (multiplier)` de
            // `parse_memory_layout`) multiplica solo cuando esa letra es K o M: por `64*1024` sin letra da 64
            // sectores de 1024 bytes, que es el reparto de la forma del espectro, y fwupd rechaza ese token
            // porque su regla pide una letra de unidad y un tamano entre 1 y 999. El desacuerdo se registra y
            // no se cierra por la mano.
            let digits_end = size_part
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(size_part.len());
            let digits = &size_part[..digits_end];

            let value: u32 = digits
                .parse()
                .map_err(|_| CommandError::invalid_request(format!("{digits} is not a size")))?;

            let unit_letter = size_part.chars().nth(digits_end);
            let size = match unit_letter {
                Some('K') => value * 1024,
                Some('M') => value * 1024 * 1024,
                _ => value,
            };

            // Lo que sigue a la unidad es el permiso, dicho como lo que la cadena escribe. Aqui la regla es
            // mas tolerante que las dos lecturas que piden una letra tras el numero: dfu-util avisa y pasa al
            // siguiente sector cuando el token no trae letra de tipo (`Non-valid multiplier 'z', interpreted as
            // type identifier instead`), y fwupd rechaza el token entero. Una letra que ninguno de los dos
            // diccionarios da se conserva como dato y el sector queda sin permiso, porque "no dicho" no es un
            // permiso. Y una letra de permiso sin letra de unidad — `01*016g` — se lee como 16 bytes, que es
            // lo que dfu-util hace cuando el multiplicador es esa letra.
            let permission = match unit_letter {
                Some('K' | 'M') => size_part
                    .chars()
                    .nth(digits_end + 1)
                    .and_then(Permission::from_letter),
                Some(letter) => Permission::from_letter(letter),
                None => None,
            };

            for _ in 0..count {
                sectors.push(Sector {
                    address: place,
                    size,
                    permission,
                });
                place += size;
            }
        }

        Ok(Self {
            name: name.to_owned(),
            base: address,
            sectors,
        })
    }

    /// The sector that starts at an address, which is what an erase has to be on.
    #[must_use]
    pub fn sector_at(&self, address: u32) -> Option<Sector> {
        self.sectors
            .iter()
            .find(|sector| sector.address == address)
            .copied()
    }

    /// Whether a run of bytes falls inside the memory the string describes.
    #[must_use]
    pub fn holds(&self, address: u32, length: usize) -> bool {
        let end = match address.checked_add(length as u32) {
            Some(end) => end,
            None => return false,
        };

        self.sectors
            .iter()
            .any(|sector| sector.address <= address && end <= sector.address + sector.size)
    }
}

/// Reads `0x` and the digits after it, the way the published reader reads them: uppercase only.
fn parse_address(text: &str) -> Option<u32> {
    let digits = text.strip_prefix("0x")?;

    if digits.is_empty()
        || !digits
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
    {
        return None;
    }

    u32::from_str_radix(digits, 16).ok()
}

/// The document that carries firmware for a device of this protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// What the first five bytes say: `DfuSe`.
    pub signature: String,
    /// The version the document says it is.
    pub version: u8,
    /// The size the document declares for itself.
    pub declared_size: usize,
    /// The targets, in the order the document lists them.
    pub targets: Vec<Target>,
    /// The suffix, as the document writes it.
    pub suffix: Suffix,
    /// The number the document carries.
    pub recorded_crc: u32,
    /// The number the rule gives for the same bytes.
    pub computed_crc: u32,
    /// What the bytes make under the rule the published writer counts with, which is not always what the
    /// field says.
    pub bytes_under_the_field: usize,
}

impl Document {
    /// Reads the document.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are not the document — which is what happens when a raw image is
    /// handed to a path that speaks the document — or when a target says a size the bytes do not make.
    pub fn read(bytes: &[u8]) -> CommandResult<Self> {
        if bytes.len() < 11 + 16 {
            return Err(CommandError::invalid_request(
                "the file is shorter than a document of this format can be",
            ));
        }

        let signature = String::from_utf8_lossy(&bytes[..5]).to_string();

        if signature != "DfuSe" {
            return Err(CommandError::invalid_request(format!(
                "the file starts with {signature}, which is not DfuSe"
            )));
        }

        let version = bytes[5];
        let declared_size = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let targets = bytes[10] as usize;

        let suffix_bytes = &bytes[bytes.len() - 16..bytes.len() - 4];
        let recorded_crc = u32::from_le_bytes([
            bytes[bytes.len() - 4],
            bytes[bytes.len() - 3],
            bytes[bytes.len() - 2],
            bytes[bytes.len() - 1],
        ]);

        let mut rest = &bytes[11..bytes.len() - 16];
        let mut read_targets = Vec::new();
        let mut targets_total = 0_usize;

        for _ in 0..targets {
            if rest.len() < 274 {
                return Err(CommandError::invalid_request(format!(
                    "a target prefix is {} bytes rather than the 274 the format says it is",
                    rest.len()
                )));
            }

            if &rest[..6] != b"Target" {
                return Err(CommandError::invalid_request(
                    "the bytes where a target should be do not say Target",
                ));
            }

            let alt_setting = rest[6];
            let named = u32::from_le_bytes([rest[7], rest[8], rest[9], rest[10]]) == 1;
            let name_field = &rest[11..266];
            let size = u32::from_le_bytes([rest[266], rest[267], rest[268], rest[269]]) as usize;
            let elements =
                u32::from_le_bytes([rest[270], rest[271], rest[272], rest[273]]) as usize;

            let name = if named {
                Some(String::from_utf8_lossy(cstring(name_field)).to_string())
            } else {
                None
            };

            rest = &rest[274..];

            if rest.len() < size {
                return Err(CommandError::invalid_request(format!(
                    "the target says {size} bytes and the file has {} left",
                    rest.len()
                )));
            }

            let data = &rest[..size];
            rest = &rest[size..];
            targets_total += 274 + size;

            let mut items = Vec::new();
            let mut cursor = 0;

            for _ in 0..elements {
                if cursor + 8 > data.len() {
                    return Err(CommandError::invalid_request(
                        "an element prefix is shorter than the format says it is",
                    ));
                }

                let address = u32::from_le_bytes([
                    data[cursor],
                    data[cursor + 1],
                    data[cursor + 2],
                    data[cursor + 3],
                ]) as usize;
                let length = u32::from_le_bytes([
                    data[cursor + 4],
                    data[cursor + 5],
                    data[cursor + 6],
                    data[cursor + 7],
                ]) as usize;

                cursor += 8;

                if cursor + length > data.len() {
                    return Err(CommandError::invalid_request(format!(
                        "the element says {length} bytes and the target has {} left",
                        data.len() - cursor
                    )));
                }

                items.push(Element {
                    address,
                    data: data[cursor..cursor + length].to_vec(),
                });
                cursor += length;
            }

            read_targets.push(Target {
                alt_setting,
                name,
                declared_size: size,
                elements: items,
            });
        }

        Ok(Self {
            signature,
            version,
            declared_size,
            targets: read_targets,
            suffix: Suffix::read(suffix_bytes)?,
            recorded_crc,
            // El numero se calcula sobre todo lo que el documento tiene delante de él — el encabezado
            // incluido —, que es lo que las dos lecturas publicadas del formato hacen: `compute_crc(data)`
            // sobre el cuerpo entero en dfuse-pack.py y `crc32_byte` sobre `firmware[0 .. size + suffixlen
            // - 4]` en dfu-util.
            computed_crc: crc_rule(&bytes[..bytes.len() - 4]),
            bytes_under_the_field: 11 + targets_total,
        })
    }

    /// Whether the number the document carries is the one the rule gives.
    #[must_use]
    pub fn crc_agrees(&self) -> bool {
        self.recorded_crc == self.computed_crc
    }
}

/// One target of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The alternate setting the target belongs to.
    pub alt_setting: u8,
    /// The name the document gives it, when it gives one.
    pub name: Option<String>,
    /// The size the document says for this target.
    pub declared_size: usize,
    /// The elements, in the order the document lists them.
    pub elements: Vec<Element>,
}

/// One run of bytes inside a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Where the document says these bytes go.
    pub address: usize,
    /// The bytes.
    pub data: Vec<u8>,
}

/// The suffix of the document: the numbers it says the device is, and the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suffix {
    /// The device the document was made for.
    pub device: u16,
    /// The product it was made for.
    pub product: u16,
    /// Who made it.
    pub vendor: u16,
    /// The version of the protocol the document assumes.
    pub dfu: u16,
    /// What the document says it is: `UFD`.
    pub marker: String,
    /// The length the document says for the suffix.
    pub length: u8,
}

impl Suffix {
    /// Reads the sixteen bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when there are not sixteen, or when the three characters are not what the format
    /// says they are.
    pub fn read(bytes: &[u8]) -> CommandResult<Self> {
        if bytes.len() != 12 {
            return Err(CommandError::invalid_request(format!(
                "the suffix is {} bytes rather than the 12 before its check",
                bytes.len()
            )));
        }

        let marker = String::from_utf8_lossy(&bytes[8..11]).to_string();

        if marker != "UFD" {
            return Err(CommandError::invalid_request(format!(
                "the suffix says {marker}, which is not UFD"
            )));
        }

        Ok(Self {
            device: u16::from_le_bytes([bytes[0], bytes[1]]),
            product: u16::from_le_bytes([bytes[2], bytes[3]]),
            vendor: u16::from_le_bytes([bytes[4], bytes[5]]),
            dfu: u16::from_le_bytes([bytes[6], bytes[7]]),
            marker,
            length: bytes[11],
        })
    }
}

/// The text of a field up to the zero that ends it.
fn cstring(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|byte| *byte == 0) {
        Some(end) => &bytes[..end],
        None => bytes,
    }
}

/// The rule the three published implementations agree on: the reflected accumulator, started at
/// 0xFFFFFFFF, without its final inversion.
#[must_use]
pub fn crc_rule(data: &[u8]) -> u32 {
    let mut state = 0xFFFF_FFFF_u32;

    for byte in data {
        state = crc_table()[((state ^ *byte as u32) & 0xFF) as usize] ^ (state >> 8);
    }

    // La regla para aqui, en el acumulador, sin la inversion final. Un lector que la aplica — como un
    // lector de zlib — da otro numero por los mismos bytes: por `abc` las dos lecturas coinciden en
    // 0xcadbbe3d, pero por el cuerpo del fixture dan 0xbcebc9b4 y 0x94b3e7bc, que es el desacuerdo que el
    // gate compara entre los dos caminos.
    state
}

/// The table of the reflected polynomial 0xEDB88320, worked out rather than copied.
fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();

    TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];

        for (index, entry) in table.iter_mut().enumerate() {
            let mut value = index as u32;

            for _ in 0..8 {
                value = if value & 1 != 0 {
                    (value >> 1) ^ 0xEDB8_8320
                } else {
                    value >> 1
                };
            }

            *entry = value;
        }

        table
    })
}

/// The device plus the memory the interface says it has.
pub struct Flash<D> {
    /// The device the requests go to.
    device: D,
    /// What the interface said about itself.
    descriptor: FunctionalDescriptor,
    /// The memory the string describes.
    layout: Layout,
    /// Where the next run of bytes goes.
    address: u32,
    /// The block the next run of bytes rides on.
    block: u16,
}

impl<D: Device> Flash<D> {
    /// Reads what the interface says about itself and the string that describes its memory.
    ///
    /// # Errors
    ///
    /// Returns an error when the device is not there, when it refuses a request, or when the string is not
    /// the shape the rule says it is.
    pub async fn open(mut device: D, string_index: u16) -> CommandResult<Self> {
        // El descriptor se lee antes de que el dispositivo pase a formar parte del intercambio: es lo que
        // la interfaz dijo de sí misma, y el test lo compara con los nueve bytes que una lectura publicada
        // escribe para esta placa.
        let descriptor = *device.functional_descriptor();
        let text = device.string(string_index).await?;

        Ok(Self {
            device,
            descriptor,
            layout: Layout::read(&text)?,
            address: 0,
            block: 2,
        })
    }

    /// El dispositivo al que las peticiones van. Se lee por referencia porque la capa de peticiones es la
    /// que las hace, y el test necesita contar lo que viajó para decir que una negativa no envió nada.
    pub fn device(&self) -> &D {
        &self.device
    }

    /// What the interface says about itself.
    #[must_use]
    pub fn descriptor(&self) -> &FunctionalDescriptor {
        &self.descriptor
    }

    /// The memory the string describes.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Where the next run of bytes goes.
    #[must_use]
    pub const fn address(&self) -> u32 {
        self.address
    }

    /// Says where the next thing goes.
    ///
    /// # Errors
    ///
    /// Returns an error when the address is outside the memory the string describes, or when the device
    /// answers with a state that is not the one the rule says a command answers with.
    pub async fn set_address(&mut self, address: u32) -> CommandResult<Answer> {
        if !self.layout.holds(address, 0) {
            return Err(CommandError::invalid_request(format!(
                "the address 0x{address:08X} is outside the memory the interface describes"
            )));
        }

        self.address = address;
        self.run_command(Command::SetAddressPointer, address).await
    }

    /// Erases one sector of the memory.
    ///
    /// The two rules are read before anything goes out: the address has to be the start of a sector, and
    /// the sector has to be one the string says can be erased — a sector whose permission was not said is
    /// not an erasable one, and a sector whose letter only says it can be read is not one either, which is
    /// what the published reader refuses before sending (`find_segment` and `DFUSE_ERASABLE`).
    ///
    /// # Errors
    ///
    /// Returns an error for an address that is not a sector boundary or a sector that cannot be erased, and
    /// for an answer that is not the state a command answers with.
    pub async fn erase_sector(&mut self, address: u32) -> CommandResult<Answer> {
        let Some(sector) = self.layout.sector_at(address) else {
            return Err(CommandError::invalid_request(format!(
                "0x{address:08X} is not the start of a sector of the memory the interface describes"
            )));
        };

        if !sector.erasable() {
            return Err(CommandError::invalid_request(format!(
                "the sector at 0x{address:08X} says {} and nothing about being erased",
                letter_of(sector.permission)
            )));
        }

        self.run_command(Command::Erase, address).await
    }

    /// Erases everything the device has.
    ///
    /// The number is sent alone, which is the form two published implementations write. ArduPilot's own
    /// peer refuses the same byte with a length of one — it reads only the five-byte form — so this is a
    /// disagreement between two readings rather than a rule both agree on; the test says which side this
    /// code stands on and what the other side would have done with the same bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the device refuses the request or answers with a state that is not the one a
    /// command answers with.
    pub async fn mass_erase(&mut self) -> CommandResult<Answer> {
        self.send_command(&[Command::Erase.as_byte()]).await?;
        self.read_answer().await
    }

    /// Takes the read protection away.
    ///
    /// # Errors
    ///
    /// Returns an error when the device refuses the request or answers with a state that is not the one a
    /// command answers with.
    pub async fn read_unprotect(&mut self) -> CommandResult<Answer> {
        self.send_command(&[Command::ReadUnprotect.as_byte()])
            .await?;
        self.read_answer().await
    }

    /// Sends a run of bytes on the block the counter is on, and moves the counter.
    ///
    /// # Errors
    ///
    /// Returns an error when the bytes are more than the transfer size the interface says it can take, when
    /// the block is not one that carries data, or when the run does not fall inside the memory the string
    /// describes.
    pub async fn write(&mut self, data: &[u8]) -> CommandResult<Answer> {
        if data.len() > self.descriptor.transfer_size as usize {
            return Err(CommandError::invalid_request(format!(
                "{} bytes are more than the {} the interface says it can take in one request",
                data.len(),
                self.descriptor.transfer_size
            )));
        }

        if data.is_empty() {
            return Err(CommandError::invalid_request(
                "an empty run of bytes is how the device is told to apply what was written, and that is a different request",
            ));
        }

        let place = self.address + (self.block - 2) as u32 * self.descriptor.transfer_size as u32;

        if !self.layout.holds(place, data.len()) {
            return Err(CommandError::invalid_request(format!(
                "the run of {} bytes at 0x{place:08X} falls outside the memory the interface describes",
                data.len()
            )));
        }

        self.device
            .send(Request::Download.as_byte(), self.block, data)
            .await?;

        self.block += 1;

        self.read_answer().await
    }

    /// Reads a run of bytes back.
    ///
    /// # Errors
    ///
    /// Returns an error for a block below 2 — block 0 carries a command and block 1 is nothing — or for a
    /// place outside the memory the string describes.
    pub async fn read(&mut self, block: u16, length: u16) -> CommandResult<Vec<u8>> {
        if block < 2 {
            return Err(CommandError::invalid_request(format!(
                "block {block} does not carry bytes: block 0 carries a command and block 1 is nothing"
            )));
        }

        let place = self.address + (block - 2) as u32 * self.descriptor.transfer_size as u32;

        if !self.layout.holds(place, length as usize) {
            return Err(CommandError::invalid_request(format!(
                "the {} bytes asked for at 0x{place:08X} fall outside the memory the interface describes",
                length as usize
            )));
        }

        let answer = self
            .device
            .receive(Request::Upload.as_byte(), block, length)
            .await?;

        if answer.len() != length as usize {
            return Err(CommandError::invalid_request(format!(
                "{} bytes were asked for and {} came back",
                length as usize,
                answer.len()
            )));
        }

        Ok(answer)
    }

    /// Tells the device to apply what was written.
    ///
    /// # Errors
    ///
    /// Returns an error when the device answers with a state that is not the one this step answers with.
    pub async fn finish(&mut self) -> CommandResult<Answer> {
        self.device
            .send(Request::Download.as_byte(), 0, &[])
            .await?;

        self.read_answer().await
    }

    /// What the device says its state is.
    ///
    /// # Errors
    ///
    /// Returns an error when the device refuses the request or answers with a number the dictionary does not
    /// give.
    pub async fn state(&mut self) -> CommandResult<State> {
        let answer = self
            .device
            .receive(Request::GetState.as_byte(), 0, 1)
            .await?;

        State::from_byte(answer.first().copied().unwrap_or(0)).ok_or_else(|| {
            CommandError::invalid_request(format!(
                "the state is {}, which the dictionary does not name",
                answer.first().copied().unwrap_or(0)
            ))
        })
    }

    /// One command on block 0, with its address.
    async fn run_command(&mut self, command: Command, address: u32) -> CommandResult<Answer> {
        let mut payload = vec![command.as_byte()];
        payload.extend_from_slice(&address.to_le_bytes());

        self.send_command(&payload).await?;
        self.read_answer().await
    }

    async fn send_command(&mut self, payload: &[u8]) -> DeviceResult<()> {
        self.device
            .send(Request::Download.as_byte(), 0, payload)
            .await
    }

    /// The answer a request gets, read as the rule says it is read.
    ///
    /// A device that answers with `DFU_ERROR` is reported as a refusal rather than as a step that went
    /// well, and a state that is not the one the rule expects for this step is said as what the device
    /// said rather than read as a pass. It is public because the next GETSTATUS after a send is its own
    /// step on the wire — the second reading of a two-answer exchange is a request, not a detail of the
    /// first.
    pub async fn read_answer(&mut self) -> CommandResult<Answer> {
        let bytes = self
            .device
            .receive(Request::GetStatus.as_byte(), 0, 6)
            .await?;

        let answer = Answer::read(&bytes)?;

        if answer.status != Status::Ok {
            return Err(CommandError::rejected(format!(
                "the device answered {}{}",
                name_of(answer.status),
                offer_of(answer.string_index)
            )));
        }

        Ok(answer)
    }
}

/// The word the dictionary gives a status, or the number when the dictionary does not give one.
fn name_of(status: Status) -> String {
    match status.name() {
        Some(name) => name.to_owned(),
        None => {
            let number = status.as_byte();
            format!("0x{number:02x}")
        }
    }
}

/// The letter the string writes for a permission, or the words that say nothing was said.
fn letter_of(permission: Option<Permission>) -> String {
    match permission {
        Some(permission) => permission.letter.to_string(),
        None => "nothing".to_owned(),
    }
}

/// The string the device offers beside an answer, said as what it offers rather than as prose.
fn offer_of(index: u8) -> String {
    if index == 0 {
        return String::new();
    }

    format!(", with the string {index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El fixture, escrito con las reglas del formato. Su numero es el que una segunda lectura del formato
    /// (`scripts/check-dfu-crc.js`, hecha desde la formula publicada y no desde esta lectura) da por los
    /// mismos bytes: los dos llegan al mismo numero por caminos que no salen uno del otro.
    const FIXTURE_CRC: u32 = 0xbceb_c9b4;

    /// Lo que la lectura del espectro da por esos mismos bytes — otro numero, que se registra como
    /// desacuerdo entre las dos lecturas y no como algo cerrado por la mano.
    const FIXTURE_SPEC_CRC: u32 = 0xa02b_6bff;

    const GOLDEN: &[(&[u8], u32, u32)] = &[
        (b"", 0xffff_ffff, 0x0000_0000),
        (b"abc", 0xcadb_be3d, 0x9e62_63ff),
        (b"the quick brown fox", 0x6e3e_fd35, 0x0908_d5ff),
        (b"mpcraft fixture\x00", 0x8955_0363, 0x3b72_88ff),
        (b"golden", 0xcc00_79d8, 0xfc0a_02ff),
    ];

    fn fixture() -> Vec<u8> {
        let element_one = b"mpcraft fixture\x00";
        let element_two = b"golden";
        // El tamano del elemento es lo que el escritor publicado cuenta: los bytes de la imagen, con el cero
        // que la cierra, porque el campo de la longitud se escribe antes de ellos y el lector lee esa
        // longitud entera.
        let data_len = 8 + element_one.len() + 8 + element_two.len();

        let mut name_field = [0_u8; 255];
        for (index, byte) in b"MPCRAFT F405".iter().enumerate() {
            name_field[index] = *byte;
        }

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"DfuSe");
        bytes.push(1);
        // El campo de tamano dice cuanto mide el objetivo entero — su prefijo de 274 bytes y sus datos — y el
        // escritor lo escribe con los once bytes del encabezado delante: `len(data) + 11`, donde `data` son
        // los objetivos enteros, prefijo incluido. Los dos numeros se guardan aparte porque un documento
        // puede decir uno y hacer otro.
        bytes.extend_from_slice(&((274 + data_len + 11) as u32).to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(b"Target");
        bytes.push(0);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&name_field);
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0x0800_0000_u32.to_le_bytes());
        bytes.extend_from_slice(&(element_one.len() as u32).to_le_bytes());
        bytes.extend_from_slice(element_one);
        bytes.extend_from_slice(&0x0800_0100_u32.to_le_bytes());
        bytes.extend_from_slice(&(element_two.len() as u32).to_le_bytes());
        bytes.extend_from_slice(element_two);

        // El sufijo, con los numeros que esta placa dijo de sí misma.
        bytes.extend_from_slice(&0x0413_u16.to_le_bytes());
        bytes.extend_from_slice(&0xDF11_u16.to_le_bytes());
        bytes.extend_from_slice(&0x0483_u16.to_le_bytes());
        bytes.extend_from_slice(&0x011A_u16.to_le_bytes());
        bytes.extend_from_slice(b"UFD");
        bytes.push(16);
        bytes.extend_from_slice(&FIXTURE_CRC.to_le_bytes());

        bytes
    }

    #[test]
    fn the_rule_of_the_three_published_readings_is_the_one_this_carries() {
        for (bytes, published, spec_worded) in GOLDEN {
            assert_eq!(crc_rule(bytes), *published);

            // The disagreement is a fact about the two readings, not something to close: the wording of the
            // specification counts with a polynomial that is not reflected and with a final inversion, and
            // the same bytes give a different number.
            assert_ne!(crc_rule(bytes), *spec_worded);
        }

        assert_eq!(crc_rule(&fixture()[..fixture().len() - 4]), FIXTURE_CRC);

        // Y el mismo cuerpo, con la regla escrita como el espectro la describe — polinomio no reflejado,
        // inicial 0xFFFFFFFF, inversion final — da otro numero. Los dos se guardan, porque el desacuerdo es
        // un hecho sobre las dos lecturas y no algo que la mano pueda cerrar.
        assert_ne!(
            crc_rule(&fixture()[..fixture().len() - 4]),
            FIXTURE_SPEC_CRC
        );
    }

    #[test]
    fn the_document_is_read_and_the_number_it_carries_is_kept_beside_the_one_the_rule_gives() {
        let document = Document::read(&fixture()).expect("the fixture is the document");

        assert_eq!(document.signature, "DfuSe");
        assert_eq!(document.version, 1);
        assert_eq!(document.targets.len(), 1);
        assert_eq!(document.targets[0].name.as_deref(), Some("MPCRAFT F405"));
        assert_eq!(document.targets[0].elements.len(), 2);
        assert_eq!(document.targets[0].elements[0].address, 0x0800_0000);
        assert_eq!(
            document.targets[0].elements[0].data,
            b"mpcraft fixture\x00".to_vec()
        );
        assert_eq!(document.targets[0].elements[1].address, 0x0800_0100);
        assert_eq!(document.suffix.device, 0x0413);
        assert_eq!(document.suffix.product, 0xDF11);
        assert_eq!(document.suffix.vendor, 0x0483);
        assert_eq!(document.suffix.dfu, 0x011A);
        assert_eq!(document.suffix.marker, "UFD");
        assert_eq!(document.recorded_crc, FIXTURE_CRC);
        assert!(document.crc_agrees());

        // El numero que el documento declara para sí no es lo que los bytes hacen bajo la regla con la que
        // el escritor publicado lo escribe: se guardan los dos, porque un documento puede decir uno y hacer
        // otro.
        assert_eq!(document.declared_size, 323);
        assert_eq!(document.bytes_under_the_field, 323);
    }

    #[test]
    fn a_number_that_does_not_match_the_bytes_is_said_as_the_document_says_it() {
        let mut bytes = fixture();
        let check_place = bytes.len() - 4;
        bytes[check_place] ^= 0xFF;

        let document = Document::read(&bytes).expect("the shape is still the document");

        assert!(!document.crc_agrees());
        assert_eq!(document.recorded_crc, FIXTURE_CRC ^ 0xFF);
        assert_eq!(document.computed_crc, FIXTURE_CRC);
    }

    #[test]
    fn a_field_that_lies_about_its_own_bytes_is_kept_as_the_field_says_it() {
        let mut bytes = fixture();
        bytes[6..10].copy_from_slice(&400_u32.to_le_bytes());

        let document = Document::read(&bytes).expect("the field is not read as a fault");

        assert_eq!(document.declared_size, 400);
        assert_eq!(document.bytes_under_the_field, 323);
    }

    #[test]
    fn the_two_published_readings_of_the_same_string_agree_only_where_the_unit_letter_is_written() {
        // La primera cadena es la forma que el espectro escribe; la segunda es la que el propio escritor de
        // ArduPilot produce para esta familia (f405, 1024K). Lo que se habia dicho antes sobre las dos — que
        // su lectura publicada da los mismos sectores para 1024K — era incorrecto y se corrige aqui: no lo da.
        // Los dos lectores publicados que coinciden (`src/dfuse_mem.c` de dfu-util y `get_memory_layout` de
        // pydfu) toman el numero tras el `*` como el tamano en bytes y multiplican solo cuando la letra de
        // unidad esta escrita, asi que la forma del espectro reparte en 24 tramos que no cubren la tierra del
        // escritor. La geometria que el propio escritor declara para la familia (`flash_sectors('f405', 1024K)`:
        // 4 sectores de 16K, 1 de 64K, 7 de 128K) es la que su cadena produce, y ahi si coinciden los caminos.
        let spec_form =
            Layout::read("@Internal Flash /0x08000000/000016*1024,0001*64*1024,0007*128*1024")
                .expect("the string the specification gives");
        let writer_form = Layout::read("@Internal Flash /0x08000000/04*016Kg,01*064Kg,07*128Kg")
            .expect("the string the published writer produces");

        assert_eq!(spec_form.base, 0x0800_0000);
        assert_eq!(writer_form.base, 0x0800_0000);
        // El nombre se conserva como la cadena lo escribe, con el espacio que trae antes de la barra: la
        // lectura publicada (`sscanf(intf_desc, "@%[^/]%n")`) no lo recorta, y fwupd sí lo hace
        // (`g_strchomp`). Se guarda como dato, que es lo que la placa dice de sí misma.
        assert_eq!(spec_form.name, "Internal Flash ");
        assert_eq!(writer_form.name, "Internal Flash ");

        let covered =
            |layout: &Layout| layout.sectors.iter().map(|sector| sector.size).sum::<u32>();

        // La cadena del escritor, leida con la regla de dfu-util: 12 sectores que son la tierra entera.
        assert_eq!(covered(&writer_form), 1_048_576);
        assert_eq!(writer_form.sectors.len(), 12);
        assert_eq!(writer_form.sectors[0].size, 16_384);
        assert_eq!(writer_form.sectors[4].address, 0x0801_0000);
        assert_eq!(writer_form.sectors[4].size, 65_536);
        assert_eq!(writer_form.sectors[5].size, 131_072);
        assert_eq!(writer_form.sectors[11].address, 0x080E_0000);
        assert!(writer_form.sectors.iter().all(|sector| sector.erasable()));

        // Y la forma del espectro, leida con la misma regla, que no multiplica sin letra: 16 tramos de 1024
        // bytes, uno de 64 y siete de 128, que suman 17 344 en lugar del terabyte de tierra. Ninguno dice
        // permiso, porque esta forma no escribe letra de tipo, y un sector que no lo dice no se borra.
        assert_eq!(covered(&spec_form), 17_344);
        assert_eq!(spec_form.sectors.len(), 24);
        assert_eq!(spec_form.sectors[0].size, 1_024);
        assert_eq!(spec_form.sectors[16].size, 64);
        assert_eq!(spec_form.sectors[16].address, 0x0800_4000);
        assert_eq!(spec_form.sectors[17].size, 128);
        assert_eq!(spec_form.sectors[17].address, 0x0800_4040);
        assert_eq!(spec_form.sectors[23].address, 0x0800_4340);
        assert!(spec_form.sectors.iter().all(|sector| !sector.erasable()));

        // El desacuerdo se registra y no se cierra: la lectura de fwupd (`plugins/dfu/fu-dfu-target.c`)
        // rechaza los tramos sin letra porque pide una letra de unidad y un tamano entre 1 y 999, y la
        // expresion de pydfu no los lee porque pide unidad y permiso los dos; dfu-util los acepta como bytes y
        // avisa cuando falta la letra de tipo. Aqui se lee el token y el sector queda sin permiso, porque "no
        // dicho" no es un permiso, y lo que se sigue para esta placa es la cadena que el propio escritor de
        // ArduPilot produce, que escribe siempre unidad y permiso.
    }

    #[test]
    fn a_string_that_cannot_be_read_is_refused_rather_than_guessed_at() {
        for bad in [
            "Internal Flash /0x08000000/01*016Kg",
            "@Flash",
            "@Flash /0x08000000",
            "@Flash /notanaddress/01*016Kg",
            "@Flash /0x08000000/016Kg",
            "@Flash /0x08000000/00*016Kg",
        ] {
            let error = Layout::read(bad).expect_err("{bad} is not the shape the rule says");
            assert_eq!(error.code, "invalid_request", "{bad}");
        }

        // Lo que la regla lee sin inventar, aunque las lecturas publicadas rechacen el token: fwupd pide una
        // letra de unidad y un tamano entre 1 y 999, y dfu-util pide una letra de tipo — avisa y pasa al
        // siguiente sector (`Non-valid multiplier 'z', interpreted as type identifier instead`). Aqui el token
        // se lee y la letra que ninguno de los dos diccionarios da queda como dato, con el sector sin permiso,
        // porque "no dicho" no es un permiso.
        let odd = Layout::read("@Flash /0x08000000/01*016Kz").expect("the size is readable");
        assert_eq!(odd.sectors[0].size, 16_384);
        assert_eq!(odd.sectors[0].permission, None);

        // Una letra de permiso sin letra de unidad se lee como lo que dfu-util hace cuando el multiplicador es
        // esa letra: 16 bytes, y el sector con el permiso que la cadena dice.
        let plain = Layout::read("@Flash /0x08000000/01*016g").expect("the size is readable");
        assert_eq!(plain.sectors[0].size, 16);
        assert_eq!(plain.sectors[0].permission.map(|p| p.letter), Some('g'));
    }

    #[test]
    fn a_sector_that_says_nothing_about_being_erased_is_not_an_erasable_one() {
        // El lector publicado rechaza una pagina cuya letra de permiso no esta; aqui la letra se conserva
        // como dato y la regla la lee, porque "no dicho" no es un permiso. Los tres significados salen del
        // diccionario que dfu-util y fwupd escriben igual: `a` solo se lee, `g` se lee, se escribe y se
        // borra, y lo que no se dijo no se puede borrar.
        let layout = Layout::read("@Internal Flash /0x08000000/01*016Ka,01*016Kg,01*016K").unwrap();

        assert_eq!(layout.sectors[0].permission.map(|p| p.letter), Some('a'));
        assert!(!layout.sectors[0].erasable());
        assert_eq!(layout.sectors[1].permission.map(|p| p.letter), Some('g'));
        assert!(layout.sectors[1].erasable());
        assert_eq!(layout.sectors[2].permission, None);
        assert!(!layout.sectors[2].erasable());

        // Y los dos diccionarios coinciden letra por letra en las siete, que es lo que hace posible afirmar
        // el significado sin escribirlo dos veces: se comparan las tablas de `dfuse_mem.c` (1=leer, 2=borrar,
        // 4=escribir) con las de `fu-dfu-target.c`, y las siete letras dan los mismos tres bits.
        for (letter, readable, erasable, writable) in [
            ('a', true, false, false),
            ('b', false, true, false),
            ('c', true, true, false),
            ('d', false, false, true),
            ('e', true, false, true),
            ('f', false, true, true),
            ('g', true, true, true),
        ] {
            let permission =
                Permission::from_letter(letter).expect("the dictionary gives this letter");
            assert_eq!(permission.readable, readable, "{letter}");
            assert_eq!(permission.erasable, erasable, "{letter}");
            assert_eq!(permission.writable, writable, "{letter}");
        }
    }

    #[test]
    fn the_numbers_of_the_requests_and_the_states_are_the_dictionary_read_from_its_source() {
        assert_eq!(Request::Detach.as_byte(), 0x00);
        assert_eq!(Request::Abort.as_byte(), 0x06);
        assert_eq!(Command::SetAddressPointer.as_byte(), 0x21);
        assert_eq!(Command::Erase.as_byte(), 0x41);
        assert_eq!(Command::ReadUnprotect.as_byte(), 0x92);

        for (number, name) in [
            (0, "APP_IDLE"),
            (2, "DFU_IDLE"),
            (3, "DFU_DNLOAD_SYNC"),
            (4, "DFU_DNLOAD_BUSY"),
            (5, "DFU_DNLOAD_IDLE"),
            (6, "DFU_MANIFEST_SYNC"),
            (7, "DFU_MANIFEST"),
            (8, "DFU_MANIFEST_WAIT_RESET"),
            (9, "DFU_UPLOAD_IDLE"),
            (10, "DFU_ERROR"),
        ] {
            assert_eq!(State::from_byte(number).map(State::name), Some(name));
        }

        // A number the dictionary does not give stays a number rather than becoming a word.
        assert_eq!(State::from_byte(11), None);
        assert_eq!(Status::from_byte(0x10), Status::Other(0x10));
        assert_eq!(Status::Other(0x10).name(), None);
    }

    #[test]
    fn the_answer_is_read_as_the_six_bytes_the_request_asks_for() {
        let answer = Answer::read(&[0x04, 0x01, 0x00, 0x00, 0x04, 0]).expect("six bytes");

        assert_eq!(answer.status, Status::ErrErase);
        assert_eq!(answer.poll_ms, 1);
        assert_eq!(answer.state, State::DownloadBusy);
        assert_eq!(answer.string_index, 0);

        assert!(Answer::read(&[0x00, 0x02]).is_err());
    }
}
