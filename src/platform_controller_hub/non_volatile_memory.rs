//! **Reading the I219's NVM on Sunrise Point**, where the `GbE` flash region is reached through
//! registers in BAR0 rather than the chipset's serial peripheral interface (SPI) controller.
//!
//! [`super::sequence`] needs it for the post-reset `LCD` configuration and the NVM-present check.
//! Read only: nothing here can write or erase the flash. Each item names the SPT branch of
//! FreeBSD's `e1000_ich8lan.c` it follows.

//! ```text
//! SPDX-License-Identifier: BSD-3-Clause
//!
//! Copyright (c) 2001-2020, Intel Corporation
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//!  1. Redistributions of source code must retain the above copyright notice,
//!     this list of conditions and the following disclaimer.
//!
//!  2. Redistributions in binary form must reproduce the above copyright
//!     notice, this list of conditions and the following disclaimer in the
//!     documentation and/or other materials provided with the distribution.
//!
//!  3. Neither the name of the Intel Corporation nor the names of its
//!     contributors may be used to endorse or promote products derived from
//!     this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
//! LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//! ```

use super::Hardware;

/// Why an NVM read failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// `HSFSTS.FLDESVALID` is clear: no flash descriptor, so hardware sequencing is unavailable.
    NoDescriptor,
    /// A previous flash cycle never finished.
    Busy,
    /// The cycle did not complete, or completed with `FLCERR`, after every retry.
    Cycle,
    /// The word is outside the shadow RAM's 2048 words.
    OutOfRange,
    /// Neither bank carries the valid signature.
    NoValidBank,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::NoDescriptor => "no flash descriptor, so hardware sequencing is unavailable",
            Error::Busy => "a previous flash cycle never finished",
            Error::Cycle => "the flash cycle did not complete",
            Error::OutOfRange => "the word is outside the shadow RAM",
            Error::NoValidBank => "neither bank carries the valid signature",
        })
    }
}

impl core::error::Error for Error {}

/// Where the flash registers begin in BAR0 on SPT (`E1000_FLASH_BASE_ADDR`). It is an offset into
/// BAR0, not a memory address, so the name ends `_OFFSET_BYTES` as the region offsets of
/// [`crate::layout`] do.
#[doc(alias = "FLASH_BASE_ADDR")]
#[doc(alias = "E1000_FLASH_BASE_ADDR")]
pub(crate) const FLASH_REGISTERS_OFFSET_BYTES: u64 = 0xe000;
/// Hardware Sequencing Flash Status (`HSFSTS`), low half; Hardware Sequencing Flash Control, high half
/// (OpenBSD's `ICH_FLASH_HSFSTS`). An offset from the flash registers at `0xe000` in BAR0.
#[doc(alias = "HSFSTS")]
#[doc(alias = "E1000_HSFSTS")]
#[doc(alias = "ICH_FLASH_HSFSTS")]
pub const HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER: u64 = 0x04;
/// Flash linear address (OpenBSD's `ICH_FLASH_FADDR`).
#[doc(alias = "FADDR")]
#[doc(alias = "ICH_FLASH_FADDR")]
pub(crate) const FLASH_LINEAR_BYTE_OFFSET_REGISTER: u64 = 0x08;
/// Flash data 0 (OpenBSD's `ICH_FLASH_FDATA0`).
#[doc(alias = "FDATA0")]
#[doc(alias = "ICH_FLASH_FDATA0")]
pub(crate) const FLASH_DATA_0_REGISTER: u64 = 0x10;

/// `HSFSTS` bits (`union ich8_hws_flash_status`).
pub mod flash_status {
    /// Flash cycle done: the `flcdone` field of `union ich8_hws_flash_status` in OpenBSD's
    /// `if_em_hw.h` and FreeBSD's `e1000_ich8lan.c`.
    #[doc(alias = "flcdone")]
    pub const FLASH_CYCLE_DONE: u16 = 1 << 0;
    /// Flash cycle error: the `flcerr` field of `union ich8_hws_flash_status` in OpenBSD's
    /// `if_em_hw.h` and FreeBSD's `e1000_ich8lan.c`.
    #[doc(alias = "flcerr")]
    pub const FLASH_CYCLE_ERROR: u16 = 1 << 1;
    /// Direct access error log.
    pub(crate) const DIRECT_ACCESS_ERROR: u16 = 1 << 2;
    /// A cycle is in progress.
    pub(crate) const IN_PROGRESS: u16 = 1 << 5;
    /// The flash descriptor is valid.
    pub(crate) const DESCRIPTOR_VALID: u16 = 1 << 14;
}

