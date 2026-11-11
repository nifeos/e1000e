//! **A Kani harness over the receive ring's cursor**, against a device that writes every
//! descriptor's status word afresh at each read, and a caller whose buffer may be any size.

use core::cell::Cell;

use super::*;

/// What the receive path did, as the device and the caller can see it.
struct ReceiveDevice {
    /// The caller's buffer length, in bytes.
    room: usize,
    /// Receive tail writes so far. Each one hands back the descriptor just consumed, so this
    /// counts consumed descriptors.
    consumed: Cell<u16>,
    /// The descriptor whose status was read last.
    last_status_slot: Cell<Option<u16>>,
    /// Buffer reads so far, and the bytes they covered.
    buffer_reads: Cell<u32>,
    bytes_read: Cell<usize>,
}

struct Region<'a>(&'a ReceiveDevice);

impl DirectMemoryAccessRegion for Region<'_> {
    fn read_u64(&self, offset: u64) -> u64 {
        if offset < layout::TRANSMIT_RING_OFFSET_BYTES && offset % 16 == 8 {
            let slot = ((offset - layout::RECEIVE_RING_OFFSET_BYTES) / 16) as u16;
            // Every status read is of the descriptor after the last one consumed.
            assert!(slot == self.0.consumed.get() % RECEIVE_ENTRIES);
            self.0.last_status_slot.set(Some(slot));
        }
        kani::any()
    }
    fn write_u64(&mut self, _offset: u64, _value: u64) {}
    fn read_bytes(&self, offset: u64, out: &mut [u8]) {
        // Only the buffer of the descriptor whose status was just read, and no more than the
        // caller has room for.
        assert!(self.0.last_status_slot.get().is_some());
        let slot = self.0.last_status_slot.get().unwrap_or(0);
        assert!(offset == layout::receive_buffer(slot));
        assert!(out.len() <= self.0.room);
        assert!(out.len() as u64 <= BUFFER_BYTES);
        self.0.buffer_reads.set(self.0.buffer_reads.get() + 1);
        self.0.bytes_read.set(self.0.bytes_read.get() + out.len());
    }
    fn write_bytes(&mut self, _offset: u64, _source: &[u8]) {}
}

struct DeviceTails<'a>(&'a ReceiveDevice);

impl Tails for DeviceTails<'_> {
    fn set_receive_tail(&mut self, value: u32) {
        // The tail names the descriptor just consumed, which is the one whose status was read.
        assert!(Some(value as u16) == self.0.last_status_slot.get());
        assert!(value as u16 == self.0.consumed.get() % RECEIVE_ENTRIES);
        self.0.consumed.set(self.0.consumed.get() + 1);
    }
    fn set_transmit_tail(&mut self, _value: u32) {}
    fn barrier(&mut self) {}
}

/// Falsification: replayable `falsifications/receive_proofs.a_receive_reads_only_its_own_slot_and_no_more_than_the_caller_has_room_for.patch`
/// **Whatever the device writes and however small the caller's buffer, one `receive` reads frame
/// bytes at most once, only from the buffer of the descriptor it consumed and only as many bytes
/// as the frame it returns, never more than the caller's room, and leaves the cursor on the
/// descriptor after the last one consumed.** Two calls run in a row, so the second starts from
/// wherever the first stopped.
///
#[kani::proof]
#[kani::unwind(18)]
fn a_receive_reads_only_its_own_slot_and_no_more_than_the_caller_has_room_for() {
    let room: usize = kani::any();
    kani::assume(room <= 64);
    let device = ReceiveDevice {
        room,
        consumed: Cell::new(0),
        last_status_slot: Cell::new(None),
        buffer_reads: Cell::new(0),
        bytes_read: Cell::new(0),
    };
    let mut data_plane = DataPlane::new(0x8000_0000);
    let mut out = [0u8; 64];
    for _ in 0..2 {
        let reads_before = device.buffer_reads.get();
        let bytes_before = device.bytes_read.get();
        let got = data_plane.receive(
            &mut Region(&device),
            &mut DeviceTails(&device),
            &mut out[..room],
        );
        assert!(device.buffer_reads.get() - reads_before <= 1);
        match got {
            Some(n) => {
                assert!(n <= room);
                assert!(device.buffer_reads.get() - reads_before == 1);
                assert!(device.bytes_read.get() - bytes_before == n);
            }
            None => assert!(device.buffer_reads.get() == reads_before),
        }
        assert!(data_plane.receive_cursor() == device.consumed.get() % RECEIVE_ENTRIES);
    }
}
