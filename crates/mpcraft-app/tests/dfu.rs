//! The rules of the device layer, answered by a scripted peer rather than by a hand-written one.
//!
//! The numbers the module carries come from three published readings that were not built from each other
//! (cherryusb's `class/dfu/usb_dfu.h`, micropython's `tools/pydfu.py`, ArduPilot's `Tools/renode/dfu.py`)
//! and from the companion writer (`betaflight`'s `dfuse-pack.py`). The literals below are what those
//! readings give for the same bytes; `scripts/check-dfu-crc.js` recomputes them with its own table written
//! from the polynomial, which is what says the literals did not come from the module being checked.
//!
//! What can be asserted here is the shape: the descriptor, the string, the state machine, the block rule,
//! and the refusals that happen before anything reaches the wire. What cannot be asserted is said rather
//! than assumed — there is no physical bus on this machine, so nothing here proves that a real STM32
//! answers these requests; that half lives behind `hardware-tests` against the board.

use async_trait::async_trait;

use mpcraft_app::dfu::{Answer, Command, Flash, Layout, Request, State, Status};
use mpcraft_transport::{Device, DeviceError, DeviceResult, FunctionalDescriptor};

/// Lo que una lectura independiente del formato da para el cuerpo del fixture del modulo, trabajado con la
/// formula que tres implementaciones publicadas escriben igual: `0xFFFFFFFF & -zlib.crc32(data) - 1`.
const FIXTURE_CRC: u32 = 0xbceb_c9b4;

/// Lo que la lectura del espectro da por los mismos bytes: otro numero, registrado como desacuerdo entre
/// las dos lecturas y no como algo cerrado por la mano.
const FIXTURE_SPEC_CRC: u32 = 0xa02b_6bff;

/// Las muestras, por separado, de las dos lecturas.
const SAMPLES: &[(&[u8], u32, u32)] = &[
    (b"", 0xffff_ffff, 0x0000_0000),
    (b"abc", 0xcadb_be3d, 0x9e62_63ff),
    (b"the quick brown fox", 0x6e3e_fd35, 0x0908_d5ff),
    (b"mpcraft fixture\x00", 0x8955_0363, 0x3b72_88ff),
    (b"golden", 0xcc00_79d8, 0xfc0a_02ff),
];

/// El descriptor funcional, como una lectura publicada lo escribe en el cable para esta placa:
/// `struct.pack("<BBBHHH", 9, 0x21, 0x0B, 255, 2048, 0x011A)`.
const DESCRIPTOR_BYTES: &[u8] = &[9, 0x21, 0x0B, 0xFF, 0x00, 0x00, 0x08, 0x1A, 0x01];

/// Las dos formas publicadas de la misma cadena de memoria para 1024K: la que el espectro escribe y la que
/// el propio escritor de ArduPilot produce para esta familia.
const SPEC_LAYOUT: &str = "@Internal Flash /0x08000000/000016*1024,0001*64*1024,0007*128*1024";
const ARDUPILOT_LAYOUT: &str = "@Internal Flash /0x08000000/04*016Kg,01*064Kg,07*128Kg";

/// Lo que el peer pone en el cable y lo que contesta, contado: una negativa decidida antes de enviar tiene
/// que dejar los contadores como estaban, porque si no la regla solo diria lo que el codigo decidio.
#[derive(Debug)]
struct Wire {
    sends: usize,
    receives: usize,
    last_value: u16,
    last_payload: Vec<u8>,
}

/// Un peer que responde las peticiones de clase como las responde la lectura publicada de ArduPilot: con la
/// maquina de estados, el numero de bloque, y la negativa a un sitio fuera de su memoria o a un bloque que
/// no lleva datos.
struct Peer {
    descriptor: FunctionalDescriptor,
    layout_text: String,
    sectors: Vec<(u32, u32)>,
    state: State,
    status: Status,
    busy_once: bool,
    address: u32,
    wire: Wire,
}

