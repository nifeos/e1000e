//! **Kani harnesses over the transmit ring**, against a device that completes the descriptors it
//! was handed in order, as many at a time as the model checker chooses.

use core::cell::Cell;

use super::*;

const ENTRIES: u16 = TRANSMIT_ENTRIES;

/// The device's side of the transmit ring: the second word of each descriptor, the tail the
/// driver last wrote, and the next descriptor the device completes. The descriptors from
/// `completed` up to `tail` are the device's; every other one is the driver's.
struct TransmitDevice {
    second_words: [Cell<u64>; ENTRIES as usize],
    tail: Cell<u16>,
    completed: Cell<u16>,
}

impl TransmitDevice {
    /// How many descriptors the device holds.
    fn held(&self) -> u16 {
        (self.tail.get() + ENTRIES - self.completed.get()) % ENTRIES
    }

    /// Is descriptor `index` the device's?
    fn holds(&self, index: u16) -> bool {
        (index + ENTRIES - self.completed.get()) % ENTRIES < self.held()
    }

    /// The transmit descriptor or buffer slot `offset` falls in, if it is in either.
    fn transmit_slot(offset: u64) -> Option<(u16, bool)> {
        let ring = layout::TRANSMIT_RING_OFFSET_BYTES;
        let buffers = layout::TRANSMIT_BUFFERS_OFFSET_BYTES;
        if (ring..ring + u64::from(ENTRIES) * 16).contains(&offset) {
            Some((((offset - ring) / 16) as u16, true))
        } else if (buffers..layout::DIRECT_MEMORY_ACCESS_REGION_BYTES).contains(&offset) {
            Some((((offset - buffers) / BUFFER_BYTES) as u16, false))
        } else {
            None
        }
    }

    /// Complete `count` descriptors in order, writing descriptor done into each.
    fn complete(&self, count: u16) {
        for _ in 0..count {
            let index = self.completed.get();
            let word = &self.second_words[usize::from(index)];
            word.set(word.get() | u64::from(TRANSMIT_STATUS_DESCRIPTOR_DONE) << 32);
            self.completed.set((index + 1) % ENTRIES);
        }
    }
}

struct Region<'a>(&'a TransmitDevice);

impl DirectMemoryAccessRegion for Region<'_> {
    fn read_u64(&self, offset: u64) -> u64 {
        match TransmitDevice::transmit_slot(offset) {
            Some((index, true)) if offset % 16 == 8 => {
                self.0.second_words[usize::from(index)].get()
            }
            _ => 0,
        }
    }
    fn write_u64(&mut self, offset: u64, value: u64) {
        if let Some((index, is_descriptor)) = TransmitDevice::transmit_slot(offset) {
            assert!(is_descriptor);
            assert!(
                !self.0.holds(index),
                "a descriptor the device holds was written"
            );
            if offset % 16 == 8 {
                self.0.second_words[usize::from(index)].set(value);
            }
        }
    }
    fn read_bytes(&self, _offset: u64, _out: &mut [u8]) {}
    fn write_bytes(&mut self, offset: u64, source: &[u8]) {
        let slot = TransmitDevice::transmit_slot(offset);
        assert!(
            matches!(slot, Some((_, false))),
            "a frame was written outside a transmit buffer"
        );
        if let Some((index, _)) = slot {
            assert!(
                !self.0.holds(index),
                "a buffer the device holds was written"
            );
            assert!(source.len() as u64 <= BUFFER_BYTES);
        }
    }
}

struct DeviceTails<'a>(&'a TransmitDevice);

impl Tails for DeviceTails<'_> {
    fn set_receive_tail(&mut self, _value: u32) {}
    fn set_transmit_tail(&mut self, value: u32) {
        assert!(value < u32::from(ENTRIES));
        self.0.tail.set(value as u16);
    }
    fn barrier(&mut self) {}
}

/// How many transmits the harness tries: enough to wrap the ring and fill it.
const STEPS: u32 = 18;

/// Falsification: replayable `falsifications/ring_proofs.the_transmit_ring_never_writes_what_the_device_holds.patch`
/// **Against a device that completes descriptors in order whenever it likes, `transmit` never
/// writes a descriptor or a buffer the device holds, the device holds exactly the frames sent and
/// not yet completed, one slot always separates the tail from the device's head, and `Full` comes
/// only when every other slot is the device's.**
///
#[kani::proof]
#[kani::unwind(19)]
fn the_transmit_ring_never_writes_what_the_device_holds() {
    let device = TransmitDevice {
        second_words: core::array::from_fn(|_| Cell::new(0)),
        tail: Cell::new(0),
        completed: Cell::new(0),
    };
    let mut data_plane = DataPlane::new(0x1000_0000);
    data_plane.start(&mut Region(&device), &mut DeviceTails(&device));
    let frame = [0u8; 64];
    let mut outstanding: u16 = 0;
    for _ in 0..STEPS {
        let completions: u16 = kani::any();
        kani::assume(completions <= device.held());
        device.complete(completions);
        outstanding -= completions;
        let length: usize = kani::any();
        kani::assume(length >= 1 && length <= frame.len());
        match data_plane.transmit(
            &mut Region(&device),
            &mut DeviceTails(&device),
            &frame[..length],
        ) {
            Ok(()) => outstanding += 1,
            Err(TransmitError::Full) => assert!(device.held() == ENTRIES - 1),
            Err(TransmitError::BadLength) => panic!("a frame of 1 to 64 bytes was refused"),
        }
        assert!(device.held() == outstanding);
        assert!(outstanding < ENTRIES);
    }
}
