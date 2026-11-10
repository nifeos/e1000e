//! Fuzz `DataPlane`'s transmit and reclaim side against a device that writes arbitrary status
//! words.
//!
//! The first byte picks the device: an honest one completes the descriptors it holds in order, as
//! many as the input says, while a lying one reports descriptor done on whatever the input says.
//! Every following byte is an operation: transmit a frame of an arbitrary length (including empty
//! and over-long ones), let the device complete some descriptors, ask `can_transmit`, or change
//! the junk in the status bits the driver does not use. Against the honest device the target holds
//! the ring to what `ring_proofs` states: a descriptor or buffer the device holds is never
//! written, the device holds exactly the frames sent and not yet completed, one slot always
//! separates the tail from the device's head, and `Full` comes exactly when every other slot is the
//! device's. Against either device every write stays inside the transmit ring and its buffers once
//! `start` has run, and the tail stays inside the ring.
//!
//! The target starts the data plane and never receives.

#![no_main]

use core::cell::Cell;

use e1000e::{
    BUFFER_BYTES, DataPlane, DirectMemoryAccessRegion, MAXIMUM_FRAME_BYTES, TRANSMIT_ENTRIES,
    TRANSMIT_STATUS_DESCRIPTOR_DONE, Tails, TransmitError, layout,
    transmit_descriptor_command_word,
};
use libfuzzer_sys::fuzz_target;

const ENTRIES: u16 = TRANSMIT_ENTRIES;
const PHYSICAL_ADDRESS: u64 = 0x1000_0000;

/// The device's side of the transmit ring.
struct Device {
    /// The second word of each descriptor as the driver last wrote it.
    second_words: [Cell<u64>; ENTRIES as usize],
    /// The tail the driver last wrote.
    tail: Cell<u16>,
    /// The next descriptor an honest device completes.
    completed: Cell<u16>,
    /// Junk for the status bits other than descriptor done.
    junk: Cell<u64>,
    /// A lying device reports descriptor done from `junk` instead of from its cursor.
    lying: bool,
    /// `start` has returned, so only the transmit side may be touched from here on.
    started: Cell<bool>,
    /// The slot the driver may write next, which the driver's own cursor names.
    writable: Cell<Option<u16>>,
}

impl Device {
    fn held(&self) -> u16 {
        (self.tail.get() + ENTRIES - self.completed.get()) % ENTRIES
    }

    fn holds(&self, index: u16) -> bool {
        (index + ENTRIES - self.completed.get()) % ENTRIES < self.held()
    }

    fn slot(offset: u64) -> Option<(u16, bool)> {
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
}

struct Region<'a>(&'a Device);

impl DirectMemoryAccessRegion for Region<'_> {
    fn read_u64(&self, offset: u64) -> u64 {
        match Device::slot(offset) {
            Some((index, true)) if offset % 16 == 8 => {
                let word = self.0.second_words[usize::from(index)].get();
                let junk = self.0.junk.get() & !(u64::from(TRANSMIT_STATUS_DESCRIPTOR_DONE) << 32);
                let done = if self.0.lying {
                    self.0.junk.get() & (u64::from(TRANSMIT_STATUS_DESCRIPTOR_DONE) << 32)
                } else if word != 0 && !self.0.holds(index) {
                    u64::from(TRANSMIT_STATUS_DESCRIPTOR_DONE) << 32
                } else {
                    0
                };
                // The status byte and the bits above it are the device's to write.
                (word & 0xffff_ffff) | (junk & 0xffff_ffff_0000_0000) | done
            }
            _ => 0,
        }
    }

    fn write_u64(&mut self, offset: u64, value: u64) {
        let slot = Device::slot(offset);
        if self.0.started.get() {
            let Some((index, true)) = slot else {
                panic!("a write outside the transmit ring after start");
            };
            assert_eq!(self.0.writable.get(), Some(index));
            assert!(
                self.0.lying || !self.0.holds(index),
                "a descriptor the device holds was written"
            );
        }
        if let Some((index, true)) = slot
            && offset % 16 == 8
        {
            self.0.second_words[usize::from(index)].set(value);
        }
    }

    fn read_bytes(&self, _offset: u64, _out: &mut [u8]) {
        panic!("the transmit side read frame bytes");
    }

    fn write_bytes(&mut self, offset: u64, source: &[u8]) {
        let Some((index, false)) = Device::slot(offset) else {
            panic!("a frame was written outside a transmit buffer");
        };
        assert!(offset.is_multiple_of(BUFFER_BYTES));
        assert_eq!(self.0.writable.get(), Some(index));
        assert!(source.len() as u64 <= BUFFER_BYTES);
        assert!(
            self.0.lying || !self.0.holds(index),
            "a buffer the device holds was written"
        );
    }
}

