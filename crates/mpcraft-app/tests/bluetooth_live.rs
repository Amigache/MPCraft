//! What this machine answers about its Bluetooth serial ports, read as it answers rather than as one expected
//! outcome.
//!
//! `tests/bluetooth_peer.rs` asserts the rules against literals that published sources document, and a scripted
//! peer answers the transport contract there. What this area adds is the answer of the machine itself: the rows the
//! key `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` carries today, and what the registry says for the instance a serial row
//! was installed under. The numbers are attributed to the calls that answered them, through the dictionary those
//! calls answer from — which is the same reason a Config Manager 13 and a Win32 13 are two facts and not one number.
//!
//! ```text
//! MPCRAFT_BT_ROW="BTHENUM\{00001101-0000-1000-8000-00805f9b34fb}_LOCALMFG&08E7\B&1DB17D3F&0&0424051406B2_C00000000" \
//!   cargo test -p mpcraft-app --features hardware-tests --test bluetooth_live -- --test-threads=1
//! ```
//!
//! Nothing is opened on a port here: COM40 belongs to a module that may be using it, so the area reads only the map
//! and the registry. That is also the honest scope — bytes crossing a port would prove nothing about the rules this
//! file asserts. A second reader (`reg query`, kept outside the repository) is what showed what this machine answers
//! for the same keys, including the service a serial row was bound to, and the two readings agree.
//!
//! Two things are said rather than assumed. No peripheral on this machine advertises the Nordic UART service, so
//! upstream's enumeration filter has nothing to find and its rule stays outside this crate; and no runtime is
//! carried here, since the plugin sandbox is the one place a runtime dependency is allowed, which is why the GATT
//! half is not ported at all.

#![cfg(all(feature = "hardware-tests", windows))]

use mpcraft_transport::{
    address_with_colons, address_without_colons, belongs_to_class, service_is_rfcomm,
    upstream_port_label, EnumRow, MapRow, AUDIO_CLASS, BLUETOOTH_CLASS, BTH_ENUMERATOR,
    NETWORK_CLASS, PORTS_CLASS, RFCOMM_PREFIX, RFCOMM_SERVICE, SERIAL_PROFILE,
};
use mpcraft_winusb::{bus, dictionary_of, NO_MORE_ITEMS, WIN32_DICTIONARY};

/// The service upstream's own filter looks for (`CommsBLE.cs`). Nothing on this machine advertises it, so the number
/// is read as data in `docs/PARITY.md` and cannot be asserted against a device.
const NORDIC_UART_SERVICE: &str = "6e400001-b5a3-f393-e0a9-e50e24dcca9e";

/// The row the reading is asked against, named by the environment as the registry writes it. What is asserted below
/// is what that instance says about itself, not what a rule would guess from its name.
fn row() -> String {
    std::env::var("MPCRAFT_BT_ROW").expect("the gate names the row the reading is asked against")
}

