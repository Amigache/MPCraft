//! Serial ports that belong to Bluetooth.
//!
//! The bytes are moved by the same code that moves them for any other serial port: a Bluetooth serial
//! channel is a COM port, and a transport does not care what carried them. What this module adds is the
//! knowledge of which port belongs to Bluetooth, read as the machine writes it rather than as somebody
//! chose it.
//!
//! Three facts decide that, and each comes from a published source that is not this file:
//!
//! * `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` is what a reader lists ports from — Microsoft's own `SerialPort`
//!   reads it (`System/sys/system/IO/ports/SerialPort.cs`), so a row is always the device name and the port
//!   it answers with;
//! * a row belongs to Bluetooth through the **prefix** the RFCOMM service writes in that map, and the service is
//!   what the install bound the instance to: `BTHMODEM` is the name the published driver lists carry and the same
//!   string this machine writes for its serial row. It has to be the rule rather than the class, because a serial
//!   row does not say it is in the Bluetooth class at all — it says the Ports class, and a hands-free row says the
//!   audio class with a different service behind it. SuperCollider's parser shows the other half: it matches any
//!   port name, so a virtual pair named with the word Bluetooth somewhere inside itself would be read as a
//!   Bluetooth row by a looser comparison;
//! * which profile a row carries is what makes it a channel bytes can travel on. The published number for the
//!   Serial Port profile is `00001101-…` (zephyr names it in its own SPP shell help), and the rows this
//!   machine has answer with other profiles for other things — `110B` audio, `110E` AVRCP, `111E`
//!   hands-free — none of which carries a stream.
//!
//! ## What upstream knows and this does not carry
//!
//! `CommsBLE.cs` reaches peripherals through GATT with a runtime library (`simpleble-c.dll`), which this
//! project does not carry: the plugin sandbox is the one place a runtime dependency is allowed, so the GATT
//! half is not ported. Its naming rule, its scan window, and the two characteristics it writes and reads
//! through are recorded in `docs/PARITY.md` as the part that stays outside this crate. What is ported here is
//! the half that needs no runtime: the ports the machine already has, and the rules that say which of them
//! belong to Bluetooth.
//!
//! Two further differences, kept as differences rather than as gaps:
//!
//! * upstream carries a baud rate on its BLE class where there is no rate at all. Through a COM port the
//!   number reaches the driver, so an option that said nothing would be a lie; what it does here is tell the
//!   driver a rate, which the medium does not use.
//! * `toggleDTR()` upstream is empty because that class has no line. A COM port has one, so `dtr` and `nodtr`
//!   mean what they say here, with the house default still low.

use std::fmt;

use async_trait::async_trait;

use crate::serial::SerialTransport;
use crate::{Transport, TransportKind};

/// The class name a Bluetooth device says it is in, from `tdibth.inf` read on this machine — which agrees with the
/// name Microsoft's own enumeration sample uses as the Bluetooth protocol id
/// (`System.Devices.Aep.ProtocolId`). Two implementations that were not built from each other, so the comparison is
/// read as data in the live area rather than written twice here. It is not what decides that a row of the map is a
/// serial channel: the row itself says the Ports class, so membership comes from the service the install bound the
/// instance to.
pub const BLUETOOTH_CLASS: &str = "e0cbf06c-cd8b-4647-bb8a-263b43f0f974";

/// The class a serial row says it is in, which is the Ports class. This machine answers with it for both of its
/// RFCOMM rows — the one that names a device and the one that names nothing — so a rule that decided Bluetooth
/// membership by the class would say nothing about the ports it was asked about.
pub const PORTS_CLASS: &str = "4d36e978-e325-11ce-bfc1-08002be10318";

/// The class the hands-free row says it is in, with a different service behind it (`BthHFEnum`). It is kept beside
/// the Ports class because the two are what says a profile is not a stream: the same enumerator, two classes, two
/// services, and only one of them carries bytes.
pub const AUDIO_CLASS: &str = "4d36e97d-e325-11ce-bfc1-08002be10318";

