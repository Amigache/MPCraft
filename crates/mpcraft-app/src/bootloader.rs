//! The bootloader of the board.
//!
//! When a board is asked to write its own firmware, it answers on the serial port with
//! a small protocol of its own: a byte that says what is wanted, the arguments, and a
//! byte that says the end of the command. The answer is two bytes: `INSYNC`, which
//! says the board understood, and `OK`, `FAILED` or `INVALID`.
//!
//! The exchange is:
//!
//! 1. the board is asked whether it is there (`GET_SYNC`), again and again, until it
//!    answers — it is booting, and booting takes a while;
//! 2. the board is asked what it is (`GET_DEVICE`): the revision of the protocol, the
//!    board, the size of its flash;
//! 3. the file is compared with the board before anything is erased, because an erase is
//!    the part that cannot be taken back;
//! 4. the flash is erased (`CHIP_ERASE`), which takes seconds;
//! 5. the firmware is written in blocks (`PROG_MULTI`);
//! 6. the board is asked for the CRC of what its flash now holds (`GET_CRC`, or `CHIP_VERIFY`
//!    with `READ_MULTI` on revision 2), so that what was written is read from the board rather
//!    than assumed from the send;
//! 7. the board is told to start again (`REBOOT`).
//!
//! A board that already holds this image says so when it is asked for the CRC, and that is not the
//! same as a failure: the station refuses to erase what matches, unless the operator says otherwise.
//!
//! The protocol is spoken over the transport of the project, so that the board is
//! spoken to in the same way as everything else, and it can be tried against a board
//! that is not there.

use std::time::Duration;

use async_trait::async_trait;

use crate::error::{CommandError, CommandResult};

/// The board says it understood the command.
const INSYNC: u8 = 0x12;

/// The command was carried out.
const OK: u8 = 0x10;

/// The command was not carried out.
const FAILED: u8 = 0x11;

/// The command is not one the board knows.
const INVALID: u8 = 0x13;

/// The end of a command.
const EOC: u8 = 0x20;

/// Asks the board whether it is there.
const GET_SYNC: u8 = 0x21;

/// Asks the board what it is.
const GET_DEVICE: u8 = 0x22;

/// Erases the flash of the board.
const CHIP_ERASE: u8 = 0x23;

/// Asks the board to read back what was written, one block at a time (revision 2).
const CHIP_VERIFY: u8 = 0x24;

/// Writes a block of the firmware.
const PROG_MULTI: u8 = 0x27;

/// Reads a block back from the board (revision 2).
const READ_MULTI: u8 = 0x28;

/// Asks the board for the CRC of what its flash holds (revision 3 and newer).
const GET_CRC: u8 = 0x29;

/// Tells the board to start again.
const REBOOT: u8 = 0x30;

/// The oldest revision of the protocol that is spoken.
pub const BL_REV_MIN: u8 = 2;

/// The newest revision of the protocol that is spoken.
pub const BL_REV_MAX: u8 = 20;

/// How many bytes go in one block.
///
/// The protocol counts the length in a byte, and the bootloader wants the blocks to be
/// a multiple of four.
pub const PROG_MULTI_MAX: usize = 252;

/// How many bytes come back from one read.
///
/// The protocol can ask for 255, and the published implementation stops at 252 because something
/// overflows past 64 on the way back.
pub const READ_MULTI_MAX: usize = 252;

/// How long the board is given to answer a command.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

/// How long the board is given to wake up and answer the first question.
pub const WAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// A port the board is spoken to.
///
/// It is what the transport of the project offers, and nothing more: the bootloader
/// only writes bytes and waits for them to come back.
#[async_trait]
pub trait Port: Send {
    /// Writes the bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be written to.
    async fn write_all(&mut self, data: &[u8]) -> CommandResult<()>;

    /// Reads whatever there is.
    ///
    /// The answer is the number of bytes that were read, which is zero when the port
    /// had nothing to say. The caller decides how long it waits.
    ///
    /// # Errors
    ///
    /// Returns an error when the port cannot be read.
    async fn read_some(&mut self, data: &mut [u8]) -> CommandResult<usize>;
}

