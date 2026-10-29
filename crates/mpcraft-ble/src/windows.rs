//! The Windows GATT path, through `btleplug`.

use std::time::Duration;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use mpcraft_transport::{Transport, TransportKind};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    BleDevice, BleError, HM10_CHARACTERISTIC, HM10_SERVICE, NUS_NOTIFY, NUS_SERVICE, NUS_WRITE,
    SCAN_TIME,
};

/// The number of bytes written in one GATT request.
///
/// A module's maximum is negotiated during discovery and is at least twenty bytes; twenty keeps the
/// first request inside that floor, and the link carries the rest of a frame in the next ones.
const WRITE_CHUNK: usize = 20;

/// The address of the device as the platform writes it, with or without the `BLE_` prefix Mission
/// Planner builds.
fn wanted(address: &str) -> String {
    let text = address.trim();

    if let Some(rest) = text.strip_prefix("BLE_") {
        if let Some((_, address)) = rest.rsplit_once('_') {
            return address.to_ascii_uppercase();
        }
    }

    text.to_ascii_uppercase()
}

/// Finds the adapter and starts a scan.
async fn adapter() -> Result<Adapter, BleError> {
    let manager = Manager::new()
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;
    let adapters = manager
        .adapters()
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    adapters.into_iter().next().ok_or(BleError::NoAdapter)
}

/// The profile a set of services and characteristics carries.
///
/// The write side and the notify side are kept apart: on the Nordic UART they are two
/// characteristics, and on the HM-10 they are the same one.
fn profile(
    _services: &[Uuid],
    characteristics: &[Characteristic],
) -> Option<(Characteristic, Option<Characteristic>)> {
    let nus_write = Uuid::parse_str(NUS_WRITE).ok()?;
    let nus_notify = Uuid::parse_str(NUS_NOTIFY).ok()?;

    if let (Some(write), Some(notify)) = (
        characteristics
            .iter()
            .find(|c| c.uuid == nus_write)
            .cloned(),
        characteristics
            .iter()
            .find(|c| c.uuid == nus_notify)
            .cloned(),
    ) {
        return Some((write, Some(notify)));
    }

    let hm10 = Uuid::parse_str(HM10_CHARACTERISTIC).ok()?;

    if let Some(write) = characteristics
        .iter()
        .find(|c| {
            c.uuid == hm10
                && (c.properties.contains(CharPropFlags::WRITE)
                    || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
        })
        .cloned()
    {
        let notify = write.properties.contains(CharPropFlags::NOTIFY);

        return Some((write.clone(), notify.then_some(write)));
    }

    None
}

/// Whether the device advertises a serial profile.
fn advertised_profile(services: &[Uuid]) -> &'static str {
    let nus = Uuid::parse_str(NUS_SERVICE).ok();
    let hm10_service = Uuid::parse_str(HM10_SERVICE).ok();
    let hm10_char = Uuid::parse_str(HM10_CHARACTERISTIC).ok();

    if nus.is_some_and(|nus| services.contains(&nus)) {
        "nus"
    } else if hm10_service.is_some_and(|service| services.contains(&service))
        || hm10_char.is_some_and(|characteristic| services.contains(&characteristic))
    {
        "ffe1"
    } else {
        "unknown"
    }
}

/// The devices that advertise a serial profile, or that have a name.
pub async fn scan(timeout: Duration) -> Result<Vec<BleDevice>, BleError> {
    let adapter = adapter().await?;

    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    tokio::time::sleep(if timeout.is_zero() {
        SCAN_TIME
    } else {
        timeout
    })
    .await;

    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    let _ = adapter.stop_scan().await;

    let mut found = Vec::new();

    for peripheral in peripherals {
        let Ok(Some(properties)) = peripheral.properties().await else {
            continue;
        };

        let name = properties
            .local_name
            .or(properties.advertisement_name)
            .unwrap_or_default();

        let profile = advertised_profile(&properties.services).to_owned();

        // A device with no name is still listed when it carries one of the serial profiles, because
        // the phone that advertises a test service does not always write a local name; an unnamed
        // device with no profile is not something this station can use.
        if name.is_empty() && profile == "unknown" {
            continue;
        }

        found.push(BleDevice {
            address: properties.address.to_string(),
            name,
            profile,
        });
    }

    found.sort_by(|left, right| left.name.cmp(&right.name));
    found.dedup_by(|left, right| left.address == right.address);

    Ok(found)
}

