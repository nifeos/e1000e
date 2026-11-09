//! **The I219's own bring-up, ported from FreeBSD's `e1000` shared code and its `if_em.c` driver.**
//!
//! The I219 is a MAC inside the platform controller hub (PCH) talking to a separate PHY that the
//! Management Engine shares. FreeBSD's Intel shared code (BSD-3-Clause, notice below) and its driver
//! files `if_em.c` and `if_em.h` (BSD-2-Clause, second notice below) are the only sources read for
//! it: [`ultra_low_power`], [`flush`], [`reset`], [`physical_layer_device`],
//! [`non_volatile_memory`] and [`sequence`] each name the FreeBSD functions they follow. Every
//! sequence is generic over [`Hardware`], so it runs on the host against a simulated I219 (`simulator`)
//! as well as in a kernel. The constants are facts about the hardware; the sequences are
//! FreeBSD's, adapted, and carry the licenses below as their first conditions require.

//! The files under `src/platform_controller_hub/` are `SPDX-License-Identifier: BSD-2-Clause AND BSD-3-Clause`, with
//! the crate's own `MIT OR Apache-2.0` beside them. The Intel shared code (`e1000_ich8lan.c` and
//! the files beside it) carries, as found:
//!
//! ```text
//! SPDX-License-Identifier: BSD-3-Clause
//!
//! Copyright (c) 2001-2020, Intel Corporation
//! All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//!  1. Redistributions of source code must retain the above copyright notice,
//!     this list of conditions and the following disclaimer.
//!
//!  2. Redistributions in binary form must reproduce the above copyright
//!     notice, this list of conditions and the following disclaimer in the
//!     documentation and/or other materials provided with the distribution.
//!
//!  3. Neither the name of the Intel Corporation nor the names of its
//!     contributors may be used to endorse or promote products derived from
//!     this software without specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//! AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
//! LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
//! CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
//! SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
//! INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
//! CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//! ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
//! POSSIBILITY OF SUCH DAMAGE.
//! ```

//! FreeBSD's driver files `sys/dev/e1000/if_em.c` and `if_em.h` are not Intel shared code, and
//! their license is BSD-2-Clause, not BSD-3-Clause. [`flush`] follows `em_flush_desc_rings`,
//! `em_flush_tx_ring` and `em_flush_rx_ring` with the constants `if_em.h` defines for them, and
//! [`sequence`] takes `em_initialize_transmit_unit`'s transmit errata and `em_reset`'s
//! packet-buffer allocation from `if_em.c`. The kernel runs the flush through
//! [`flush::empty_descriptor_rings`], so this notice covers it there too. The two files carry the same
//! notice, copied as found at `freebsd/freebsd-src@9da2c05a748fd17757638e1a9e78215cdb8b80d0`,
//! fetched 2026-11-04 (UTC):
//!
//! ```text
//! SPDX-License-Identifier: BSD-2-Clause
//!
//! Copyright (c) 2001-2024, Intel Corporation
//! Copyright (c) 2016 Nicole Graziano <nicole@nextbsd.org>
//! Copyright (c) 2024 Kevin Bowling <kbowling@FreeBSD.org>
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions
//! are met:
//! 1. Redistributions of source code must retain the above copyright
//!    notice, this list of conditions and the following disclaimer.
//! 2. Redistributions in binary form must reproduce the above copyright
//!    notice, this list of conditions and the following disclaimer in the
//!    documentation and/or other materials provided with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
//! ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//! IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//! ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
//! FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
//! DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
//! OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
//! HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
//! LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//! OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
//! SUCH DAMAGE.
//! ```

//! # BUGS
//!
//! - **It has run on silicon once, with no cable.** Otherwise it is proven only against `simulator`,
//!   which is this crate's reading of FreeBSD and not the device.
//! - **The link-up half is not ported** (`e1000_check_for_copper_link_ich8lan`). A link that comes
//!   up and then loses frames points here first.
//! - **Flow control is not configured, so it is not advertised** ([`sequence::setup_copper_link`]).
//! - **Four more steps of FreeBSD's bring-up are left out**: the `GCR` no-snoop write, `WUFC`
//!   cleared after the reset, `VET`, and `PBA` before the flush. [`sequence`] lists them.
//! - **On the smallest NVM, a word past a bank's end is read from beyond it**, as FreeBSD's
//!   `e1000_read_nvm_spt` reads it: a 4 KiB NVM has 1024-word banks, and any word up to the shadow
//!   RAM's 2048 is accepted.
//! - **The transmit flush diverges from FreeBSD in one place**, said at [`flush::tail_after_dummy_transmit`].
//! - **A failed `CV_SMB_CTRL` read is not written back**, where FreeBSD writes a stale value.
//! - **The licenses' second condition is the distributor's**: a binary built from this crate must
//!   reproduce the notices below in its documentation or other materials.

pub mod non_volatile_memory;
pub mod physical_layer_device;
pub mod sequence;
#[cfg(test)]
mod simulator;

/// MAC registers the I219 adds. Offsets from FreeBSD's `e1000_regs.h` and `e1000_ich8lan.h`.
pub mod registers;

/// Leaving ultra-low-power (ULP) mode through the Management Engine: the Management Engine branch of FreeBSD's
/// `e1000_disable_ulp_lpt_lp` (`e1000_ich8lan.c`).
#[doc(alias = "ulp")]
pub mod ultra_low_power;

/// The SPT descriptor-ring flush. FreeBSD's comment: "In I219, the descriptor rings must be
/// emptied before resetting the hardware ... Failure to do this will cause the hardware to enter a unit hang
/// state which can only be released by PCI reset on the device." FreeBSD's `em_flush_desc_rings`,
/// `em_flush_tx_ring` and `em_flush_rx_ring` (`if_em.c`).
pub mod flush;

/// The MAC-register constants around the global reset: FreeBSD's `e1000_reset_hw_ich8lan`
/// (`e1000_ich8lan.c`) and `e1000_disable_pcie_master_generic` (`e1000_mac.c`).
pub mod reset;

#[cfg(test)]
mod tests;

mod hardware_access;

use hardware_access::*;
pub use hardware_access::{Hardware, is_platform_controller_hub_device};
