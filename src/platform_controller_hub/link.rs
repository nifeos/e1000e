//! **The I219's link-up half: FreeBSD's `e1000_check_for_copper_link_ich8lan`, its SPT path.**
//!
//! [`check_for_copper_link`] is what FreeBSD runs each time `em` asks whether the link changed. It
//! reads the PHY's link bit, and with link it fits the transmit gap, the PHY's receive
//! configuration, its clock gate, the K1 clock request and the pointer gap to the speed. Then, link
//! or not, it sets the beacon duration, reports a latency tolerance to the platform
//! (`e1000_platform_pm_pch_lpt`), copies the K1-off enable and sets the K0s minimum time. With
//! link it then turns energy efficient ethernet (EEE) off, as `em` does by default, and sets the
//! collision distance. Nothing here reads or writes the NVM or the flash.
//!
//! The branches FreeBSD takes only for other MAC types (`e1000_pchlan`, `e1000_pch2lan`, Lynx
//! Point low power and the I218 device ids) are left out, because every part this crate claims is
//! Sunrise Point ([`super::is_platform_controller_hub_device`]).

//! # Departures from FreeBSD
//!
//! - Flow control is not resolved after link up (`e1000_config_fc_after_link_up_generic`), because
//!   this driver advertises none ([`super::sequence::setup_copper_link`]).
//! - EEE is only ever turned off: `em`'s default `eee_setting` disables it, and the enabling half
//!   of `e1000_set_eee_pchlan` is not carried.
//! - A failed read of the clock gate or of `HV_PM_CTRL` skips the write after it, where FreeBSD
//!   writes back whatever its variable held, and the failure is noted.
//! - The platform's latency limits come from [`LinkWatch::new`], read once by the caller, where
//!   FreeBSD reads them from configuration space on every check.

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

use super::physical_layer_device::{self, SoftwareFlag, registers as phy};
use super::sequence::{BringUpObservation, Error};
use super::{Hardware, delay_milliseconds, flush_writes, registers};
use crate::registers as mac;