/// The class a network row says it is in. This machine answers with it for its `MS_BTHPAN` rows, which is what
/// makes "is this row Bluetooth" a reading per row rather than a reading of the folder: a rule that decided by
/// folder would call a personal-area-network row a serial row.
pub const NETWORK_CLASS: &str = "4d36e972-e325-11ce-bfc1-08002be10318";

/// The Serial Port profile, from the published number zephyr writes in its SPP shell help. Only a row that
/// carries it carries a stream.
pub const SERIAL_PROFILE: &str = "00001101-0000-1000-8000-00805f9b34fb";

/// The service a serial row was bound to, as the published driver lists write it. It is the fact the prefix in the
/// map stands on, and it is a different fact from the class name — as `SPDRP_DRIVER` names the class-install key
/// while `SPDRP_SERVICE` names the service behind it, so a key can never be the name a rule compares with.
pub const RFCOMM_SERVICE: &str = "BTHMODEM";

/// The prefix the RFCOMM service writes for a device in the map. The published driver lists write the service
/// name in capitals (`BTHMODEM`) while this machine writes the device name as `\Device\BthModem0`, so the rule
/// compares the words rather than the spelling — one fact, two spellings, as with `SPDRP_DRIVER` and
/// `SPDRP_SERVICE`.
pub const RFCOMM_PREFIX: &str = "BthModem";

/// The enumerator a published reader addresses these rows through (`BthPS3` and BlueGauge both name rows by
/// what starts with it).
pub const BTH_ENUMERATOR: &str = "BTHENUM";

/// A failure at the rules of this module.
///
/// The code is stable, as the codes of the device layer are, because the interface branches on it and never
/// reads a message for meaning.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BluetoothError {
    /// The text is not the shape this rule reads. Nothing is guessed: a port value that is not a `COM` number,
    /// or a row that does not carry the marker its profile lives beside, is said as the shape it has rather
    /// than turned into one.
    #[error("the row {row} is not the shape this rule reads: {reason}")]
    NotTheShape {
        /// The text the reading was given.
        row: String,
        /// What the reading could not find in it.
        reason: String,
    },

    /// The row is there and is the shape, but nothing behind it was decided: the group that would name a
    /// device is all zeros. That is a different fact from a row that is not the shape — one is a row the
    /// machine presents with no device behind it, the other is text a reader cannot read.
    #[error("the row {row} names no device: {reason}")]
    NoDeviceNamed {
        /// The row the reading was given.
        row: String,
        /// Which part of the row says nothing, and what it says.
        reason: String,
    },
}

impl BluetoothError {
    /// The stable code the frontend branches on.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotTheShape { .. } => "bluetooth_row_not_the_shape",
            Self::NoDeviceNamed { .. } => "bluetooth_no_device_named",
        }
    }
}

/// A group of zeros names nothing: it is what an install leaves where nothing was written, not a value the
/// machine decided. This machine shows both halves of one row at once — `LOCALMFG&0000` beside a group of
/// twelve zeros — so the pair is held as data and refused rather than read as a device.
#[must_use]
pub fn names_nothing(group: &str) -> bool {
    !group.is_empty() && group.chars().all(|c| c == '0')
}

/// One row of the map the machine writes for serial ports: the device name and the port it answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapRow {
    /// The device name as the machine writes it, for example `\Device\BthModem0`.
    pub device_name: String,
    /// The port, in the shape the map carries it (`COM40`).
    pub port: String,
}