/// `HSFCTL` bits (`union ich8_hws_flash_ctrl`), in the high half of the `u32` at [`HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER`].
pub mod flash_control {
    /// Start the cycle: the `flcgo` field of `union ich8_hws_flash_ctrl` in OpenBSD's
    /// `if_em_hw.h` and FreeBSD's `e1000_ich8lan.c`.
    #[doc(alias = "flcgo")]
    pub const FLASH_CYCLE_GO: u16 = 1 << 0;
    /// The cycle-type field; zero is a read (`ICH_CYCLE_READ`).
    pub(crate) const CYCLE_MASK: u16 = 0b11 << 1;
    /// The byte-count field, bits 9:8, holding the count minus one.
    pub(crate) const BYTE_COUNT_SHIFT: u16 = 8;
    /// Its mask.
    pub(crate) const BYTE_COUNT_MASK: u16 = 0b11 << 8;
}

// `initialize_cycle` ORs the two write-one-to-clear error bits into the status it just read, and
// they share no bit, so | and ^ compose them identically; the compiler checks that here, so
// the function's OR-to-XOR swaps are equivalent mutants (the `FLASH_CYCLE_DONE` write's
// swap is equivalent on the write-back beside it, which has already carried the bit).
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    flash_status::FLASH_CYCLE_ERROR ^ flash_status::DIRECT_ACCESS_ERROR
        == flash_status::FLASH_CYCLE_ERROR | flash_status::DIRECT_ACCESS_ERROR
);
// The byte count `read_u32_at_byte` ORs in sits inside the field it just masked out, so | and ^
// compose them identically; the compiler checks that here, so the
// function's OR-to-XOR swap is an equivalent mutant.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () =
    assert!((3u16 << flash_control::BYTE_COUNT_SHIFT) & !flash_control::BYTE_COUNT_MASK == 0);

/// `ICH_FLASH_LINEAR_ADDR_MASK`.
pub(crate) const LINEAR_BYTE_OFFSET_MASK: u32 = 0x00ff_ffff;
/// `ICH_FLASH_READ_COMMAND_TIMEOUT`, in one-microsecond polls: ten seconds, FreeBSD's bound.
#[doc(alias = "ICH_FLASH_READ_COMMAND_TIMEOUT")]
pub(crate) const MAXIMUM_READ_COMMAND_POLLS: u32 = 10_000_000;
/// `ICH_FLASH_CYCLE_REPEAT_COUNT`.
pub(crate) const CYCLE_REPEAT_COUNT: u32 = 10;
/// The signature word (`E1000_ICH_NVM_SIG_WORD`).
pub(crate) const SIGNATURE_WORD: u32 = 0x13;
/// `E1000_ICH_NVM_VALID_SIG_MASK`.
pub(crate) const SIGNATURE_MASK: u8 = 0xc0;
/// `E1000_ICH_NVM_SIG_VALUE`.
pub(crate) const SIGNATURE_VALUE: u8 = 0x80;
/// The shadow RAM's size in 16-bit words: `E1000_SHADOW_RAM_WORDS` in OpenBSD's `if_em_hw.h` and
/// FreeBSD's `e1000_hw.h`.
#[doc(alias = "E1000_SHADOW_RAM_WORDS")]
pub const SHADOW_RAM_WORDS: u32 = 2048;
/// `NVM_SIZE_MULTIPLIER_BYTES`.
pub(crate) const NVM_SIZE_MULTIPLIER_BYTES: u32 = 4096;

/// The NVM's size, from `STRAP` on SPT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Geometry {
    /// One bank's size in 16-bit words.
    pub bank_words: u32,
}

impl Geometry {
    /// `e1000_init_nvm_params_ich8lan`'s SPT branch: `STRAP` bits 5:1 hold the size in 4 KiB
    /// units, minus one; the NVM is two banks.
    pub(crate) const fn from_strap(strap: u32) -> Self {
        let bytes = ((strap >> 1 & 0x1f) + 1) * NVM_SIZE_MULTIPLIER_BYTES;
        Geometry {
            bank_words: bytes / 2 / 2,
        }
    }
    /// Read the strap register and decode the size from it.
    pub fn read_from_hardware(hardware: &mut impl Hardware) -> Self {
        Self::from_strap(hardware.read(super::registers::STRAP_REGISTER))
    }
}