// `STATUS` bits (FreeBSD's `e1000_defines.h`).
const STATUS_FULL_DUPLEX: u32 = 0x0000_0001;
const STATUS_SPEED_100: u32 = 0x0000_0040;
const STATUS_SPEED_1000: u32 = 0x0000_0080;
// `TIPG` and `TCTL` (`e1000_defines.h`).
const TRANSMIT_INTER_PACKET_GAP_TRANSMIT_MASK: u32 = 0x0000_03ff;
const TRANSMIT_CONTROL_COLLISION_DISTANCE_MASK: u32 = 0x003f_f000;
const COLLISION_DISTANCE: u32 = 63;
const COLLISION_DISTANCE_SHIFT: u32 = 12;
// The PHY (`e1000_defines.h`, `e1000_ich8lan.h`).
const PHYSICAL_LAYER_DEVICE_STATUS_LINK: u16 = 0x0004;
const I217_RECEIVE_CONFIGURATION: u16 = 0xb20c;
const I217_ENERGY_EFFICIENT_ETHERNET_PHYSICAL_CODING_SUBLAYER_STATUS: u16 = 0x9401;
const PHASE_LOCKED_LOOP_CLOCK_GATE_MASK: u16 = 0x07ff;
const POWER_MANAGEMENT_CONTROL_K1_CLOCK_REQUEST: u16 = 0x0200;
const POINTER_GAP_MASK: u16 = 0x3ff << 2;
const POINTER_GAP_MINIMUM: u16 = 0x18;
const POINTER_GAP_BELOW_GIGABIT: u16 = 0xc023;
const LOW_POWER_IDLE_CONTROL_ENABLE_MASK: u16 = 0x6000;
// MAC registers of the link check (`e1000_ich8lan.h`).
const FEXTNVM4_BEACON_DURATION_MASK: u32 = 0x7;
const FEXTNVM4_BEACON_DURATION_8_MICROSECONDS: u32 = 0x7;
const FEXTNVM6_K1_OFF_ENABLE: u32 = 0x8000_0000;
const PACKET_BUFFER_ALLOCATION_RECEIVE_MASK: u32 = 0xffff;
const LATENCY_TOLERANCE_VALUE_MASK: u16 = 0x03ff;
const LATENCY_TOLERANCE_SCALE_MAXIMUM: u16 = 5;
const LATENCY_TOLERANCE_SCALE_FACTOR: u32 = 5;
const LATENCY_TOLERANCE_SCALE_SHIFT: u32 = 10;
const LATENCY_TOLERANCE_SCALE_MASK: u16 = 0x1c00;
const LATENCY_TOLERANCE_REQUIREMENT_SHIFT: u32 = 15;
const LATENCY_TOLERANCE_NO_SNOOP_SHIFT: u32 = 16;
const LATENCY_TOLERANCE_SEND: u32 = 1 << 30;
const SIDEBAND_THRESHOLD_HIGH_WATER_MARK_MASK: u32 = 0x1f;
const SIDEBAND_CONTROL_BUFFER_FLUSH_ENABLE: u32 = 0x0000_0001;
const SIDEBAND_CONTROL_BUFFER_FLUSH_UNMASK_INTERRUPTS: u32 = 0x0000_1000;
// Kumeran (`e1000_phy.h`).
const KUMERAN_OFFSET_MASK: u32 = 0x001f_0000;
const KUMERAN_OFFSET_SHIFT: u32 = 16;
const KUMERAN_READ_ENABLE: u32 = 0x0020_0000;
const KUMERAN_K0S_CONTROL: u32 = 0x1e;
const KUMERAN_K0S_MINIMUM_TIME_MASK: u16 = 7 << 4;
/// FreeBSD's `K1_MIN_TIME`, 1, in the K0s minimum time field: 256 ns of transmit idle.
const KUMERAN_K0S_MINIMUM_TIME: u16 = 1 << 4;

// Each value below lands inside the field the word it joins has just masked out, or two operands
// share no bit, so | and ^ compose the word identically; the compiler checks that here, so this
// module's OR-to-XOR swaps are equivalent mutants (`.github/mutants-exempts`).
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    FEXTNVM4_BEACON_DURATION_8_MICROSECONDS & !FEXTNVM4_BEACON_DURATION_MASK == 0
        && (COLLISION_DISTANCE << COLLISION_DISTANCE_SHIFT)
            & !TRANSMIT_CONTROL_COLLISION_DISTANCE_MASK
            == 0
        && (0xff | 0x0c | 0x08) & !TRANSMIT_INTER_PACKET_GAP_TRANSMIT_MASK == 0
        && (0xfa | 0x3e8) & !PHASE_LOCKED_LOOP_CLOCK_GATE_MASK == 0
        && (POINTER_GAP_MINIMUM << 2) & !POINTER_GAP_MASK == 0
        && KUMERAN_K0S_MINIMUM_TIME & !KUMERAN_K0S_MINIMUM_TIME_MASK == 0
        && LATENCY_TOLERANCE_VALUE_MASK >> LATENCY_TOLERANCE_SCALE_SHIFT == 0
        && 0xffff & (KUMERAN_OFFSET_MASK | KUMERAN_READ_ENABLE) == 0
        && KUMERAN_OFFSET_MASK & KUMERAN_READ_ENABLE == 0
        && SIDEBAND_CONTROL_BUFFER_FLUSH_ENABLE & SIDEBAND_CONTROL_BUFFER_FLUSH_UNMASK_INTERRUPTS
            == 0
);
// `LTRV`: the no-snoop requirement (bit 31), the send bit (30), the no-snoop encoding (bits 28:16),
// the snoop requirement (15) and the snoop encoding (bits 12:0) share no bit.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    (1 << (LATENCY_TOLERANCE_REQUIREMENT_SHIFT + LATENCY_TOLERANCE_NO_SNOOP_SHIFT))
        ^ LATENCY_TOLERANCE_SEND
        ^ 0x1fff << LATENCY_TOLERANCE_NO_SNOOP_SHIFT
        ^ 1 << LATENCY_TOLERANCE_REQUIREMENT_SHIFT
        ^ 0x1fff
        == (1 << (LATENCY_TOLERANCE_REQUIREMENT_SHIFT + LATENCY_TOLERANCE_NO_SNOOP_SHIFT))
            | LATENCY_TOLERANCE_SEND
            | 0x1fff << LATENCY_TOLERANCE_NO_SNOOP_SHIFT
            | 1 << LATENCY_TOLERANCE_REQUIREMENT_SHIFT
            | 0x1fff
);