impl MapRow {
    /// Reads a pair as the map writes it.
    ///
    /// # Errors
    ///
    /// Returns [`BluetoothError::NotTheShape`] when the value is not a `COM` number, which is what Microsoft's
    /// own reader returns for these rows. A value that is not the shape is refused rather than made into a
    /// port number by dropping letters nobody wrote.
    pub fn read(device_name: &str, value: &str) -> Result<Self, BluetoothError> {
        let value = value.trim();

        if value.len() < 4 || !value.to_ascii_uppercase().starts_with("COM") {
            return Err(BluetoothError::NotTheShape {
                row: format!("{device_name} = {value}"),
                reason: "the value is not a COM number".to_owned(),
            });
        }

        if !value[3..].chars().all(|c| c.is_ascii_digit()) {
            let rest = &value[3..];
            return Err(BluetoothError::NotTheShape {
                row: format!("{device_name} = {value}"),
                reason: format!("the part after COM says {rest}, which is not digits"),
            });
        }

        Ok(Self {
            device_name: device_name.trim().to_owned(),
            port: value.to_ascii_uppercase(),
        })
    }

    /// The part of the device name the service's prefix stands on, as the map writes it.
    fn name_part(&self) -> &str {
        let name = self.device_name.as_str();

        match name.rfind(['\\', '/']) {
            Some(last) => &name[last + 1..],
            None => name,
        }
    }

    /// Whether this row belongs to Bluetooth through the prefix the service writes.
    ///
    /// The whole prefix is compared, not a word inside the name: a virtual pair whose device name happens to
    /// say Bluetooth is not an RFCOMM channel, and the looser comparison SuperCollider carries would read it
    /// as one.
    #[must_use]
    pub fn belongs_to_bluetooth(&self) -> bool {
        self.name_part()
            .to_ascii_lowercase()
            .starts_with(&RFCOMM_PREFIX.to_ascii_lowercase())
    }

    /// The number the prefix carries, kept as data.
    ///
    /// A name with no digits after the prefix has no number; it is not read as zero, because that would be a
    /// number the reader invented.
    #[must_use]
    pub fn instance_number(&self) -> Option<u32> {
        let tail = &self.name_part()[RFCOMM_PREFIX.len()..];
        if tail.is_empty() || !tail.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        tail.parse().ok()
    }
}

/// What a row of the Bluetooth enumerator says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumRow {
    /// The text the reading was given, kept so a finding can name the row it came from.
    pub source: String,
    /// The profile inside the braces, or nothing when the name carries no braces.
    pub profile: Option<String>,
    /// The manufacturer part as the row writes it, with the group of zeros said as nothing decided rather than
    /// as a value.
    pub manufacturer: Option<String>,
    /// The instance part, when the row has one — the published hardware id does not, which is a fact about what
    /// a hardware id is and not a field the reader lost.
    pub instance_part: Option<String>,
    /// The device the row names.
    pub device: Option<String>,
}

impl EnumRow {
    /// Reads a row as a published reader addresses it. The two shapes this rule was written against are
    /// `BthPS3`'s hardware id and BlueGauge's full instance path.
    ///
    /// # Errors
    ///
    /// Returns [`BluetoothError::NotTheShape`] when the first part is not the enumerator these rows live under,
    /// or when the group that would name a device is not twelve hexadecimal digits; and
    /// [`BluetoothError::NoDeviceNamed`] when that group is the group of zeros.
    pub fn read(input: &str) -> Result<Self, BluetoothError> {
        let parts: Vec<&str> = input.split('\\').collect();

        let said = parts.first().copied().unwrap_or("");

        if !said.eq_ignore_ascii_case(BTH_ENUMERATOR) {
            return Err(BluetoothError::NotTheShape {
                row: input.to_owned(),
                reason: format!(
                    "the first part says {said}, which is not the enumerator these rows live under"
                ),
            });
        }

        let declared = parts.get(1).copied().unwrap_or("");

        if !declared.is_empty() && !declared.contains("LOCALMFG&") {
            return Err(BluetoothError::NotTheShape {
                row: input.to_owned(),
                reason: format!(
                    "the second part says {declared}, which does not carry the marker these rows write"
                ),
            });
        }

        let instance_part = parts.get(2).copied();

        Ok(Self {
            source: input.to_owned(),
            profile: declared
                .split('_')
                .next()
                .and_then(|name| name.strip_prefix('{')?.strip_suffix('}'))
                .map(|name| name.to_ascii_lowercase()),
            manufacturer: declared.split("LOCALMFG&").nth(1).and_then(|group| {
                if names_nothing(group) {
                    None
                } else {
                    Some(group.to_ascii_uppercase())
                }
            }),
            instance_part: instance_part.map(String::from),
            device: named_device(instance_part, input)?,
        })
    }

