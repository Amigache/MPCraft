//! The Bluetooth LE adapter, read on the machine that has one.
//!
//! The unit tests carry the profile rule; this area reads what the adapter answers, which is what a
//! scripted peer cannot show. It is skipped unless `MPCRAFT_BLE_LIVE=1`, so a machine without an
//! adapter still runs the rest of the suite. With `MPCRAFT_BLE_ADDRESS` it also opens that device
//! and reports the serial profile it carries.

use std::time::Duration;

use mpcraft_ble::{open, scan};

#[tokio::test]
async fn the_adapter_lists_the_devices_it_can_see() {
    if std::env::var("MPCRAFT_BLE_LIVE").ok().as_deref() != Some("1") {
        println!(
            "MPCRAFT_BLE_LIVE is not 1, so no adapter is read; the profile rule is in the unit tests"
        );
        return;
    }

    let devices = scan(Duration::from_secs(6))
        .await
        .expect("the adapter answers");

    println!("{} devices with a name", devices.len());

    for device in &devices {
        println!("  {} {} ({})", device.address, device.name, device.profile);
        assert!(!device.address.is_empty(), "an address is not empty");
    }
}

#[tokio::test]
async fn the_named_device_opens_and_reports_its_profile() {
    if std::env::var("MPCRAFT_BLE_LIVE").ok().as_deref() != Some("1") {
        return;
    }

    let Some(address) = std::env::var("MPCRAFT_BLE_ADDRESS").ok() else {
        println!("MPCRAFT_BLE_ADDRESS does not name a device, so none is opened");
        return;
    };

    let mut transport = open(&address).await.expect("the device opens");

    println!("opened {}", transport.describe());
    assert!(transport.is_open());

    let mut buffer = [0_u8; 64];

    // A notification that never arrives is not a failure of opening: it says the device is quiet,
    // which is what a module with nothing to send does. The wait is bounded so the test ends.
    match tokio::time::timeout(Duration::from_secs(3), transport.read(&mut buffer)).await {
        Ok(Ok(count)) => println!("the device notified {count} bytes without being asked"),
        Ok(Err(error)) => println!("the device refused the read: {error}"),
        Err(_) => println!("the device was quiet"),
    }
}
