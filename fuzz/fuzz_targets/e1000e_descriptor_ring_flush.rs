//! Fuzz `flush::empty_descriptor_rings` against a device whose registers and status words are
//! arbitrary.
//!
//! The input sets the value of each register the flush reads, the two status words it reads (one
//! before the dummy transmit and one after), whether the caller's transmit ring is the one the
//! device holds, and the ring's address otherwise. The target keeps a log of everything the flush
//! did and holds it to the sequence's rules: nothing is sent unless the device asked for a flush
//! and its transmit ring is the caller's; the one dummy descriptor goes to the device's tail,
//! reduced into the ring, and the tail then moves by exactly one slot, inside the ring and after
//! the descriptor was stored; the receive ring is flushed only when the device still asked after
//! the transmit flush, and receive is switched off again at the end; and the result names what
//! happened.

#![no_main]

use e1000e::platform_controller_hub::flush::{
    DESCRIPTOR_RING_FLUSH_REQUIRED, EmptiedRings,
    FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_DISABLE_MULR_FIX, FlushSupport,
    dummy_transmit_descriptor_command_word, empty_descriptor_rings,
    receive_descriptor_control_register_value_for_flush,
};
use e1000e::platform_controller_hub::registers::{
    FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER, RECEIVE_DESCRIPTOR_CONTROL_REGISTER,
};
use e1000e::platform_controller_hub::sequence::BringUpObservation;
use e1000e::registers::{
    RECEIVE_CONTROL_REGISTER, TRANSMIT_CONTROL_REGISTER,
    TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
    TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
    TRANSMIT_DESCRIPTOR_LENGTH_BYTES_REGISTER, TRANSMIT_DESCRIPTOR_TAIL_REGISTER,
};
use e1000e::{Hardware, TRANSMIT_ENTRIES};
use libfuzzer_sys::fuzz_target;
use std::cell::RefCell;
use std::rc::Rc;

/// The receive enable bit of the receive control register.
const RECEIVE_ENABLE: u32 = 1 << 1;
/// The transmit enable bit of the transmit control register.
const TRANSMIT_ENABLE: u32 = 1 << 1;

#[derive(Debug, PartialEq)]
enum Event {
    Write(u64, u32),
    Descriptor(u16, [u64; 2]),
}

type Log = Rc<RefCell<Vec<Event>>>;

/// The registers: the flush reads each one's starting value, and every write goes to the log.
struct Registers {
    values: [(u64, u32); 8],
    log: Log,
}

impl Hardware for Registers {
    fn read(&mut self, offset: u64) -> u32 {
        self.values
            .iter()
            .find(|(known, _)| *known == offset)
            .map_or(0, |(_, value)| *value)
    }

    fn write(&mut self, offset: u64, value: u32) {
        self.log.borrow_mut().push(Event::Write(offset, value));
    }

    fn read_flash_status(&mut self, _offset: u64) -> u16 {
        panic!("the flush read the flash status");
    }

    fn delay_microseconds(&mut self, _microseconds: u64) {}

    fn pci_vendor_id(&mut self) -> u16 {
        panic!("the flush read the vendor id");
    }

    fn note(&mut self, _note: BringUpObservation) {}
}

/// The configuration-space status word, which the input gives once before the dummy transmit and
/// once after, and the transmit ring's memory.
struct Rings {
    statuses: [u16; 2],
    reads: usize,
    log: Log,
}

impl FlushSupport for Rings {
    fn descriptor_ring_status(&mut self) -> u16 {
        let status = self.statuses[self.reads.min(1)];
        self.reads += 1;
        status
    }

    fn write_transmit_descriptor(&mut self, index: u16, words: [u64; 2]) {
        assert!(
            index < TRANSMIT_ENTRIES,
            "a descriptor outside the ring was stored"
        );
        self.log.borrow_mut().push(Event::Descriptor(index, words));
    }
}

fn take<const N: usize>(data: &mut &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    let n = N.min(data.len());
    out[..n].copy_from_slice(&data[..n]);
    *data = &data[n..];
    out
}

