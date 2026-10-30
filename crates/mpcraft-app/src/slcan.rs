//! DroneCAN through a serial port in SLCAN mode.
//!
//! A vehicle is not the only way to a CAN bus. An adapter of the Lawicel family, plugged into the
//! machine and switched to SLCAN, is a bus of its own: the station opens the port, sets the rate,
//! and every frame is a line of text each way. Mission Planner carries both paths
//! (`ConfigDroneCAN.cs`); this is the second one, and it is what makes the sheet usable without an
//! autopilot that forwards.
//!
//! What is asked of the port is only two things — write these bytes, read some bytes — so the client
//! below is the same whether the port is a real adapter or a scripted one, and the framing lives in
//! `mpcraft_dronecan::slcan`.

use std::time::Duration;

use mpcraft_dronecan::can::{encode, CanId, Kind, Reassembler};
use mpcraft_dronecan::messages::DataType;
use mpcraft_dronecan::slcan::{close_line, encode as encode_slcan, open_lines, Decoder};
use mpcraft_transport::Transport;

use crate::error::CommandError;

/// The node id this station speaks as on a bus.
///
/// DroneCAN numbers its nodes 1 to 127, and a station is one of them: it has to have a number to send
/// a service request, and 127 is the last one, which is where a station belongs.
pub const STATION_NODE_ID: u8 = 127;

/// How long one exchange with a node is given over a serial bus.
///
/// A node on a bus answers in milliseconds; the budget is what a station waiting on a port that also
/// carries everything else gives it before saying it did not answer.
#[must_use]
pub const fn call_timeout() -> Duration {
    Duration::from_millis(1_500)
}

/// A bus reached through a serial port.
pub struct SlcanBus {
    /// The port, as a stream of bytes.
    port: Box<dyn Transport>,
    /// The lines that arrived and have not been read as frames yet.
    decoder: Decoder,
}

impl SlcanBus {
    /// Opens the port at a rate, and leaves the bus open.
    ///
    /// # Errors
    ///
    /// Returns an error when the rate is not one the protocol numbers, when the port cannot be
    /// written, or when it does not confirm that the bus is open.
    pub async fn open(
        mut port: Box<dyn Transport>,
        bits_per_second: u32,
    ) -> Result<Self, CommandError> {
        let lines = open_lines(bits_per_second).ok_or_else(|| {
            CommandError::invalid_request(format!(
                "{bits_per_second} bits per second is not a rate this adapter takes"
            ))
        })?;

        port.write(&lines).await.map_err(|error| {
            CommandError::internal(format!("the adapter could not be opened: {error}"))
        })?;

        Ok(Self {
            port,
            decoder: Decoder::new(),
        })
    }

    /// Closes the bus. A port that cannot be told so is not an error worth showing: the station is
    /// finished with it either way.
    pub async fn close(&mut self) {
        let _ = self.port.write(&close_line()).await;
    }

    /// Asks one node one thing, and hands back the payload of its answer.
    ///
    /// The frames of the request go out first, then the lines are read until the answer is whole or
    /// the budget runs out. What arrives from another node, or of another type, is fed to the
    /// reassembly and dropped: a bus carries everything, and the answer to a question is the one that
    /// matches it.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be written, or when the node does not answer in time.
    pub async fn call(
        &mut self,
        node: u8,
        payload: &[u8],
        data_type: DataType,
        timeout: Duration,
    ) -> Result<Vec<u8>, CommandError> {
        let can_id = CanId::service(
            mpcraft_dronecan::NOMINAL_PRIORITY,
            data_type.id() as u8,
            true,
            node,
            STATION_NODE_ID,
        );

        let frames = encode(can_id, 0, payload, data_type.signature());

        for frame in &frames {
            self.port
                .write(&encode_slcan(frame))
                .await
                .map_err(|error| {
                    CommandError::internal(format!("the frame could not be written: {error}"))
                })?;
        }

        let mut reassembler = Reassembler::default();
        let mut buffer = [0_u8; 256];
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());

            if left.is_zero() {
                return Err(CommandError::new(
                    CommandError::CODE_TIMEOUT,
                    format!("the node {node} did not answer"),
                ));
            }

            let read = match tokio::time::timeout(left, self.port.read(&mut buffer)).await {
                Ok(Ok(count)) => count,
                Ok(Err(error)) => {
                    return Err(CommandError::internal(format!(
                        "the adapter could not be read: {error}"
                    )))
                }
                Err(_) => continue,
            };

            if read == 0 {
                continue;
            }

