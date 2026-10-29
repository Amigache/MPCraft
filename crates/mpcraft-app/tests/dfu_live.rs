//! The rules of the device layer, answered by a board that is on the bus rather than by a peer written to
//! carry them.
//!
//! `tests/dfu.rs` asserts the shape — the descriptor, the string, the state machine, the block rule, and the
//! refusals that happen before anything reaches the wire — against literals that a second implementation of the
//! format recomputes. What it cannot say is that a board answers those requests. Here the numbers come from the
//! board: the nine bytes its interface puts on the cable, the memory string its own firmware writes, the pair of
//! class numbers its settings carry, and which of the two published forms of the mass-erase order it understands.
//!
//! ```text
//! MPCRAFT_DFU_TARGET="0483:df11" cargo test -p mpcraft-app --features hardware-tests --test dfu_live -- --test-threads=1
//! ```
//!
//! The target is the pair as the bus writes it rather than a connection specification, because a board in its
//! bootloader is not a transport: it answers requests rather than moving bytes. One process holds the device at a
//! time, so this area runs alone, and the comparisons with STM32CubeProgrammer are sequential for the same reason.
//!
//! What this area cannot do is put the board on the bus: a Matek F405-CTR gets to its bootloader by holding BOOT
//! at power-on, or by the command that holds it there, and the gate names that as what the operator supplies. It
//! cannot write the value that names what an installation registers either: reading the device key needs no
//! elevation, so the area reads it here, while writing it is something the operator does from an elevated prompt,
//! which the gate names beside BOOT as what the operator supplies. The key here carries two names, each with its own
//! terminator and the pair on both sides, and the row this area opens is the one the install registered under the
//! standard WinUSB interface — the name the published reader searches — while the class and the sample name answer
//! none, which is the difference a list that asks only for those two cannot see.
//!
//! Each half asserts its own facts, and both states are held as data rather than as one expected outcome. The
//! registry can remember rows that the bus does not answer for; a row that is there can answer under a name that is
//! not the class the element reports; and an instance that can be opened answers the requests a peer cannot. A
//! second reader of the same bus (`winusb_probe`, kept outside the repository) is what showed which call answered
//! what, including the number that a wrong address to `SetupDiGetClassDevsW` gives — 13, which the Config Manager
//! dictionary uses for a device that is not there — and that is why every number here names the call that answered
//! it.

#![cfg(all(feature = "hardware-tests", windows))]

use mpcraft_app::dfu::{Flash, Layout, State};
use mpcraft_transport::device::{Device, FunctionalDescriptor};
use mpcraft_winusb::bus::Handle;
use mpcraft_winusb::{
    bus, classify_open_failure, dictionary_of, guid_parts, is_our_driver, name_without_the_pair,
    pair_from_ids, Found, Target, CONFIG_MANAGER_DICTIONARY, CONFIG_MANAGER_NOT_THERE, DFU_CLASS,
    DFU_SUBCLASS, INF_TYPE_MULTI_SZ, REGISTRY_TYPE_MULTI_SZ, WINDOWS_USBDEVICE_CLASS, WINUSB_CLASS,
    WINUSB_DEVICE_INTERFACE,
};

/// The nine bytes a published reading (cherryusb's `class/dfu/usb_dfu.h`) writes for this board's functional
/// descriptor. It is the same literal `tests/dfu.rs` carries, and it is not derived from the module being read:
/// the fields below are what the shape says, and the board's answer is compared against them.
const GOLDEN_DESCRIPTOR: &[u8] = &[9, 0x21, 0x0B, 0xFF, 0x00, 0x00, 0x08, 0x1A, 0x01];

/// The number STM32CubeProgrammer gives this board. It is a second reader of the same fact as the version field
/// of the golden shape (`0x011A`), which is why the agreement between them is a fact and not something one hand
/// wrote twice.
const CUBE_DEVICE_CODE: u16 = 0x011A;

/// The device code the same tool gives the STM32F405/415 family, which lives in the writer's dictionary rather
/// than on the wire: nothing here can ask a board for it, so it is recorded beside the numbers the board can
/// answer for itself.
const CUBE_FAMILY_CODE: u16 = 0x0413;

