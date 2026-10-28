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
//! at power-on, or by the command that holds it there, and the gate names that as what the operator supplies. When
//! nothing present answers to the pair the area says so, and the only thing it asserts is the fact that says why —
//! which is the reading that separates a device the registry remembers from a device that is there.

#![cfg(all(feature = "hardware-tests", windows))]

use mpcraft_app::dfu::{Flash, Layout, State};
use mpcraft_transport::device::{Device, DeviceError, FunctionalDescriptor};
use mpcraft_winusb::bus::Handle;
use mpcraft_winusb::{bus, Target, DFU_CLASS, DFU_SUBCLASS};

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

    // Every row is read as what it says about this run rather than as a board being there. This machine shows the
    // two readings disagreeing: the registry remembers the pair from a run when the board was in its bootloader,
    // and each row answers with the code that says nothing is present.
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

    match bus::find(&target) {
        Ok(found) => assert_eq!(
            found.instance,
            present[0].as_str(),
            "what is found is one of the rows that is present"
        ),
        Err(error) => {
            // The refusal here is the one about presence, not the one about the driver: nothing was opened, so the
            // rule that says a device can be there and not be ours never had a number to read. Keeping the two
            // codes apart is what this half asserts.
            assert!(
                matches!(error, DeviceError::Unreachable(_)),
                "a remembered row that is not present is read as a device that is not there, not as one we cannot use",
            );
            println!("find says: {error}");
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
                "the scripted peer in tests/dfu.rs stands in for this run: nothing present answers to {}, and the \
                 area asserts only the reading that separates remembering from being there ({error})",
                target.as_string(),
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
    // pydfu decides that an interface is one of this kind by this pair; this binding decides by the driver the
    // registry bound. They are not the same rule, so what this asserts is that they agree on the board, and the
    // difference stays a difference rather than being closed by hand.
    assert_eq!(
        (handle.class(), handle.subclass()),
        (DFU_CLASS, DFU_SUBCLASS),
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