/// `ETHERMTU + ETHER_HDR_LEN + ETHER_CRC_LEN`, the frame size `em` gives the latency tolerance at
/// the standard 1500-byte maximum transmission unit (`em_if_mtu_set` in `if_em.c`).
pub const MAXIMUM_FRAME_SIZE_BYTES: u32 = 1518;
/// `em_if_update_admin_status`'s wait before each check on Sunrise Point (`if_em.c`).
pub const SUNRISE_POINT_SETTLE_MILLISECONDS: u64 = 50;

/// **The platform's latency limits**: the two 16-bit words at offsets `0xA8` and `0xAA` of the
/// I219's own PCI configuration space (FreeBSD's `E1000_PCI_LTR_CAP_LPT` and the word after it),
/// each encoded as a latency tolerance (scale in bits 12:10, value in bits 9:0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformLatencyTolerance {
    /// The maximum snoop latency, configuration offset `0xA8`.
    pub maximum_snoop: u16,
    /// The maximum no-snoop latency, configuration offset `0xAA`.
    pub maximum_no_snoop: u16,
}

/// **FreeBSD's `mac->get_link_status`**, and the platform limits a check needs.
///
/// A new watch is due, as `em` sets the flag at init. A due check that finds link clears it; one
/// that fails or finds no link leaves it due, so the next call checks again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkWatch {
    is_check_due: bool,
    platform: PlatformLatencyTolerance,
}

impl LinkWatch {
    /// A watch whose first check is due.
    pub const fn new(platform: PlatformLatencyTolerance) -> Self {
        LinkWatch {
            is_check_due: true,
            platform,
        }
    }

    /// Make the next check due: call it on a link status change interrupt (`ICR.LSC`), or when
    /// polling sees `STATUS.LU` differ from what the last check reported (`em_handle_link`).
    pub fn link_status_changed(&mut self) {
        self.is_check_due = true;
    }

    /// Will the next [`check_for_copper_link`] go to the hardware?
    pub const fn is_check_due(&self) -> bool {
        self.is_check_due
    }
}

/// The speed `STATUS` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkSpeed {
    /// 10 Mbit/s.
    TenMegabitsPerSecond,
    /// 100 Mbit/s.
    HundredMegabitsPerSecond,
    /// 1000 Mbit/s.
    OneGigabitPerSecond,
}

impl LinkSpeed {
    /// The speed in Mbit/s, FreeBSD's `SPEED_10`, `SPEED_100` or `SPEED_1000`.
    pub const fn megabits_per_second(self) -> u32 {
        match self {
            LinkSpeed::TenMegabitsPerSecond => 10,
            LinkSpeed::HundredMegabitsPerSecond => 100,
            LinkSpeed::OneGigabitPerSecond => 1000,
        }
    }

    /// `e1000_get_speed_and_duplex_copper_generic`'s reading of `STATUS`.
    const fn from_status(status: u32) -> Self {
        if status & STATUS_SPEED_1000 != 0 {
            LinkSpeed::OneGigabitPerSecond
        } else if status & STATUS_SPEED_100 != 0 {
            LinkSpeed::HundredMegabitsPerSecond
        } else {
            LinkSpeed::TenMegabitsPerSecond
        }
    }
}

