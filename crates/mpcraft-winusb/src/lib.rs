//! The binding that reaches a device on a USB bus.
//!
//! [`mpcraft_transport::device::Device`] carries no operating-system types, which is what lets the scripted peer
//! in `crates/mpcraft-app/tests/dfu.rs` answer it without a bus. This is the half that cannot be answered
//! without one, and it is its own crate for two reasons that cannot be reconciled inside `mpcraft-app`: the
//! system's own functions are unsafe, and `mpcraft-app` forbids unsafe code at the crate level; and a crate
//! below the application layer is not supposed to know that WinUSB exists, which is exactly what this crate
//! knows. So the rule is written here rather than left implicit: this is the one crate that knows a physical
//! bus and the one where `unsafe` lives, and the application layer is still the one that decides whether to
//! reach it — which it does behind `hardware-tests`, in `crates/mpcraft-app/tests/dfu_live.rs`. This is not a
//! runtime dependency: what the crate named above carries are import names resolved by the loader
//! (`windows-link` writes them as `kind = "raw-dylib"`, which is what makes it link with the toolchain the gates
//! use), so nothing has to be installed for it to work.
//!
//! What is read without a bus stays compiled for the ordinary suite: the pair as the environment writes it, the
//! number a failure to open answers with, and the name the registry gives the driver bound to an interface. The
//! calls themselves are behind `cfg(windows)`, so on another system there is no binding to test.
//!
//! Three numbers come from a run against this machine rather than from a guess, and each is written down beside
//! the rule it belongs to. The value a device-interface detail buffer has to carry on a 64-bit process (8) is not
//! the size the crate's `[u16; 1]` definition gives, so a call that is told only the header answers
//! `ERROR_INSUFFICIENT_BUFFER` (122) while the same call with the room answers with the path. These APIs read a
//! string as the text plus its terminator: without it an earlier probe answered `CR_INVALID_DEVICE_ID` (30) for a
//! board that was there, which is what makes that number the reader's own mistake rather than a fact about the
//! bus. And a failure reaches Rust as an `HRESULT`, whose low word is the number — `0x8007007A` is 122 — so the
//! number is taken from the low word rather than read as the whole value.

#![warn(missing_docs)]

use mpcraft_transport::device::DeviceError;

/// The variable that names a board in its bootloader. It is not a connection specification: a board in its
/// bootloader is not a transport, so the grammar `serial:`/`udp:`/`tcp:` does not reach it.
pub const ENV_TARGET: &str = "MPCRAFT_DFU_TARGET";

/// The variable that says the half that erases or writes may run, as §10 of the architecture requires.
pub const ENV_ALLOW_ERASE: &str = "MPCRAFT_DFU_ALLOW_ERASE";

/// The pair as the bus writes it, which is what the environment names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// The vendor number.
    pub vendor: u16,
    /// The product number.
    pub product: u16,
}

impl Target {
    /// Reads the pair as the environment writes it: four hexadecimal digits, the colon, four more. Nothing is
    /// guessed — a part that cannot be read as a number is refused, the same rule the connection specifications
    /// follow for an option they do not recognise.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let (vendor, product) = text.split_once(':')?;

        let vendor = hex4(vendor)?;
        let product = hex4(product)?;

        Some(Self { vendor, product })
    }

    /// The pair as the environment says it.
    #[must_use]
    pub fn as_string(&self) -> String {
        format!("{:04x}:{:04x}", self.vendor, self.product)
    }

    /// The part of an instance id that names the pair, as the bus writes it.
    #[must_use]
    pub fn instance_fragment(&self) -> String {
        format!("VID_{:04X}&PID_{:04X}", self.vendor, self.product)
    }
}

fn hex4(part: &str) -> Option<u16> {
    if part.len() != 4 {
        return None;
    }

    u16::from_str_radix(part, 16).ok()
}

/// The driver that makes an interface this program's. A board whose interface is bound to another one is on the
/// bus and answers to the address, so `device_unreachable` would say the wrong thing about it.
pub const OUR_DRIVER: &str = "winusb";

/// Says whether the name the registry gives the driver bound to an interface is the one that makes it ours.
#[must_use]
pub fn is_our_driver(name: &str) -> bool {
    name.eq_ignore_ascii_case(OUR_DRIVER)
}

/// The pair a published reader uses to decide that an interface is one of this kind: pydfu's `FilterDFU` takes
/// the class 0xFE and the sub-class 1. This binding decides by the driver the registry bound rather than by the
/// pair, so the two are not the same rule and the live area compares them against the same board.
pub const DFU_CLASS: u8 = 0xFE;

/// The sub-class of that pair, from the same published filter.
pub const DFU_SUBCLASS: u8 = 1;

