//! Fuzz `DataPlane`'s receive side against a device that writes arbitrary status words.
//!
//! The first byte picks the device. An honest one writes frames only into the descriptors it
//! holds, in ring order, with arbitrary length, status, error and checksum fields (descriptor done
//! always set), and arbitrary junk over the address word, and it writes a waiting frame the moment
//! a tail write hands it a descriptor, as QEMU's model does. A lying one writes any status word
//! into any descriptor at any time, including descriptors the driver holds. Every following byte is
//! an operation: queue a frame at the device, receive into a buffer of an arbitrary size, or let
//! the lying device scribble on a descriptor.
//!
//! Each `receive` is checked against an independent reading of the status words it read, so the
//! target does not trust `receive_verdict`: it reads the descriptors after the cursor in order,
//! consumes each written one and stops at the first deliverable frame or after one lap. Each
//! consumed descriptor is reposted with its buffer address and a cleared status before the tail
//! names it, and the tail then moves by exactly one slot. A frame is copied only from its own
//! buffer, only as many bytes as it is long and never more than the caller has room for, and no
//! byte of the caller's buffer past the frame changes. The two drop counters move by exactly the
//! descriptors that were consumed and not delivered. Against the honest device the driver also
//! never reads or writes a descriptor the device holds, and the device never holds more than all
//! descriptors but one.
//!
//! The target starts the data plane and never transmits.

#![no_main]

use core::cell::{Cell, RefCell};
use std::collections::VecDeque;

use e1000e::{BUFFER_BYTES, DataPlane, DirectMemoryAccessRegion, RECEIVE_ENTRIES, Tails, layout};
use libfuzzer_sys::fuzz_target;

const ENTRIES: u16 = RECEIVE_ENTRIES;
const PHYSICAL_ADDRESS: u64 = 0x1000_0000;
/// The most frames the device keeps waiting for a descriptor.
const QUEUE_LIMIT: usize = 64;
/// The largest buffer the caller offers, a little over one receive buffer.
const LARGEST_ROOM: usize = BUFFER_BYTES as usize + 16;
/// Status-word lengths run from zero to a little past one receive buffer.
const LENGTH_SPAN: u64 = BUFFER_BYTES + 64;

/// The receive status bits and error mask, written out here rather than imported, so the target
/// judges a word independently of the crate.
const DESCRIPTOR_DONE: u8 = 1 << 0;
const END_OF_PACKET: u8 = 1 << 1;
const FRAME_ERRORS: u8 = 0x97;
const SMALLEST_FRAME: usize = 14;
const LARGEST_FRAME: usize = 1518;

/// What the target expects of one status word: not written yet, a frame to deliver, or a
/// descriptor to consume and drop, saying whether more room alone would have delivered it.
#[derive(Debug, PartialEq)]
enum Judged {
    NotYet,
    Deliver(usize),
    Drop { for_room: bool },
}

fn judge(word: u64, room: usize) -> Judged {
    let status = (word >> 32) as u8;
    if status & DESCRIPTOR_DONE == 0 {
        return Judged::NotYet;
    }
    let errors = (word >> 40) as u8;
    let length = (word & 0xffff) as usize;
    let whole = status & END_OF_PACKET != 0 && errors & FRAME_ERRORS == 0;
    let sized = (SMALLEST_FRAME..=LARGEST_FRAME).contains(&length);
    if whole && sized && length <= room {
        Judged::Deliver(length)
    } else {
        Judged::Drop {
            for_room: whole && sized,
        }
    }
}

/// One thing the driver did inside a `receive` call, in order.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    StatusRead { slot: u16, word: u64 },
    AddressWrite { slot: u16, value: u64 },
    StatusWrite { slot: u16, value: u64 },
    BytesRead { slot: u16, length: usize },
    Tail(u16),
}