/// What a [`check_for_copper_link`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LinkReport {
    /// No check was due, so the hardware was not touched; the link is as last reported.
    NotChecked,
    /// The PHY reports no link.
    Down,
    /// The PHY reports link, at this speed and duplex.
    Up {
        /// From `STATUS`.
        speed: LinkSpeed,
        /// `STATUS.FD`.
        is_full_duplex: bool,
    },
}

/// **`e1000_check_for_copper_link_ich8lan`, as `em_if_update_admin_status` calls it.**
///
/// Call it after the bring-up has written `TCTL` and `TIPG`, because a later `TIPG` write undoes the
/// gap it sets. Then call it on every link status change, after [`LinkWatch::link_status_changed`],
/// and periodically besides, as `em` does from its timer; a call with no check due returns
/// [`LinkReport::NotChecked`] without touching the hardware. A due check first waits
/// [`SUNRISE_POINT_SETTLE_MILLISECONDS`], as `em` does, so it blocks for at least that long.
///
/// It notes [`BringUpObservation::LatencyToleranceReported`] on every due check that gets that far,
/// and the other link observations when they happen.
///
/// # Errors
///
/// - [`Error::PhysicalLayerDevice`] when the semaphore cannot be taken or a PHY access whose result
///   FreeBSD checks fails.
/// - [`Error::LatencyToleranceOutOfRange`] when the receive buffer gives a latency or high water
///   mark the registers cannot hold.
///
/// Either way the check stays due. A failure after the link is found up (turning EEE off) is
/// noted instead, the collision distance is left as the bring-up wrote it, and the link is
/// reported up, as `em` reports it.
#[doc(alias = "e1000_check_for_copper_link_ich8lan")]
pub fn check_for_copper_link(
    hardware: &mut impl Hardware,
    watch: &mut LinkWatch,
) -> Result<LinkReport, Error> {
    if !watch.is_check_due {
        return Ok(LinkReport::NotChecked);
    }
    delay_milliseconds(hardware, SUNRISE_POINT_SETTLE_MILLISECONDS);
    let link = if has_link(hardware)? {
        let status = hardware.read(mac::DEVICE_STATUS_REGISTER);
        let speed = LinkSpeed::from_status(status);
        let is_full_duplex = status & STATUS_FULL_DUPLEX != 0;
        configure_for_speed(hardware, speed, is_full_duplex)?;
        configure_pointer_gap(hardware, speed)?;
        Some((speed, is_full_duplex))
    } else {
        None
    };
    // "I217 Packet Loss issue: ensure that FEXTNVM4 Beacon Duration is set correctly on power up."
    let b = hardware.read(registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER);
    hardware.write(
        registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER,
        (b & !FEXTNVM4_BEACON_DURATION_MASK) | FEXTNVM4_BEACON_DURATION_8_MICROSECONDS,
    );
    report_latency_tolerance(hardware, watch.platform, link.map(|l| l.0))?;
    copy_k1_off_enable(hardware);
    configure_k0s_minimum_time(hardware);
    let Some((speed, is_full_duplex)) = link else {
        return Ok(LinkReport::Down);
    };
    watch.is_check_due = false;
    let up = LinkReport::Up {
        speed,
        is_full_duplex,
    };
    if let Err(error) = disable_energy_efficient_ethernet(hardware) {
        // FreeBSD returns here, past the collision distance; `em` ignores the result and reports
        // the link up, since the flag is already clear.
        hardware.note(BringUpObservation::EnergyEfficientEthernetNotDisabled(
            error,
        ));
        return Ok(up);
    }
    // `e1000_config_collision_dist_generic`.
    let t = hardware.read(mac::TRANSMIT_CONTROL_REGISTER);
    hardware.write(
        mac::TRANSMIT_CONTROL_REGISTER,
        (t & !TRANSMIT_CONTROL_COLLISION_DISTANCE_MASK)
            | COLLISION_DISTANCE << COLLISION_DISTANCE_SHIFT,
    );
    flush_writes(hardware);
    Ok(up)
}