#[test]
fn the_map_answers_rows_and_the_numbers_keep_the_calls_that_answered_them() {
    let readings = bus::map_rows();

    // Both calls answer through the Win32 dictionary, which is the one this binding names for them, and nothing here
    // writes to the key: reading it needs no elevation.
    assert_eq!(dictionary_of(bus::OPEN_CALL), WIN32_DICTIONARY);
    assert_eq!(dictionary_of(bus::LIST_CALL), WIN32_DICTIONARY);

    if readings.open_answer != 0 {
        println!(
            "{} refused the key {}: {}",
            bus::OPEN_CALL,
            readings.key,
            readings.open_answer
        );
        return;
    }

    println!(
        "the map answered {} rows; the loop ended with {} from {}",
        readings.rows.len(),
        readings.list_answer,
        bus::LIST_CALL,
    );

    // A list that stops is not a list that is whole, so the count does not stand alone: what ended it is kept beside
    // it, and a stop by the reading's own bound is said as a gap rather than read as a pass.
    assert!(readings.read_to_the_end || readings.cap_reached);

    if readings.cap_reached {
        println!(
            "  the reading stopped at its own bound ({})",
            bus::MAP_ROWS_CAP
        );
    } else {
        assert_eq!(
            readings.list_answer, NO_MORE_ITEMS,
            "the list was read to its end"
        );
    }

    let bluetooth_rows: Vec<&MapRow> = readings
        .rows
        .iter()
        .filter_map(|row| row.as_ref().ok())
        .filter(|row| row.belongs_to_bluetooth())
        .collect();

    println!(
        "  rows the prefix the service writes makes Bluetooth: {}",
        bluetooth_rows.len()
    );

    for row in &readings.rows {
        match row {
            Ok(pair) => println!(
                "  {} = {}{}",
                pair.device_name,
                pair.port,
                if pair.belongs_to_bluetooth() {
                    format!(
                        " (the number the prefix carries: {:?})",
                        pair.instance_number()
                    )
                } else {
                    " (not an RFCOMM channel)".to_owned()
                }
            ),
            Err(error) => {
                // A value the reader cannot read is said as the shape it has rather than turned into a port number by
                // dropping letters nobody wrote.
                assert!(matches!(
                    error.code(),
                    "bluetooth_row_not_the_shape" | "bluetooth_no_device_named"
                ));
                println!("  refusal: {error}");
            }
        }
    }

    // What this machine answered when these facts were written down: two rows carrying the prefix, one for a device
    // and one that names nothing. The comparison is held as data — a run can answer with either, or with neither.
    let recorded = [
        ("\\Device\\BthModem0", "COM40"),
        ("\\Device\\BthModem1", "COM41"),
    ];

    for (device_name, port) in recorded {
        let found = readings
            .rows
            .iter()
            .filter_map(|row| row.as_ref().ok())
            .find(|row| row.device_name.eq_ignore_ascii_case(device_name) && row.port == port);

        match found {
            Some(pair) => {
                assert!(
                    pair.belongs_to_bluetooth(),
                    "the whole prefix is what makes the row Bluetooth"
                );
                println!("  the recorded pair answers today: {device_name} = {port}");
            }
            None => println!(
                "  the recorded pair {device_name} = {port} is not in what the map answers today"
            ),
        }
    }

    // The looser comparison SuperCollider carries matches any port name, so a virtual pair named with the word would
    // be read as a Bluetooth row by it. This machine has such pairs in its map, and they read as what they are.
    let virtual_pair = MapRow::read("\\Device\\com0comBluetooth0", "COM18").unwrap();
    assert!(!virtual_pair.belongs_to_bluetooth());
    println!(
        "  shown against a name that only says the word inside itself: {}",
        virtual_pair.device_name
    );
}

