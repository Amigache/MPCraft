use mpcraft_mavlink::dialect::MavMessage;
use mpcraft_mavlink::frame::{calculate_crc, FrameParser};
use mpcraft_mavlink::{MavlinkVersion, Message};

/// The payload of an `ADSB_VEHICLE`, the way a vehicle sends it.
fn payload() -> Vec<u8> {
    let mut bytes = Vec::new();

    bytes.extend_from_slice(&0x00AB_C123_u32.to_le_bytes());
    bytes.extend_from_slice(&(-35.3e7 as i32).to_le_bytes());
    bytes.extend_from_slice(&(149.1e7 as i32).to_le_bytes());
    bytes.extend_from_slice(&800_000_i32.to_le_bytes());
    bytes.extend_from_slice(&4500_u16.to_le_bytes());
    bytes.extend_from_slice(&4500_u16.to_le_bytes());
    bytes.extend_from_slice(&100_i16.to_le_bytes());
    bytes.extend_from_slice(&31_u16.to_le_bytes());
    bytes.extend_from_slice(&1200_u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(b"TEST123\0");
    bytes.push(0);
    bytes.push(0);

    bytes
}

#[test]
fn the_traffic_of_the_air_decodes() {
    let decoded = MavMessage::parse(MavlinkVersion::V2, 246, &payload());

    match decoded {
        Ok(MavMessage::ADSB_VEHICLE(traffic)) => {
            assert_eq!(traffic.ICAO_address, 0x00AB_C123);
            assert_eq!(traffic.heading, 4500);
        }
        Ok(other) => panic!("expected the traffic of the air, was {other:?}"),
        Err(error) => panic!("the traffic of the air could not be decoded: {error:?}"),
    }
}

#[test]
fn the_frame_of_the_traffic_of_the_air_is_read() {
    let payload = payload();

    // The frame, the way a vehicle sends it: the zeros at the end are trimmed.
    let mut body = vec![payload.len() as u8, 0, 0, 7, 1, 1];
    body.extend_from_slice(&246_u32.to_le_bytes()[..3]);
    body.extend_from_slice(&payload);

    let crc = calculate_crc(&body, 184);

    let mut frame = vec![0xFD];
    frame.extend_from_slice(&body);
    frame.extend_from_slice(&crc.to_le_bytes());

    let mut parser = FrameParser::new();
    let mut frames = Vec::new();
    parser.feed(&frame, &mut frames).expect("the frame is read");

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].message_id, 246);

    let decoded = MavMessage::parse(MavlinkVersion::V2, 246, &frames[0].payload)
        .expect("the payload is decoded");

    match decoded {
        MavMessage::ADSB_VEHICLE(traffic) => {
            assert_eq!(traffic.ICAO_address, 0x00AB_C123);
        }
        other => panic!("expected the traffic of the air, was {other:?}"),
    }
}