impl Peer {
    fn new(layout_text: &str) -> Self {
        let layout = Layout::read(layout_text).expect("the string the peer carries");

        Self {
            descriptor: FunctionalDescriptor::read(DESCRIPTOR_BYTES).expect("nine bytes"),
            layout_text: layout_text.to_owned(),
            sectors: layout
                .sectors
                .iter()
                .map(|sector| (sector.address, sector.size))
                .collect(),
            state: State::DfuIdle,
            status: Status::Ok,
            busy_once: false,
            address: 0,
            wire: Wire {
                sends: 0,
                receives: 0,
                last_value: 0,
                last_payload: Vec::new(),
            },
        }
    }

    fn holds(&self, place: u32, length: u32) -> bool {
        self.sectors
            .iter()
            .any(|(start, size)| place >= *start && place + length <= start + size)
    }

    /// Lo que la lectura publicada hace con un comando de bloque 0 (`_dfuse_command`): el numero de la orden
    /// y lo que puede leer de sus slots. La forma de cinco bytes es la unica que esta lectura entiende para
    /// borrar; la del solo byte que dfu-util y pydfu escriben no le entra, y eso es el desacuerdo que el test
    /// de la orden de borrado registra.
    fn command(&mut self, payload: &[u8]) -> Result<(), String> {
        match payload.first() {
            Some(byte) if *byte == Command::SetAddressPointer.as_byte() && payload.len() == 5 => {
                let address = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
                if self.holds(address, 0) {
                    self.address = address;
                    Ok(())
                } else {
                    Err(format!("0x{address:08X} is outside internal flash"))
                }
            }
            Some(byte) if *byte == Command::Erase.as_byte() && payload.len() == 5 => {
                let address = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
                if self.sectors.iter().any(|(start, _)| *start == address) {
                    Ok(())
                } else {
                    Err(format!("0x{address:08X} is not a sector boundary"))
                }
            }
            Some(byte) if *byte == Command::ReadUnprotect.as_byte() => Ok(()),
            Some(_) => Err("unsupported DfuSe command".to_owned()),
            None => Err("no command to read".to_owned()),
        }
    }
}

#[async_trait]
impl Device for Peer {
    fn functional_descriptor(&self) -> &FunctionalDescriptor {
        &self.descriptor
    }

    async fn send(&mut self, number: u8, value: u16, payload: &[u8]) -> DeviceResult<()> {
        self.wire.sends += 1;
        self.wire.last_value = value;
        self.wire.last_payload = payload.to_vec();

        match Request::from_byte(number) {
            Some(Request::Download) => {
                // La lectura publicada de ArduPilot (`_download`) decide el estado por el bloque y por lo que
                // el bloque trae: un DNLOAD vacio es la senal de aplicar lo escrito, un bloque 0 lleva un
                // comando, y un bloque 2 o mas lleva datos en el sitio que la regla del bloque dice. Lo que no
                // se entiende se contesta con el numero del diccionario y el estado de error, y la marca de
                // "ocupado una vez" se pone en cada peticion, no una sola vez por peer.
                if payload.is_empty() {
                    self.state = State::ManifestSync;
                    self.busy_once = false;
                    return Ok(());
                }

                let accepted = if value == 0 {
                    self.command(payload)
                } else if value == 1 {
                    Err("block 1 does not carry bytes".to_owned())
                } else {
                    let place =
                        self.address + (value - 2) as u32 * self.descriptor.transfer_size as u32;
                    if self.holds(place, payload.len() as u32) {
                        Ok(())
                    } else {
                        Err(format!("0x{place:08X} is outside internal flash"))
                    }
                };

                match accepted {
                    Ok(()) => {
                        self.state = State::DownloadSync;
                        self.busy_once = true;
                    }
                    Err(_) => {
                        self.status = Status::ErrAddress;
                        self.state = State::DfuError;
                    }
                }
            }
            Some(Request::Detach) => self.state = State::AppDetach,
            Some(Request::ClearStatus) => self.status = Status::Ok,
            Some(Request::Abort) => self.state = State::DfuIdle,
            _ => {
                return Err(DeviceError::BadAnswer {
                    number,
                    reason: "not a request the peer answers in this direction".to_owned(),
                })
            }
        }

        Ok(())
    }