/// `e1000_phy_has_link_generic(hw, 1, 0, ..)`: the link bit latches low, so the status register is
/// read twice and the first read's result is dropped, as FreeBSD drops it.
fn has_link(hardware: &mut impl Hardware) -> Result<bool, Error> {
    let f = SoftwareFlag::acquire(hardware)?;
    let _ = physical_layer_device::read(hardware, &f, phy::PHYSICAL_LAYER_DEVICE_STATUS_REGISTER);
    let status =
        physical_layer_device::read(hardware, &f, phy::PHYSICAL_LAYER_DEVICE_STATUS_REGISTER);
    f.release(hardware);
    Ok(status? & PHYSICAL_LAYER_DEVICE_STATUS_LINK != 0)
}

/// FreeBSD's gap for 10 Mbit/s half duplex, where "some parts are excessively aggressive resulting
/// in many collisions", Sunrise Point's for full duplex below 1000 Mbit/s, and the default
/// otherwise; with the value for the PHY's receive configuration, 0 to "reduce Rx latency".
const fn transmit_gap_and_receive_configuration(
    speed: LinkSpeed,
    is_full_duplex: bool,
) -> (u32, u16) {
    match (speed, is_full_duplex) {
        (LinkSpeed::TenMegabitsPerSecond, false) => (0xff, 0),
        (LinkSpeed::OneGigabitPerSecond, _) | (_, false) => (0x08, 1),
        (_, true) => (0x0c, 1),
    }
}

/// The transmit gap, the PHY's receive configuration, its clock gate and, at 1000 Mbit/s, the K1
/// clock request: the first block under `link` in `e1000_check_for_copper_link_ich8lan`.
fn configure_for_speed(
    hardware: &mut impl Hardware,
    speed: LinkSpeed,
    is_full_duplex: bool,
) -> Result<(), Error> {
    let (gap, receive_configuration) =
        transmit_gap_and_receive_configuration(speed, is_full_duplex);
    let t = hardware.read(mac::TRANSMIT_INTER_PACKET_GAP_REGISTER);
    hardware.write(
        mac::TRANSMIT_INTER_PACKET_GAP_REGISTER,
        (t & !TRANSMIT_INTER_PACKET_GAP_TRANSMIT_MASK) | gap,
    );
    let f = SoftwareFlag::acquire(hardware)?;
    // FreeBSD checks the EMI write's result only after the clock gate and K1 writes and the release.
    let written = write_extended(
        hardware,
        &f,
        I217_RECEIVE_CONFIGURATION,
        receive_configuration,
    );
    let gate = if speed == LinkSpeed::OneGigabitPerSecond {
        0xfa
    } else {
        0x3e8
    };
    update_unchecked(
        hardware,
        &f,
        phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER,
        |v| (v & !PHASE_LOCKED_LOOP_CLOCK_GATE_MASK) | gate,
    );
    if speed == LinkSpeed::OneGigabitPerSecond {
        update_unchecked(hardware, &f, phy::POWER_MANAGEMENT_CONTROL_REGISTER, |v| {
            v | POWER_MANAGEMENT_CONTROL_K1_CLOCK_REQUEST
        });
    }
    f.release(hardware);
    Ok(written?)
}

/// A read-modify-write whose failures FreeBSD does not check: noted, and a failed read writes
/// nothing.
fn update_unchecked(
    hardware: &mut impl Hardware,
    held: &SoftwareFlag,
    offset: u32,
    change: impl FnOnce(u16) -> u16,
) {
    if let Err(error) = physical_layer_device::update(hardware, held, offset, change) {
        hardware.note(BringUpObservation::LinkPhysicalLayerDeviceAccessFailed { offset, error });
    }
}