fuzz_target!(|input: &[u8]| {
    let mut data = input;
    let [shape] = take::<1>(&mut data);
    let mut word = || u32::from_le_bytes(take::<4>(&mut data));
    let extended = word();
    let length = word();
    let low = word();
    let high = word();
    let tail = word();
    let transmit_control = word();
    let receive_control = word();
    let receive_descriptor_control = word();
    let first_status = u16::from_le_bytes(take::<2>(&mut data));
    let second_status = u16::from_le_bytes(take::<2>(&mut data));
    let other_ring = u64::from_le_bytes(take::<8>(&mut data));
    let device_base = u64::from(low) + (u64::from(high) << 32);
    // Half the inputs give the caller the ring the device holds, so the flush runs past the check.
    let transmit_ring = if shape & 1 != 0 {
        device_base
    } else {
        other_ring
    };

    let log: Log = Rc::default();
    let mut registers = Registers {
        values: [
            (FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER, extended),
            (TRANSMIT_DESCRIPTOR_LENGTH_BYTES_REGISTER, length),
            (TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER, low),
            (
                TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
                high,
            ),
            (TRANSMIT_DESCRIPTOR_TAIL_REGISTER, tail),
            (TRANSMIT_CONTROL_REGISTER, transmit_control),
            (RECEIVE_CONTROL_REGISTER, receive_control),
            (
                RECEIVE_DESCRIPTOR_CONTROL_REGISTER,
                receive_descriptor_control,
            ),
        ],
        log: Rc::clone(&log),
    };
    let mut rings = Rings {
        statuses: [first_status, second_status],
        reads: 0,
        log: Rc::clone(&log),
    };
    let result = empty_descriptor_rings(&mut registers, &mut rings, transmit_ring);
    let events = log.borrow();

    // The first thing the flush does, whatever else happens, is set the "disable fix" bit.
    assert_eq!(
        events.first(),
        Some(&Event::Write(
            FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER,
            extended | FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_DISABLE_MULR_FIX
        ))
    );

    let required = first_status & DESCRIPTOR_RING_FLUSH_REQUIRED != 0 && length != 0;
    if !required {
        assert_eq!(result, EmptiedRings::NotRequired);
        assert_eq!(events.len(), 1);
        return;
    }
    if device_base != transmit_ring {
        assert_eq!(
            result,
            EmptiedRings::ForeignTransmitRing { base: device_base }
        );
        assert_eq!(events.len(), 1);
        return;
    }

    // One dummy descriptor at the device's tail reduced into the ring, stored before the tail
    // moves, and the tail moves by exactly one slot and stays in the ring.
    let index = (tail % u32::from(TRANSMIT_ENTRIES)) as u16;
    let stored: Vec<usize> = (0..events.len())
        .filter(|&at| matches!(events[at], Event::Descriptor(..)))
        .collect();
    let moved: Vec<usize> = (0..events.len())
        .filter(|&at| {
            matches!(
                events[at],
                Event::Write(TRANSMIT_DESCRIPTOR_TAIL_REGISTER, _)
            )
        })
        .collect();
    assert_eq!(stored.len(), 1);
    assert_eq!(moved.len(), 1);
    assert!(stored[0] < moved[0]);
    assert_eq!(
        events[stored[0]],
        Event::Descriptor(
            index,
            [transmit_ring, dummy_transmit_descriptor_command_word()]
        )
    );
    let new_tail = (u32::from(index) + 1) % u32::from(TRANSMIT_ENTRIES);
    assert_eq!(
        events[moved[0]],
        Event::Write(TRANSMIT_DESCRIPTOR_TAIL_REGISTER, new_tail)
    );
    assert!(events.contains(&Event::Write(
        TRANSMIT_CONTROL_REGISTER,
        transmit_control | TRANSMIT_ENABLE
    )));

    let receive_writes: Vec<u32> = events
        .iter()
        .filter_map(|event| match event {
            Event::Write(RECEIVE_CONTROL_REGISTER, value) => Some(*value),
            _ => None,
        })
        .collect();
    let receive_flush = Event::Write(
        RECEIVE_DESCRIPTOR_CONTROL_REGISTER,
        receive_descriptor_control_register_value_for_flush(receive_descriptor_control),
    );
    if second_status & DESCRIPTOR_RING_FLUSH_REQUIRED == 0 {
        assert_eq!(result, EmptiedRings::TransmitFlushed);
        assert!(receive_writes.is_empty());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, Event::Write(RECEIVE_DESCRIPTOR_CONTROL_REGISTER, _)))
        );
    } else {
        assert_eq!(result, EmptiedRings::BothFlushed);
        assert!(events.contains(&receive_flush));
        // Receive is switched off, on, and off again, ending off.
        assert_eq!(
            receive_writes,
            [
                receive_control & !RECEIVE_ENABLE,
                receive_control | RECEIVE_ENABLE,
                receive_control & !RECEIVE_ENABLE
            ]
        );
    }
});
