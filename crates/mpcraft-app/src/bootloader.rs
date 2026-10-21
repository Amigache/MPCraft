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
//! 3. the flash is erased (`CHIP_ERASE`), which takes seconds;
//! 4. the firmware is written in blocks (`PROG_MULTI`);
//! 5. the board is told to start again (`REBOOT`).
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

/// Writes a block of the firmware.
const PROG_MULTI: u8 = 0x27;

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
/// The board is waited for, asked what it is, its flash is erased, the firmware is
/// written and the board is told to start. The callback says how much of the firmware
/// has been written, so that the operator can see the progress.
///
/// # Errors
///
/// Returns an error when the board does not answer, when it refuses a command, or when
/// the firmware does not fit in its flash.
pub async fn write_firmware<P: Port>(
    port: P,
    firmware: &[u8],
    progress: impl FnMut(usize, usize),
) -> CommandResult<BoardInfo> {
    let mut board = Bootloader::new(port);

    board.wake_up(WAKE_TIMEOUT).await?;

    let info = board.info().await?;

    if info.flash_size > 0 && firmware.len() as u32 > info.flash_size {
        return Err(CommandError::invalid_request(format!(
            "the firmware is {} bytes and the flash of the board holds {}",
            firmware.len(),
            info.flash_size
        )));
    }

    board.erase().await?;
    board.program(firmware, progress).await?;
    board.reboot().await?;

    Ok(info)
}

/// What a failure of the port becomes.
fn io_error(error: std::io::Error) -> CommandError {
    CommandError::internal(format!("the board could not be spoken to: {error}"))
}

#[cfg(test)]
mod tests {
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
}
