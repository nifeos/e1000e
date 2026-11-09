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
Intel shared code and its `if_em.c` driver. You supply the volatile register accesses and the DMA memory; everything computed from them
is here, `no_std`, and tested on the host against a simulated device.

## EXAMPLES

Claim a device, then judge each receive descriptor's status word before trusting its length.

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
assert_eq!(receive_verdict(word(60, done), 1514), ReceiveVerdict::Frame(60));
assert_eq!(receive_verdict(word(60, 0), 1514), ReceiveVerdict::NotYet);
assert_eq!(receive_verdict(word(0xffff, done), 1514), ReceiveVerdict::Drop);
```

For the rings, implement `DirectMemoryAccessRegion` over the DMA memory and `Tails` over the queue registers, then
drive a `DataPlane` with `start`, `receive` and `transmit`. Each trait's documentation states the
contract an implementation keeps: every access volatile, and the barrier each architecture needs.

## What is proven

Three Kani harnesses cover the input a device or a caller controls:

- `proofs::a_delivered_frame_always_fits_the_buffer_and_the_room`: for every status word a device
  can write and every caller buffer size, a frame that `receive_verdict` delivers fits both the
  receive buffer and the caller's space.
- `proofs::every_ring_offset_is_inside_the_region`: for any index, every descriptor and buffer
  offset lies inside its part of the DMA region.
- `platform_controller_hub::physical_layer_device::proofs::a_paged_access_selects_the_offsets_own_page_and_register`:
  every paged access to the physical (PHY) layer device names a PHY address that exists, the
  offset's own register and the offset's own page.

The proofs need Kani, which `rustup` does not install. Run `cargo install --locked
kani-verifier --version 0.68.0`, then `cargo kani setup`. The setup step downloads a model
checker, so it needs network access. CI uses version 0.68.0, and another version may verify
differently.

Rerun them with `cargo kani`. Each harness has a falsification record at
`falsifications/<module.path>.<harness>.patch`; applying it with `patch -p1` from the crate root
makes that harness fail at its own assertion, which shows the proof checks something.

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

The I219 needs a port rather than a reading of registers. It is a media access control (MAC) block inside the platform
controller hub, talking to a separate PHY that it shares with the Management Engine, and it
carries workarounds that only field exposure finds. For `platform_controller_hub`, the author read
only FreeBSD's Intel shared code (BSD-3-Clause) and its `if_em.c` and `if_em.h` (BSD-2-Clause).
The author did not consult Linux's `e1000e`, which is GPL, for any of it. Each item in
`platform_controller_hub` names the FreeBSD function it follows.

The author read everything from `sys/dev/e1000/` on FreeBSD's `main`: `ultra_low_power`, `flush`
and `reset` on 2026-10-04, the rest on 2026-10-05. The last commit touching `e1000_ich8lan.c`
then was 46cf612d (2026-08-30). The author copied the notice of `if_em.c` and `if_em.h` from
`freebsd/freebsd-src@9da2c05a748f` and compared it against that source on 2026-11-04.

## BUGS

- One receive and one transmit queue, polled. No interrupts, checksum offload, segmentation offload
  or jumbo frames.
- The crate claims only the 82574L and the Sunrise Point I219 rows of `DEVICE_IDS`. Later platform
  controller hub (PCH) generations need workarounds this crate does not carry.
- The crate does not port the I219's link-up half, so it does not reconfigure the PHY when link
  comes up. A link that comes up and then loses frames points there first. The crate does not
  configure flow control.
- The I219 port has run on real hardware once, with no cable. Otherwise only the simulator, which
  is this crate's reading of FreeBSD, checks it.
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
SPDX expression is `BSD-2-Clause AND BSD-3-Clause`.
The manifest's `license` field names all three: `(MIT OR Apache-2.0) AND BSD-3-Clause AND BSD-2-Clause`.