/// Sunrise Point's `PHY_REG(776, 20)` block: at 1000 Mbit/s a pointer gap below `0x18` is raised to
/// it, and below 1000 Mbit/s the register is written `0xC023`.
fn configure_pointer_gap(hardware: &mut impl Hardware, speed: LinkSpeed) -> Result<(), Error> {
    let f = SoftwareFlag::acquire(hardware)?;
    let out = (|| {
        if speed != LinkSpeed::OneGigabitPerSecond {
            return physical_layer_device::write(
                hardware,
                &f,
                phy::POINTER_GAP_REGISTER,
                POINTER_GAP_BELOW_GIGABIT,
            );
        }
        let v = physical_layer_device::read(hardware, &f, phy::POINTER_GAP_REGISTER)?;
        let gap = (v & POINTER_GAP_MASK) >> 2;
        if gap >= POINTER_GAP_MINIMUM {
            return Ok(());
        }
        hardware.note(BringUpObservation::PointerGapRaised { from: gap });
        physical_layer_device::write(
            hardware,
            &f,
            phy::POINTER_GAP_REGISTER,
            (v & !POINTER_GAP_MASK) | POINTER_GAP_MINIMUM << 2,
        )
    })();
    f.release(hardware);
    Ok(out?)
}

/// `e1000_ltr2ns`: an encoded latency tolerance in nanoseconds.
const fn latency_tolerance_nanoseconds(encoded: u16) -> u64 {
    let value = (encoded & LATENCY_TOLERANCE_VALUE_MASK) as u64;
    let scale = ((encoded & LATENCY_TOLERANCE_SCALE_MASK) >> LATENCY_TOLERANCE_SCALE_SHIFT) as u32;
    value << (scale * LATENCY_TOLERANCE_SCALE_FACTOR)
}

/// The link-up arithmetic of `e1000_platform_pm_pch_lpt`: the time the receive buffer takes to fill
/// at `speed`, less room for two frames, encoded and held to the platform's limit; and the
/// optimized buffer flush/fill high water mark that leaves. Returns (encoded, KiB).
pub(crate) fn latency_tolerance(
    receive_buffer_kibibytes: u32,
    speed: LinkSpeed,
    platform: PlatformLatencyTolerance,
) -> Result<(u16, u32), Error> {
    let megabits_per_second = i64::from(speed.megabits_per_second());
    let buffered = (i64::from(receive_buffer_kibibytes) * 1024
        - 2 * i64::from(MAXIMUM_FRAME_SIZE_BYTES))
        * 8
        * 1000;
    let mut nanoseconds = if buffered < 0 {
        0
    } else {
        buffered / megabits_per_second
    };
    let mut value = nanoseconds;
    let mut scale = 0;
    while value > i64::from(LATENCY_TOLERANCE_VALUE_MASK) {
        scale += 1;
        value = (value + 31) / 32;
    }
    if scale > LATENCY_TOLERANCE_SCALE_MAXIMUM {
        return Err(Error::LatencyToleranceOutOfRange);
    }
    let mut encoded = scale << LATENCY_TOLERANCE_SCALE_SHIFT | value as u16;
    let maximum = if platform.maximum_snoop > platform.maximum_no_snoop {
        platform.maximum_snoop
    } else {
        platform.maximum_no_snoop
    };
    let maximum_nanoseconds = latency_tolerance_nanoseconds(maximum);
    if latency_tolerance_nanoseconds(encoded) > maximum_nanoseconds {
        encoded = maximum;
        nanoseconds = maximum_nanoseconds as i64;
    }
    // A zero latency leaves FreeBSD's `obff_hwm` at its initial zero rather than the whole buffer.
    let mark = if nanoseconds == 0 {
        0
    } else {
        i64::from(receive_buffer_kibibytes)
            - nanoseconds * megabits_per_second * 1000 / 8 / 1_000_000_000
    };
    if !(0..=i64::from(SIDEBAND_THRESHOLD_HIGH_WATER_MARK_MASK)).contains(&mark) {
        return Err(Error::LatencyToleranceOutOfRange);
    }
    Ok((encoded, mark as u32))
}