    /// Whether this row carries the profile that carries a stream.
    #[must_use]
    pub fn is_serial_row(&self) -> bool {
        self.profile.as_deref() == Some(SERIAL_PROFILE)
    }
}

/// The device a row names, read from the last group of its instance part.
fn named_device(instance_part: Option<&str>, row: &str) -> Result<Option<String>, BluetoothError> {
    let Some(part) = instance_part else {
        return Ok(None);
    };

    let tail = part.rsplit('&').next().unwrap_or("");
    let group = tail.split('_').next().unwrap_or("");

    if group.is_empty() {
        return Err(BluetoothError::NotTheShape {
            row: row.to_owned(),
            reason: format!("the instance part says {part}, which has no group for a device"),
        });
    }

    if group.len() != 12 || !group.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BluetoothError::NotTheShape {
            row: row.to_owned(),
            reason: format!("the group says {group}, which is not twelve hexadecimal digits"),
        });
    }

    if names_nothing(group) {
        return Err(BluetoothError::NoDeviceNamed {
            row: row.to_owned(),
            reason: format!("the group that would name a device says {group}"),
        });
    }

    Ok(Some(group.to_ascii_uppercase()))
}

/// The form upstream writes an address in without its separators, from its own rule
/// (`peripheral_address.Replace(":", "")`).
#[must_use]
pub fn address_without_colons(address: &str) -> String {
    address.replace(':', "")
}

/// The same address read with its separators, so that the two spellings are one fact and not a table.
/// Round-tripping is what says the reading did not lose a digit.
///
/// # Errors
///
/// Returns [`BluetoothError::NotTheShape`] when the text is not twelve hexadecimal digits, because six groups of
/// two cannot be worked out of something else.
pub fn address_with_colons(address: &str) -> Result<String, BluetoothError> {
    let plain = address_without_colons(address);

    if plain.len() != 12 || !plain.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BluetoothError::NotTheShape {
            row: address.to_owned(),
            reason: format!("the address says {address}, which is not twelve hexadecimal digits"),
        });
    }

    let groups: Vec<String> = plain
        .as_bytes()
        .chunks(2)
        .map(|pair| String::from_utf8_lossy(pair).to_ascii_uppercase())
        .collect();

    Ok(groups.join(":"))
}

/// The name upstream gives a port it found through its own filter. It is kept as data here because through a
/// COM port the machine names the port itself, so this rule only says what a reader that had the GATT path
/// would have written for the same peripheral.
///
/// # Errors
///
/// Returns [`BluetoothError::NotTheShape`] when there is no identifier to put in the name, because a name built
/// from nothing would look like a name the peripheral said.
pub fn upstream_port_label(identifier: &str, address: &str) -> Result<String, BluetoothError> {
    if identifier.trim().is_empty() {
        return Err(BluetoothError::NotTheShape {
            row: format!("{identifier}/{address}"),
            reason: "there is no identifier to put in the name".to_owned(),
        });
    }

    let plain = address_without_colons(address);
    if plain.len() != 12 || !plain.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BluetoothError::NotTheShape {
            row: format!("{identifier}/{address}"),
            reason: format!("the address says {address}, which is not twelve hexadecimal digits"),
        });
    }

    Ok(format!("BLE_{identifier}_{}", plain.to_ascii_uppercase()))
}

