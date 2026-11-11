use super::*;

/// Falsification: replayable `falsifications/proofs.a_delivered_frame_always_fits_the_buffer_and_the_room.patch`
/// **No word a device can write makes the driver read outside a buffer or past its caller's
/// space, and no written descriptor with an error bit or without end-of-packet is delivered.** The
/// descriptor's second word is entirely the device's; this is every value of it.
///
#[kani::proof]
fn a_delivered_frame_always_fits_the_buffer_and_the_room() {
    let word: u64 = kani::any();
    let room: usize = kani::any();
    let verdict = receive_verdict(word, room);
    if let ReceiveVerdict::Frame(n) = verdict {
        assert!(n <= room);
        assert!(n as u64 <= BUFFER_BYTES);
        assert!(n <= MAXIMUM_FRAME_BYTES);
    }
    let status = (word >> 32) as u8;
    let errors = (word >> 40) as u8;
    if status & RECEIVE_STATUS_DESCRIPTOR_DONE != 0
        && (status & RECEIVE_STATUS_END_OF_PACKET == 0 || errors & RECEIVE_FRAME_ERROR_MASK != 0)
    {
        assert!(verdict == ReceiveVerdict::Drop);
    }
}

/// Falsification: replayable `falsifications/proofs.every_ring_offset_is_inside_the_region.patch`
/// **Every descriptor and buffer offset, for any index, lies inside the region.**
///
#[kani::proof]
fn every_ring_offset_is_inside_the_region() {
    let index: u16 = kani::any();
    assert!(layout::receive_descriptor(index) + 16 <= layout::TRANSMIT_RING_OFFSET_BYTES);
    assert!(layout::transmit_descriptor(index) + 16 <= layout::RECEIVE_BUFFERS_OFFSET_BYTES);
    assert!(layout::receive_buffer(index) + BUFFER_BYTES <= layout::TRANSMIT_BUFFERS_OFFSET_BYTES);
    assert!(
        layout::transmit_buffer(index) + BUFFER_BYTES <= layout::DIRECT_MEMORY_ACCESS_REGION_BYTES
    );
}

/// A region whose every word is whatever the device chose at the moment of the read, counting the
/// receive status words the data plane reads.
struct AnyWordRegion {
    status_reads: core::cell::Cell<u32>,
}

impl DirectMemoryAccessRegion for AnyWordRegion {
    fn read_u64(&self, offset: u64) -> u64 {
        if offset < layout::TRANSMIT_RING_OFFSET_BYTES && offset % 16 == 8 {
            self.status_reads.set(self.status_reads.get() + 1);
        }
        kani::any()
    }
    fn write_u64(&mut self, _offset: u64, _value: u64) {}
    fn read_bytes(&self, _offset: u64, _out: &mut [u8]) {}
    fn write_bytes(&mut self, _offset: u64, _source: &[u8]) {}
}

struct AnyTails;

impl Tails for AnyTails {
    fn set_receive_tail(&mut self, _value: u32) {}
    fn set_transmit_tail(&mut self, _value: u32) {}
    fn barrier(&mut self) {}
}

/// Falsification: replayable `falsifications/proofs.one_receive_examines_at_most_one_lap_of_the_ring.patch`
/// **One call to `receive` examines at most one lap of the ring, whatever the device writes.** Every
/// status word is chosen afresh at each read, so the device may write each descriptor again the
/// moment it is handed back.
///
#[kani::proof]
#[kani::unwind(18)]
fn one_receive_examines_at_most_one_lap_of_the_ring() {
    let mut region = AnyWordRegion {
        status_reads: core::cell::Cell::new(0),
    };
    let mut data_plane = DataPlane::new(0x8000_0000);
    let mut out = [0u8; 64];
    let _ = data_plane.receive(&mut region, &mut AnyTails, &mut out);
    assert!(region.status_reads.get() <= u32::from(RECEIVE_ENTRIES));
}