/// What the board is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Info {
    /// The revision of the protocol of the bootloader.
    BlRev,
    /// The board itself.
    BoardId,
    /// The revision of the board.
    BoardRev,
    /// How big the flash is, in bytes.
    FlashSize,
    /// The size of the vector table of the firmware.
    VecArea,
    /// How big the external flash is, in bytes.
    ExtFlashSize,
}

impl Info {
    /// The byte that asks for this thing.
    const fn as_byte(self) -> u8 {
        match self {
            Self::BlRev => 1,
            Self::BoardId => 2,
            Self::BoardRev => 3,
            Self::FlashSize => 4,
            Self::VecArea => 5,
            Self::ExtFlashSize => 6,
        }
    }
}

/// What the board said about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardInfo {
    /// The revision of the protocol of the bootloader.
    pub bl_rev: u8,
    /// The board itself.
    pub board_id: u32,
    /// The revision of the board.
    pub board_rev: u32,
    /// How big the flash is, in bytes.
    pub flash_size: u32,
}

/// The bootloader of a board.
pub struct Bootloader<P> {
    /// The port the board is spoken to.
    port: P,
}

impl<P: Port> Bootloader<P> {
    /// Speaks to the board over the given port.
    #[must_use]
    pub fn new(port: P) -> Self {
        Self { port }
    }

    /// Asks the board whether it is there, and waits for it to wake up.
    ///
    /// The board is asked again and again: it is booting, and booting takes a while.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer in the time given, or when it
    /// says it does not understand the question.
    pub async fn wake_up(&mut self, deadline: Duration) -> CommandResult<()> {
        let start = std::time::Instant::now();

        loop {
            self.port.write_all(&[GET_SYNC, EOC]).await?;

            if self.try_sync().await? {
                return Ok(());
            }

            if start.elapsed() >= deadline {
                return Err(CommandError::timeout(
                    "the board did not answer in the time given",
                ));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Asks the board whether it is there, once.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub async fn sync(&mut self) -> CommandResult<()> {
        self.port.write_all(&[GET_SYNC, EOC]).await?;
        self.read_sync().await
    }

    /// Asks the board what it is.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub async fn device(&mut self, what: Info) -> CommandResult<u32> {
        self.port
            .write_all(&[GET_DEVICE, what.as_byte(), EOC])
            .await?;

        let mut answer = [0_u8; 4];
        self.read_exact(&mut answer).await?;

        let value = u32::from_le_bytes(answer);

        self.read_sync().await?;

        Ok(value)
    }

    /// Asks the board everything about itself.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub async fn info(&mut self) -> CommandResult<BoardInfo> {
        Ok(BoardInfo {
            bl_rev: self.device(Info::BlRev).await? as u8,
            board_id: self.device(Info::BoardId).await?,
            board_rev: self.device(Info::BoardRev).await?,
            flash_size: self.device(Info::FlashSize).await?,
        })
    }

    /// Asks the board for the CRC of what its flash holds.
    ///
    /// The number is worked out by the board, over the whole area the board says it has, so it is what
    /// the board is rather than what was last sent to it. A board of revision 2 cannot be asked, and
    /// the caller has to read the blocks back instead.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer or refuses the question.
    pub async fn crc(&mut self) -> CommandResult<u32> {
        self.port.write_all(&[GET_CRC, EOC]).await?;

        let mut answer = [0_u8; 4];
        self.read_exact(&mut answer).await?;

        let value = u32::from_le_bytes(answer);

        self.read_sync().await?;

        Ok(value)
    }

    /// Reads one block back from the board, which is the way a board of revision 2 is checked.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer or refuses the question.
    pub async fn read_multi(&mut self, length: usize) -> CommandResult<Vec<u8>> {
        self.port
            .write_all(&[READ_MULTI, length as u8, EOC])
            .await?;

        let mut block = vec![0_u8; length];
        self.read_exact(&mut block).await?;

        self.read_sync().await?;

        Ok(block)
    }

    /// Checks what the flash holds against what was written.
    ///
    /// A board of revision 3 and newer answers with the CRC over its whole area, so that is what is
    /// compared. A board of revision 2 cannot be asked for one, and its blocks are read back one by
    /// one instead — which is why the number of the protocol is read from the board before this is
    /// decided, rather than assumed from the file.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, when it refuses, or when what it says does not
    /// match what was written.
    pub async fn verify(
        &mut self,
        file: &crate::apj::FirmwareImage,
        info: &BoardInfo,
    ) -> CommandResult<()> {
        if info.bl_rev >= 3 {
            let holds = self.crc().await?;
            let expected = file.expected_crc(info.flash_size);

            if holds != expected {
                return Err(CommandError::rejected(format!(
                    "the board says its flash holds {:08x} and what was written gives {expected:08x}",
                    holds
                )));
            }

            return Ok(());
        }

        self.port.write_all(&[CHIP_VERIFY, EOC]).await?;
        self.read_sync().await?;

        for block in file.image.chunks(READ_MULTI_MAX) {
            let programmed = self.read_multi(block.len()).await?;

            if programmed != block {
                return Err(CommandError::rejected(
                    "the board read back a block that is not the one it was given",
                ));
            }
        }

        Ok(())
    }

    /// Erases the flash of the board.
    ///
    /// The board answers with a byte when the flash is clean, and that takes a while:
    /// the caller has to be patient.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub async fn erase(&mut self) -> CommandResult<()> {
        // The bootloader of some boards wants to be asked something before it erases
        // anything: the upstream does the same.
        let _ = self.device(Info::BlRev).await;

        self.port.write_all(&[CHIP_ERASE, EOC]).await?;

        let mut done = [0_u8; 1];
        self.read_exact(&mut done).await?;

        self.read_sync().await
    }