/// **`e1000_platform_pm_pch_lpt`**: tell the platform, through `LTRV`'s send bit, how much latency
/// the receive buffer tolerates (none required when there is no link), set the optimized buffer
/// flush/fill (OBFF) high water mark, and enable it with interrupts unmasked. FreeBSD: "Always
/// unblock interrupts to the CPU even when the system is in OBFF mode. This ensures that small
/// round-robin traffic (like ping) does not get dropped or experience long latency."
fn report_latency_tolerance(
    hardware: &mut impl Hardware,
    platform: PlatformLatencyTolerance,
    link: Option<LinkSpeed>,
) -> Result<(), Error> {
    let (requirement, (encoded, mark)) = match link {
        Some(speed) => {
            let receive = hardware.read(registers::PACKET_BUFFER_ALLOCATION_REGISTER)
                & PACKET_BUFFER_ALLOCATION_RECEIVE_MASK;
            (
                1 << (LATENCY_TOLERANCE_REQUIREMENT_SHIFT + LATENCY_TOLERANCE_NO_SNOOP_SHIFT)
                    | 1 << LATENCY_TOLERANCE_REQUIREMENT_SHIFT,
                latency_tolerance(receive, speed, platform)?,
            )
        }
        None => (0, (0, 0)),
    };
    hardware.write(
        registers::LATENCY_TOLERANCE_REPORTING_VALUE_REGISTER,
        requirement
            | LATENCY_TOLERANCE_SEND
            | u32::from(encoded)
            | u32::from(encoded) << LATENCY_TOLERANCE_NO_SNOOP_SHIFT,
    );
    let t = hardware.read(registers::SIDEBAND_THRESHOLD_REGISTER);
    hardware.write(
        registers::SIDEBAND_THRESHOLD_REGISTER,
        (t & !SIDEBAND_THRESHOLD_HIGH_WATER_MARK_MASK) | mark,
    );
    let c = hardware.read(registers::SIDEBAND_CONTROL_REGISTER);
    hardware.write(
        registers::SIDEBAND_CONTROL_REGISTER,
        c | SIDEBAND_CONTROL_BUFFER_FLUSH_ENABLE | SIDEBAND_CONTROL_BUFFER_FLUSH_UNMASK_INTERRUPTS,
    );
    hardware.note(BringUpObservation::LatencyToleranceReported {
        encoded,
        high_water_mark_kibibytes: mark,
    });
    Ok(())
}

/// Sunrise Point's `FEXTNVM6` K1-off workaround: `FEXTNVM6.K1_OFF_ENABLE` follows the same bit of
/// `PCIEANACFG`. FreeBSD's `disable_k1_off`, which would clear it, is never set.
fn copy_k1_off_enable(hardware: &mut impl Hardware) {
    let analog = hardware.read(registers::PCI_EXPRESS_ANALOG_CONFIGURATION_REGISTER);
    let v = hardware.read(registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER);
    hardware.write(
        registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER,
        (v & !FEXTNVM6_K1_OFF_ENABLE) | (analog & FEXTNVM6_K1_OFF_ENABLE),
    );
}