/// The number that says nothing is there in the Win32 dictionary (`ERROR_FILE_NOT_FOUND`), which is what the calls
/// that open a path answer from.
pub const NOTHING_THERE: u32 = 2;

/// The number that says the instance is there and its handle is held by somebody else, in the same dictionary
/// (`ERROR_ACCESS_DENIED`).
pub const HANDLE_HELD: u32 = 5;

/// The number that says the instance is there and is held without sharing, in the same dictionary
/// (`ERROR_SHARING_VIOLATION`).
pub const HELD_WITHOUT_SHARING: u32 = 32;

/// The number that says invalid data in the Win32 dictionary (`ERROR_INVALID_DATA`).
pub const WIN32_INVALID_DATA: u32 = 13;

/// The number that says an instance is not there in the Config Manager dictionary (`CR_NO_SUCH_DEVNODE`). It is the
/// same 13 as the number above, and that is why a number cannot be read without naming the call that answered it:
/// an earlier green on this machine read a wrong address as a missing board because it took the 13 from the wrong
/// dictionary, and the two facts are not the same thing — one is how a call was addressed, the other is that the
/// instance is not on the bus.
pub const CONFIG_MANAGER_NOT_THERE: u32 = 13;

/// The dictionary the `CM_*` calls answer from.
pub const CONFIG_MANAGER_DICTIONARY: &str = "Config Manager";

/// The dictionary the `SetupDi*`, `GetLastError` and `CreateFileW` calls answer from.
pub const WIN32_DICTIONARY: &str = "Win32 error";

/// Which dictionary a number comes from, said by the call that answered it rather than by the number.
#[must_use]
pub fn dictionary_of(answered_by: &str) -> &'static str {
    if answered_by.starts_with("CM_") {
        CONFIG_MANAGER_DICTIONARY
    } else {
        WIN32_DICTIONARY
    }
}

/// The part of an instance path these calls read as the enumerator: what comes before the first backslash. Passing
/// the whole path where the call asks for the enumerator answers `ERROR_INVALID_DATA` (13), which is the number
/// above, so this is the shape the reading takes rather than a convenience.
#[must_use]
pub fn enumerator(instance: &str) -> &str {
    instance.split('\\').next().unwrap_or(instance)
}

/// What a failure says, given the number and the call that answered it.
///
/// The element was reached, which is what excludes "nothing is there" for every number but the one each dictionary
/// reserves for it. So the other numbers are read as the device being ours to look at and not ours to use, with the
/// number kept as what the system said and the dictionary named with it.
pub fn classify_open_failure(address: &str, answered_by: &str, code: u32) -> DeviceError {
    let dictionary = dictionary_of(answered_by);

    if (dictionary == WIN32_DICTIONARY && code == NOTHING_THERE)
        || (dictionary == CONFIG_MANAGER_DICTIONARY && code == CONFIG_MANAGER_NOT_THERE)
    {
        return DeviceError::Unreachable(address.to_owned());
    }

    let reason = match code {
        HANDLE_HELD => "the handle is held by another process".to_owned(),
        HELD_WITHOUT_SHARING => "the handle is held without sharing".to_owned(),
        WIN32_INVALID_DATA if dictionary == WIN32_DICTIONARY => format!(
            "answered with {code} as {dictionary} invalid data, which is a fact about how the call was addressed \
             rather than about the board"
        ),
        other => format!("answered with {other}, read as a {dictionary} number from {answered_by}"),
    };

    DeviceError::NotOurs {
        address: address.to_owned(),
        reason,
    }
}

/// The two class names a published reader would look under, written from the published sources rather than from what
/// this machine answered: pydfu's own class for libusb-win32 (`FilterDFU` reads its rows through it), and the class
/// WinUSB publishes interfaces under. Both are compared against what the element says about itself in the live area,
/// which is why they are kept as data here.
pub const PYDFU_LIBUSB_CLASS: &str = "88bae032-5a81-49f0-bc3d-a4ff138216d6";

/// The class WinUSB publishes device interfaces under, from the published constant.
pub const WINUSB_CLASS: &str = "a5dcbf13-4437-40bb-9a12-8e18f591d448";

/// A class name as these APIs read it: the five parts, in the lengths they have — 8, 4, 4, 4 and 12 hexadecimal
/// digits. Nothing is guessed: a name that cannot be read as the shape is refused, which is what keeps a class from
/// being invented by the reader.
#[must_use]
pub fn guid_parts(name: &str) -> Option<(u32, u16, u16, [u8; 8])> {
    let parts: Vec<&str> = name.split('-').collect();

    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return None;
    }

    let data1 = u32::from_str_radix(parts[0], 16).ok()?;
    let data2 = u16::from_str_radix(parts[1], 16).ok()?;
    let data3 = u16::from_str_radix(parts[2], 16).ok()?;

    let mut data4 = [0_u8; 8];

    for (index, pair) in parts[3..].concat().as_bytes().chunks(2).enumerate() {
        data4[index] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }

    Some((data1, data2, data3, data4))
}

