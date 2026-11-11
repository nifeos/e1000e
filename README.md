# e1000e

[![check](https://github.com/nifeos/e1000e/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/nifeos/e1000e/actions/workflows/ci.yml?query=branch%3Amain)
[![proofs](https://github.com/nifeos/e1000e/actions/workflows/proofs.yml/badge.svg?branch=main)](https://github.com/nifeos/e1000e/actions/workflows/proofs.yml?query=branch%3Amain)
[![license](https://img.shields.io/badge/license-%28MIT%20OR%20Apache--2.0%29%20AND%20BSD--3--Clause%20AND%20BSD--2--Clause-blue.svg)](https://github.com/nifeos/e1000e#license)
[![minimum Rust version](https://img.shields.io/badge/rust-1.88%2B-blue.svg)](https://github.com/nifeos/e1000e/blob/main/Cargo.toml)

The logic of a driver for Intel's e1000e network controllers, the 82574L that QEMU's `-device
e1000e` presents and the Sunrise Point I219, with no register access of its own. It holds the
register offsets and bits, the descriptor formats and the direct memory access (DMA) region's
layout. It also holds the whole receive and transmit ring logic behind two small traits
(`DirectMemoryAccessRegion` and `Tails`), and the I219 bring-up sequences ported from FreeBSD's
Intel shared code and its `if_em.c` driver. You supply the volatile register accesses and the DMA
memory; everything computed from them is here, `no_std`, and tested on the host against a simulated
device.

## Getting started

`cargo build` and `cargo test` need no setup beyond a Rust toolchain at the minimum version below.
The Kani proofs under "What is proven" are the only part that needs extra tools.

## EXAMPLES

Claim a device, then judge each receive descriptor's status word before trusting its length.
This example is a doctest, so `cargo test` runs it.

```rust
use e1000e::{
    RECEIVE_STATUS_DESCRIPTOR_DONE, RECEIVE_STATUS_END_OF_PACKET, ReceiveVerdict, VENDOR_INTEL,
    is_supported_device, model_name,
    receive_verdict,
};

assert!(is_supported_device(VENDOR_INTEL, 0x10d3));
assert_eq!(model_name(0x10d3), Some("82574L (QEMU's e1000e)"));

let word = |len: u64, status: u8| len | (status as u64) << 32;
let done = RECEIVE_STATUS_DESCRIPTOR_DONE | RECEIVE_STATUS_END_OF_PACKET;
assert_eq!(receive_verdict(word(60, done), 1518), ReceiveVerdict::Frame(60));
assert_eq!(receive_verdict(word(60, 0), 1518), ReceiveVerdict::NotYet);
assert_eq!(receive_verdict(word(0xffff, done), 1518), ReceiveVerdict::Drop);
```

For the rings, implement `DirectMemoryAccessRegion` over the DMA memory and `Tails` over the queue
registers, then drive a `DataPlane` with `start`, `receive` and `transmit`. Each trait's
documentation states the contract an implementation keeps: every access volatile, and the barrier
each architecture needs.
[`examples/host_ring.rs`](examples/host_ring.rs) drives both rings against a fake device
(`cargo run --example host_ring`), and [`docs/bring-up.md`](docs/bring-up.md) gives the register
order a driver performs before `start`. [`docs/qemu.md`](docs/qemu.md) runs the same data plane
against QEMU's emulated 82574L from a small bare-metal program, `tests/qemu-smoke/`, which sends a
frame and receives the reply.

## What is proven

These Kani harnesses cover the input a device or a caller controls.

- `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room`: for every status word a device
  can write and every caller buffer size, a frame that `receive_verdict` delivers fits both the
  receive buffer and the caller's space.
- `proofs::every_ring_offset_is_inside_the_region`: for any index, every descriptor and buffer
  offset lies inside its part of the DMA region.
- `platform_controller_hub::physical_layer_device::proofs::a_paged_access_selects_the_offsets_own_page_and_register`:
  every paged access to the physical (PHY) layer device names a PHY address that exists, the
  offset's own register and the offset's own page.
- `receive_proofs::a_receive_reads_only_its_own_slot_and_no_more_than_the_caller_has_room_for`:
  the device rewrites every status word at each read and the caller's buffer is any size. One
  `receive` then reads frame bytes at most once. It reads only the buffer of the descriptor it
  consumed, only as many bytes as the frame it returns, and never more than the caller's room. It
  leaves the cursor on the descriptor after the last one consumed.
- `proofs::one_receive_examines_at_most_one_lap_of_the_ring`: whatever the device writes, one
  `receive` reads at most one status word per descriptor in the ring, so it always returns.
- `ring_proofs::the_transmit_ring_never_writes_what_the_device_holds`: against a device that
  completes descriptors in order, `transmit` never writes a descriptor or buffer the device holds,
  and it reports `Full` only when every other slot is the device's.
- `platform_controller_hub::non_volatile_memory::proofs::a_word_read_stays_in_range_for_every_strap`:
  for every strap value and every word, a read past the shadow RAM fails before it touches any
  register, and every other read lands inside the bank the signature chose.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently. Setup also downloads the Kani release's pinned nightly Rust toolchain and a CBMC
bundle. A full `cargo kani` run takes about nine minutes on a desktop machine. It prints an
unstable-feature warning and an "unsupported constructs" note about code the proofs never reach, and
it still succeeds.

Rerun them with `cargo kani`, or one of them with `cargo kani --harness <name>`. Each harness has
a falsification record at `falsifications/<module.path>.<harness>.patch`. Applying it with
`patch -p1 < <file>` from the crate root makes that harness fail at its own assertion, which shows
the proof checks something; the patch header names the assertion. `patch -R -p1 < <file>` takes it
out again.

The simulated device tests the non-volatile memory (NVM) reads, the reset, the ultra-low-power (ULP)
and PHY workaround sequences, and transmit descriptor status. No model checker covers them.

## Sources

The author read offsets, bits, descriptor layouts and device ids on 2026-10-04 (UTC) instead of
recalling them. The sources were Linux's `drivers/net/ethernet/intel/e1000e/regs.h`, `defines.h`
and `hw.h`. The author cross-checked them the same day against FreeBSD's BSD-licensed
`sys/dev/e1000/`: `e1000_regs.h`, and the SPT (Sunrise Point) class of `e1000_api.c`. That class
holds every I219 row of `DEVICE_IDS` and two Comet Lake ids (`0x0d53`, `0x0d55`) the crate does not
claim. The crate copies no Linux code.

QEMU's `hw/net/e1000e_core.c` and `hw/net/e1000x_common.c` say what the emulated 82574L does with
them. QEMU uses legacy descriptors unless software sets `RFCTL.EXTEN`, and it receives only with
`RCTL.EN`, link up and bus mastering.

The author did not read Intel's own I219 datasheet. Where the I219 likely differs from the 82574L,
the item says so.

The I219 needs a port rather than a reading of registers. It is a media access control (MAC) block
inside the platform controller hub, talking to a separate PHY that it shares with the Management Engine,
and it carries workarounds that only field exposure finds. For `platform_controller_hub`, the author read
only FreeBSD's Intel shared code (BSD-3-Clause) and its `if_em.c` and `if_em.h` (BSD-2-Clause).
The author did not consult Linux's `e1000e`, which is GPL, for any of it. Each item in
`platform_controller_hub` names the FreeBSD function it follows.

The author read everything from `sys/dev/e1000/` on FreeBSD's `main`: `ultra_low_power`, `flush`
and `reset` on 2026-10-04, the rest on 2026-10-05. The last commit touching `e1000_ich8lan.c`
then was 46cf612d (2026-08-30). The author copied the notice of `if_em.c` and `if_em.h` from
`freebsd/freebsd-src@9da2c05a748f` and compared it against that source on 2026-11-04.

## BUGS

- One receive and one transmit queue, polled. No interrupts, checksum offload, segmentation offload
  or jumbo frames. `registers` carries the interrupt cause, throttling, mask-set and mask-clear
  registers, and the crate names the common cause bits, but no code here enables or handles an
  interrupt.
- Each ring has 16 descriptors (`RECEIVE_ENTRIES`, `TRANSMIT_ENTRIES`), fixed by constants that
  shape the region's layout and the proofs. A later version may change that.
- The largest frame is 1518 bytes (`MAXIMUM_FRAME_BYTES`): 1500 bytes of payload, the Ethernet
  header and one virtual local area network (VLAN) tag. A frame with two tags is dropped on receive
  and refused on transmit.
- The test doubles are private to the crate's tests. `examples/host_ring.rs` is the fake to start
  from.
- The crate claims only the 82574L and the Sunrise Point I219 rows of `DEVICE_IDS`. Later platform
  controller hub (PCH) generations need workarounds this crate does not carry.
- The crate does not port the I219's link-up half, so it does not reconfigure the PHY when link
  comes up. A link that comes up and then loses frames points there first. The crate does not
  configure flow control.
- No version of this crate has run on an I219. Only the simulator, which is this crate's reading of
  FreeBSD, checks the port.
- Legacy descriptors on the I219 are an assumption from the 8254x lineage.

## Minimum Rust version

Rust 1.88.

## License

Licensed under either of MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), at your option. The files under `src/platform_controller_hub/`
adapt FreeBSD's Intel shared code and `if_em.c` and `if_em.h`. They also carry Intel's BSD-3-Clause
notice ([LICENSE-BSD-3-Clause-Intel](LICENSE-BSD-3-Clause-Intel)) and the BSD-2-Clause notice of
`if_em.c` and `if_em.h` (Intel, Nicole Graziano, Kevin Bowling;
[LICENSE-BSD-2-Clause-FreeBSD](LICENSE-BSD-2-Clause-FreeBSD)). A binary redistribution must
reproduce both notices. Their
SPDX expression is `BSD-2-Clause AND BSD-3-Clause`. The manifest's `license` field names all three:
`(MIT OR Apache-2.0) AND BSD-3-Clause AND BSD-2-Clause`.