/// Whether a service name is the one a serial row was bound to, read through the words rather than through the
/// spelling: the published driver lists write `BTHMODEM` in capitals and this machine writes it in capitals too,
/// while its hands-free row answers with a different service (`BthHFEnum`) behind a different class. The whole word
/// is compared, not a part of it, because a rule that matched a substring would read a service named with the same
/// letters inside itself as the RFCOMM one.
#[must_use]
pub fn service_is_rfcomm(name: &str) -> bool {
    name.trim().eq_ignore_ascii_case(RFCOMM_SERVICE)
}

/// Whether a class name is the one a row says it is in, read through the words rather than through the
/// spelling: `tdibth.inf` writes it in lowercase inside braces and this machine writes the rows in capitals.
#[must_use]
pub fn belongs_to_class(name: &str, class: &str) -> bool {
    let shape = name
        .strip_prefix('{')
        .unwrap_or(name)
        .strip_suffix('}')
        .unwrap_or(name);

    shape.eq_ignore_ascii_case(class)
}

/// A Bluetooth serial channel: bytes moved by the transport that was opened for the port, with the reading the
/// port was opened against. When nothing was read the row is `None`, and `describe` says that as the fact it
/// is rather than naming a device nobody knows.
pub struct BluetoothTransport {
    link: Box<dyn Transport>,
    row: Option<MapRow>,
    named: Option<String>,
    /// The rate the driver was given. On this medium the peer hears no rate, so the number is said as what the
    /// driver was told, not as what the vehicle talks at.
    driver_rate: u32,
}

impl BluetoothTransport {
    /// Opens the port a row names, which is what Microsoft's own reader would open for the same pair.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be opened: it does not exist, it is held by another process, or
    /// the driver will not take the rate it is given.
    pub fn open(row: &MapRow, driver_rate: u32, dtr_on_open: bool) -> std::io::Result<Self> {
        let inner = SerialTransport::open(&row.port, driver_rate, dtr_on_open)?;
        Ok(Self {
            link: Box::new(inner),
            row: Some(row.clone()),
            named: None,
            driver_rate,
        })
    }

    /// Opens a port as it was typed, with no row read against it.
    pub fn open_as_typed(port: &str, driver_rate: u32, dtr_on_open: bool) -> std::io::Result<Self> {
        let inner = SerialTransport::open(port, driver_rate, dtr_on_open)?;
        Ok(Self {
            link: Box::new(inner),
            row: None,
            named: None,
            driver_rate,
        })
    }

    /// Puts a channel around a transport that was already opened, with what the reading said about it. This is
    /// what the scripted peer answers through, and what the live area gives back for a row this machine has.
    pub fn wrap(
        link: Box<dyn Transport>,
        row: Option<MapRow>,
        named: Option<String>,
        driver_rate: u32,
    ) -> Self {
        Self {
            link,
            row,
            named,
            driver_rate,
        }
    }

    /// The row the port was opened against, if anything was read.
    #[must_use]
    pub fn row(&self) -> Option<&MapRow> {
        self.row.as_ref()
    }

    /// The device the row names, when a row named one.
    #[must_use]
    pub fn device(&self) -> Option<&str> {
        self.named.as_deref()
    }
}

#[async_trait]
impl Transport for BluetoothTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.link.read(buf).await
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.link.write_some(buf).await
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Bluetooth
    }

    fn is_open(&self) -> bool {
        self.link.is_open()
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        self.link.reconnect().await
    }

    fn describe(&self) -> String {
        match &self.row {
            Some(row) => {
                let said = self
                    .named
                    .as_deref()
                    .map(|device| format!("the device at {device}"))
                    .unwrap_or_else(|| "its row names no device".to_owned());
                format!(
                    "bluetooth {} ({}), {said}, the rate {} is what the driver was given and not what the \
                     medium uses",
                    row.port, row.device_name, self.driver_rate
                )
            }
            None => format!(
                "bluetooth {} (opened as typed, no row read), the rate {} is what the driver was given and \
                 not what the medium uses",
                self.link.describe(),
                self.driver_rate
            ),
        }
    }
}