/// What a device that is there says about itself: the instance, the two names the registry gives the same fact —
/// the class-install key and the service behind it, which are not the same thing and cannot be read as one — the
/// compatible ids the element carries, and the interface path it publishes, or nothing when it publishes none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The instance id as the bus writes it.
    pub instance: String,
    /// The class name the element says it belongs to, as the call wrote it — which is compared with the two
    /// published class names above rather than assumed from either.
    pub class: String,
    /// What the element says the class-install key is, read through `SPDRP_DRIVER`. That property names the key
    /// rather than the driver, so it can never be the name this binding compares with [`OUR_DRIVER`].
    pub class_key: String,
    /// What the element says the service behind the key is, read through `SPDRP_SERVICE`. This is the name the rule
    /// that decides whether an interface is ours reads, because the key above answered `{88bae032-...}\0011` for a
    /// board whose service says `WinUSB`.
    pub service: String,
    /// The compatible ids as the element writes them. A published reader decides that an interface is one of this
    /// kind by the pair 0xFE and 1; here the pair appears inside the declaration as text, which is a third reading
    /// of the same fact and is read by [`pair_from_ids`] rather than assumed from a row.
    pub compatible_ids: String,
    /// The device interface path, as the call wrote it, or nothing when the instance publishes no interface —
    /// which is what this machine answers today for this board under both class names above.
    pub path: Option<String>,
}

/// The pair a published filter reads from an interface, read out of the compatible ids as the element writes them.
///
/// This is not the same rule as pydfu's, which reads the pair from the interface settings after the device is open:
/// here it is what the element declares before anything is opened. So the two readings are kept apart, and a string
/// that does not say the pair is refused rather than guessed at.
#[must_use]
pub fn pair_from_ids(ids: &str) -> Option<(u8, u8)> {
    let number_after = |prefix: &str| {
        ids.split('&').find_map(|part| {
            // The declaration writes the pair as a word inside a segment, so the part before it is taken off rather
            // than assumed to be nothing.
            u8::from_str_radix(part.rsplit('\\').next()?.strip_prefix(prefix)?, 16).ok()
        })
    };

    Some((number_after("Class_")?, number_after("SubClass_")?))
}

/// A string as these APIs read it: the text and the terminator. Without the terminator the answer is the reader's
/// own mistake — `CR_INVALID_DEVICE_ID` (30) — which is why this is written down before any call.
#[must_use]
pub fn nul_terminated(text: &str) -> Vec<u16> {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.push(0);
    units
}

/// The header a device-interface detail buffer carries on a 64-bit process: what the API says the process writes,
/// not the size the crate's definition of the struct gives.
pub const DETAIL_HEADER: usize = 8;

/// The length of a device interface path, in UTF-16 units, as the API writes it.
pub const PATH_UNITS: usize = 260;

/// The room the detail call has to be given: the header plus the path. A call told only the header answers with
/// `ERROR_INSUFFICIENT_BUFFER` (122), which is what keeps these two numbers apart.
#[must_use]
pub const fn detail_room() -> usize {
    DETAIL_HEADER + PATH_UNITS * 2
}

/// Reads a wide value as the text it says, stopping at the terminator rather than at the end of the buffer.
fn as_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    String::from_utf16_lossy(&units)
        .split('\0')
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The half that speaks to the bus. It is behind `cfg(windows)` because on another system there is no such thing
/// to call; what it can say is asserted by the live area, not here.
#[cfg(windows)]
pub mod bus {
    use async_trait::async_trait;

    use windows::core::{GUID, PCWSTR};
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Devices::Usb::*;
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::*;

    use mpcraft_transport::device::{
        read_string, Device, DeviceError, DeviceResult, FunctionalDescriptor,
    };

    use super::{
        as_text, classify_open_failure, detail_room, enumerator, guid_parts, is_our_driver,
        nul_terminated, Found, Target,
    };

    /// The call that answers for a registry property, named so that a refusal can say which dictionary its number
    /// came from: this one answers with a Win32 error number, while `CM_Locate_DevNodeW` answers with a Config
    /// Manager number, and the two dictionaries give the same 13 for two different facts.
    const PROPERTY_CALL: &str = "SetupDiGetDeviceRegistryPropertyW";