/// The device's side of the receive ring.
struct Device {
    /// Each descriptor's two words as they are in memory.
    words: [[Cell<u64>; 2]; ENTRIES as usize],
    /// The frame number each descriptor's buffer holds, which fixes its bytes.
    frame_numbers: [Cell<u32>; ENTRIES as usize],
    /// The next descriptor an honest device writes.
    head: Cell<u16>,
    /// The tail the driver last wrote.
    tail: Cell<u16>,
    /// Frames waiting for a descriptor: their status words and address-word junk.
    waiting: RefCell<VecDeque<(u64, u64)>>,
    next_frame_number: Cell<u32>,
    lying: bool,
    /// `start` has returned.
    started: Cell<bool>,
    events: RefCell<Vec<Event>>,
}

impl Device {
    /// How many descriptors the honest device holds: from its head up to the tail.
    fn held(&self) -> u16 {
        (self.tail.get() + ENTRIES - self.head.get()) % ENTRIES
    }

    fn holds(&self, slot: u16) -> bool {
        (slot + ENTRIES - self.head.get()) % ENTRIES < self.held()
    }

    /// The honest device writes waiting frames into the descriptors it holds, in ring order.
    fn write_waiting(&self) {
        if self.lying {
            return;
        }
        while self.held() > 0 {
            let Some((word, junk)) = self.waiting.borrow_mut().pop_front() else {
                return;
            };
            let slot = self.head.get();
            self.write_descriptor(slot, junk, word | u64::from(DESCRIPTOR_DONE) << 32);
            self.head.set((slot + 1) % ENTRIES);
        }
    }

    fn write_descriptor(&self, slot: u16, address_word: u64, status_word: u64) {
        let index = usize::from(slot);
        self.words[index][0].set(address_word);
        self.words[index][1].set(status_word);
        self.frame_numbers[index].set(self.next_frame_number.get());
        self.next_frame_number
            .set(self.next_frame_number.get().wrapping_add(1));
    }

    fn byte(&self, slot: u16, at: usize) -> u8 {
        let number = self.frame_numbers[usize::from(slot)].get();
        (number as u8) ^ (at as u8) ^ ((at >> 8) as u8).wrapping_mul(31) ^ 0x5a
    }

    /// The receive descriptor and word `offset` names, if it names one exactly.
    fn descriptor(offset: u64) -> Option<(u16, usize)> {
        let ring = layout::RECEIVE_RING_OFFSET_BYTES;
        if (ring..ring + u64::from(ENTRIES) * 16).contains(&offset) && offset % 8 == 0 {
            Some((((offset - ring) / 16) as u16, ((offset % 16) / 8) as usize))
        } else {
            None
        }
    }

    fn log(&self, event: Event) {
        if self.started.get() {
            self.events.borrow_mut().push(event);
        }
    }
}

struct Region<'a>(&'a Device);

impl DirectMemoryAccessRegion for Region<'_> {
    fn read_u64(&self, offset: u64) -> u64 {
        let Some((slot, 1)) = Device::descriptor(offset) else {
            panic!("a read of {offset:#x}, which is no receive status word");
        };
        let word = self.0.words[usize::from(slot)][1].get();
        self.0.log(Event::StatusRead { slot, word });
        word
    }

    fn write_u64(&mut self, offset: u64, value: u64) {
        let Some((slot, half)) = Device::descriptor(offset) else {
            assert!(
                !self.0.started.get(),
                "a write of {offset:#x}, outside the receive ring, after start"
            );
            return;
        };
        if self.0.started.get() && !self.0.lying {
            assert!(
                !self.0.holds(slot),
                "descriptor {slot}, which the device holds, was written"
            );
        }
        self.0.words[usize::from(slot)][half].set(value);
        self.0.log(if half == 0 {
            Event::AddressWrite { slot, value }
        } else {
            Event::StatusWrite { slot, value }
        });
    }

    fn read_bytes(&self, offset: u64, out: &mut [u8]) {
        let buffers = layout::RECEIVE_BUFFERS_OFFSET_BYTES;
        assert!(
            (buffers..layout::TRANSMIT_BUFFERS_OFFSET_BYTES).contains(&offset),
            "frame bytes read from {offset:#x}, outside the receive buffers"
        );
        let slot = ((offset - buffers) / BUFFER_BYTES) as u16;
        assert_eq!(
            offset,
            layout::receive_buffer(slot),
            "a read inside a buffer"
        );
        assert!(out.len() as u64 <= BUFFER_BYTES);
        if !self.0.lying {
            assert!(
                !self.0.holds(slot),
                "the buffer of descriptor {slot}, which the device holds, was read"
            );
        }
        for (at, byte) in out.iter_mut().enumerate() {
            *byte = self.0.byte(slot, at);
        }
        self.0.log(Event::BytesRead {
            slot,
            length: out.len(),
        });
    }

    fn write_bytes(&mut self, _offset: u64, _source: &[u8]) {
        panic!("the receive side wrote frame bytes");
    }
}

