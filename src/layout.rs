use super::{BUFFER_BYTES, PAGE_BYTES, RECEIVE_ENTRIES, TRANSMIT_ENTRIES};
/// The receive descriptor ring's offset in the region, in bytes.
pub const RECEIVE_RING_OFFSET_BYTES: u64 = 0;
/// The transmit descriptor ring's offset in the region, in bytes.
pub const TRANSMIT_RING_OFFSET_BYTES: u64 = PAGE_BYTES;
/// The first receive buffer's offset in the region, in bytes.
pub const RECEIVE_BUFFERS_OFFSET_BYTES: u64 = 2 * PAGE_BYTES;
/// The first transmit buffer's offset in the region, in bytes.
pub const TRANSMIT_BUFFERS_OFFSET_BYTES: u64 =
    RECEIVE_BUFFERS_OFFSET_BYTES + RECEIVE_ENTRIES as u64 * BUFFER_BYTES;
/// The direct memory access region's size in bytes.
pub const DIRECT_MEMORY_ACCESS_REGION_BYTES: u64 =
    TRANSMIT_BUFFERS_OFFSET_BYTES + TRANSMIT_ENTRIES as u64 * BUFFER_BYTES;
/// The direct memory access region's size in pages.
pub const DIRECT_MEMORY_ACCESS_REGION_PAGES: u64 =
    DIRECT_MEMORY_ACCESS_REGION_BYTES.div_ceil(PAGE_BYTES);

/// Receive buffer `index`'s offset. `index` is reduced modulo the ring so no input leaves the
/// region.
pub const fn receive_buffer(index: u16) -> u64 {
    RECEIVE_BUFFERS_OFFSET_BYTES + (index % RECEIVE_ENTRIES) as u64 * BUFFER_BYTES
}
/// Transmit buffer `index`'s offset, reduced the same way.
pub const fn transmit_buffer(index: u16) -> u64 {
    TRANSMIT_BUFFERS_OFFSET_BYTES + (index % TRANSMIT_ENTRIES) as u64 * BUFFER_BYTES
}
/// Receive descriptor `index`'s offset.
pub const fn receive_descriptor(index: u16) -> u64 {
    RECEIVE_RING_OFFSET_BYTES + (index % RECEIVE_ENTRIES) as u64 * 16
}
/// Transmit descriptor `index`'s offset.
pub const fn transmit_descriptor(index: u16) -> u64 {
    TRANSMIT_RING_OFFSET_BYTES + (index % TRANSMIT_ENTRIES) as u64 * 16
}
