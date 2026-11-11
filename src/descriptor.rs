//! **The queues and their descriptors**: where each queue's registers and rings sit, how big they
//! are, and what a descriptor's status says.

use super::*;

/// The page of BAR0 holding receive queue 0's registers, mapped into the process.
pub const RECEIVE_QUEUE_PAGE: u64 = 0x2000;

/// The page of BAR0 holding transmit queue 0's registers, mapped into the process.
pub const TRANSMIT_QUEUE_PAGE: u64 = 0x3000;

/// Where `RDT` sits inside [`RECEIVE_QUEUE_PAGE`], and `TDT` inside [`TRANSMIT_QUEUE_PAGE`]: the
/// same offset.
pub const TAIL_IN_PAGE: u64 = registers::RECEIVE_DESCRIPTOR_TAIL_REGISTER - RECEIVE_QUEUE_PAGE;

pub(crate) const PAGE_BYTES: u64 = 4096;

/// Receive descriptors. Sixteen 16-byte descriptors are 256 bytes, which satisfies `RDLEN`'s
/// multiple-of-128 rule and fits one page with room to grow.
pub const RECEIVE_ENTRIES: u16 = 16;

/// Transmit descriptors.
pub const TRANSMIT_ENTRIES: u16 = 16;

/// Bytes per buffer, matching `RCTL`'s size field of zero.
pub const BUFFER_BYTES: u64 = 2048;

/// The largest frame either direction carries, in bytes: 1500 bytes of payload, the 14-byte
/// Ethernet header and one 4-byte virtual local area network (VLAN) tag, no CRC (the device strips
/// it on receive and appends it on transmit). This is not the maximum transmission unit, which is
/// the 1500 bytes of payload alone.
pub const MAXIMUM_FRAME_BYTES: usize = 1518;

/// Receive status: descriptor done. OpenBSD's `E1000_RXD_STAT_DD` (`sys/dev/pci/if_em_hw.h`),
/// which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "E1000_RXD_STAT_DD")]
pub const RECEIVE_STATUS_DESCRIPTOR_DONE: u8 = 1 << 0;

/// Receive status: end of packet. A frame without it spans buffers, which a 2048-byte buffer and
/// a 1518-byte maximum never need, so such a frame is dropped rather than reassembled. OpenBSD's
/// `E1000_RXD_STAT_EOP` (`sys/dev/pci/if_em_hw.h`), spelled the same by FreeBSD and Linux.
#[doc(alias = "E1000_RXD_STAT_EOP")]
pub const RECEIVE_STATUS_END_OF_PACKET: u8 = 1 << 1;

/// Receive errors that mean the frame is bad: CRC, symbol, sequence, carrier extension, receive data
/// (bits 0, 1, 2, 4 and 7, a mask of `0x97`). OpenBSD's `E1000_RXD_ERR_FRAME_ERR_MASK`
/// (`sys/dev/pci/if_em_hw.h`), spelled the same by FreeBSD and Linux.
#[doc(alias = "E1000_RXD_ERR_FRAME_ERR_MASK")]
pub const RECEIVE_FRAME_ERROR_MASK: u8 = 0x01 | 0x02 | 0x04 | 0x10 | 0x80;

/// Transmit command: end of packet, insert the frame check sequence (FCS), report status. OpenBSD's `E1000_TXD_CMD_EOP`,
/// `E1000_TXD_CMD_IFCS` and `E1000_TXD_CMD_RS` (`sys/dev/pci/if_em_hw.h`; FreeBSD and Linux agree)
/// are `0x01000000`, `0x02000000` and `0x08000000`, the bits here at bit 24 of the second word. No
/// single BSD name exists for the OR of the three.
pub const TRANSMIT_COMMAND: u8 = 0x01 | 0x02 | 0x08;

// Both words OR single distinct bits, so | and ^ compose them identically; the compiler checks
// that here, so the constants' OR-to-XOR swaps are equivalent mutants.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    RECEIVE_FRAME_ERROR_MASK == 0x01 ^ 0x02 ^ 0x04 ^ 0x10 ^ 0x80
        && TRANSMIT_COMMAND == 0x01 ^ 0x02 ^ 0x08
);

/// Transmit status: descriptor done. OpenBSD's `E1000_TXD_STAT_DD` (`sys/dev/pci/if_em_hw.h`),
/// which FreeBSD and Linux spell the same.
#[doc(alias = "E1000_TXD_STAT_DD")]
pub const TRANSMIT_STATUS_DESCRIPTOR_DONE: u8 = 1 << 0;

/// What a receive descriptor's second word says, judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveVerdict {
    /// The device has not written this descriptor yet.
    NotYet,
    /// A whole, good frame of this many bytes, which fits the buffer and the caller's space.
    Frame(usize),
    /// The device wrote it and it is not a frame to deliver: an error bit, no end-of-packet, or a
    /// length it could not have meant. Consumed and the buffer reposted.
    Drop,
}

/// **Judge a receive descriptor's status word**, where `word` is the descriptor's second
/// little-endian `u64` (length, checksum, status, errors, special) and `room` is how many bytes the
/// caller can take. No field is trusted, since the device writes them all: a `Frame(n)` always has
/// `n <= room` and `n <= BUFFER_BYTES`, which the Kani harness below proves for every word.
///
/// # Examples
///
/// ```
/// use e1000e::{ReceiveVerdict, receive_verdict, RECEIVE_STATUS_DESCRIPTOR_DONE, RECEIVE_STATUS_END_OF_PACKET};
///
/// let word = |len: u64, status: u8| len | (status as u64) << 32;
/// let done = RECEIVE_STATUS_DESCRIPTOR_DONE | RECEIVE_STATUS_END_OF_PACKET;
/// assert_eq!(receive_verdict(word(60, done), 1518), ReceiveVerdict::Frame(60));
/// assert_eq!(receive_verdict(word(60, 0), 1518), ReceiveVerdict::NotYet);
/// assert_eq!(receive_verdict(word(0xffff, done), 1518), ReceiveVerdict::Drop);
/// ```
pub fn receive_verdict(word: u64, room: usize) -> ReceiveVerdict {
    let status = (word >> 32) as u8;
    if status & RECEIVE_STATUS_DESCRIPTOR_DONE == 0 {
        return ReceiveVerdict::NotYet;
    }
    let errors = (word >> 40) as u8;
    let len = (word & 0xffff) as usize;
    let fits =
        (14..=MAXIMUM_FRAME_BYTES).contains(&len) && len <= room && len as u64 <= BUFFER_BYTES;
    if status & RECEIVE_STATUS_END_OF_PACKET == 0 || errors & RECEIVE_FRAME_ERROR_MASK != 0 || !fits
    {
        return ReceiveVerdict::Drop;
    }
    ReceiveVerdict::Frame(len)
}

// The u16 length and the command byte at bit 24 share no bit, so | and ^ compose them
// identically; the compiler checks that here, so the word's
// OR-to-XOR swap is an equivalent mutant.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!((u16::MAX as u64) & ((TRANSMIT_COMMAND as u64) << 24) == 0);

/// A transmit descriptor's second word, which carries the length, the command and the status:
/// `length` bytes, [`TRANSMIT_COMMAND`], status cleared.
pub const fn transmit_descriptor_command_word(length: u16) -> u64 {
    length as u64 | (TRANSMIT_COMMAND as u64) << 24
}