struct DeviceTails<'a>(&'a Device);

impl Tails for DeviceTails<'_> {
    fn set_receive_tail(&mut self, _value: u32) {
        assert!(!self.0.started.get(), "the receive tail moved after start");
    }

    fn set_transmit_tail(&mut self, value: u32) {
        assert!(value < u32::from(ENTRIES));
        self.0.tail.set(value as u16);
    }

    fn barrier(&mut self) {}
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, mut rest)) = data.split_first() else {
        return;
    };
    let device = Device {
        second_words: core::array::from_fn(|_| Cell::new(0)),
        tail: Cell::new(0),
        completed: Cell::new(0),
        junk: Cell::new(0),
        lying: mode & 1 != 0,
        started: Cell::new(false),
        writable: Cell::new(None),
    };
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    data_plane.start(&mut Region(&device), &mut DeviceTails(&device));
    device.started.set(true);

    let mut next_slot: u16 = 0;
    let mut frame = [0u8; 2048];
    while let Some((&operation, after)) = rest.split_first() {
        rest = after;
        match operation % 4 {
            0 => {
                let Some((&low, after)) = rest.split_first() else {
                    return;
                };
                let Some((&high, after)) = after.split_first() else {
                    return;
                };
                rest = after;
                let length = usize::from(u16::from_le_bytes([low, high])) % (frame.len() + 1);
                frame[..length].fill(operation);
                let before = device.held();
                device.writable.set(Some(next_slot));
                let outcome = data_plane.transmit(
                    &mut Region(&device),
                    &mut DeviceTails(&device),
                    &frame[..length],
                );
                device.writable.set(None);
                if length == 0 || length > MAXIMUM_FRAME_BYTES {
                    assert_eq!(outcome, Err(TransmitError::BadLength));
                    continue;
                }
                match outcome {
                    Ok(()) => {
                        next_slot = (next_slot + 1) % ENTRIES;
                        assert_eq!(device.tail.get(), next_slot);
                        let slot = (next_slot + ENTRIES - 1) % ENTRIES;
                        assert_eq!(
                            device.second_words[usize::from(slot)].get(),
                            transmit_descriptor_command_word(length as u16)
                        );
                        if !device.lying {
                            assert!(device.held() < ENTRIES);
                            assert!(device.held() >= 1);
                        }
                    }
                    Err(TransmitError::Full) => {
                        assert_eq!(device.held(), before);
                        if !device.lying {
                            assert_eq!(device.held(), ENTRIES - 1);
                        }
                    }
                    Err(other) => panic!("a frame of {length} bytes was refused: {other:?}"),
                }
                if !device.lying {
                    assert_eq!(
                        device.held(),
                        (next_slot + ENTRIES - device.completed.get()) % ENTRIES
                    );
                }
            }
            1 => {
                let Some((&count, after)) = rest.split_first() else {
                    return;
                };
                rest = after;
                let count = u16::from(count) % (device.held() + 1);
                device
                    .completed
                    .set((device.completed.get() + count) % ENTRIES);
            }
            2 => {
                let can = data_plane.can_transmit(&mut Region(&device));
                if !device.lying {
                    assert_eq!(can, device.held() != ENTRIES - 1);
                }
            }
            _ => {
                let Some((&seed, after)) = rest.split_first() else {
                    return;
                };
                rest = after;
                device
                    .junk
                    .set(u64::from(seed).wrapping_mul(0x0101_0101_0101_0101));
            }
        }
    }
});
