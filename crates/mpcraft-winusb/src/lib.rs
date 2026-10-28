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

/// The number the system answers with when nothing present answers to the instance.
pub const NOTHING_THERE: u32 = 2;

/// The number it answers with when the instance is there and its handle is held by somebody else.
pub const HANDLE_HELD: u32 = 5;

/// The number it answers with when the instance is there and is held without sharing.
pub const HELD_WITHOUT_SHARING: u32 = 32;

/// What a failure to open says, given the number the system answered with.
///
/// The enumeration already found the device, which is what excludes "nothing is there" for every number but the
/// one that says the instance is not present. So the other numbers are read as the device being ours to look at
/// and not ours to use, with the number kept as what the system said rather than turned into a word.
pub fn classify_open_failure(address: &str, code: u32) -> DeviceError {
    if code == NOTHING_THERE {
        return DeviceError::Unreachable(address.to_owned());
    }

    let reason = match code {
        HANDLE_HELD => "the handle is held by another process".to_owned(),
        HELD_WITHOUT_SHARING => "the handle is held without sharing".to_owned(),
        other => format!("the system answered with {other}"),
    };

    DeviceError::NotOurs {
        address: address.to_owned(),
        reason,
    }
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

/// What the enumeration says about an instance that is there: the id, the driver bound to it, and the path its
/// interface publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The instance id as the bus writes it.
    pub instance: String,
    /// The name the registry gives the driver bound to the instance.
    pub driver: String,
    /// The device interface path, as the API wrote it.
    pub path: String,
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

    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::*;
    use windows::Win32::Devices::Usb::*;
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE,
    };
    use windows::Win32::Storage::FileSystem::*;

    use mpcraft_transport::device::{
        read_string, Device, DeviceError, DeviceResult, FunctionalDescriptor,
    };

    use super::{
        as_text, classify_open_failure, detail_room, is_our_driver, nul_terminated, Found, Target,
        DETAIL_HEADER,
    };

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

    /// Whether an instance is on the bus for this run, read by locating it rather than by finding it in the
    /// list. The two are different facts, and this machine shows them disagreeing: the registry remembers the
    /// pair from a run when the board was in its bootloader, and every one of those rows answers with the code
    /// that says nothing is there.
    pub fn present(instance: &str) -> bool {
        let id = nul_terminated(instance);
        let mut devinst = 0_u32;

        unsafe {
            CM_Locate_DevNodeW(&mut devinst, PCWSTR(id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL)
                == CR_SUCCESS
        }
    }

    /// Finds the instance the pair names and says what the registry and the class say about it.
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

            let driver = property(&instance, SPDRP_DRIVER)?;
            let path = interface_path(&instance)?;

            return Ok(Found {
                instance,
                driver,
                path,
            });
        }

        Err(DeviceError::Unreachable(address))
    }

    /// A wide registry property, read through the instance it belongs to.
    fn property(instance: &str, property: SETUP_DI_REGISTRY_PROPERTY) -> DeviceResult<String> {
        let id = nul_terminated(instance);

        unsafe {
            let set = SetupDiGetClassDevsW(
                None,
                PCWSTR(id.as_ptr()),
                None,
                DIGCF_ALLCLASSES | DIGCF_PRESENT,
            )
            .map_err(|error| classify_open_failure(instance, number(error.code().0)))?;

            // cbSize is part of what the call reads, so it is written where the value is made rather than after
            // it, which is the form that keeps the reader from seeing two facts about one struct.
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            if SetupDiEnumDeviceInfo(set, 0, &mut info).is_err() {
                let _ = SetupDiDestroyDeviceInfoList(set);
                return Err(classify_open_failure(
                    instance,
                    number(GetLastError().0 as i32),
                ));
            }

            let mut raw = vec![0_u8; detail_room()];

            let answer =
                SetupDiGetDeviceRegistryPropertyW(set, &info, property, None, Some(&mut raw), None);
            let _ = SetupDiDestroyDeviceInfoList(set);

            answer.map_err(|error| classify_open_failure(instance, number(error.code().0)))?;

            Ok(as_text(&raw))
        }
    }

    /// The path an instance publishes, read through the class the instance itself says it is in — which is what
    /// keeps a guess out of it: nothing here says that a board in its bootloader is in the USB class.
    fn interface_path(instance: &str) -> DeviceResult<String> {
        let id = nul_terminated(instance);

        unsafe {
            let set = SetupDiGetClassDevsW(
                None,
                PCWSTR(id.as_ptr()),
                None,
                DIGCF_ALLCLASSES | DIGCF_PRESENT,
            )
            .map_err(|error| classify_open_failure(instance, number(error.code().0)))?;

            // cbSize is part of what the call reads, so it is written where the value is made rather than after
            // it, which is the form that keeps the reader from seeing two facts about one struct.
            let mut info = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            if SetupDiEnumDeviceInfo(set, 0, &mut info).is_err() {
                let _ = SetupDiDestroyDeviceInfoList(set);
                return Err(DeviceError::NotOurs {
                    address: instance.to_owned(),
                    reason: "publishes no device interface".to_owned(),
                });
            }

            let class = info.ClassGuid;

            let interfaces = SetupDiGetClassDevsW(
                Some(&class),
                PCWSTR(id.as_ptr()),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
            .map_err(|error| classify_open_failure(instance, number(error.code().0)));

            let interfaces = match interfaces {
                Ok(interfaces) => interfaces,
                Err(error) => {
                    let _ = SetupDiDestroyDeviceInfoList(set);
                    return Err(error);
                }
            };

            let mut own = SP_DEVINFO_DATA {
                cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            let mut iface = SP_DEVICE_INTERFACE_DATA {
                cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                ..Default::default()
            };

            let found_interface = SetupDiEnumDeviceInfo(interfaces, 0, &mut own).is_ok()
                && SetupDiEnumDeviceInterfaces(
                    interfaces,
                    Some(&own as *const SP_DEVINFO_DATA),
                    &class,
                    0,
                    &mut iface,
                )
                .is_ok();

            if !found_interface {
                let _ = SetupDiDestroyDeviceInfoList(interfaces);
                let _ = SetupDiDestroyDeviceInfoList(set);

                return Err(DeviceError::NotOurs {
                    address: instance.to_owned(),
                    reason: "publishes no device interface".to_owned(),
                });
            }

            // The header is what the API says a 64-bit process writes; the room asked for is the header plus the
            // path, which the crate's definition of the struct does not carry.
            let mut raw = vec![0_u8; detail_room()];
            raw[..4].copy_from_slice(&(DETAIL_HEADER as u32).to_le_bytes());

            let detail = raw.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();

            let answer = SetupDiGetDeviceInterfaceDetailW(
                interfaces,
                &iface,
                Some(detail),
                raw.len() as u32,
                None,
                None,
            );

            let _ = SetupDiDestroyDeviceInfoList(interfaces);
            let _ = SetupDiDestroyDeviceInfoList(set);

            answer.map_err(|error| classify_open_failure(instance, number(error.code().0)))?;

            Ok(as_text(&raw[DETAIL_HEADER..]))
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

            if !is_our_driver(&found.driver) {
                return Err(DeviceError::NotOurs {
                    address: target.as_string(),
                    reason: format!("the driver bound to it says {}", found.driver),
                });
            }

            unsafe {
                let path = nul_terminated(&found.path);

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
                    classify_open_failure(&target.as_string(), number(error.code().0))
                })?;

                let mut interface = WINUSB_INTERFACE_HANDLE::default();

                WinUsb_Initialize(handle, &mut interface).map_err(|error| {
                    classify_open_failure(&target.as_string(), number(error.code().0))
                })?;

                let mut settings = USB_INTERFACE_DESCRIPTOR::default();

                WinUsb_QueryInterfaceSettings(interface, 0, &mut settings).map_err(|error| {
                    classify_open_failure(&target.as_string(), number(error.code().0))
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
                "{} — the interface {} publishes, bound to {}",
                self.found.instance,
                u32::from(self.number),
                self.found.driver
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
            classify_open_failure(address, 2).code(),
            "device_unreachable"
        );
        assert_eq!(classify_open_failure(address, 5).code(), "device_not_ours");
        assert_eq!(classify_open_failure(address, 32).code(), "device_not_ours");
        assert_eq!(
            classify_open_failure(address, 4319).code(),
            "device_not_ours"
        );

        // The last one keeps the number as data: a code that turned every failure into one word would tell the
        // operator to look for a board that is already there.
        let error = classify_open_failure(address, 4319);
        assert!(matches!(error, DeviceError::NotOurs { .. }));
        assert!(format!("{error}").contains("4319"));
    }

    #[test]
    fn a_driver_that_is_not_the_one_that_makes_it_ours_is_said_as_the_name_the_registry_gives() {
        assert!(is_our_driver("winusb"));
        assert!(is_our_driver("WinUsb"));
        // What this machine says about the board while it answers as a vehicle: the class key of the port, not a
        // driver. That is data, and it is not `winusb`.
        assert!(!is_our_driver(
            "{4d36e978-e325-11ce-bfc1-08002be10318}\\0035"
        ));
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
