//! The two readings of one stored value, asserted against bytes written down before the rule was called, and the pair
//! a string form can carry, asserted as its own fact.
//!
//! The gate (`scripts/check-dfu-crc.js`) reads these literals as text and compares them with what the crate carries;
//! it does not execute this module, so the agreement between the rule here and the numbers there is what this file
//! carries. What the bus answers for a real instance is the live area's job, not this one.

use mpcraft_winusb::{
    as_wide, entries_of, guid_parts, name_without_the_pair, INF_TYPE_MULTI_SZ,
    REGISTRY_TYPE_MULTI_SZ,
};

/// The name the class installer writes for this family, from `c_usbdevice.inf` as read on this machine — the shape
/// these APIs print, without the pair.
const CLASS_NAME: &str = "88bae032-5a81-49f0-bc3d-a4ff138216d6";

/// The same name with the pair around it, which is the shape `StringFromCLSID` writes and the shape a tool puts on the
/// device key.
const CLASS_NAME_WITH_THE_PAIR: &str = "{88bae032-5a81-49f0-bc3d-a4ff138216d6}";

/// The state this machine answered after the corrected write: the pair opened and not closed, because the value that
/// landed on the key lost its closing brace. It is not a name on either reading, so the rule refuses it rather than
/// inventing one.
const CLASS_NAME_OPEN_ONLY: &str = "{88bae032-5a81-49f0-bc3d-a4ff138216d6";

/// The name WinUSB samples write, kept as a second fact and not as the class this program reads through.
const SAMPLE_NAME: &str = "a5dcbf13-4437-40bb-9a12-8e18f591d448";

/// The same value the machine holds today, written as UTF-16 units: two names inside one word with a comma between
/// them, and no terminator inside the word. This is what an operator's write put on the device key — the letters say
/// two names, and the stored value says one entry.
fn today() -> Vec<u16> {
    let text = format!("{CLASS_NAME},{SAMPLE_NAME}");
    text.encode_utf16().collect()
}

/// The form that answers as a list of names: one name per entry, each with its own terminator, which is what a tool
/// writing this value as an array produces.
fn as_entries() -> Vec<u16> {
    let mut units = Vec::new();

    for name in [CLASS_NAME, SAMPLE_NAME] {
        units.extend(name.encode_utf16());
        units.push(0);
    }

    units.push(0);
    units
}

#[test]
fn one_word_with_a_comma_inside_is_one_entry_and_the_shape_rule_refuses_it() {
    let units = today();

    /* Read the two ways. The reading that stops at the terminator answers with the whole word, because there is no
    terminator inside it; the reading that splits at the terminators answers with the same word as one entry. So
    here the two give the same fact — and neither is a name the shape rule can read, which is what kept this
    instance answering no rows. */
    let entries = entries_of(&units);

    assert_eq!(entries.len(), 1, "one word gives one entry");
    assert_eq!(
        as_wide(&units),
        entries[0],
        "both readings see the same word"
    );
    assert!(
        guid_parts(&entries[0]).is_none(),
        "the entry is not one name"
    );

    /* The letters say two names, so a reader that counted them would say two entries. The rule does not count
    letters: it splits at terminators, and this value has none inside, so it says one. */
    assert_eq!(entries.len(), 1, "the value carries one entry");
}

#[test]
fn one_name_per_entry_gives_two_entries_and_the_two_readings_disagree() {
    let units = as_entries();
    let entries = entries_of(&units);

    assert_eq!(entries.len(), 2, "two names written as two entries");

    for name in &entries {
        assert!(guid_parts(name).is_some(), "{name} reads as the shape");
    }

    /* The disagreement is the point: the reading that stops at the first terminator sees only the first name, so
    under this shape the two readings do not answer with the same thing. A file or a test that treated them as one
    rule would read the second name as if the program could see it. */
    assert_eq!(as_wide(&units), entries[0]);
    assert_ne!(as_wide(&units), entries[1].clone());
}

#[test]
fn the_pair_is_read_as_its_own_fact_and_a_name_that_only_opens_it_is_refused() {
    /* Two published readings, not one rule written twice: `StringFromCLSID` writes the name with the pair, and these
    APIs print it without, while `CLSIDFromString` takes either form. So the pair has to be read before the parts,
    and the two have to agree when the pair is complete — which is a fact about two implementations, not something
    this file decided. */
    let bare = name_without_the_pair(CLASS_NAME_WITH_THE_PAIR);

    assert_eq!(
        bare,
        Some(CLASS_NAME),
        "the pair, when it closes, gives the name"
    );
    assert_eq!(
        guid_parts(bare.unwrap()),
        guid_parts(CLASS_NAME),
        "the same name read through either route"
    );

    /* The half that refuses: an opening brace without its closing one is not a name on either reading. A reader that
    trimmed the opening brace without looking for the closing one would read this as a class and put a row on the
    board under a name the value never said — which is what the gate's known-bad case shows firing. */
    assert_eq!(name_without_the_pair(CLASS_NAME_OPEN_ONLY), None);
    assert_eq!(
        name_without_the_pair(
            &entries_of(&CLASS_NAME_WITH_THE_PAIR.encode_utf16().collect::<Vec<_>>())[0]
        ),
        Some(CLASS_NAME),
        "an entry keeps the pair as the value writes it, and the rule takes it before the parts"
    );
    assert_eq!(
        name_without_the_pair(
            &entries_of(&CLASS_NAME_OPEN_ONLY.encode_utf16().collect::<Vec<_>>())[0]
        ),
        None,
        "the entry that opened the pair is refused by the same rule read through it"
    );
}

#[test]
fn the_two_spellings_of_the_value_type_stay_two_numbers() {
    /* An AddReg line writes 0x10000 for a list of names; the call that answers the value reported 7 for this
    machine's instance, which is what the published table of value types gives for a list. They are read from
    different sides, so writing them as one number would close the difference the reading carries. */
    assert_ne!(INF_TYPE_MULTI_SZ, REGISTRY_TYPE_MULTI_SZ);

    /* The spelling that changes what a reader gets is the word rather than the list, and it is a third number, not
    one of these two. */
    assert_ne!(INF_TYPE_MULTI_SZ, "0x1");
    assert_ne!(REGISTRY_TYPE_MULTI_SZ, "0x1");
}