fn status_16_bit(hardware: &mut impl Hardware) -> u16 {
    hardware
        .read_flash_status(FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER)
}

/// `e1000_flash_cycle_init_ich8lan`, SPT's 32-bit writes.
fn initialize_cycle(hardware: &mut impl Hardware) -> Result<(), Error> {
    let s = status_16_bit(hardware);
    if s & flash_status::DESCRIPTOR_VALID == 0 {
        return Err(Error::NoDescriptor);
    }
    // Clear the cycle and direct-access errors by writing them as one.
    let s = s | flash_status::FLASH_CYCLE_ERROR | flash_status::DIRECT_ACCESS_ERROR;
    hardware.write(
        FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER,
        u32::from(s),
    );
    if s & flash_status::IN_PROGRESS == 0 {
        hardware.write(
            FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER,
            u32::from(s | flash_status::FLASH_CYCLE_DONE),
        );
        return Ok(());
    }
    for _ in 0..MAXIMUM_READ_COMMAND_POLLS {
        let now = status_16_bit(hardware);
        if now & flash_status::IN_PROGRESS == 0 {
            hardware.write(
                FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER,
                u32::from(now | flash_status::FLASH_CYCLE_DONE),
            );
            return Ok(());
        }
        hardware.delay_microseconds(1);
    }
    Err(Error::Busy)
}

/// `e1000_flash_cycle_ich8lan`: set `FLASH_CYCLE_GO` and wait for `FLASH_CYCLE_DONE`.
fn cycle(hardware: &mut impl Hardware) -> Result<(), Error> {
    let flash_control = (hardware
        .read(FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER)
        >> 16) as u16
        | flash_control::FLASH_CYCLE_GO;
    hardware.write(
        FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER,
        u32::from(flash_control) << 16,
    );
    let mut s = 0;
    for _ in 0..=MAXIMUM_READ_COMMAND_POLLS {
        s = status_16_bit(hardware);
        if s & flash_status::FLASH_CYCLE_DONE != 0 {
            break;
        }
        hardware.delay_microseconds(1);
    }
    if s & flash_status::FLASH_CYCLE_DONE != 0 && s & flash_status::FLASH_CYCLE_ERROR == 0 {
        Ok(())
    } else {
        Err(Error::Cycle)
    }
}

/// `e1000_read_flash_data32_ich8lan`: one `u32` (a doubleword, in Intel's word sizes) at byte offset
/// `byte`.
#[doc(alias = "dword")]
pub(crate) fn read_u32_at_byte(hardware: &mut impl Hardware, byte: u32) -> Result<u32, Error> {
    if byte > LINEAR_BYTE_OFFSET_MASK {
        return Err(Error::OutOfRange);
    }
    let mut last = Err(Error::Cycle);
    for _ in 0..=CYCLE_REPEAT_COUNT {
        hardware.delay_microseconds(1);
        initialize_cycle(hardware)?;
        let mut flash_control = (hardware
            .read(FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER)
            >> 16) as u16;
        flash_control = (flash_control & !flash_control::BYTE_COUNT_MASK)
            | (3 << flash_control::BYTE_COUNT_SHIFT);
        flash_control &= !flash_control::CYCLE_MASK;
        hardware.write(
            FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER,
            u32::from(flash_control) << 16,
        );
        hardware.write(
            FLASH_REGISTERS_OFFSET_BYTES + FLASH_LINEAR_BYTE_OFFSET_REGISTER,
            byte & LINEAR_BYTE_OFFSET_MASK,
        );
        last = cycle(hardware);
        if last.is_ok() {
            return Ok(hardware.read(FLASH_REGISTERS_OFFSET_BYTES + FLASH_DATA_0_REGISTER));
        }
        let s = status_16_bit(hardware);
        if s & flash_status::FLASH_CYCLE_ERROR != 0 {
            continue;
        }
        if s & flash_status::FLASH_CYCLE_DONE == 0 {
            break;
        }
    }
    last.map(|()| 0)
}

