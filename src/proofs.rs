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