    async fn receive(&mut self, number: u8, value: u16, length: u16) -> DeviceResult<Vec<u8>> {
        self.wire.receives += 1;

        match Request::from_byte(number) {
            Some(Request::GetStatus) => {
                // La maquina que la lectura publicada de ArduPilot implementa (`_get_status`): la marca de
                // ocupado se consume al contestar, y el tiempo que se dice con ella es el que la propia
                // lectura escribe — un milimetro mientras esta ocupado, cero cuando ya no lo esta.
                let mut poll = 0;

                match self.state {
                    State::DownloadSync => {
                        if self.busy_once {
                            self.state = State::DownloadBusy;
                            poll = 1;
                        } else {
                            self.state = State::DownloadIdle;
                        }
                        self.busy_once = false;
                    }
                    State::DownloadBusy => self.state = State::DownloadIdle,
                    State::ManifestSync => self.state = State::Manifest,
                    _ => {}
                }

                if length != 6 {
                    return Err(DeviceError::BadAnswer {
                        number,
                        reason: format!("{length} bytes asked for, not the 6"),
                    });
                }

                Ok(vec![self.status.as_byte(), poll, 0, 0, self.state as u8, 0])
            }
            Some(Request::GetState) => Ok(vec![self.state as u8]),
            Some(Request::Upload) => {
                if value < 2 || self.descriptor.attributes & 0x02 == 0 {
                    return Err(DeviceError::Stalled { number });
                }

                let place =
                    self.address + (value - 2) as u32 * self.descriptor.transfer_size as u32;

                if !self.holds(place, length as u32) {
                    return Err(DeviceError::BadAnswer {
                        number,
                        reason: format!("0x{place:08X} is outside the peer's memory"),
                    });
                }

                self.state = State::UploadIdle;
                Ok(vec![0_u8; length as usize])
            }
            _ => Err(DeviceError::BadAnswer {
                number,
                reason: "not a request the peer answers in this direction".to_owned(),
            }),
        }
    }

    async fn string(&mut self, index: u16) -> DeviceResult<String> {
        self.wire.receives += 1;

        if index != 4 {
            return Err(DeviceError::Stalled { number: 0x06 });
        }

        let units: Vec<u8> = self
            .layout_text
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();

        let mut bytes = vec![units.len() as u8 + 2, 3];
        bytes.extend(units);

        mpcraft_transport::read_string(&bytes)
    }

    fn describe(&self) -> String {
        "scripted peer".to_owned()
    }
}

#[tokio::test]
async fn the_golden_numbers_come_from_a_second_implementation_of_the_format_rule() {
    for (bytes, published, spec_worded) in SAMPLES {
        assert_eq!(mpcraft_app::dfu::crc_rule(bytes), *published);
        assert_ne!(mpcraft_app::dfu::crc_rule(bytes), *spec_worded);
    }

    assert_eq!(mpcraft_app::dfu::crc_rule(&fixture_body()), FIXTURE_CRC);
    assert_ne!(
        mpcraft_app::dfu::crc_rule(&fixture_body()),
        FIXTURE_SPEC_CRC
    );
}