/// Opens the device whose address is given.
pub async fn open(address: &str) -> Result<Box<dyn Transport>, BleError> {
    let adapter = adapter().await?;
    let address = wanted(address);

    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    let mut peripheral: Option<Peripheral> = None;

    for _ in 0..50 {
        for candidate in adapter
            .peripherals()
            .await
            .map_err(|error| BleError::Platform(error.to_string()))?
        {
            let Ok(Some(properties)) = candidate.properties().await else {
                continue;
            };

            if properties.address.to_string().to_ascii_uppercase() == address {
                peripheral = Some(candidate);
                break;
            }
        }

        if peripheral.is_some() {
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let peripheral = peripheral.ok_or_else(|| BleError::NotFound(address.clone()))?;

    peripheral
        .connect_with_timeout(Duration::from_secs(10))
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;
    peripheral
        .discover_services()
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    let _ = adapter.stop_scan().await;
    let characteristics: Vec<Characteristic> = peripheral.characteristics().into_iter().collect();
    let services: Vec<Uuid> = peripheral.services().into_iter().map(|s| s.uuid).collect();

    let (write, notify) =
        profile(&services, &characteristics).ok_or_else(|| BleError::NoProfile(address.clone()))?;

    if let Some(notify) = notify {
        peripheral
            .subscribe(&notify)
            .await
            .map_err(|error| BleError::Platform(error.to_string()))?;
    }

    let write_type = if write
        .properties
        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
    {
        WriteType::WithoutResponse
    } else {
        WriteType::WithResponse
    };

    let mut notifications = peripheral
        .notifications()
        .await
        .map_err(|error| BleError::Platform(error.to_string()))?;

    let (sender, receiver) = mpsc::channel(64);

    tokio::spawn(async move {
        while let Some(notification) = notifications.next().await {
            if sender.send(notification.value).await.is_err() {
                break;
            }
        }
    });

    let name = peripheral
        .properties()
        .await
        .ok()
        .flatten()
        .and_then(|properties| properties.local_name)
        .unwrap_or_default();

    Ok(Box::new(GattTransport {
        peripheral,
        write,
        write_type,
        receiver,
        pending: Vec::new(),
        connected: true,
        description: format!("{name} ({address})"),
    }))
}

/// A GATT serial link.
struct GattTransport {
    /// The device.
    peripheral: Peripheral,
    /// The characteristic the station writes to.
    write: Characteristic,
    /// Whether the module acknowledges the writes.
    write_type: WriteType,
    /// What the module notifies.
    receiver: mpsc::Receiver<Vec<u8>>,
    /// Bytes of the last notification that did not fit in the caller's buffer.
    pending: Vec<u8>,
    /// Whether the link is expected to answer.
    connected: bool,
    /// The device and address, shown by the interface.
    description: String,
}

#[async_trait::async_trait]
impl Transport for GattTransport {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if self.pending.is_empty() {
            match self.receiver.recv().await {
                Some(bytes) => self.pending = bytes,
                None => {
                    self.connected = false;

                    return Ok(0);
                }
            }
        }

        let count = buf.len().min(self.pending.len());
        buf[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);

        Ok(count)
    }

    async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let count = buf.len().min(WRITE_CHUNK);

        self.peripheral
            .write(&self.write, &buf[..count], self.write_type)
            .await
            .map_err(|error| {
                self.connected = false;

                std::io::Error::other(error.to_string())
            })?;

        Ok(count)
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Gatt
    }

    fn is_open(&self) -> bool {
        self.connected
    }

    async fn reconnect(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "a GATT device is opened again by the supervisor rather than reconnected here",
        ))
    }

    fn describe(&self) -> String {
        self.description.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn characteristic(uuid: &str, properties: CharPropFlags) -> Characteristic {
        let uuid = Uuid::parse_str(uuid).expect("a uuid");

        Characteristic {
            uuid,
            service_uuid: Uuid::nil(),
            properties,
            descriptors: BTreeSet::new(),
        }
    }

    #[test]
    fn the_nordic_uart_keeps_its_write_and_notify_apart() {
        let characteristics = vec![
            characteristic(NUS_WRITE, CharPropFlags::WRITE),
            characteristic(NUS_NOTIFY, CharPropFlags::NOTIFY),
        ];

        let (write, notify) = profile(&[], &characteristics).expect("the profile is read");

        assert_eq!(write.uuid, Uuid::parse_str(NUS_WRITE).unwrap());
        assert_eq!(
            notify.map(|characteristic| characteristic.uuid),
            Some(Uuid::parse_str(NUS_NOTIFY).unwrap())
        );
    }

    #[test]
    fn the_hm10_characteristic_is_written_and_notified_on_one_handle() {
        let characteristics = vec![characteristic(
            HM10_CHARACTERISTIC,
            CharPropFlags::WRITE | CharPropFlags::NOTIFY,
        )];

        let (write, notify) = profile(&[], &characteristics).expect("the profile is read");

        assert_eq!(write.uuid, Uuid::parse_str(HM10_CHARACTERISTIC).unwrap());
        assert_eq!(
            notify.map(|characteristic| characteristic.uuid),
            Some(Uuid::parse_str(HM10_CHARACTERISTIC).unwrap())
        );
    }

    #[test]
    fn a_device_without_a_serial_service_is_refused() {
        let characteristics = vec![characteristic(
            "0000180f-0000-1000-8000-00805f9b34fb",
            CharPropFlags::READ,
        )];

        assert!(profile(&[], &characteristics).is_none());
    }
}
