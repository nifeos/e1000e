//! Drive both rings against a fake device on the host: the two traits over a `Vec<u8>` and two
//! integers, a frame received and a frame sent.
//!
//! Run it with `cargo run --example host_ring`. On hardware, the region is the mapped direct memory
//! access (DMA) memory and the tails are the queue registers, with the volatile accesses and the
//! barrier that each trait's documentation states; everything else here stays the same.

use e1000e::{
    DataPlane, DirectMemoryAccessRegion, RECEIVE_ENTRIES, RECEIVE_STATUS_DESCRIPTOR_DONE,
    RECEIVE_STATUS_END_OF_PACKET, TRANSMIT_STATUS_DESCRIPTOR_DONE, Tails, layout,
};

/// The DMA region. Plain memory is right here because the "device" below is this program; a real
/// one needs volatile accesses.
struct Region(Vec<u8>);

impl DirectMemoryAccessRegion for Region {
    fn read_u64(&self, offset: u64) -> u64 {
        let at = offset as usize;
        u64::from_le_bytes(self.0[at..at + 8].try_into().unwrap())
    }
    fn write_u64(&mut self, offset: u64, value: u64) {
        let at = offset as usize;
        self.0[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    fn read_bytes(&self, offset: u64, out: &mut [u8]) {
        let at = offset as usize;
        out.copy_from_slice(&self.0[at..at + out.len()]);
    }
    fn write_bytes(&mut self, offset: u64, source: &[u8]) {
        let at = offset as usize;
        self.0[at..at + source.len()].copy_from_slice(source);
    }
}

/// The two tail registers, as integers.
#[derive(Default)]
struct TailRegisters {
    receive: u32,
    transmit: u32,
}

impl Tails for TailRegisters {
    fn set_receive_tail(&mut self, value: u32) {
        self.receive = value;
    }
    fn set_transmit_tail(&mut self, value: u32) {
        self.transmit = value;
    }
    fn barrier(&mut self) {
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// Where the region would sit in the device's view of memory.
const REGION_PHYSICAL_ADDRESS: u64 = 0x1000_0000;

fn main() {
    let mut region = Region(vec![0; layout::DIRECT_MEMORY_ACCESS_REGION_BYTES as usize]);
    let mut tails = TailRegisters::default();
    let mut data_plane = DataPlane::new(REGION_PHYSICAL_ADDRESS);
    data_plane.start(&mut region, &mut tails);
    assert_eq!(tails.receive, u32::from(RECEIVE_ENTRIES - 1));

    // The device receives a 60-byte frame into descriptor 0: it reads the buffer address the
    // driver posted, writes the bytes there, then writes the length and status back.
    let descriptor = layout::receive_descriptor(0);
    let buffer = region.read_u64(descriptor) - REGION_PHYSICAL_ADDRESS;
    let frame: Vec<u8> = (0..60).collect();
    region.write_bytes(buffer, &frame);
    let status = RECEIVE_STATUS_DESCRIPTOR_DONE | RECEIVE_STATUS_END_OF_PACKET;
    region.write_u64(descriptor + 8, 60 | u64::from(status) << 32);

    let mut out = [0u8; 2048];
    let length = data_plane
        .receive(&mut region, &mut tails, &mut out)
        .expect("the device wrote a frame");
    assert_eq!(&out[..length], &frame[..]);
    assert_eq!(
        tails.receive, 0,
        "descriptor 0 is handed back as the new tail"
    );
    println!("received {length} bytes");

    // Send a frame: the driver fills transmit descriptor 0 and moves the tail past it.
    let reply = [0xffu8; 42];
    data_plane
        .transmit(&mut region, &mut tails, &reply)
        .expect("the ring has room");
    assert_eq!(tails.transmit, 1);
    let descriptor = layout::transmit_descriptor(0);
    let sent_from = region.read_u64(descriptor) - REGION_PHYSICAL_ADDRESS;
    let mut sent = [0u8; 42];
    region.read_bytes(sent_from, &mut sent);
    assert_eq!(sent, reply);

    // The device sends it and reports done, which frees the descriptor for the next frame.
    let word = region.read_u64(descriptor + 8);
    region.write_u64(
        descriptor + 8,
        word | u64::from(TRANSMIT_STATUS_DESCRIPTOR_DONE) << 32,
    );
    assert!(data_plane.can_transmit(&mut region));
    println!("sent {} bytes", sent.len());
}