/// El cuerpo del fixture del modulo — el prefijo, un objetivo nombrado `MPCRAFT F405` con dos elementos y
/// el sufijo de dieciseis bytes con los numeros que esta placa dio de sí misma — escrito desde las reglas
/// del formato por el camino del test, para leerlo por otro lado.
fn fixture_body() -> Vec<u8> {
    let element_one = b"mpcraft fixture\x00";
    let element_two = b"golden";
    let data_len = 8 + element_one.len() + 8 + element_two.len();

    let mut name_field = [0_u8; 255];
    name_field[..12].copy_from_slice(b"MPCRAFT F405");

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"DfuSe");
    bytes.push(1);
    // El campo de tamano dice cuanto mide el objetivo entero — su prefijo de 274 bytes y sus datos — con los
    // once bytes del encabezado delante, que es lo que el escritor publicado escribe.
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
    bytes.extend_from_slice(&0x0413_u16.to_le_bytes());
    bytes.extend_from_slice(&0xDF11_u16.to_le_bytes());
    bytes.extend_from_slice(&0x0483_u16.to_le_bytes());
    bytes.extend_from_slice(&0x011A_u16.to_le_bytes());
    bytes.extend_from_slice(b"UFD");
    bytes.push(16);

    bytes
}

#[tokio::test]
async fn the_descriptor_is_the_shape_a_published_implementation_writes_for_this_board() {
    let descriptor =
        FunctionalDescriptor::read(DESCRIPTOR_BYTES).expect("nine bytes of the right type");

    assert_eq!(descriptor.attributes, 0x0B);
    assert_eq!(descriptor.detach_timeout, 255);
    assert_eq!(descriptor.transfer_size, 2048);
    assert_eq!(descriptor.version, 0x011A);
    assert_eq!(
        descriptor.capabilities(),
        vec![
            mpcraft_transport::Capability::WillDetach,
            mpcraft_transport::Capability::CanUpload,
            mpcraft_transport::Capability::CanDownload
        ]
    );
}

#[tokio::test]
async fn the_two_published_readings_of_the_memory_string_agree_only_where_the_unit_letter_is_written(
) {
    // El peer lleva la cadena que el propio escritor de ArduPilot produce; la otra es la forma que el
    // espectro escribe para la misma familia. Los dos lectores publicados del token sin letra de unidad
    // (dfu-util y pydfu) toman el numero como el tamano en bytes y solo multiplican cuando la letra esta,
    // asi que las dos cadenas no dan los mismos sectores: 12 que son la tierra entera frente a 24 tramos
    // que suman 17 344. Eso es lo que se compara aqui, y el desacuerdo se registra en lugar de cerrarlo.
    let spec_form = Layout::read(SPEC_LAYOUT).expect("the string the specification gives");
    let writer_form =
        Layout::read(ARDUPILOT_LAYOUT).expect("the string the published writer produces");

    let covered = |layout: &Layout| layout.sectors.iter().map(|sector| sector.size).sum::<u32>();

    assert_eq!(spec_form.base, writer_form.base);
    assert_eq!(covered(&writer_form), 1_048_576);
    assert_eq!(covered(&spec_form), 17_344);

    assert_eq!(writer_form.sectors.len(), 12);
    assert_eq!(spec_form.sectors.len(), 24);
    assert!(writer_form.sectors.iter().all(|sector| sector.erasable()));
    assert!(spec_form.sectors.iter().all(|sector| !sector.erasable()));

    // El peer se queda con la lectura del escritor publicado, que es la cadena que la placa trae en su
    // descriptor de interfaz; un sector que no dice permiso no se puede borrar y lo que el test afirma es
    // que la negativa llega antes de que nada viaje por el cable.
    assert_eq!(writer_form.sectors[0].size, 16_384);
    assert_eq!(
        writer_form.sectors[0].permission.map(|p| p.letter),
        Some('g')
    );
}