/// **`e1000_configure_k0s_lpt(hw, K1_ENTRY_LATENCY, K1_MIN_TIME)`**: the K0s minimum time, with
/// the entry latency left alone ("for now don't touch the latency"). FreeBSD calls it without the
/// semaphore its comment assumes and ignores its result; the Kumeran window is a MAC register,
/// and its unlocked access cannot fail, so neither is taken here either.
fn configure_k0s_minimum_time(hardware: &mut impl Hardware) {
    let v = read_kumeran(hardware, KUMERAN_K0S_CONTROL);
    write_kumeran(
        hardware,
        KUMERAN_K0S_CONTROL,
        (v & !KUMERAN_K0S_MINIMUM_TIME_MASK) | KUMERAN_K0S_MINIMUM_TIME,
    );
}

/// `__e1000_read_kmrn_reg` with the semaphore not taken: offset and read enable, a flush, 2 µs.
fn read_kumeran(hardware: &mut impl Hardware, offset: u32) -> u16 {
    hardware.write(
        registers::KUMERAN_CONTROL_AND_STATUS_REGISTER,
        (offset << KUMERAN_OFFSET_SHIFT & KUMERAN_OFFSET_MASK) | KUMERAN_READ_ENABLE,
    );
    flush_writes(hardware);
    hardware.delay_microseconds(2);
    hardware.read(registers::KUMERAN_CONTROL_AND_STATUS_REGISTER) as u16
}

/// `__e1000_write_kmrn_reg` with the semaphore not taken: offset and data, a flush, 2 µs.
fn write_kumeran(hardware: &mut impl Hardware, offset: u32, data: u16) {
    hardware.write(
        registers::KUMERAN_CONTROL_AND_STATUS_REGISTER,
        (offset << KUMERAN_OFFSET_SHIFT & KUMERAN_OFFSET_MASK) | u32::from(data),
    );
    flush_writes(hardware);
    hardware.delay_microseconds(2);
}

/// `e1000_write_emi_reg_locked`.
fn write_extended(
    hardware: &mut impl Hardware,
    held: &SoftwareFlag,
    extended_register: u16,
    data: u16,
) -> Result<(), physical_layer_device::Error> {
    physical_layer_device::write(
        hardware,
        held,
        phy::EXTENDED_MANAGEMENT_INTERFACE_SELECT_REGISTER,
        extended_register,
    )?;
    physical_layer_device::write(
        hardware,
        held,
        phy::EXTENDED_MANAGEMENT_INTERFACE_DATA_REGISTER,
        data,
    )
}

/// `e1000_read_emi_reg_locked`.
fn read_extended(
    hardware: &mut impl Hardware,
    held: &SoftwareFlag,
    extended_register: u16,
) -> Result<u16, physical_layer_device::Error> {
    physical_layer_device::write(
        hardware,
        held,
        phy::EXTENDED_MANAGEMENT_INTERFACE_SELECT_REGISTER,
        extended_register,
    )?;
    physical_layer_device::read(
        hardware,
        held,
        phy::EXTENDED_MANAGEMENT_INTERFACE_DATA_REGISTER,
    )
}

/// **`e1000_set_eee_pchlan` with `eee_disable` set**, `em`'s default: clear both speeds' EEE enables
/// in `I82579_LPI_CTRL`, after reading `I217_EEE_PCS_STATUS`, whose transmit and receive
/// low-power-idle-received bits clear on that read.
fn disable_energy_efficient_ethernet(hardware: &mut impl Hardware) -> Result<(), Error> {
    let f = SoftwareFlag::acquire(hardware)?;
    let out = (|| {
        let v = physical_layer_device::read(hardware, &f, phy::LOW_POWER_IDLE_CONTROL_REGISTER)?;
        read_extended(
            hardware,
            &f,
            I217_ENERGY_EFFICIENT_ETHERNET_PHYSICAL_CODING_SUBLAYER_STATUS,
        )?;
        physical_layer_device::write(
            hardware,
            &f,
            phy::LOW_POWER_IDLE_CONTROL_REGISTER,
            v & !LOW_POWER_IDLE_CONTROL_ENABLE_MASK,
        )
    })();
    f.release(hardware);
    Ok(out?)
}

#[cfg(test)]
mod tests;