impl fmt::Debug for BluetoothTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BluetoothTransport")
            .field("row", &self.row)
            .field("named", &self.named)
            .field("driver_rate", &self.driver_rate)
            .field("open", &self.link.is_open())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_of_the_map_is_read_as_the_pair_it_carries() {
        // Three pairs written from the shape Microsoft's own reader reads these rows through and SuperCollider's
        // comment shows: the device name, then the port it answers with.
        let pairs = [
            ("\\Device\\Serial0", "COM3"),
            ("\\Device\\USBSER000", "COM5"),
            ("\\Device\\BthModem7", "COM9"),
        ];

        for (device_name, value) in pairs {
            let row =
                MapRow::read(device_name, value).expect("the pair is the shape the map writes");
            assert_eq!(row.port, value);
        }

        // A value that is not a COM number is refused rather than turned into one by dropping letters.
        assert_eq!(
            MapRow::read("\\Device\\LSIModem5", "MODEM5"),
            Err(BluetoothError::NotTheShape {
                row: "\\Device\\LSIModem5 = MODEM5".to_owned(),
                reason: "the value is not a COM number".to_owned(),
            })
        );
        assert!(matches!(
            MapRow::read("\\Device\\BthModem1", "COM4X"),
            Err(BluetoothError::NotTheShape { .. })
        ));
    }

    #[test]
    fn a_row_belongs_to_bluetooth_through_the_prefix_and_not_through_a_word_inside_the_name() {
        // The looser comparison SuperCollider carries matches any port name, so these two have to read as they
        // are: one is a virtual pair named with the word, the other is the prefix the service writes.
        let bluetooth = MapRow::read("\\Device\\BthModem0", "COM40").unwrap();
        assert!(bluetooth.belongs_to_bluetooth());
        assert_eq!(bluetooth.instance_number(), Some(0));

        let named_with_the_word = MapRow::read("\\Device\\com0comBluetooth0", "COM18").unwrap();
        assert!(
            !named_with_the_word.belongs_to_bluetooth(),
            "a virtual pair that says Bluetooth inside its name is not an RFCOMM channel"
        );

        // The published driver lists write the service in capitals and this machine writes the device name in
        // mixed case, so the rule reads the words rather than the spelling.
        let capitals = MapRow::read("\\device\\BTHMODEM2", "COM41").unwrap();
        assert!(capitals.belongs_to_bluetooth());

        // A number is only data when the name carries one; nothing here is read as zero.
        let no_number = MapRow::read("\\Device\\BthModemX", "COM42").unwrap();
        assert_eq!(no_number.instance_number(), None);
    }

    #[test]
    fn the_instance_shape_a_published_reader_addresses_is_read_as_data() {
        // `BthPS3`'s published hardware id: a declared name with no instance part, which is what a hardware id
        // is. The zeros are said as nothing decided.
        let declared =
            EnumRow::read("BTHENUM\\{1cb831ea-79cd-4508-b0fc-85f7c85ae8e0}_LOCALMFG&0000").unwrap();
        assert_eq!(
            declared.profile.as_deref(),
            Some("1cb831ea-79cd-4508-b0fc-85f7c85ae8e0")
        );
        assert_eq!(declared.manufacturer, None);
        assert_eq!(declared.instance_part, None);
        assert_eq!(
            declared.device, None,
            "there is no instance part to read a device from"
        );
        assert!(!declared.is_serial_row());

        // BlueGauge's full instance path, which is a hands-free row: the profile says what kind of row it is,
        // and a rule that read every row as a serial row would put bytes on it.
        let hands_free = EnumRow::read(
            "BTHENUM\\{0000111E-0000-1000-8000-00805F9B34FB}_HCIBYPASS_LOCALMFG&001D\\\
             5&1CD044E&0&B0A3F27C9EA3_C00000000",
        )
        .expect("the row is the shape these readers address");
        assert_eq!(hands_free.manufacturer.as_deref(), Some("001D"));
        assert_eq!(hands_free.device.as_deref(), Some("B0A3F27C9EA3"));
        assert!(
            !hands_free.is_serial_row(),
            "111E is the hands-free profile, not the one that carries a stream"
        );
    }

    #[test]
    fn twelve_zeros_are_not_read_as_a_device() {
        // This machine answers with both halves of one row at once: `LOCALMFG&0000` beside a group of twelve
        // zeros for the row that gives COM41. Windows presents the row and nothing behind it was decided, so
        // the reading refuses rather than inventing an address — the same reason a half of 0 is refused in the
        // settings, because zero is not a board.
        let zeros = EnumRow::read(
            "BTHENUM\\{00001101-0000-1000-8000-00805F9B34FB}_LOCALMFG&0000\\\
             B&1DB17D3F&0&000000000000_00000000",
        )
        .unwrap_err();
        assert_eq!(zeros.code(), "bluetooth_no_device_named");

        // The same shape with a device answers as the pair this machine holds for the module on COM40.
        let named = EnumRow::read(
            "BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7\\\
             B&1DB17D3F&0&0424051406B2_C00000000",
        )
        .expect("the row is the shape these readers address");
        assert!(named.is_serial_row());
        assert_eq!(named.device.as_deref(), Some("0424051406B2"));
        assert_eq!(named.manufacturer.as_deref(), Some("08E7"));

        // A group that is not twelve digits has no address, and is said as the shape it has.
        assert!(matches!(
            EnumRow::read(
                "BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7\\5&1&0&ABC_C0"
            ),
            Err(BluetoothError::NotTheShape { .. })
        ));
    }

    #[test]
    fn one_class_name_read_two_ways_and_a_row_is_not_named_by_its_folder() {
        // `tdibth.inf` on this machine writes the class in lowercase inside braces; the rows the machine holds
        // write it in capitals. The rule compares the words, so both spellings are the same fact.
        assert!(belongs_to_class(
            "{E0CBF06C-CD8B-4647-BB8A-263B43F0F974}",
            BLUETOOTH_CLASS
        ));
        assert!(belongs_to_class(BLUETOOTH_CLASS, BLUETOOTH_CLASS));

        // The personal-area-network rows on this machine answer with the network class. A rule that decided by
        // folder would call them Bluetooth serial rows.
        assert!(!belongs_to_class(
            "{4D36E972-E325-11CE-BFC1-08002BE10318}",
            BLUETOOTH_CLASS
        ));
        assert!(belongs_to_class(NETWORK_CLASS, NETWORK_CLASS));

        // Membership comes from the service the install bound the instance to, not from the class the row says it
        // is in: this machine answers its two RFCOMM rows as Ports rows, and the hands-free row as an audio row
        // with another service behind it. A rule that decided by class would say nothing about the ports it was
        // asked about, which is why these two facts are kept apart rather than merged into one.
        assert!(service_is_rfcomm("BTHMODEM"));
        assert!(service_is_rfcomm("bthmodem"));
        assert!(!service_is_rfcomm("BthHFEnum"));
        assert!(belongs_to_class(
            "{4D36E978-E325-11CE-BFC1-08002BE10318}",
            PORTS_CLASS
        ));
        assert!(!belongs_to_class(
            "{4D36E978-E325-11CE-BFC1-08002BE10318}",
            BLUETOOTH_CLASS
        ));
        assert!(belongs_to_class(
            "{4D36E97D-E325-11CE-BFC1-08002BE10318}",
            AUDIO_CLASS
        ));

        // A substring is not a service name: a service named with the same letters inside itself is a different
        // binding, so the whole word is compared.
        assert!(!service_is_rfcomm("BTHMODEM2"));
    }

    #[test]
    fn two_spellings_of_an_address_are_one_fact_and_round_trip() {
        // The name this machine writes for its paired peripheral, and the form upstream's rule builds from it.
        let address = "00:1B:10:62:C1:3C";
        assert_eq!(address_without_colons(address), "001B1062C13C");
        assert_eq!(address_with_colons("001B1062C13C").unwrap(), address);
        assert_eq!(
            upstream_port_label("Makeblock_LE001b1062c13c", address).unwrap(),
            "BLE_Makeblock_LE001b1062c13c_001B1062C13C"
        );

        // The same rule cannot be built from nothing, and an address that is not twelve digits cannot be split
        // into six groups of two.
        assert_eq!(
            upstream_port_label("", "001B1062C13C"),
            Err(BluetoothError::NotTheShape {
                row: "/001B1062C13C".to_owned(),
                reason: "there is no identifier to put in the name".to_owned(),
            })
        );
        assert!(matches!(
            address_with_colons("001B1062C1"),
            Err(BluetoothError::NotTheShape { .. })
        ));
    }

    #[test]
    fn a_row_that_is_not_the_shape_is_said_as_the_shape_it_has() {
        // A row written without the enumerator these readers address would make a reader invent one.
        assert_eq!(
            EnumRow::read("USB\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7"),
            Err(BluetoothError::NotTheShape {
                row: "USB\\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7".to_owned(),
                reason:
                    "the first part says USB, which is not the enumerator these rows live under"
                        .to_owned(),
            })
        );

        // And a declared name that does not carry the marker is not read as if it did.
        assert!(matches!(
            EnumRow::read("BTHENUM\\{00001101-0000-1000-8000-00805f9b34fb}_SOMETHING"),
            Err(BluetoothError::NotTheShape { .. })
        ));
    }

    #[test]
    fn describe_says_which_reading_the_port_was_opened_against() {
        // The strings are written down before the rule is called, which is what says they came from the reading
        // rather than from a form somebody chose.
        let row = MapRow::read("\\Device\\BthModem0", "COM40").unwrap();
        let read = BluetoothTransport::wrap(
            Box::new(ClosedChannel),
            Some(row.clone()),
            Some("0424051406B2".to_owned()),
            9600,
        );
        assert_eq!(read.kind(), TransportKind::Bluetooth);
        assert_eq!(
            read.describe(),
            "bluetooth COM40 (\\Device\\BthModem0), the device at 0424051406B2, the rate 9600 is what the \
             driver was given and not what the medium uses"
        );
        assert_eq!(read.row(), Some(&row));
        assert_eq!(read.device(), Some("0424051406B2"));

        // A row that names nothing is said as such, beside the port it presents.
        let unnamed =
            BluetoothTransport::wrap(Box::new(ClosedChannel), Some(row.clone()), None, 9600);
        assert_eq!(
            unnamed.describe(),
            "bluetooth COM40 (\\Device\\BthModem0), its row names no device, the rate 9600 is what the driver \
             was given and not what the medium uses"
        );

        // With nothing read there is nothing to name, and saying that is the honest thing.
        let typed = BluetoothTransport::wrap(Box::new(ClosedChannel), None, None, 9600);
        assert_eq!(
            typed.describe(),
            "bluetooth closed channel (opened as typed, no row read), the rate 9600 is what the driver was \
             given and not what the medium uses"
        );
    }

    /// A channel that answers with nothing, which is what a scripted peer is for these rules.
    struct ClosedChannel;

    #[async_trait]
    impl Transport for ClosedChannel {
        async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }

        async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            Ok(buf.len())
        }

        fn kind(&self) -> TransportKind {
            TransportKind::Serial
        }

        fn is_open(&self) -> bool {
            false
        }

        async fn reconnect(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn describe(&self) -> String {
            "closed channel".to_owned()
        }
    }
}