#[tokio::test]
async fn the_state_machine_walks_as_the_published_peer_implements_it() {
    let peer = Peer::new(ARDUPILOT_LAYOUT);
    let mut flash = Flash::open(peer, 4).await.expect("the peer answers");

    assert_eq!(flash.state().await.unwrap(), State::DfuIdle);

    // Un comando en bloque 0: la lectura publicada pasa por BUSY y luego queda en DOWNLOAD_IDLE, y el tiempo
    // que dice mientras esta ocupado es el milimetro que la propia lectura escribe.
    let command = flash
        .set_address(0x0800_0000)
        .await
        .expect("inside the peer's memory");
    assert_eq!(command.state, State::DownloadBusy);
    assert_eq!(
        command.poll_ms, 1,
        "the published peer answers one millisecond while busy"
    );
    assert_eq!(flash.device().wire.last_value, 0);
    assert_eq!(
        flash.device().wire.last_payload,
        vec![0x21, 0x00, 0x00, 0x00, 0x08]
    );

    // Una escritura: la marca de ocupado se pone en cada peticion, asi que el primer GETSTATUS dice BUSY y el
    // segundo DOWNLOAD_IDLE — no es que el peer estuviera ocupado una sola vez para todo el test.
    let first = flash
        .write(b"golden")
        .await
        .expect("the run goes out on block 2");
    assert_eq!(first.state, State::DownloadBusy);
    assert_eq!(first.poll_ms, 1, "the same answer for a second request");
    assert_eq!(flash.device().wire.last_value, 2);

    let second = flash.read_answer().await.expect("the peer answers");
    assert_eq!(second.state, State::DownloadIdle);

    // El peer contesta con la espera ya consumida y sin ofrecer una cadena: los dos campos se leen como lo
    // que el formato dice que son, no como lo que el paso acaba de hacer.
    let quiet = Answer {
        status: Status::Ok,
        poll_ms: 0,
        state: State::DownloadIdle,
        string_index: 0,
    };
    assert_eq!(second, quiet);

    // El cierre: un DNLOAD vacio deja al peer en MANIFEST_SYNC y el GETSTATUS que sigue responde MANIFEST,
    // porque esta lectura hace la transicion al contestar y no al recibir. Lo que se afirma es la respuesta;
    // el estado que el envio deja detrás solo se veria con un GETSTATE entre los dos pasos, y eso no se puede
    // pedir sin partir el cierre en sus dos pasos del cable.
    let closed = flash
        .finish()
        .await
        .expect("the empty run is the finish signal");
    assert_eq!(closed.state, State::Manifest);
}

#[tokio::test]
async fn a_refusal_happens_before_anything_reaches_the_wire() {
    let peer = Peer::new(ARDUPILOT_LAYOUT);
    let mut flash = Flash::open(peer, 4).await.expect("the peer answers");

    // Lo que la apertura ya conto se toma como base; las negativas de abajo no añaden nada.
    let before_sends = flash.device().wire.sends;
    let before_receives = flash.device().wire.receives;

    assert!(
        flash.set_address(0x1000_0000).await.is_err(),
        "outside the memory"
    );
    assert!(
        flash.erase_sector(0x0800_2000).await.is_err(),
        "inside the memory but not a sector start"
    );
    assert!(flash.read(1, 64).await.is_err(), "block 1 carries no bytes");
    assert!(
        flash.write(&vec![0_u8; 2049]).await.is_err(),
        "more than the interface says it can take"
    );

    assert_eq!(flash.device().wire.sends, before_sends, "nothing was sent");
    assert_eq!(
        flash.device().wire.receives,
        before_receives,
        "nothing was asked for"
    );

    // Y la negativa por el permiso, que es la otra mitad de la regla: una cadena que dice `a` describe un
    // sector que se lee y no se borra, y eso se decide antes de que nada viaje por el cable. Lo que la placa
    // trae en su descriptor de interfaz escribe `g` para sus sectores, que es lo que el peer de arriba lleva.
    let quiet_peer = Peer::new("@Internal Flash /0x08000000/01*016Ka");
    let mut read_only = Flash::open(quiet_peer, 4)
        .await
        .expect("the peer answers with its string");
    let open_counts = (
        read_only.device().wire.sends,
        read_only.device().wire.receives,
    );

    assert!(
        read_only.erase_sector(0x0800_0000).await.is_err(),
        "`a` does not erase"
    );
    assert_eq!(
        read_only.device().wire.sends,
        open_counts.0,
        "nothing was sent"
    );
    assert_eq!(
        read_only.device().wire.receives,
        open_counts.1,
        "nothing was asked for"
    );
}