            for frame in self.decoder.push(&buffer[..read]) {
                let Some(received) = reassembler.accept(&frame, DataType::signature_of) else {
                    continue;
                };

                if received.can_id.kind == Kind::Response
                    && received.can_id.type_id == data_type.id()
                    && received.can_id.source == node
                {
                    return Ok(received.payload);
                }
            }
        }
    }
}

impl Drop for SlcanBus {
    fn drop(&mut self) {
        // The line that closes the bus is written by `close`, which is asynchronous; a station that
        // drops the bus without it leaves the adapter to time out, which is what the protocol says
        // happens and is not worth blocking a shutdown for.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use mpcraft_transport::TransportKind;

    /// A port that answers with lines written down before the call, and records what was written.
    struct ScriptedPort {
        to_read: Arc<Mutex<VecDeque<u8>>>,
        written: Arc<Mutex<Vec<u8>>>,
    }

    #[async_trait::async_trait]
    impl Transport for ScriptedPort {
        async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let mut to_read = self.to_read.lock().expect("the script");
            let mut count = 0;

            while count < buf.len() {
                match to_read.pop_front() {
                    Some(byte) => {
                        buf[count] = byte;
                        count += 1;
                    }
                    None => break,
                }
            }

            Ok(count)
        }

        async fn write_some(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.written
                .lock()
                .expect("the record")
                .extend_from_slice(buf);

            Ok(buf.len())
        }

        fn kind(&self) -> TransportKind {
            TransportKind::Serial
        }

        fn is_open(&self) -> bool {
            true
        }

        async fn reconnect(&mut self) -> std::io::Result<()> {
            Ok(())
        }

        fn describe(&self) -> String {
            "a scripted adapter".to_owned()
        }
    }

    fn port(answers: &[u8]) -> (ScriptedPort, Arc<Mutex<Vec<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));

        (
            ScriptedPort {
                to_read: Arc::new(Mutex::new(answers.iter().copied().collect())),
                written: Arc::clone(&written),
            },
            written,
        )
    }

    #[tokio::test]
    async fn the_port_is_opened_with_the_rate_and_the_bus_command() {
        let (scripted, written) = port(b"");
        let mut bus = SlcanBus::open(Box::new(scripted), 500_000)
            .await
            .expect("the bus opens");

        bus.close().await;

        let written = written.lock().expect("the record").clone();

        assert_eq!(&written[..5], b"S6\rO\r");
        assert_eq!(&written[5..], b"C\r");
    }

    #[tokio::test]
    async fn a_rate_the_protocol_does_not_number_is_refused() {
        let (scripted, _) = port(b"");

        assert!(SlcanBus::open(Box::new(scripted), 123_456).await.is_err());
    }

    #[tokio::test]
    async fn an_answer_of_one_frame_is_read() {
        // A restart answered with `ok`, which is one byte, on the line the adapter would write.
        let can_id = CanId::service(
            mpcraft_dronecan::NOMINAL_PRIORITY,
            DataType::RestartNode.id() as u8,
            false,
            127,
            42,
        );
        let answer = encode_slcan(&mpcraft_dronecan::Frame {
            id: can_id,
            data: vec![1, 0xC0],
        });

        let (scripted, written) = port(&answer);
        let mut bus = SlcanBus::open(Box::new(scripted), 1_000_000)
            .await
            .expect("the bus opens");

        let payload = bus
            .call(
                42,
                &[0x1E, 0x1B, 0x55, 0xCE, 0xAC],
                DataType::RestartNode,
                call_timeout(),
            )
            .await
            .expect("the node answers");

        assert_eq!(payload, vec![1]);

        let written = written.lock().expect("the record").clone();
        let text = String::from_utf8_lossy(&written);

        assert!(
            text.contains("T"),
            "the request went out as SLCAN lines: {text}"
        );
        assert!(text.contains("1E1B55CEAC"), "the magic is in the request");
    }

    #[tokio::test]
    async fn an_answer_for_another_node_is_not_taken_as_the_answer() {
        // The same answer, addressed from another node.
        let can_id = CanId::service(
            mpcraft_dronecan::NOMINAL_PRIORITY,
            DataType::RestartNode.id() as u8,
            false,
            127,
            9,
        );
        let answer = encode_slcan(&mpcraft_dronecan::Frame {
            id: can_id,
            data: vec![1, 0xC0],
        });

        let (scripted, _) = port(&answer);
        let mut bus = SlcanBus::open(Box::new(scripted), 1_000_000)
            .await
            .expect("the bus opens");

        let answer = bus
            .call(
                42,
                &[0x1E, 0x1B, 0x55, 0xCE, 0xAC],
                DataType::RestartNode,
                Duration::from_millis(50),
            )
            .await;

        assert!(answer.is_err(), "another node's answer is not this one's");
    }
}
