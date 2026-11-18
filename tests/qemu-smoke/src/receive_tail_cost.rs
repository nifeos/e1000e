//! **What one receive tail write per frame costs**, against a tail written once per burst.
//!
//! [`DataPlane::receive`] hands each descriptor back with its own tail write, and every write to
//! `RDT` is a register store the device acts on. This measures that cost under QEMU. Each round
//! sends a burst of ARP requests to the gateway, whose replies fill the receive ring, waits until
//! the device has written the last of them, and then times how long the data plane takes to drain
//! the burst. Even rounds use [`TailRegisters`], one tail write per frame; odd rounds use
//! [`OneTailPerBurst`], which keeps the newest receive tail the data plane asks for and writes it
//! once when the burst is drained, as [`Tails`] allows. Both hand the device the same descriptors in the end. A last
//! step times single `RDT` stores of the value it already holds, which changes nothing on the
//! device and isolates the cost of the store itself.
//!
//! Times are time-stamp counter ticks, read with `rdtsc`.

use e1000e::{
    DataPlane, DirectMemoryAccessRegion, MAXIMUM_FRAME_BYTES, Tails, layout, registers::*,
};

use crate::machine::Registers;
use crate::{TailRegisters, gateway_lookup};

/// Frames per burst: every receive descriptor the data plane posts.
const BURST: u32 = e1000e::RECEIVE_ENTRIES as u32 - 1;
/// Rounds of each kind.
const ROUNDS: usize = 32;
/// Single tail stores timed in the last step.
const STORES: u32 = 1000;
/// How many times a wait polls before the measurement fails.
const POLLS: u32 = 50_000_000;

fn ticks() -> u64 {
    // SAFETY: `rdtsc` reads the time-stamp counter and has no other effect.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// The two tails, with the receive tail kept back until [`OneTailPerBurst::flush`].
struct OneTailPerBurst<'a> {
    registers: &'a Registers,
    receive_tail: Option<u32>,
    writes: u32,
}

impl OneTailPerBurst<'_> {
    /// Write the newest receive tail the data plane asked for, if it asked for one.
    fn flush(&mut self) {
        if let Some(value) = self.receive_tail.take() {
            self.barrier();
            self.registers
                .write(RECEIVE_DESCRIPTOR_TAIL_REGISTER, value);
            self.writes += 1;
        }
    }
}

impl Tails for OneTailPerBurst<'_> {
    fn set_receive_tail(&mut self, value: u32) {
        self.receive_tail = Some(value);
    }
    fn set_transmit_tail(&mut self, value: u32) {
        self.registers
            .write(TRANSMIT_DESCRIPTOR_TAIL_REGISTER, value);
    }
    fn barrier(&mut self) {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// One tail write per frame, counted.
struct CountedTails<'a> {
    inner: TailRegisters<'a>,
    writes: u32,
}

impl Tails for CountedTails<'_> {
    fn set_receive_tail(&mut self, value: u32) {
        self.inner.set_receive_tail(value);
        self.writes += 1;
    }
    fn set_transmit_tail(&mut self, value: u32) {
        self.inner.set_transmit_tail(value);
    }
    fn barrier(&mut self) {
        self.inner.barrier();
    }
}

/// Send a burst of requests and wait until the device has written a reply into the descriptor
/// `BURST - 1` after the cursor.
fn fill(
    data_plane: &mut DataPlane,
    region: &mut impl DirectMemoryAccessRegion,
    registers: &Registers,
    station: [u8; 6],
) {
    let frame = gateway_lookup::request(station);
    let mut tails = TailRegisters(registers);
    for _ in 0..BURST {
        let sent = (0..POLLS).any(|_| data_plane.transmit(region, &mut tails, &frame).is_ok());
        if !sent {
            crate::fail("measurement: the transmit ring never freed a descriptor");
        }
    }
    let last = (data_plane.receive_cursor() + BURST as u16 - 1) % e1000e::RECEIVE_ENTRIES;
    let status_offset = layout::receive_descriptor(last) + 8;
    let written = (0..POLLS).any(|_| {
        (region.read_u64(status_offset) >> 32) as u8 & e1000e::RECEIVE_STATUS_DESCRIPTOR_DONE != 0
    });
    if !written {
        crate::fail("measurement: the burst of replies never filled the ring");
    }
}

/// Receive `BURST` frames through `tails`, returning the ticks it took.
fn drain(
    data_plane: &mut DataPlane,
    region: &mut impl DirectMemoryAccessRegion,
    tails: &mut impl Tails,
) -> u64 {
    let mut buffer = [0; MAXIMUM_FRAME_BYTES];
    let start = ticks();
    let mut frames = 0;
    let mut polls = 0;
    while frames < BURST {
        if data_plane.receive(region, tails, &mut buffer).is_some() {
            frames += 1;
        }
        polls += 1;
        if polls == POLLS {
            crate::fail("measurement: a burst never drained");
        }
    }
    ticks() - start
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

/// Run the rounds and print the medians.
pub fn measure(
    data_plane: &mut DataPlane,
    region: &mut impl DirectMemoryAccessRegion,
    registers: &Registers,
    station: [u8; 6],
) {
    let mut per_frame = [0u64; ROUNDS];
    let mut per_burst = [0u64; ROUNDS];
    let mut per_frame_writes = 0;
    let mut per_burst_writes = 0;
    for round in 0..2 * ROUNDS {
        fill(data_plane, region, registers, station);
        if round % 2 == 0 {
            let mut tails = CountedTails {
                inner: TailRegisters(registers),
                writes: 0,
            };
            per_frame[round / 2] = drain(data_plane, region, &mut tails);
            per_frame_writes += tails.writes;
        } else {
            let mut tails = OneTailPerBurst {
                registers,
                receive_tail: None,
                writes: 0,
            };
            let start = ticks();
            drain(data_plane, region, &mut tails);
            tails.flush();
            per_burst[round / 2] = ticks() - start;
            per_burst_writes += tails.writes;
        }
    }
    let tail = registers.read(RECEIVE_DESCRIPTOR_TAIL_REGISTER);
    let start = ticks();
    for _ in 0..STORES {
        registers.write(RECEIVE_DESCRIPTOR_TAIL_REGISTER, tail);
    }
    let per_store = (ticks() - start) / u64::from(STORES);
    let per_frame = median(&mut per_frame);
    let per_burst = median(&mut per_burst);
    say!(
        "measurement: {ROUNDS} bursts of {BURST} frames each way, median ticks per burst\n\
         measurement: one tail write per frame: {per_frame} ticks, {} per frame, {} tail writes\n\
         measurement: one tail write per burst: {per_burst} ticks, {} per frame, {} tail writes\n\
         measurement: one receive tail store: {per_store} ticks\n",
        per_frame / u64::from(BURST),
        per_frame_writes,
        per_burst / u64::from(BURST),
        per_burst_writes,
    );
}