    /// Writes the firmware to the board.
    ///
    /// The firmware is written in blocks, and the callback says how much of it has been
    /// written, so that the operator can see the progress.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the command.
    pub async fn program(
        &mut self,
        firmware: &[u8],
        mut progress: impl FnMut(usize, usize),
    ) -> CommandResult<()> {
        let total = firmware.len();
        let mut written = 0_usize;

        for block in firmware.chunks(PROG_MULTI_MAX) {
            self.port
                .write_all(&[PROG_MULTI, block.len() as u8])
                .await?;
            self.port.write_all(block).await?;
            self.port.write_all(&[EOC]).await?;

            self.read_sync().await?;

            written += block.len();
            progress(written, total);
        }

        Ok(())
    }

    /// Tells the board to start again.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the command.
    pub async fn reboot(&mut self) -> CommandResult<()> {
        self.port.write_all(&[REBOOT, EOC]).await?;
        self.read_sync().await
    }

    /// Reads the answer of the board to a command.
    async fn read_sync(&mut self) -> CommandResult<()> {
        let mut answer = [0_u8; 1];
        self.read_exact(&mut answer).await?;

        if answer[0] != INSYNC {
            return Err(CommandError::rejected(format!(
                "the board answered {:#04x} instead of saying it understood",
                answer[0]
            )));
        }

        self.read_exact(&mut answer).await?;

        match answer[0] {
            OK => Ok(()),
            FAILED => Err(CommandError::rejected(
                "the board said the command was not carried out",
            )),
            INVALID => Err(CommandError::rejected(
                "the board said the command is not one it knows",
            )),
            other => Err(CommandError::rejected(format!(
                "the board answered {other:#04x} instead of saying it was done"
            ))),
        }
    }