#[tokio::test]
async fn the_block_rule_is_a_place_and_not_an_index_of_a_buffer() {
    let peer = Peer::new(ARDUPILOT_LAYOUT);
    let mut flash = Flash::open(peer, 4).await.expect("the peer answers");

    flash
        .set_address(0x0801_0000)
        .await
        .expect("the sector boundary");

    // Bloque 2 es el sitio, y bloque 3 es el sitio mas lo que la interfaz dice que puede tomar en una
    // peticion; los dos se leen contra el cable que el peer recibio.
    flash.write(&[0x01]).await.expect("a run on block 2");
    assert_eq!(flash.device().wire.last_value, 2);

    flash.write(&[0x02]).await.expect("a run on block 3");
    assert_eq!(flash.device().wire.last_value, 3);
}

#[tokio::test]
async fn the_mass_erase_form_is_a_disagreement_between_two_readings_and_this_code_names_its_side() {
    // Tres lecturas publicadas, dos formas. dfu-util (`src/dfuse.c`, `dfuse_special_command`) escribe el numero
    // con longitud 1 para el borrado de todo el dispositivo — su propio comentario dice "Mass erase command
    // when length = 1" — y pydfu manda `"\x41"` solo; la lectura publicada de ArduPilot (`Tools/renode/dfu.py`,
    // `_dfuse_command`) entiende solo la forma de cinco bytes y contesta la otra como una orden que no se
    // entiende. Este codigo sigue la de dfu-util, y lo que el test afirma son las dos mitades: el numero que
    // pone en el cable y lo que la otra lectura habria contestado con los mismos bytes.
    let peer = Peer::new(ARDUPILOT_LAYOUT);
    let mut flash = Flash::open(peer, 4).await.expect("the peer answers");

    let answer = flash.mass_erase().await;
    assert_eq!(
        flash.device().wire.last_payload,
        vec![0x41],
        "the single byte this code writes"
    );
    assert_eq!(
        flash.device().wire.last_value,
        0,
        "a command travels on block 0"
    );

    let error = answer.expect_err("the published reading refused the same bytes");
    assert_eq!(
        error.code, "rejected_by_vehicle",
        "a refusal, not a silence"
    );
    assert!(
        error.message.contains("ERR_ADDRESS"),
        "the word the dictionary gives that number"
    );

    // Lo que no se puede afirmar aqui es cual de las dos formas contesta una placa real; esa mitad vive detrás
    // de `hardware-tests`, donde la placa es la que decide.
}

#[tokio::test]
async fn a_status_that_is_not_ok_reaches_the_interface_as_a_refusal_with_the_dictionary_word() {
    let mut peer = Peer::new(ARDUPILOT_LAYOUT);
    // El peer responde con ERR_ADDRESS: el codigo que la interfaz lee es el de una negativa, no el de un
    // paso que salio bien, y la palabra es la que el diccionario da al numero.
    peer.status = Status::ErrAddress;
    let mut flash = Flash::open(peer, 4).await.expect("the peer answers");

    let error = flash
        .erase_sector(0x0800_0000)
        .await
        .expect_err("the peer refused");

    assert_eq!(error.code, "rejected_by_vehicle");
    assert!(error.message.contains("ERR_ADDRESS"));
}

#[tokio::test]
async fn the_codes_of_the_device_layer_survive_the_trip_through_the_command_layer() {
    // Un stall y una negativa son dos cosas distintas para quien lee el codigo, y el frontend nunca lee el
    // mensaje para buscar significado.
    let stalled = mpcraft_app::error::CommandError::from(DeviceError::Stalled { number: 0x01 });
    assert_eq!(stalled.code, "device_stalled");

    let unreachable = mpcraft_app::error::CommandError::from(DeviceError::Unreachable(
        "nothing on the bus".to_owned(),
    ));
    assert_eq!(unreachable.code, "device_unreachable");
}