struct DeviceTails<'a>(&'a Device);

impl Tails for DeviceTails<'_> {
    fn set_receive_tail(&mut self, value: u32) {
        assert!(value < u32::from(ENTRIES), "a receive tail of {value}");
        let value = value as u16;
        if self.0.started.get() {
            assert_eq!(
                value,
                (self.0.tail.get() + 1) % ENTRIES,
                "the tail moved by other than one slot"
            );
        }
        self.0.tail.set(value);
        self.0.log(Event::Tail(value));
        if !self.0.lying {
            assert!(self.0.held() < ENTRIES);
        }
        // QEMU's model writes a waiting frame during the tail write that hands it a descriptor.
        self.0.write_waiting();
    }

    fn set_transmit_tail(&mut self, _value: u32) {
        panic!("the receive side moved the transmit tail");
    }

    fn barrier(&mut self) {}
}

/// Check one `receive` call's events and result against what its status reads said.
fn check_receive(
    events: &[Event],
    cursor_before: u16,
    cursor_after: u16,
    room: usize,
    got: Option<usize>,
    dropped: u32,
    dropped_for_room: u32,
) {
    let mut events = events.iter().copied();
    let mut slot = cursor_before;
    let mut consumed: u32 = 0;
    let mut expected_dropped: u32 = 0;
    let mut expected_dropped_for_room: u32 = 0;
    let mut delivered = None;
    loop {
        if consumed == u32::from(ENTRIES) {
            break;
        }
        let Some(Event::StatusRead { slot: read, word }) = events.next() else {
            panic!("descriptor {slot}'s status was not read next");
        };
        assert_eq!(read, slot, "a status read out of ring order");
        let judged = judge(word, room);
        if judged == Judged::NotYet {
            break;
        }
        if let Judged::Deliver(length) = judged {
            assert_eq!(
                events.next(),
                Some(Event::BytesRead { slot, length }),
                "the frame was not copied from its own buffer at its own length"
            );
            delivered = Some(length);
        } else {
            expected_dropped += 1;
            if judged == (Judged::Drop { for_room: true }) {
                expected_dropped_for_room += 1;
            }
        }
        assert_eq!(
            events.next(),
            Some(Event::AddressWrite {
                slot,
                value: PHYSICAL_ADDRESS + layout::receive_buffer(slot),
            }),
            "descriptor {slot} was not reposted with its own buffer"
        );
        assert_eq!(
            events.next(),
            Some(Event::StatusWrite { slot, value: 0 }),
            "descriptor {slot}'s status was not cleared"
        );
        assert_eq!(
            events.next(),
            Some(Event::Tail(slot)),
            "the tail did not name it"
        );
        consumed += 1;
        slot = (slot + 1) % ENTRIES;
        if delivered.is_some() {
            break;
        }
    }
    assert_eq!(
        events.next(),
        None,
        "the driver did more than one receive needs"
    );
    assert_eq!(got, delivered);
    assert_eq!(cursor_after, slot);
    assert_eq!(dropped, expected_dropped);
    assert_eq!(dropped_for_room, expected_dropped_for_room);
}

/// A status word from eight input bytes. Every field is the input's, but the length is reduced
/// below [`LENGTH_SPAN`], so lengths near the frame and buffer limits come up often.
fn status_word(bytes: [u8; 8]) -> u64 {
    let raw = u64::from_le_bytes(bytes);
    (raw & !0xffff) | (raw & 0xffff) % LENGTH_SPAN
}