#[test]
fn a_serial_row_says_the_ports_class_and_the_service_behind_it() {
    let instance = row();
    let shape = EnumRow::read(&instance)
        .expect("the row named by the environment is the shape these readers write");

    assert_eq!(shape.profile.as_deref(), Some(SERIAL_PROFILE));
    assert_eq!(BTH_ENUMERATOR, instance.split('\\').next().unwrap_or(""));

    println!("row: {instance}");
    println!("  device the row names: {:?}", shape.device);
    println!("  manufacturer group: {:?}", shape.manufacturer);

    // What the install wrote on the device key for this instance, read through the path as the registry stores it —
    // the same reading `interface_guid_value` uses. Reading needs no elevation, and nothing is written here.
    let class_said = bus::device_key_value(&instance, "ClassGUID");
    let service_said = bus::device_key_value(&instance, "Service");

    println!("  class the instance says it is in: {class_said:?}");
    println!("  service behind the class-install key: {service_said:?}");

    if let (Some(class), Some(service)) = (&class_said, &service_said) {
        // Membership comes from the service the install bound the instance to, not from the class the row names: a
        // serial row says the Ports class, while the Bluetooth name is what the adapter's own rows answer with. A
        // rule that decided by class would say nothing about the ports it was asked about, so the two facts are kept
        // apart. The published spelling is capitals and `tdibth.inf` writes the class in lowercase inside braces, so
        // each is compared through its words rather than through its casing.
        assert!(service_is_rfcomm(service));
        assert_eq!(
            service.to_ascii_uppercase(),
            RFCOMM_SERVICE.to_ascii_uppercase()
        );
        assert!(belongs_to_class(class, PORTS_CLASS));
        assert!(!belongs_to_class(class, BLUETOOTH_CLASS));
        println!("  so this row is a Ports row through its service, not through its class");
    } else {
        println!(
            "  the key answered with nothing, so the two spellings are not compared for this run"
        );
    }

    // The hands-free row on this machine answers another class with another service, which is what makes "is this a
    // channel bytes travel on" a reading per row rather than a reading of the folder.
    let hands_free = EnumRow::read(
        "BTHENUM\\{0000111E-0000-1000-8000-00805F9B34FB}_LOCALMFG&08E7\\\
         B&714B849&0&FC58FA11A9C5_C00000000",
    )
    .expect("the row is the shape these readers address");
    assert!(
        !hands_free.is_serial_row(),
        "111E is the hands-free profile"
    );
    println!(
        "  a hands-free row names the audio class {AUDIO_CLASS} and the network rows name {NETWORK_CLASS}"
    );

    // The interface list answers with what it answers today. Nothing was opened, so the bytes are reached through the
    // port the map names rather than through an interface — which is what Microsoft's own reader and SuperCollider
    // both do, since both list ports from the map.
    let rows =
        bus::rows_for_class(BLUETOOTH_CLASS, &instance).expect("the list answers as it says");
    println!(
        "  rows this instance publishes under the Bluetooth class: {}",
        rows.len()
    );

    // And the group of zeros that names nothing, kept as its own code because it says something different from a row
    // that is not the shape: the machine presents the row and nothing behind it was decided.
    let unnamed = EnumRow::read(
        "BTHENUM\\{00001101-0000-1000-8000-00805F9B34FB}_LOCALMFG&0000\\\
         B&1DB17D3F&0&000000000000_00000003",
    )
    .expect_err("the group that would name a device says nothing");
    assert_eq!(unnamed.code(), "bluetooth_no_device_named");
    println!("  a row that names nothing keeps its own code: {unnamed}");
}

#[test]
fn what_this_area_cannot_reach_is_said_rather_than_assumed() {
    // The peripheral paired on this machine answers with four services and none of them is the Nordic UART number
    // upstream filters on, so upstream's rule has nothing to find here. That is recorded as the reason its half stays
    // outside this crate rather than as a failure of this run.
    let address = "00:1B:10:62:C1:3C";
    assert_eq!(address_without_colons(address), "001B1062C13C");
    assert_eq!(
        address_with_colons("001B1062C13C").expect("twelve hexadecimal digits"),
        address
    );

    // The name upstream would have built for it, kept as data: through a COM port the machine names the port itself,
    // so this rule only says what a reader that had the GATT path would have written for the same peripheral.
    assert_eq!(
        upstream_port_label("Makeblock_LE001b1062c13c", address)
            .expect("an identifier and an address"),
        "BLE_Makeblock_LE001b1062c13c_001B1062C13C"
    );

    println!("upstream's filter looks for {NORDIC_UART_SERVICE}, which nothing on this machine advertises");
    println!("the two spellings of one address are read as one fact: {address} and 001B1062C13C");
    println!("the prefix the rule compares is {RFCOMM_PREFIX}; nothing was opened on a port by this area");

    // What cannot be asserted is said rather than assumed: no runtime is carried, so there is no scan, no peripheral
    // list, and no characteristic to write or read through.
    assert_eq!(RFCOMM_PREFIX.to_ascii_lowercase(), "bthmodem");
    println!("the half that scans and reads characteristics is not carried here, so nothing about a device is claimed");
}