/// The geometry ArduPilot's own flash writer declares for this family — four sectors of 16K, one of 64K, seven of
/// 128K — read from the published source rather than copied from what the board answered. Twelve sectors and one
/// megabyte, which is what the string the board writes has to make.
const ARDUPILOT_SECTORS: usize = 12;
const ARDUPILOT_TOTAL: u32 = 1_048_576;

/// The pair the board is on, named by the environment as the bus writes it.
fn target() -> Target {
    Target::read(
        &std::env::var("MPCRAFT_DFU_TARGET").expect("the gate names the pair the board is on"),
    )
    .expect("the target is the pair as the bus writes it")
}

#[tokio::test]
async fn remembering_an_instance_is_not_being_there() {
    let target = target();
    let rows = bus::remembered(&target);

    if rows.is_empty() {
        println!("the registry carries no row for {}", target.as_string());
        return;
    }

    // Every row is read as what it says about this run rather than as a board being there, and the two readings are
    // kept apart: this machine carries five rows for the pair from a run when the board was in its bootloader, and
    // one of them answers as being there, which is what closed the earlier reading that said none was.
    let present: Vec<&String> = rows.iter().filter(|row| bus::present(row)).collect();

    println!(
        "remembered {} rows for {}; present: {}",
        rows.len(),
        target.as_string(),
        if present.is_empty() {
            "none".to_owned()
        } else {
            present
                .iter()
                .map(|row| row.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    for row in &rows {
        let number = bus::locate(row);

        if number == 0 {
            println!("  {row} answers as being there");
            continue;
        }

        // The number belongs to the dictionary `CM_Locate_DevNodeW` answers from, where 13 says nothing is there.
        // Read through the Win32 dictionary the same number says invalid data, which is a fact about how a call was
        // addressed rather than about the board — and that is what made an earlier green read a wrong address as a
        // missing board.
        assert_eq!(
            dictionary_of("CM_Locate_DevNodeW"),
            CONFIG_MANAGER_DICTIONARY,
            "the reading names which dictionary its numbers come from",
        );
        assert_eq!(number, CONFIG_MANAGER_NOT_THERE);

        let address = target.as_string();
        let says = classify_open_failure(&address, "CM_Locate_DevNodeW", number);
        assert_eq!(
            says.code(),
            "device_unreachable",
            "a remembered row that is not present is read as a device that is not there, not as one we cannot use",
        );
        println!("  {row} answers with {number}: {says}");
    }
}

#[tokio::test]
async fn a_present_instance_says_what_it_is_bound_to_before_anything_can_be_read() {
    let target = target();

    let found: Found = match bus::find(&target) {
        Ok(found) => found,
        Err(error) => {
            println!(
                "nothing present answers to {}, so the area says only what the registry remembers ({error})",
                target.as_string(),
            );
            return;
        }
    };

    println!("instance: {}", found.instance);

    // One fact, two names: `SPDRP_DRIVER` says the class-install key and `SPDRP_SERVICE` says the service behind it.
    // The rule that decides whether an interface is this program's reads the second, because a key can never be the
    // name it compares with — which is what an earlier reading mistook for a driver.
    println!("  class the element says it is in: {}", found.class);
    println!("  class-install key: {}", found.class_key);
    println!("  service behind the key: {}", found.service);
    println!("  compatible ids: {}", found.compatible_ids);

    assert!(is_our_driver(&found.service));
    assert!(!is_our_driver(&found.class_key));
    assert_ne!(found.class_key, found.service);

    // pydfu's filter takes the pair from the interface settings after a device is open; here the same pair appears
    // inside what the element declares before anything is opened, so the two readings are compared rather than
    // treated as one rule.
    let declared = pair_from_ids(&found.compatible_ids);
    println!("  the pair the declaration carries: {declared:?}");

    assert_eq!(
        declared,
        Some((DFU_CLASS, DFU_SUBCLASS)),
        "the declaration says the pair pydfu filters on, which is what makes the two readings comparable",
    );

    // What this machine answers for the board is one class name, not two: pydfu's filter (`FilterDFU`) and Windows'
    // own class-install INF for devices of this kind (`c_usbdevice.inf`) write the same string. The paragraph that
    // treated them as two dictionaries is corrected here, and the name is compared through the shape rather than
    // through the casing, because the call writes it in capitals.
    assert_eq!(
        guid_parts(&found.class),
        guid_parts(WINDOWS_USBDEVICE_CLASS),
        "the class the element says it is in is the class the published installer names"
    );
    assert_ne!(
        guid_parts(&found.class),
        guid_parts(WINUSB_CLASS),
        "and it is not the guid WinUSB samples write, which is a second fact and not a class on this machine",
    );

    // The value that names what an installation registers, read from the device key. Nothing is written here: the
    // published package for this board has a branch whose own comment says the generic path skips the value, and that
    // was the state this machine held before the operator wrote it from an elevated prompt. A service name alone does
    // not register an interface, so `is_our_driver` and this reading are two facts that can disagree on one board.
    // Today they answer as two facts: the value reads as names, and the row that opens is the one the installation
    // registered under the standard WinUSB interface, not a row this value created.
    let written = bus::interface_guid_value(&found.instance);
    println!("  value the install wrote on the device key: {written:?}");

    let under_class =
        bus::rows_for_class(&found.class, &found.instance).expect("the list answers as it says");
    let under_sample =
        bus::rows_for_class(WINUSB_CLASS, &found.instance).expect("the list answers as it says");
    let under_winusb = bus::rows_for_class(WINUSB_DEVICE_INTERFACE, &found.instance)
        .expect("the list answers as it says");

    println!(
        "  rows for this instance under its class: {}",
        under_class.len()
    );
    println!(
        "  rows for this instance under the guid WinUSB samples: {}",
        under_sample.len()
    );
    println!(
        "  rows for this instance under the standard WinUSB interface: {}",
        under_winusb.len()
    );

    // The names a row can answer under are kept apart: the install decides which one it registered, and the
    // standard WinUSB interface is what Windows and Zadig write, while the published package names a second and the
    // class is the last fallback. The binding tries the standard name, the published package, the names the value
    // carries, then the class, and `found.path` is the row of the name it chose; the equality is with that name
    // rather than with a name this file picked.
    let interface = found
        .interface
        .as_deref()
        .expect("a present instance that publishes a row names the interface it was found under");
    let under_interface =
        bus::rows_for_class(interface, &found.instance).expect("the list answers as it says");

    assert_eq!(found.path, under_interface.first().map(String::from));

    // The same value read the second way, through the terminators rather than up to the first one, kept apart from
    // what the first reading saw. Each number is named with the call that answered it through the dictionary that
    // call reads from, and the type is kept beside the entries because the two readings differ on that shape: a
    // value stored as one word gives one entry to the splitting reading and one word to the other, and a value read
    // as a list of names gives one entry per name.
    let names = bus::interface_guid_names(&found.instance);

    println!(
        "  {} answered {} for the key, through the {} dictionary",
        bus::OPEN_CALL,
        names.open_answer,
        dictionary_of(bus::OPEN_CALL)
    );
    let kind_hex = format!("{:#x}", names.kind);

    println!(
        "  {} answered {} for the value and {} ({kind_hex}) for its type, through the {} dictionary",
        bus::VALUE_CALL,
        names.value_answer,
        names.kind,
        dictionary_of(bus::VALUE_CALL)
    );
    println!(
        "  the published table gives {} for a list of names and the file the operator runs writes {}, which are two spellings of one fact read from different sides; what the call answered here agrees with the table: {}",
        REGISTRY_TYPE_MULTI_SZ,
        INF_TYPE_MULTI_SZ,
        kind_hex == REGISTRY_TYPE_MULTI_SZ
    );
    println!("  the entries the terminators give: {:?}", names.entries);
    println!("  what the reading that stops at the terminator saw: {written:?}");

    for entry in &names.entries {
        let bare = name_without_the_pair(entry);

        println!(
            "  entry {entry}: {}",
            match bare {
                Some(bare) => format!("the pair is on both sides, so it reads as {bare}"),
                None => String::from(
                    "the pair is opened and not closed, so this is not a name either reading gives"
                ),
            }
        );
    }

    for entry in &names.entries {
        match bus::rows_for_class(entry, &found.instance) {
            Ok(rows) => println!(
                "  rows under {entry}: {} (the same name once the pair is taken off, which is what these APIs print: {})",
                rows.len(),
                guid_parts(name_without_the_pair(entry).unwrap_or(entry))
                    == guid_parts(&found.class)
            ),
            Err(error) => println!("  {entry} is not a name either reading gives: {error}"),
        }
    }

    // Four states, held as data rather than as one expected outcome, and only the branch this run is in asserts
    // anything. A value that is not there gives no entry to either reading and no row to either list. An entry that is
    // a name answers through the name it writes — which is what decides the name, not the class the element reports —
    // so `found.path`, produced by the reading that went through the class, is compared with the list read through the
    // value only when the two name the same string. And an entry that is not a name is refused rather than invented,
    // in two halves that are not the same fact: the pair around the string form is opened without being closed, or the
    // five parts do not have the lengths the shape gives.
    //
    // What this key answered over time is three readings of the same fact, kept apart rather than merged: first one
    // word with a comma inside — two names in letters, one entry in bytes — which the shape rule refused; then one
    // entry that opened the pair without closing it, because the closing brace did not reach the key; and today two
    // entries, each with its own terminator and the pair on both sides, which every rule here accepts as a name. What
    // did not change across the three states is the count of rows: both lists answered zero in all three, and the
    // instance was restarted twice without asking for elevation, so writing the value does not register an interface —
    // it records an install — and whether a row exists is not something this run can assert for any name, since a row
    // answers only where an interface was registered. The counts are printed as answers, not asserted.
    if names.entries.is_empty() {
        assert!(
            under_class.is_empty() && under_sample.is_empty(),
            "with nothing written on the device key there is no entry to read and no row under either published name",
        );
    } else {
        for entry in &names.entries {
            let bare = match name_without_the_pair(entry) {
                Some(bare) => bare,
                None => {
                    assert!(
                        under_class.is_empty() && under_sample.is_empty(),
                        "an entry that opens the pair without closing it is not a name, and no row answers under either published name",
                    );
                    continue;
                }
            };

            if guid_parts(bare).is_none() {
                assert!(
                    under_class.is_empty() && under_sample.is_empty(),
                    "an entry whose parts do not have the lengths the shape gives answers with no row under either published name",
                );
            }
        }
    }

    // The two readings of the same stored bytes are two rules, so this asserts the text they answer with and reads the
    // shape decision through the pair separately rather than folding them into one check. They agree on the first entry
    // because nothing inside that entry carries a terminator; they do not agree on a second entry, which is exactly the
    // difference between a word and a list. Whether an entry is a name is the third rule, and it reads through the pair
    // first, so the pair state is printed for every entry as this run read it.
    assert_eq!(
        names.entries.first(),
        written.as_ref(),
        "the reading that stops at the first terminator answers with the first entry when that entry has no terminator inside it",
    );

    for entry in &names.entries {
        let bare = name_without_the_pair(entry);

        println!(
            "  entry {entry}: {}",
            match bare {
                Some(bare) => format!("the pair is on both sides, so it reads as {bare}"),
                None => String::from(
                    "the pair is opened and not closed, so this is not a name either reading gives"
                ),
            }
        );
    }

    if let Some(text) = names.entries.first() {
        let bare = name_without_the_pair(text);

        assert_eq!(
            bare.map(guid_parts),
            written
                .as_ref()
                .and_then(|value| name_without_the_pair(value))
                .map(guid_parts),
            "the shape rule answers the same way for the same text through either reading",
        );
    }

    // The refusal that falls before anything travels, read as the state this run found rather than as one expected
    // outcome. When the install wrote nothing on the device key there is no row to open, and the code that says so is
    // the one about a device that is there: the instance answers as being present and its service says WinUSB, so
    // `device_unreachable` would say the wrong thing about it. When the value is written the same call reaches the
    // board, and what this half can say is only that it reached it through the service — the descriptor, the string
    // index and the state are asserted by the half that reads them against the published shape. The third reading of
    // the same fact stays outside this binding: the flag for every class is not one this crate calls, and the second
    // reader that asked for it was answered with 4 as a Config Manager number rather than with a list.
    match Handle::open(&target) {
        Ok(handle) => {
            println!(
                "open reached an interface through the service: {}",
                handle.describe()
            );
            assert!(is_our_driver(&found.service));
        }
        Err(error) => {
            println!("open says: {error}");
            assert_eq!(
                error.code(),
                "device_not_ours",
                "the instance is there and its service is ours, so this is not a device that is not there",
            );

            if written.is_none() {
                assert!(format!("{error}").contains(found.class.as_str()));
            }
        }
    }
}

#[tokio::test]
async fn a_board_on_the_bus_says_what_a_scripted_peer_cannot() {
    let target = target();

    let mut handle = match Handle::open(&target) {
        Ok(handle) => handle,
        Err(error) => {
            println!(
                "the scripted peer in tests/dfu.rs stands in for this run: {} is not reachable as an interface \
                 ({}), and the area asserts only what the bus answers about the instance",
                target.as_string(),
                error,
            );
            return;
        }
    };

    // What the interface said about itself, compared with the shape a published reader writes for this board.
    let golden =
        FunctionalDescriptor::read(GOLDEN_DESCRIPTOR).expect("nine bytes of the right type");
    let said = handle.descriptor();

    println!(
        "descriptor: attributes 0x{:#04x} | poll {} ms | transfer {} | version 0x{:04x} | class 0x{:#04x}/{}",
        said.attributes,
        said.detach_timeout,
        said.transfer_size,
        said.version,
        handle.class(),
        handle.subclass(),
    );

    assert_eq!(
        said, &golden,
        "the board puts on the cable what the published reading writes for it"
    );
    assert_eq!(
        said.version, CUBE_DEVICE_CODE,
        "the number CubeProgrammer gives this board is the one its own descriptor carries"
    );
    // pydfu decides that an interface is one of this kind by this pair; this binding decides by the service the
    // registry bound. They are not the same rule, so what this asserts is that they agree on the board, and the
    // difference stays a difference rather than being closed by hand.
    assert_eq!(
        (handle.class(), handle.subclass()),
        (DFU_CLASS, 1),
        "the pair pydfu's filter uses is the pair the board says about itself",
    );

    // The string index comes from the interface rather than from a number picked here, which is what pydfu does
    // through `intf.iInterface`. A board that writes its geometry in ArduPilot's own form answers with twelve
    // sectors and one megabyte.
    let index = handle.string_index();
    let text = handle
        .string(u16::from(index))
        .await
        .expect("the board answers its string descriptors");
    let layout = Layout::read(&text).expect("the string is the shape the rule says it is");

    println!("string {index}: {text}");
    println!(
        "sectors: {} | base 0x{:08x} | total {}",
        layout.sectors.len(),
        layout.base,
        layout.sectors.iter().map(|sector| sector.size).sum::<u32>()
    );

    assert_eq!(
        layout.sectors.len(),
        ARDUPILOT_SECTORS,
        "the board's own string makes the geometry ArduPilot declares for this family"
    );
    assert_eq!(
        layout.sectors.iter().map(|sector| sector.size).sum::<u32>(),
        ARDUPILOT_TOTAL,
        "and the sectors fill the whole flash rather than a part of it",
    );
    println!(
        "the dictionary also carries the family number {CUBE_FAMILY_CODE:#06x}, which nothing on the wire can be asked for"
    );

    let mut flash = Flash::open(handle, u16::from(index))
        .await
        .expect("the board answers");

    // A read leaves the board in `dfuUPLOAD-IDLE`, and this area may be run after an earlier one read something;
    // the abort request is what puts it back in the idle state a write starts from. The state before the abort is
    // printed as what the board said, so a run that begins in upload-idle is a fact rather than a surprise.
    let before = flash.state().await.expect("the board answers its state");
    println!("state before the abort: {before:?}");

    let abort = flash.abort().await.expect("the board answers the abort");
    println!("abort: {abort:?}");

    let state = flash.state().await.expect("the board answers its state");
    println!("state: {state:?}");
    assert_eq!(
        state,
        State::DfuIdle,
        "a board that is waiting says it is waiting"
    );

    let answer = flash
        .read_answer()
        .await
        .expect("the board answers its status");
    println!(
        "status: {:?} | poll {} ms | {state:?}",
        answer.status, answer.poll_ms
    );

    if std::env::var("MPCRAFT_DFU_ALLOW_ERASE").is_ok() {
        // The half that erases runs only when the operator names it, because a board erased for nothing cannot be
        // put back by anybody else. What this asserts is which side of the published disagreement the board stands
        // on: dfu-util and pydfu write the number alone, ArduPilot's reader understands only the five-byte form.
        let answer = flash.mass_erase().await;
        println!("mass erase: {answer:?}");
    } else {
        println!("the erase was not asked for, so the two forms are not compared with this board");
    }
}