fn take<const N: usize>(rest: &mut &[u8]) -> Option<[u8; N]> {
    let (head, tail) = rest.split_first_chunk::<N>()?;
    *rest = tail;
    Some(*head)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, mut rest)) = data.split_first() else {
        return;
    };
    let device = Device {
        words: core::array::from_fn(|_| [Cell::new(0), Cell::new(0)]),
        frame_numbers: core::array::from_fn(|_| Cell::new(0)),
        head: Cell::new(0),
        tail: Cell::new(0),
        waiting: RefCell::new(VecDeque::new()),
        next_frame_number: Cell::new(0),
        lying: mode & 1 != 0,
        started: Cell::new(false),
        events: RefCell::new(Vec::new()),
    };
    let mut data_plane = DataPlane::new(PHYSICAL_ADDRESS);
    data_plane.start(&mut Region(&device), &mut DeviceTails(&device));
    for slot in 0..ENTRIES {
        let words = &device.words[usize::from(slot)];
        assert_eq!(
            words[0].get(),
            PHYSICAL_ADDRESS + layout::receive_buffer(slot)
        );
        assert_eq!(words[1].get(), 0);
    }
    assert_eq!(device.tail.get(), ENTRIES - 1);
    assert_eq!(data_plane.receive_cursor(), 0);
    device.started.set(true);

    let mut out = [0u8; LARGEST_ROOM];
    while let Some([operation]) = take::<1>(&mut rest) {
        match operation % 3 {
            0 => {
                let Some(word) = take::<8>(&mut rest) else {
                    return;
                };
                let Some(junk) = take::<2>(&mut rest) else {
                    return;
                };
                let word = status_word(word);
                let junk = u64::from(u16::from_le_bytes(junk)).wrapping_mul(0x0001_0001_0001_0001);
                if device.lying {
                    continue;
                }
                let mut waiting = device.waiting.borrow_mut();
                if waiting.len() < QUEUE_LIMIT {
                    waiting.push_back((word, junk));
                }
                drop(waiting);
                device.write_waiting();
            }
            1 => {
                let Some(room) = take::<2>(&mut rest) else {
                    return;
                };
                let room = usize::from(u16::from_le_bytes(room)) % (LARGEST_ROOM + 1);
                let sentinel = operation;
                out.fill(sentinel);
                let cursor_before = data_plane.receive_cursor();
                let dropped_before = data_plane.dropped;
                let dropped_for_room_before = data_plane.dropped_for_lack_of_room;
                device.events.borrow_mut().clear();
                let got = data_plane.receive(
                    &mut Region(&device),
                    &mut DeviceTails(&device),
                    &mut out[..room],
                );
                let events = device.events.borrow().clone();
                check_receive(
                    &events,
                    cursor_before,
                    data_plane.receive_cursor(),
                    room,
                    got,
                    data_plane.dropped.wrapping_sub(dropped_before),
                    data_plane
                        .dropped_for_lack_of_room
                        .wrapping_sub(dropped_for_room_before),
                );
                let length = got.unwrap_or(0);
                if let Some(length) = got {
                    let slot = (data_plane.receive_cursor() + ENTRIES - 1) % ENTRIES;
                    for (at, &byte) in out[..length].iter().enumerate() {
                        assert_eq!(byte, device.byte(slot, at), "a delivered byte differs");
                    }
                }
                assert!(
                    out[length..].iter().all(|&byte| byte == sentinel),
                    "a byte past the frame changed"
                );
                if !device.lying {
                    // Every descriptor the device does not hold, other than the one that keeps
                    // the tail from the head, is one it wrote and the driver has not consumed.
                    let unconsumed =
                        (device.head.get() + ENTRIES - data_plane.receive_cursor()) % ENTRIES;
                    assert_eq!(device.held() + unconsumed, ENTRIES - 1);
                }
            }
            _ => {
                let Some([slot]) = take::<1>(&mut rest) else {
                    return;
                };
                let Some(word) = take::<8>(&mut rest) else {
                    return;
                };
                if !device.lying {
                    continue;
                }
                let slot = u16::from(slot) % ENTRIES;
                let address_word = device.words[usize::from(slot)][0].get();
                device.write_descriptor(slot, address_word, status_word(word));
            }
        }
    }
});