    /// The rows the registry carries for the pair. These are the instances it remembers from an earlier run,
    /// which is why they cannot be read as a board being there: `remembered` and `present` are two readings, and
    /// the live area shows them disagreeing on this machine.
    pub fn remembered(target: &Target) -> Vec<String> {
        let filter = nul_terminated("USB");

        unsafe {
            let mut needed = 0_u32;

            if CM_Get_Device_ID_List_SizeW(&mut needed, PCWSTR(filter.as_ptr()), 0) != CR_SUCCESS {
                return Vec::new();
            }

            let mut buffer = vec![0_u16; usize::try_from(needed).unwrap_or(4096)];

            if CM_Get_Device_ID_ListW(PCWSTR(filter.as_ptr()), &mut buffer, 0) != CR_SUCCESS {
                return Vec::new();
            }

            let fragment = target.instance_fragment();

            String::from_utf16_lossy(&buffer)
                .split('\0')
                .filter(|instance| instance.to_uppercase().contains(&fragment))
                .map(String::from)
                .collect()
        }
    }

    /// The number locating gives for one instance, kept as what the Config Manager dictionary says rather than
    /// turned into a word, because the same 13 means something else through the dictionary the SetupDi calls answer
    /// from and the two facts cannot be read with one code.
    pub fn locate(instance: &str) -> u32 {
        let id = nul_terminated(instance);
        let mut devinst = 0_u32;

        unsafe { CM_Locate_DevNodeW(&mut devinst, PCWSTR(id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL).0 }
    }

    /// Whether an instance is on the bus for this run, read by locating it rather than by finding it in the list.
    /// The two are different facts, and this machine shows them disagreeing: the registry carries five rows for the
    /// pair from a run when the board was in its bootloader, and exactly one of them answers as being there.
    pub fn present(instance: &str) -> bool {
        locate(instance) == CR_SUCCESS.0
    }

    /// Finds the instance the pair names and says what the element answers about itself.
    ///
    /// # Errors
    ///
    /// Returns `device_unreachable` when nothing present answers to the pair — which includes an instance the
    /// registry remembers from an earlier run, because remembering is not being there.
    pub fn find(target: &Target) -> DeviceResult<Found> {
        let address = target.as_string();

        for instance in remembered(target) {
            // Present is a fact about this run, read here rather than assumed from a row of the list.
            if !present(&instance) {
                continue;
            }

            return read_element(&instance);
        }

        Err(DeviceError::Unreachable(address))
    }

    /// One element, reached by opening it through its id. That is the documented shape, and it is not the same thing
    /// as enumerating a set built from an instance path: passing the whole path where the call asks for the
    /// enumerator answers `ERROR_INVALID_DATA` (13), which is the number the Config Manager dictionary gives for a
    /// board that is not there, so the two facts have to come from different calls.
    fn read_element(instance: &str) -> DeviceResult<Found> {
        unsafe {
            let set = open_set(enumerator(instance))?;

            // cbSize is part of what the call reads, so it is written where the value is made rather than after
            // it, which is the form that keeps the reader from seeing two facts about one struct.
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            let id = nul_terminated(instance);

            SetupDiOpenDeviceInfoW(set, PCWSTR(id.as_ptr()), None, 0, Some(&mut info)).map_err(
                |error| {
                    classify_open_failure(
                        instance,
                        "SetupDiOpenDeviceInfoW",
                        number(error.code().0),
                    )
                },
            )?;

            let guid = info.ClassGuid;
            let class = format!("{guid:?}");
            let rows = rows_for_class(&class, instance)?;

            let found = Found {
                instance: instance.to_owned(),
                class,
                class_key: read_property(set, &info, instance, SPDRP_DRIVER)?,
                service: read_property(set, &info, instance, SPDRP_SERVICE)?,
                compatible_ids: read_property(set, &info, instance, SPDRP_COMPATIBLEIDS)?,
                path: rows.first().map(String::from),
            };

            let _ = SetupDiDestroyDeviceInfoList(set);

            Ok(found)
        }
    }

    /// A device-info set built the documented way: the enumerator, and the flags that say only what is present.
    fn open_set(enumerator: &str) -> DeviceResult<HDEVINFO> {
        let id = nul_terminated(enumerator);

        unsafe {
            SetupDiGetClassDevsW(
                None,
                PCWSTR(id.as_ptr()),
                None,
                DIGCF_ALLCLASSES | DIGCF_PRESENT,
            )
            .map_err(|error| {
                classify_open_failure(enumerator, "SetupDiGetClassDevsW", number(error.code().0))
            })
        }
    }

    /// A wide registry property, read through the element it belongs to, with the answering call named so that a
    /// number kept as data says which dictionary it came from.
    fn read_property(
        set: HDEVINFO,
        info: &SP_DEVINFO_DATA,
        instance: &str,
        property: SETUP_DI_REGISTRY_PROPERTY,
    ) -> DeviceResult<String> {
        let mut raw = vec![0_u8; detail_room()];

        unsafe {
            SetupDiGetDeviceRegistryPropertyW(set, info, property, None, Some(&mut raw), None)
                .map_err(|error| {
                    classify_open_failure(instance, PROPERTY_CALL, number(error.code().0))
                })?;
        }

        Ok(as_text(&raw))
    }

    /// The rows one class publishes for one instance, read through the interface list rather than through an
    /// enumeration, because the two are different readings of the same fact and the list answers with a Config
    /// Manager number rather than with a result.
    ///
    /// What this machine answers today for this board is no rows under either published class name — pydfu's class
    /// for libusb-win32 and WinUSB's own — which is why the descriptor, the string index and the state cannot be
    /// read through this binding: there is nothing to open. A published reader finds the same nothing, since pydfu
    /// filters on its class and gets no row for this instance either, so that is said rather than assumed.
    pub fn rows_for_class(class: &str, instance: &str) -> DeviceResult<Vec<String>> {
        let parts = match guid_parts(class) {
            Some(parts) => parts,
            None => {
                return Err(DeviceError::NotOurs {
                    address: instance.to_owned(),
                    reason: format!("the class name {class} is not the shape these APIs read"),
                });
            }
        };

        let id = nul_terminated(instance);

        unsafe {
            let guid = GUID::from_values(parts.0, parts.1, parts.2, parts.3);
            let mut needed = 0_u32;

            if CM_Get_Device_Interface_List_SizeW(
                &mut needed,
                &guid,
                PCWSTR(id.as_ptr()),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) != CR_SUCCESS
            {
                return Ok(Vec::new());
            }

            let mut buffer = vec![0_u16; usize::try_from(needed).unwrap_or(4096)];

            if CM_Get_Device_Interface_ListW(
                &guid,
                PCWSTR(id.as_ptr()),
                &mut buffer,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            ) != CR_SUCCESS
            {
                return Ok(Vec::new());
            }

            let joined = String::from_utf16_lossy(&buffer);

            Ok(joined
                .split('\0')
                .filter(|row| !row.is_empty())
                .map(String::from)
                .collect())
        }
    }

    /// An interface that is open, and answers the device trait with the calls of the system. It is called
    /// `Handle` rather than `Open` because the rule that builds one is named `open`, and a type and the function
    /// that makes it cannot share a name without the reader having to guess which one a line means.
    pub struct Handle {
        found: Found,
        descriptor: FunctionalDescriptor,
        handle: HANDLE,
        interface: WINUSB_INTERFACE_HANDLE,
        /// The number of the interface the settings say this one is, read from the interface rather than assumed,
        /// because a request carries it.
        number: u8,
        /// The string the interface says describes its memory, read from the interface rather than assumed,
        /// because pydfu reads the layout through `intf.iInterface` rather than through a number it picks.
        string_index: u8,
        /// What the interface says its class is, read from the settings rather than assumed. A published reader
        /// decides that a device is one of this kind by the pair 0xFE and 1, which is not what this binding
        /// decides by — it decides by the driver the registry bound — so the two readings are compared against
        /// the same board in the live area.
        class: u8,
        /// The sub-class of the interface, read from the settings.
        subclass: u8,
    }

    // The handles name an object in the kernel rather than pointing into this process's memory, and nothing here
    // reads through them as memory. The trait hands the device to one caller at a time (`&mut self`), so what can
    // be shared between threads is the handle value itself, which is a number.
    //
    // SAFETY: those two facts are what make the marks sound; without a bus nothing runs, and the live area that
    // does run holds one device for one run, which is the rule the simulator areas already follow.
    unsafe impl Send for Handle {}
    unsafe impl Sync for Handle {}

    impl Handle {
        /// Opens the interface the enumeration found.
        ///
        /// # Errors
        ///
        /// `device_not_ours` when the driver bound to the instance is not the one that makes it ours, or when the
        /// handle cannot be taken; `device_unreachable` when nothing present answers to the pair.
        pub fn open(target: &Target) -> DeviceResult<Self> {
            let found = find(target)?;

            // The rule reads the service behind the class-install key rather than the key itself, which is what an
            // earlier reading mistook for a driver: a key can never be the name it compares with, so on this board
            // the key says one thing and the service says another, and only one of them is a driver.
            if !is_our_driver(&found.service) {
                return Err(DeviceError::NotOurs {
                    address: target.as_string(),
                    reason: format!("the service behind it says {}", found.service),
                });
            }

            // Nothing is opened here, so this refusal falls before anything travels: the instance is there and is
            // bound to the right service, but it publishes no interface that could be read.
            let Some(path) = found.path.as_ref() else {
                return Err(DeviceError::NotOurs {
                    address: target.as_string(),
                    reason: format!(
                        "publishes no device interface under the class it says it is in ({})",
                        found.class
                    ),
                });
            };

            unsafe {
                let path = nul_terminated(path);

                let handle = CreateFileW(
                    PCWSTR(path.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                )
                .map_err(|error| {
                    classify_open_failure(
                        &target.as_string(),
                        "CreateFileW",
                        number(error.code().0),
                    )
                })?;

                let mut interface = WINUSB_INTERFACE_HANDLE::default();

                WinUsb_Initialize(handle, &mut interface).map_err(|error| {
                    classify_open_failure(
                        &target.as_string(),
                        "WinUsb_Initialize",
                        number(error.code().0),
                    )
                })?;

                let mut settings = USB_INTERFACE_DESCRIPTOR::default();

                WinUsb_QueryInterfaceSettings(interface, 0, &mut settings).map_err(|error| {
                    classify_open_failure(
                        &target.as_string(),
                        "WinUsb_QueryInterfaceSettings",
                        number(error.code().0),
                    )
                })?;

                let descriptor = FunctionalDescriptor::read(&control_read(interface, 0x06, 0, 9)?)?;

                Ok(Self {
                    found,
                    descriptor,
                    handle,
                    interface,
                    number: settings.bInterfaceNumber,
                    string_index: settings.iInterface,
                    class: settings.bInterfaceClass,
                    subclass: settings.bInterfaceSubClass,
                })
            }
        }

        /// What the enumeration said about the device this interface is.
        #[must_use]
        pub fn found(&self) -> &Found {
            &self.found
        }

        /// What the interface said about itself when it was opened.
        #[must_use]
        pub fn descriptor(&self) -> &FunctionalDescriptor {
            &self.descriptor
        }

        /// The string the interface says describes its memory, read from the interface rather than assumed.
        #[must_use]
        pub fn string_index(&self) -> u8 {
            self.string_index
        }

        /// What the settings say the class of this interface is.
        #[must_use]
        pub fn class(&self) -> u8 {
            self.class
        }

        /// What the settings say the sub-class of this interface is.
        #[must_use]
        pub fn subclass(&self) -> u8 {
            self.subclass
        }

        fn packet(
            &self,
            request: u8,
            value: u16,
            length: u16,
            going_out: bool,
        ) -> WINUSB_SETUP_PACKET {
            // The direction bit comes from the published reader: a class request to an interface is 0x21 going out
            // and 0x81 coming back, which is what the request shape does not say and the wire has to.
            WINUSB_SETUP_PACKET {
                RequestType: if going_out { 0x21 } else { 0x81 },
                Request: request,
                Value: value,
                Index: u16::from(self.number),
                Length: length,
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = WinUsb_Free(self.interface);
                let _ = CloseHandle(self.handle);
            }
        }
    }

    /// A control transfer that answers with bytes.
    ///
    /// Through this API a stall and another failure look alike, so the binding cannot tell them apart: what it can
    /// say is that the interface refused the request, which is the code `device_stalled` carries. The number the
    /// system answered with has nowhere to travel inside that code, so it is not carried here; it is what the live
    /// area prints when a board answers, and the area says that a refusal read through this API is not the same as
    /// a refusal known to be a stall.
    fn control_read(
        interface: WINUSB_INTERFACE_HANDLE,
        request: u8,
        value: u16,
        length: u16,
    ) -> DeviceResult<Vec<u8>> {
        unsafe {
            let mut buffer = vec![0_u8; usize::from(length)];
            let mut read = 0_u32;

            let packet = WINUSB_SETUP_PACKET {
                RequestType: 0x81,
                Request: request,
                Value: value,
                Index: 0,
                Length: length,
            };

            WinUsb_ControlTransfer(interface, packet, Some(&mut buffer), Some(&mut read), None)
                .map_err(|_| DeviceError::Stalled { number: request })?;

            buffer.truncate(usize::try_from(read).unwrap_or(buffer.len()));
            Ok(buffer)
        }
    }

    /// The number inside an `HRESULT`: `0x8007007A` is 122, which is what makes the low word the fact and the
    /// whole value a shape the reader would have to know about.
    fn number(value: i32) -> u32 {
        (value & 0xFFFF) as u32
    }

    #[async_trait]
    impl Device for Handle {
        fn functional_descriptor(&self) -> &FunctionalDescriptor {
            &self.descriptor
        }

        async fn send(&mut self, number: u8, value: u16, payload: &[u8]) -> DeviceResult<()> {
            let Ok(length) = u16::try_from(payload.len()) else {
                return Err(DeviceError::BadAnswer {
                    number,
                    reason: format!(
                        "the payload is {} bytes, which the request shape cannot say",
                        payload.len()
                    ),
                });
            };

            unsafe {
                WinUsb_ControlTransfer(
                    self.interface,
                    self.packet(number, value, length, true),
                    Some(&mut payload.to_vec()),
                    None,
                    None,
                )
                .map_err(|_| DeviceError::Stalled { number })
            }
        }

        async fn receive(&mut self, number: u8, value: u16, length: u16) -> DeviceResult<Vec<u8>> {
            unsafe {
                let mut buffer = vec![0_u8; usize::from(length)];
                let mut read = 0_u32;

                WinUsb_ControlTransfer(
                    self.interface,
                    self.packet(number, value, length, false),
                    Some(&mut buffer),
                    Some(&mut read),
                    None,
                )
                .map_err(|_| DeviceError::Stalled { number })?;

                buffer.truncate(usize::try_from(read).unwrap_or(buffer.len()));
                Ok(buffer)
            }
        }

        async fn string(&mut self, index: u16) -> DeviceResult<String> {
            let Ok(byte) = u8::try_from(index) else {
                return Err(DeviceError::BadAnswer {
                    number: 0x06,
                    reason: format!("the index {index} is not one the request can say"),
                });
            };

            let bytes = control_read(self.interface, 0x06, u16::from(byte), 256)?;
            read_string(&bytes)
        }

        fn describe(&self) -> String {
            format!(
                "{} — the interface {} publishes, through the service {}",
                self.found.instance,
                u32::from(self.number),
                self.found.service
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pair_is_read_as_the_environment_writes_it() {
        let target = Target::read("0483:df11").expect("the pair the bus writes for this board");
        assert_eq!(target.vendor, 0x0483);
        assert_eq!(target.product, 0xDF11);
        assert_eq!(target.as_string(), "0483:df11");
        assert_eq!(target.instance_fragment(), "VID_0483&PID_DF11");
    }

    #[test]
    fn a_target_that_is_not_the_pair_is_refused_rather_than_guessed_at() {
        // A connection specification is not a DFU target, so `serial:COM5:115200` is refused here rather than read
        // as a port. A part that is not four digits is refused too.
        assert!(Target::read("serial:COM5:115200").is_none());
        assert!(Target::read("0483df11").is_none());
        assert!(Target::read("483:df11").is_none());
        assert!(Target::read("0483:zzzz").is_none());
    }

    #[test]
    fn a_device_that_is_there_and_cannot_be_opened_is_not_read_as_a_device_that_is_not_there() {
        // The numbers are written down before the rule is called, which is what says they came from the system
        // rather than from the reading.
        let address = "0483:df11";
        assert_eq!(
            classify_open_failure(address, "CreateFileW", 2).code(),
            "device_unreachable"
        );
        assert_eq!(
            classify_open_failure(address, "CreateFileW", 5).code(),
            "device_not_ours"
        );
        assert_eq!(
            classify_open_failure(address, "CreateFileW", 32).code(),
            "device_not_ours"
        );
        assert_eq!(
            classify_open_failure(address, "SetupDiOpenDeviceInfoW", 4319).code(),
            "device_not_ours"
        );

        // The last one keeps the number as data and names the call that answered it: a code that turned every
        // failure into one word would tell the operator to look for a board that is already there.
        let error = classify_open_failure(address, "SetupDiOpenDeviceInfoW", 4319);
        assert!(matches!(error, DeviceError::NotOurs { .. }));
        assert!(format!("{error}").contains("4319"));
    }

    #[test]
    fn the_same_number_says_two_things_depending_on_which_call_answered_it() {
        // 13 is `CR_NO_SUCH_DEVNODE` through the Config Manager dictionary and `ERROR_INVALID_DATA` through the
        // Win32 one, and those are two different facts about the same board: one says the instance is not on the
        // bus, the other says a call was addressed wrong. An earlier green read here as "not there" because it took
        // the number from the wrong dictionary, so the rule takes the call as its own fact.
        let address = "USB\\VID_0483&PID_DF11\\3357316C3536";

        assert_eq!(
            classify_open_failure(address, "CM_Locate_DevNodeW", 13).code(),
            "device_unreachable"
        );
        assert_eq!(
            classify_open_failure(address, "SetupDiGetClassDevsW", 13).code(),
            "device_not_ours"
        );
        assert_eq!(
            dictionary_of("CM_Get_Device_ID_ListW"),
            CONFIG_MANAGER_DICTIONARY
        );
        assert_eq!(dictionary_of("CreateFileW"), WIN32_DICTIONARY);
    }

    #[test]
    fn an_instance_path_and_the_part_of_it_these_calls_read_are_not_the_same_thing() {
        // The documented argument of SetupDiGetClassDevsW is the enumerator; passing the whole path where it asks for
        // one answered ERROR_INVALID_DATA (13) on this machine, and that is a fact about how the call was addressed
        // rather than about the board. The element is reached by opening it through its id instead.
        let instance = "USB\\VID_0483&PID_DF11\\3357316C3536";
        assert_eq!(enumerator(instance), "USB");
        assert_ne!(enumerator(instance), instance);
    }

    #[test]
    fn a_class_name_is_read_as_the_shape_it_carries_and_not_invented_by_the_reader() {
        // Both dictionaries this area compares: pydfu's class for libusb-win32, which is what this machine answered
        // for the board, and WinUSB's published class. A name that is not the shape is refused rather than guessed.
        assert_eq!(
            guid_parts(PYDFU_LIBUSB_CLASS),
            Some((
                0x88BAE032,
                0x5A81,
                0x49F0,
                [0xBC, 0x3D, 0xA4, 0xFF, 0x13, 0x82, 0x16, 0xD6]
            ))
        );
        assert_eq!(
            guid_parts(WINUSB_CLASS),
            Some((
                0xA5DCBF13,
                0x4437,
                0x40BB,
                [0x9A, 0x12, 0x8E, 0x18, 0xF5, 0x91, 0xD4, 0x48]
            ))
        );
        assert_eq!(guid_parts("{88bae032-5a81-49f0-bc3d-a4ff138216d6}"), None);
        assert_eq!(guid_parts("88bae032-5a81-49f0-bc3d-a4ff138216d"), None);
    }

    #[test]
    fn the_pair_a_published_filter_reads_appears_in_the_declaration_as_text() {
        // pydfu's `FilterDFU` takes the pair from the interface settings after a device is open; here the same pair
        // is what the element declares before anything is opened. The string is this machine's own answer for the
        // board, written down before the rule was called.
        assert_eq!(
            pair_from_ids("USB\\Class_FE&SubClass_01&Prot_02"),
            Some((DFU_CLASS, DFU_SUBCLASS))
        );
        // A declaration that does not say the pair is refused rather than read as zero, which is what a number the
        // dictionary does not give looks like everywhere else in this project.
        assert_eq!(pair_from_ids("USB\\VID_0483&PID_DF11"), None);
        assert_eq!(pair_from_ids("USB\\Class_FE"), None);
    }

    #[test]
    fn a_driver_that_is_not_the_one_that_makes_it_ours_is_said_as_the_name_the_registry_gives() {
        assert!(is_our_driver("winusb"));
        assert!(is_our_driver("WinUsb"));
        // What this machine says about the board: the service behind its class-install key is WinUSB, while the key
        // itself is a pair of numbers that can never be that name. Both are read, and the rule reads the first.
        assert!(!is_our_driver(
            "{88bae032-5a81-49f0-bc3d-a4ff138216d6}\\0011"
        ));
        // A board whose interface is bound to another one is on the bus and answers to the address, so a code that
        // called every failure "not there" would say the wrong thing about it.
        assert!(!is_our_driver("usbser"));
    }

    #[test]
    fn a_string_for_these_apis_carries_its_terminator() {
        let units = nul_terminated("USB");
        assert_eq!(units.last(), Some(&0));
        // Without it the answer was CR_INVALID_DEVICE_ID (30), which is what makes that number the reader's own
        // mistake rather than a fact about the bus: the board named by the string was there.
        assert_eq!(units, vec![85_u16, 83, 66, 0]);
    }

    #[test]
    fn the_detail_buffer_is_given_the_room_and_not_only_the_header() {
        assert_eq!(DETAIL_HEADER, 8);
        assert_eq!(detail_room(), 8 + 260 * 2);
        // A call that passed only the header answered ERROR_INSUFFICIENT_BUFFER (122) on this machine; the same
        // call with the room answered with the path. The two numbers are kept apart because they are not the same
        // thing — one is what the header says and the other is what the buffer has.
        assert_ne!(detail_room(), DETAIL_HEADER);
    }

    #[test]
    fn an_hresult_keeps_its_number_in_the_low_word() {
        // 0x8007007A is 122 read as a whole value and 122 read as the low word of a system failure; 0x80070005 is
        // 5. The shape is not the number, so the reading names which part it takes.
        assert_eq!(0x8007_007A_u32 & 0xFFFF, 122);
        assert_eq!(0x8007_0005_u32 & 0xFFFF, 5);
    }

    #[test]
    fn a_wide_value_is_read_up_to_its_terminator_and_not_to_the_end_of_the_buffer() {
        let mut raw = vec![0_u8; detail_room()];
        for (place, unit) in "winusb".encode_utf16().enumerate() {
            raw[4 + place * 2..4 + place * 2 + 2].copy_from_slice(&unit.to_le_bytes());
        }

        // The buffer is longer than the text it carries, which is what these calls do: the room is the room, and
        // the value ends where its terminator is.
        assert_eq!(as_text(&raw[4..]), "winusb");
    }
}