    /// Reads the answer of the board, waiting for it.
    ///
    /// The board has the time given to answer; after that, the port is said to be
    /// silent.
    async fn read_exact(&mut self, data: &mut [u8]) -> CommandResult<()> {
        let mut read = 0_usize;
        let start = std::time::Instant::now();

        while read < data.len() {
            let count = self.port.read_some(&mut data[read..]).await?;

            if count > 0 {
                read += count;
                continue;
            }

            if start.elapsed() >= ANSWER_TIMEOUT {
                return Err(CommandError::timeout(
                    "the board did not answer in the time given",
                ));
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        Ok(())
    }

    /// Reads the answer of the board, when it has one.
    ///
    /// The answer is the two bytes that say the board understood. A board that is not
    /// there yet says nothing, and that is not an error: the caller asks again.
    async fn try_sync(&mut self) -> CommandResult<bool> {
        let mut answer = [0_u8; 1];

        if self.port.read_some(&mut answer).await? == 0 {
            return Ok(false);
        }

        if answer[0] != INSYNC {
            return Ok(false);
        }

        if self.port.read_some(&mut answer).await? == 0 {
            return Ok(false);
        }

        Ok(answer[0] == OK)
    }
}

/// The bootloader is spoken to over the transport of the project.
#[async_trait]
impl<T: mpcraft_transport::Transport> Port for T {
    async fn write_all(&mut self, data: &[u8]) -> CommandResult<()> {
        mpcraft_transport::Transport::write(self, data)
            .await
            .map_err(io_error)
    }

    async fn read_some(&mut self, data: &mut [u8]) -> CommandResult<usize> {
        match mpcraft_transport::Transport::read(self, data).await {
            Ok(count) => Ok(count),
            // A port that had nothing to say says so with a timeout, and that is not a
            // failure: the board may simply not be awake yet.
            Err(error) if error.kind() == std::io::ErrorKind::TimedOut => Ok(0),
            Err(error) => Err(io_error(error)),
        }
    }
}

/// Writes a firmware to a board, from the moment it answers to the moment it starts
/// again.
///
/// The board is waited for, asked what it is, compared with the file before anything is erased, its
/// flash is erased, the image is written block by block, what the board says about its own flash is
/// read back, and the board is told to start. The callback says how much of the image has been
/// written, so that the operator can see the progress.
///
/// `allow_same` is what the operator says when the board already holds this image. Without it the same
/// image is not written again, because writing it means erasing first, and an erase is the part that
/// cannot be taken back. A board of revision 2 cannot be asked what its flash holds, so nothing can be
/// said about it before the erase — which is why the guard runs only where the board can answer.
///
/// # Errors
///
/// Returns an error when the board does not answer, when it refuses a command, when the file is not for
/// the board that is on the port, when the image does not fit in its flash, or when what the board says
/// about its flash afterwards is not what was written.
pub async fn write_firmware<P: Port>(
    port: P,
    file: &crate::apj::FirmwareImage,
    progress: impl FnMut(usize, usize),
    allow_same: bool,
) -> CommandResult<BoardInfo> {
    let mut board = Bootloader::new(port);

    board.wake_up(WAKE_TIMEOUT).await?;

    let info = board.info().await?;

    // Both refusals are before the erase, because the erase is the half that cannot be taken back. A
    // file for another board would leave this one with nothing on it, and an image that is already
    // there does not need to be written again.
    if file.board_id != info.board_id {
        return Err(CommandError::invalid_request(format!(
            "the file is for board {} and the board says it is {}",
            file.board_id, info.board_id
        )));
    }

    if info.flash_size > 0 && file.image.len() as u32 > info.flash_size {
        return Err(CommandError::invalid_request(format!(
            "the image is {} bytes and the flash of the board holds {}",
            file.image.len(),
            info.flash_size
        )));
    }

    if !allow_same && info.bl_rev >= 3 {
        let holds = board.crc().await?;

        if holds == file.expected_crc(info.flash_size) {
            return Err(CommandError::rejected(
                "the board already holds this image, so it was not erased for something that is there",
            ));
        }
    }

    board.erase().await?;
    board.program(&file.image, progress).await?;
    board.verify(file, &info).await?;
    board.reboot().await?;

    Ok(info)
}

/// Erases the firmware of a board.
///
/// The board is waited for, asked what it is, and its flash is erased. The board is
/// left without a firmware: it is what the operator wants when the board does not
/// start, and when the firmware that is on it is not the one that should be.
///
/// # Errors
///
/// Returns an error when the board does not answer, or when it refuses the command.
pub async fn erase_firmware<P: Port>(port: P) -> CommandResult<BoardInfo> {
    let mut board = Bootloader::new(port);

    board.wake_up(WAKE_TIMEOUT).await?;

    let info = board.info().await?;

    board.erase().await?;
    board.reboot().await?;

    Ok(info)
}

/// What a failure of the port becomes.
fn io_error(error: std::io::Error) -> CommandError {
    CommandError::internal(format!("the board could not be spoken to: {error}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A board that answers as the real one does, and remembers what it was told.
    #[derive(Default)]
    struct FakeBoard {
        /// What the board was told.
        told: Vec<u8>,
        /// What the board answers, again and again.
        answers: Vec<u8>,
        /// What the board has still to say.
        queue: Vec<u8>,
        /// Whether the board says nothing at all.
        silent: bool,
    }

    impl FakeBoard {
        /// A board that says it understood and that it did what it was told.
        fn ready() -> Self {
            Self {
                told: Vec::new(),
                answers: vec![INSYNC, OK],
                queue: Vec::new(),
                silent: false,
            }
        }

        /// A board that refuses.
        fn refusing() -> Self {
            Self {
                told: Vec::new(),
                answers: vec![INSYNC, FAILED],
                queue: Vec::new(),
                silent: false,
            }
        }

        /// A board that says nothing.
        fn silent() -> Self {
            Self {
                told: Vec::new(),
                answers: Vec::new(),
                queue: Vec::new(),
                silent: true,
            }
        }
    }

    #[async_trait]
    impl Port for FakeBoard {
        async fn write_all(&mut self, data: &[u8]) -> CommandResult<()> {
            self.told.extend_from_slice(data);
            Ok(())
        }

        async fn read_some(&mut self, data: &mut [u8]) -> CommandResult<usize> {
            if self.silent {
                return Ok(0);
            }

            // The board answers the same thing every time it is asked.
            if self.queue.is_empty() {
                self.queue = self.answers.clone();
            }

            if self.queue.is_empty() {
                return Ok(0);
            }

            let count = data.len().min(self.queue.len());
            data[..count].copy_from_slice(&self.queue[..count]);
            self.queue.drain(..count);

            Ok(count)
        }
    }

    #[tokio::test]
    async fn the_handshake_asks_whether_the_board_is_there() {
        let mut board = Bootloader::new(FakeBoard::ready());
        board.sync().await.expect("the board answers");

        assert_eq!(board.port.told, vec![GET_SYNC, EOC]);
    }

    #[tokio::test]
    async fn a_board_that_refuses_is_an_error() {
        let mut board = Bootloader::new(FakeBoard::refusing());
        let error = board.sync().await.expect_err("the board refuses");
        assert_eq!(error.code, CommandError::CODE_REJECTED);
    }

    #[tokio::test]
    async fn the_firmware_is_written_in_blocks() {
        let mut board = Bootloader::new(FakeBoard::ready());
        let firmware = vec![7_u8; PROG_MULTI_MAX * 2 + 5];

        let mut written = 0_usize;
        board
            .program(&firmware, |count, _total| written = count)
            .await
            .expect("the board takes the firmware");

        assert_eq!(written, firmware.len());

        // The blocks go one after the other, and each of them says how long it is.
        let told = board.port.told;
        assert_eq!(told[0], PROG_MULTI);
        assert_eq!(told[1] as usize, PROG_MULTI_MAX);
        // The first block is the opcode, the length, the data and the end of the command.
        assert_eq!(told[PROG_MULTI_MAX + 3], PROG_MULTI);
    }

    #[tokio::test]
    async fn a_board_that_answers_nothing_is_an_error_and_not_a_panic() {
        let mut board = Bootloader::new(FakeBoard::silent());
        let error = board.sync().await.expect_err("the board says nothing");
        assert_eq!(error.code, CommandError::CODE_TIMEOUT);
    }

    #[tokio::test]
    async fn a_board_that_is_not_awake_yet_is_asked_again() {
        let mut board = Bootloader::new(FakeBoard::silent());

        let error = board
            .wake_up(Duration::from_millis(300))
            .await
            .expect_err("the board never wakes up");

        assert_eq!(error.code, CommandError::CODE_TIMEOUT);
    }

    /// A board that answers each command with what was written down for it, and keeps what it was told.
    /// What it says about its own flash is a number, not a sentence, so the test can show what the code
    /// does when the number disagrees with what was written.
    ///
    /// It answers a command when the command is complete rather than when a piece of it arrives, because
    /// the bytes of a block go over the port in three parts — the opcode with its length, the bytes, and
    /// the end — and answering each part would put replies in the queue that nobody asked for.
    struct ScriptedBoard {
        /// What the board was told, kept where the test can still read it after the port has been
        /// handed to the bootloader.
        told: Arc<Mutex<Vec<u8>>>,
        /// What the board says to each complete command.
        replies: BTreeMap<Vec<u8>, Vec<u8>>,
        /// What the board has still to say.
        queue: Vec<u8>,
        /// The command being assembled, from its opcode to its end.
        command: Vec<u8>,
        /// How many payload bytes are still expected for the command being assembled.
        payload: usize,
        /// Whether the board is reading an opcode, a length, the bytes, or the end.
        step: Step,
    }

    /// The parts of a command, as the protocol puts them on the wire.
    #[derive(Debug, PartialEq)]
    enum Step {
        Opcode,
        Length,
        Payload,
        End,
    }

    impl ScriptedBoard {
        fn answering(replies: BTreeMap<Vec<u8>, Vec<u8>>) -> Self {
            Self {
                told: Arc::new(Mutex::new(Vec::new())),
                replies,
                queue: Vec::new(),
                command: Vec::new(),
                payload: 0,
                step: Step::Opcode,
            }
        }

        /// Records what the board was told and says what it thinks of it.
        fn receive(&mut self, byte: u8) {
            if let Ok(mut kept) = self.told.lock() {
                kept.push(byte);
            }

            match self.step {
                Step::Opcode => {
                    self.command.push(byte);

                    if byte == EOC {
                        self.answer();
                    } else if byte == PROG_MULTI {
                        // Only the writing command carries bytes from the station. The read command says
                        // how many bytes the board should answer with, and its end comes straight after
                        // the number — which is what the protocol itself says (`PROG_MULTI/<len>/<data>/EOC`
                        // against `READ_MULTI/<len>/EOC`).
                        self.step = Step::Length;
                    } else {
                        self.step = Step::End;
                    }
                }
                Step::Length => {
                    self.command.push(byte);
                    self.payload = usize::from(byte);

                    self.step = if self.payload == 0 {
                        Step::End
                    } else {
                        Step::Payload
                    };
                }
                Step::Payload => {
                    self.command.push(byte);
                    self.payload -= 1;

                    if self.payload == 0 {
                        self.step = Step::End;
                    }
                }
                Step::End => {
                    self.command.push(byte);

                    // The end of a command is its own byte; anything before it is the argument the board
                    // was given, which is how a question about itself carries its number.
                    if byte == EOC {
                        self.answer();
                    }
                }
            }
        }

        /// Says what the board thinks of the command that was just assembled.
        fn answer(&mut self) {
            let reply = self
                .replies
                .get(&self.command)
                .cloned()
                .unwrap_or_else(|| vec![INSYNC, OK]);

            self.command.clear();
            self.step = Step::Opcode;
            self.queue.extend(reply);
        }
    }

    #[async_trait]
    impl Port for ScriptedBoard {
        async fn write_all(&mut self, data: &[u8]) -> CommandResult<()> {
            for byte in data {
                self.receive(*byte);
            }

            Ok(())
        }

        async fn read_some(&mut self, data: &mut [u8]) -> CommandResult<usize> {
            if self.queue.is_empty() {
                return Err(CommandError::timeout("the board had nothing left to say"));
            }

            let count = data.len().min(self.queue.len());
            data[..count].copy_from_slice(&self.queue[..count]);
            self.queue.drain(..count);

            Ok(count)
        }
    }

    /// The fixture, read through the reader that is not this one. Its numbers are golden: board 9, an
    /// image of 305 bytes, and `4b98510b` for a flash of 1000 — the number the rule gives, not the one a
    /// zlib reader gives for the same bytes.
    fn fixture() -> crate::apj::FirmwareImage {
        crate::apj::FirmwareImage::read(include_bytes!("../tests/fixtures/sample_apj.json"))
            .expect("the fixture is the document")
    }

    /// What a board says when it is asked what it is, and what it says about its own flash.
    fn board_answers(revision: u32, holds: u32) -> BTreeMap<Vec<u8>, Vec<u8>> {
        let mut replies = BTreeMap::new();

        for (which, value) in [(1_u8, revision), (2, 9), (3, 0), (4, 1000), (5, 0), (6, 0)] {
            replies.insert(
                vec![GET_DEVICE, which, EOC],
                [value.to_le_bytes().to_vec(), vec![INSYNC, OK]].concat(),
            );
        }

        replies.insert(
            vec![GET_CRC, EOC],
            [holds.to_le_bytes().to_vec(), vec![INSYNC, OK]].concat(),
        );

        // An erase answers with the byte that says the flash is clean, then with the two that say the
        // command was carried out.
        replies.insert(vec![CHIP_ERASE, EOC], vec![0x00, INSYNC, OK]);

        replies
    }

    #[tokio::test]
    async fn a_file_for_another_board_is_refused_before_the_flash_is_erased() {
        let mut replies = board_answers(3, 0xFFFF_FFFF);
        replies.insert(
            vec![GET_DEVICE, 2, EOC],
            [5_u32.to_le_bytes().to_vec(), vec![INSYNC, OK]].concat(),
        );

        let board = ScriptedBoard::answering(replies);
        let seen = board.told.clone();

        let error = write_firmware(board, &fixture(), |_, _| {}, false)
            .await
            .expect_err("the file is for board 9 and the board says it is 5");

        assert_eq!(error.code, CommandError::CODE_INVALID_REQUEST);

        // The refusal is what an erase would have been done for, so what is read here is what the board
        // was told: nothing that erases reached it.
        assert!(!seen
            .lock()
            .expect("the board keeps what it was sent")
            .contains(&CHIP_ERASE));
    }

    #[tokio::test]
    async fn a_board_that_already_holds_the_image_is_not_erased_for_nothing() {
        // The number the board gives is the golden one for this image inside a flash of 1000 bytes, so
        // what the board is saying is that it already holds this.
        let board = ScriptedBoard::answering(board_answers(3, 0x4b98_510b));
        let seen = board.told.clone();

        let error = write_firmware(board, &fixture(), |_, _| {}, false)
            .await
            .expect_err("the board holds what the file is");

        assert_eq!(error.code, CommandError::CODE_REJECTED);
        assert!(error.message.contains("already holds"));

        assert!(!seen
            .lock()
            .expect("the board keeps what it was sent")
            .contains(&CHIP_ERASE));
    }

    #[tokio::test]
    async fn the_same_image_is_written_when_the_operator_says_so() {
        let board = ScriptedBoard::answering(board_answers(3, 0x4b98_510b));
        let seen = board.told.clone();

        let info = write_firmware(board, &fixture(), |_, _| {}, true)
            .await
            .expect("the operator said the same image could be written again");

        assert_eq!(info.board_id, 9);
        assert_eq!(info.bl_rev, 3);

        assert!(
            seen.lock()
                .expect("the board keeps what it was sent")
                .contains(&CHIP_ERASE),
            "the erase happened after the operator said so"
        );
    }

    #[tokio::test]
    async fn a_board_that_answers_a_different_number_is_said_as_a_failure() {
        let board = ScriptedBoard::answering(board_answers(3, 0x1234_5678));

        let error = write_firmware(board, &fixture(), |_, _| {}, true)
            .await
            .expect_err("the board says something else about its flash");

        assert_eq!(error.code, CommandError::CODE_REJECTED);
        assert!(error.message.contains("12345678"));
        assert!(error.message.contains("4b98510b"));
    }

    #[tokio::test]
    async fn a_board_of_revision_two_is_read_back_block_by_block() {
        // A board of revision 2 cannot be asked for a CRC, so what was written is read back. This board
        // answers the reads with an erased flash — fours of `0xFF` — which is not what it was given.
        let mut replies = board_answers(2, 0);
        replies.insert(vec![CHIP_VERIFY, EOC], vec![INSYNC, OK]);
        replies.insert(
            vec![READ_MULTI, 252, EOC],
            [vec![0xFF; 252], vec![INSYNC, OK]].concat(),
        );
        replies.insert(
            vec![READ_MULTI, 56, EOC],
            [vec![0xFF; 56], vec![INSYNC, OK]].concat(),
        );

        let board = ScriptedBoard::answering(replies);
        let seen = board.told.clone();

        let error = write_firmware(board, &fixture(), |_, _| {}, true)
            .await
            .expect_err("the board read back something that is not what it was given");

        assert_eq!(error.code, CommandError::CODE_REJECTED);

        let told = seen
            .lock()
            .expect("the board keeps what it was sent")
            .clone();
        assert!(
            told.contains(&CHIP_VERIFY),
            "the read-back is asked for first"
        );
    }
}
