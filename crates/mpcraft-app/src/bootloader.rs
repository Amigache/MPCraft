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
//! The protocol is spoken here over anything that can be read and written, so that it
//! can be tried against a board that is not there.

use std::io::{Read, Write};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

/// What a failure of the port becomes.
fn io_error(error: std::io::Error) -> CommandError {
    CommandError::internal(format!("the board could not be spoken to: {error}"))
}

/// The bootloader of a board.
pub struct Bootloader<T> {
    /// The port the board is spoken to.
    port: T,
}

impl<T: Read + Write> Bootloader<T> {
    /// Speaks to the board over the given port.
    #[must_use]
    pub fn new(port: T) -> Self {
        Self { port }
    }

    /// Asks the board whether it is there.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub fn sync(&mut self) -> CommandResult<()> {
        self.port.write_all(&[GET_SYNC, EOC]).map_err(io_error)?;
        self.port.flush().map_err(io_error)?;
        self.read_sync()
    }

    /// Asks the board what it is.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub fn device(&mut self, what: Info) -> CommandResult<u32> {
        self.port
            .write_all(&[GET_DEVICE, what.as_byte(), EOC])
            .map_err(io_error)?;
        self.port.flush().map_err(io_error)?;

        let mut answer = [0_u8; 4];
        self.port.read_exact(&mut answer).map_err(io_error)?;

        let value = u32::from_le_bytes(answer);

        self.read_sync()?;

        Ok(value)
    }

    /// Asks the board everything about itself.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the question.
    pub fn info(&mut self) -> CommandResult<BoardInfo> {
        Ok(BoardInfo {
            bl_rev: self.device(Info::BlRev)? as u8,
            board_id: self.device(Info::BoardId)?,
            board_rev: self.device(Info::BoardRev)?,
            flash_size: self.device(Info::FlashSize)?,
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
    pub fn erase(&mut self) -> CommandResult<()> {
        // The bootloader of some boards wants to be asked something before it erases
        // anything: the upstream does the same.
        let _ = self.device(Info::BlRev);

        self.port.write_all(&[CHIP_ERASE, EOC]).map_err(io_error)?;
        self.port.flush().map_err(io_error)?;

        let mut done = [0_u8; 1];
        self.port.read_exact(&mut done).map_err(io_error)?;

        self.read_sync()
    }

    /// Writes the firmware to the board.
    ///
    /// The firmware is written in blocks, and the block that is written last is the one
    /// that is checked: a board whose flash does not hold what was written says so.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the command.
    pub fn program(&mut self, firmware: &[u8]) -> CommandResult<()> {
        for block in firmware.chunks(PROG_MULTI_MAX) {
            self.port
                .write_all(&[PROG_MULTI, block.len() as u8])
                .map_err(io_error)?;
            self.port.write_all(block).map_err(io_error)?;
            self.port.write_all(&[EOC]).map_err(io_error)?;
            self.port.flush().map_err(io_error)?;

            self.read_sync()?;
        }

        Ok(())
    }

    /// Tells the board to start again.
    ///
    /// # Errors
    ///
    /// Returns an error when the board does not answer, or when it says it does not
    /// understand the command.
    pub fn reboot(&mut self) -> CommandResult<()> {
        self.port.write_all(&[REBOOT, EOC]).map_err(io_error)?;
        self.port.flush().map_err(io_error)?;
        self.read_sync()
    }

    /// Reads the answer of the board to a command.
    fn read_sync(&mut self) -> CommandResult<()> {
        let mut answer = [0_u8; 1];
        self.port.read_exact(&mut answer).map_err(io_error)?;

        if answer[0] != INSYNC {
            return Err(CommandError::rejected(format!(
                "the board answered {:#04x} instead of saying it understood",
                answer[0]
            )));
        }

        self.port.read_exact(&mut answer).map_err(io_error)?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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

    impl Read for FakeBoard {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
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

            let count = buffer.len().min(self.queue.len());
            buffer[..count].copy_from_slice(&self.queue[..count]);
            self.queue.drain(..count);

            Ok(count)
        }
    }

    impl Write for FakeBoard {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.told.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_handshake_asks_whether_the_board_is_there() {
        let mut board = Bootloader::new(FakeBoard::ready());
        board.sync().expect("the board answers");

        let port = board.port;
        assert_eq!(port.told, vec![GET_SYNC, EOC]);
    }

    #[test]
    fn a_board_that_refuses_is_an_error() {
        let mut board = Bootloader::new(FakeBoard::refusing());
        let error = board.sync().expect_err("the board refuses");
        assert_eq!(error.code, CommandError::CODE_REJECTED);
    }

    #[test]
    fn the_firmware_is_written_in_blocks() {
        let mut board = Bootloader::new(FakeBoard::ready());
        let firmware = vec![7_u8; PROG_MULTI_MAX * 2 + 5];

        board.program(&firmware).expect("the board takes the firmware");

        // The blocks go one after the other, and each of them says how long it is.
        let told = board.port.told;
        assert_eq!(told[0], PROG_MULTI);
        assert_eq!(told[1] as usize, PROG_MULTI_MAX);
        // The first block is the opcode, the length, the data and the end of the command.
        assert_eq!(told[PROG_MULTI_MAX + 3], PROG_MULTI);
    }

    #[test]
    fn a_board_that_answers_nothing_is_an_error_and_not_a_panic() {
        let mut board = Bootloader::new(FakeBoard::silent());
        let error = board.sync().expect_err("the board says nothing");
        assert_eq!(error.code, CommandError::CODE_INTERNAL);
    }

    #[test]
    fn a_board_that_answers_nothing_at_all_is_an_error() {
        let mut board = Bootloader::new(Cursor::new(Vec::new()));
        let error = board.sync().expect_err("the board says nothing");
        assert_eq!(error.code, CommandError::CODE_INTERNAL);
    }
}