/// `e1000_read_flash_dword_ich8lan`: the `u32` (Intel's doubleword) at 16-bit word offset `word`.
#[doc(alias = "dword")]
pub(crate) fn read_u32(hardware: &mut impl Hardware, word: u32) -> Result<u32, Error> {
    read_u32_at_byte(hardware, word << 1)
}

/// `e1000_valid_nvm_bank_detect_ich8lan`'s SPT branch: which bank carries the signature.
pub(crate) fn valid_bank(hardware: &mut impl Hardware, geometry: Geometry) -> Result<u32, Error> {
    for bank in 0..2 {
        let d = read_u32(hardware, SIGNATURE_WORD + bank * geometry.bank_words)?;
        if (d >> 8) as u8 & SIGNATURE_MASK == SIGNATURE_VALUE {
            return Ok(bank);
        }
    }
    Err(Error::NoValidBank)
}

/// `e1000_read_nvm_spt` for one word: find the valid bank (bank 0 if none, as FreeBSD assumes),
/// read the `u32` holding the word, and take its half.
pub(crate) fn read_word(
    hardware: &mut impl Hardware,
    geometry: Geometry,
    word: u32,
) -> Result<u16, Error> {
    if word >= SHADOW_RAM_WORDS {
        return Err(Error::OutOfRange);
    }
    let bank = valid_bank(hardware, geometry).unwrap_or(0);
    let at = bank * geometry.bank_words + word;
    let d = read_u32(hardware, at - at % 2)?;
    Ok(if at.is_multiple_of(2) {
        d as u16
    } else {
        (d >> 16) as u16
    })
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// A flash that finishes every cycle at once and answers each data read with whatever the
    /// model checker chooses, recording the byte offset each cycle was asked for.
    struct AnyFlash {
        /// The last value written to the linear address register, if any.
        last_byte_offset: Option<u32>,
        /// Whether anything at all was written.
        written: bool,
    }

    impl Hardware for AnyFlash {
        fn read(&mut self, offset: u64) -> u32 {
            if offset == FLASH_REGISTERS_OFFSET_BYTES + HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER {
                u32::from(flash_status::DESCRIPTOR_VALID | flash_status::FLASH_CYCLE_DONE)
            } else {
                kani::any()
            }
        }
        fn write(&mut self, offset: u64, value: u32) {
            self.written = true;
            if offset == FLASH_REGISTERS_OFFSET_BYTES + FLASH_LINEAR_BYTE_OFFSET_REGISTER {
                self.last_byte_offset = Some(value);
            }
        }
        fn read_flash_status(&mut self, offset: u64) -> u16 {
            self.read(offset) as u16
        }
        fn delay_microseconds(&mut self, _microseconds: u64) {}
        fn pci_vendor_id(&mut self) -> u16 {
            0x8086
        }
        fn note(&mut self, _note: super::super::sequence::BringUpObservation) {}
    }

    /// Falsification: replayable `falsifications/platform_controller_hub.non_volatile_memory.proofs.a_word_read_stays_in_range_for_every_strap.patch`
    /// **For every `STRAP` value and every word, the NVM size is one to 32 units of 4 KiB over two
    /// banks, a word past the shadow RAM is refused before any register is written, and every
    /// other word is read from the even word at or below its own in the bank the signature
    /// chose.**
    ///
    #[kani::proof]
    #[kani::unwind(12)]
    fn a_word_read_stays_in_range_for_every_strap() {
        let geometry = Geometry::from_strap(kani::any());
        assert!(geometry.bank_words >= 1024 && geometry.bank_words <= 32 * 1024);
        assert!(geometry.bank_words.is_multiple_of(1024));
        let word: u32 = kani::any();
        let mut flash = AnyFlash {
            last_byte_offset: None,
            written: false,
        };
        let result = read_word(&mut flash, geometry, word);
        if word >= SHADOW_RAM_WORDS {
            assert!(result == Err(Error::OutOfRange));
            assert!(!flash.written);
        } else {
            let byte = flash.last_byte_offset.unwrap();
            assert!(byte <= LINEAR_BYTE_OFFSET_MASK);
            let at = byte / 2;
            assert!(at.is_multiple_of(2));
            assert!(at == (word & !1) || at == geometry.bank_words + word - word % 2);
        }
    }
}
