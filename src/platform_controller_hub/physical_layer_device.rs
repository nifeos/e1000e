//! **Reaching the PHY: management data input output (MDIO) through `MDIC`, under the software/firmware semaphore.**
//!
//! [`read_management_data_input_output`] and [`write_management_data_input_output`] are the MDIO primitive the 82574L shares. Of the rest, only
//! [`read_identifier_at_device_number`] applies to an 82574L, which has no pages and no semaphore. [`read()`] and [`write()`]
//! are the paged access every PCH PHY register above page 0 needs, and each takes a [`SoftwareFlag`],
//! which only [`SoftwareFlag::acquire`] makes. That type stands where FreeBSD writes "Assumes
//! semaphore already acquired" comments. The FreeBSD sources are `e1000_phy.c` (the `MDIC` and
//! paged access) and `e1000_ich8lan.c` (the semaphore and the reset block).
//!
//! Two divergences are deliberate. There is no `MDIC` retry: FreeBSD retries only from Meteor Lake
//! on (`current_retry_counter` is 2 from `e1000_pch_mtp`, 0 below it), and every part this crate
//! claims is Sunrise Point, so one attempt is FreeBSD's behavior exactly. And two page ranges are
//! refused rather than ported, as [`Error::PageNotPorted`]: page 800, the host wakeup page with its
//! own address and data opcode protocol (`e1000_access_phy_wakeup_reg_bm`), and pages 1 to 767,
//! the 82577 and 82578 debug registers (`e1000_access_phy_debug_regs_hv`). Nothing in
//! [`super::sequence`] addresses either. Only an NVM-supplied `LCD` configuration word could, and
//! that returns [`Error::PageNotPorted`] rather than writing somewhere unexamined.

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

use super::Hardware;

/// Why a PHY access failed. FreeBSD collapses these into `-E1000_ERR_PHY`; they are kept apart
/// here because the bench boot prints them, and each points somewhere different.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// `MDIC.READY` never rose: nothing answered on the management bus.
    NotReady,
    /// `MDIC.ERROR`: the PHY, or the MAC on its behalf, refused the transaction.
    ManagementDataInterface,
    /// `MDIC` completed for a register other than the one asked for.
    WrongRegister,
    /// The register number is wider than MDIO's five bits.
    OutOfRange,
    /// Someone else's `EXTCNF_CTRL.SWFLAG` stayed set for [`SOFTWARE_FLAG_FREE_MILLISECONDS`].
    #[doc(alias = "swflag")]
    SoftwareFlagHeld,
    /// The flag would not stick for [`SOFTWARE_FLAG_TAKE_MILLISECONDS`]: firmware or hardware has the PHY.
    #[doc(alias = "swflag")]
    SoftwareFlagRefused,
    /// A page this port refuses (the module header).
    PageNotPorted,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Error::NotReady => "nothing answered on the management data input output bus",
            Error::ManagementDataInterface => {
                "the management data interface refused the transaction"
            }
            Error::WrongRegister => "the transaction completed for a different register",
            Error::OutOfRange => "the register number is wider than five bits",
            Error::SoftwareFlagHeld => "another agent holds the software flag",
            Error::SoftwareFlagRefused => "the software flag would not stick",
            Error::PageNotPorted => "the page is not ported",
        })
    }
}

impl core::error::Error for Error {}

/// `MDIC` register bits, from FreeBSD's `e1000_defines.h`.
#[doc(alias = "mdic")]
pub mod management_data_interface_control {
    /// The register field's shift.
    pub(crate) const REGISTER_SHIFT: u32 = 16;
    /// The register field.
    pub(crate) const REGISTER_MASK: u32 = 0x001f_0000;
    /// The shift of the PHY number field, the PHY's position on the management bus.
    pub(crate) const PHYSICAL_LAYER_DEVICE_SHIFT: u32 = 21;
    /// Read opcode.
    pub(crate) const OPERATION_READ: u32 = 0x0800_0000;
    /// Write opcode.
    pub(crate) const OPERATION_WRITE: u32 = 0x0400_0000;
    /// **Management data input/output transaction ready** (`E1000_MDIC_READY`, OpenBSD's
    /// `sys/dev/pci/if_em_hw.h` and FreeBSD's `e1000_defines.h`, both `0x10000000`). The transaction
    /// completed.
    #[doc(alias = "E1000_MDIC_READY")]
    pub const MANAGEMENT_DATA_INPUT_OUTPUT_TRANSACTION_READY: u32 = 0x1000_0000;
    /// **Management data input/output transaction error** (`E1000_MDIC_ERROR`, OpenBSD's
    /// `sys/dev/pci/if_em_hw.h` and FreeBSD's `e1000_defines.h`, both `0x40000000`). The transaction
    /// failed.
    #[doc(alias = "E1000_MDIC_ERROR")]
    pub const MANAGEMENT_DATA_INPUT_OUTPUT_TRANSACTION_ERROR: u32 = 0x4000_0000;
    /// Polls before giving up: `E1000_GEN_POLL_TIMEOUT * 3`.
    pub(crate) const POLLS: u32 = 640 * 3;
    /// Between polls, in microseconds.
    pub const POLL_MICROSECONDS: u64 = 50;
}

/// The highest MDIO register number (`MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER`).
pub(crate) const MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER: u32 = 0x1f;
/// Registers above this one sit behind a page select (`MAXIMUM_PHYSICAL_LAYER_DEVICE_MULTI_PAGE_REGISTER`).
pub(crate) const MAXIMUM_PHYSICAL_LAYER_DEVICE_MULTI_PAGE_REGISTER: u32 = 0xf;
/// The page-select register (`IGP01E1000_PHYSICAL_LAYER_DEVICE_PAGE_SELECT`).
pub(crate) const IGP01E1000_PHYSICAL_LAYER_DEVICE_PAGE_SELECT: u32 = 0x1f;
/// How a page and a register pack into one offset (`PHYSICAL_LAYER_DEVICE_PAGE_SHIFT`).
pub(crate) const PHYSICAL_LAYER_DEVICE_PAGE_SHIFT: u32 = 5;
/// The first page reached at PHY number 1 (the PHY's position on the management bus) rather than 2: OpenBSD's `HV_INTC_FC_PAGE_START`
/// (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `e1000_phy.h` spells the same. No source expands
/// `INTC_FC`, so the name says what the page is instead, and says whose address: the physical
/// layer device's, not a register's. It is selected as page 0.
#[doc(alias = "HV_INTC_FC_PAGE_START")]
pub(crate) const FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE: u32 = 768;
/// The host wakeup page (`BM_WUC_PAGE`), refused.
pub(crate) const BM_WUC_PAGE: u32 = 800;

/// The highest page whose select value (`page << 5`) fits the 16-bit `MDIC` data field. FreeBSD
/// passes `page << IGP_PAGE_SHIFT` to `e1000_set_page_igp` as a `u16` and so silently truncates a
/// higher one; this port refuses it instead. Found while writing this module's Kani harness.
pub(crate) const MAXIMUM_PAGE: u32 = 0x7ff;

// A value shifted left by five has no bits in the low five, where the register field is masked,
// so | and ^ compose the word identically; the compiler checks that here for the widest page
// there is, so the function's OR-to-XOR swap is an equivalent mutant.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    (u32::MAX << PHYSICAL_LAYER_DEVICE_PAGE_SHIFT) & MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER
        == 0
);

/// A page and a register packed into the one offset [`read()`] and [`write()`] take: FreeBSD's
/// `PHY_REG(page, reg)`.
#[doc(alias = "PHY_REG")]
pub const fn paged_offset(page: u32, register: u32) -> u32 {
    page << PHYSICAL_LAYER_DEVICE_PAGE_SHIFT
        | (register & MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER)
}

/// The PHY's registers this crate addresses, from FreeBSD's `e1000_defines.h`, `e1000_phy.h` and
/// `e1000_ich8lan.h`.
pub mod registers {
    use super::paged_offset;
    /// **Control Register** (OpenBSD's `PHY_CTRL`, `sys/dev/pci/if_em_hw.h`; FreeBSD's
    /// `PHY_CONTROL`, `e1000_defines.h`). Media independent interface (MII) register 0.
    #[doc(alias = "control register")]
    #[doc(alias = "PHY_CTRL")]
    #[doc(alias = "PHY_CONTROL")]
    pub const PHYSICAL_LAYER_DEVICE_CONTROL_REGISTER: u32 = 0x00;
    /// **PHY Identifier, Word 1** (`PHY_ID1`, FreeBSD's `e1000_defines.h`). The high word.
    #[doc(alias = "physical layer device identifier, word 1")]
    #[doc(alias = "PHY_ID1")]
    pub(crate) const PHYSICAL_LAYER_DEVICE_IDENTIFIER_WORD_1_REGISTER: u32 = 0x02;
    /// **PHY Identifier, Word 2** (`PHY_ID2`, FreeBSD's `e1000_defines.h`). The low word and the
    /// revision.
    #[doc(alias = "physical layer device identifier, word 2")]
    #[doc(alias = "PHY_ID2")]
    pub(crate) const PHYSICAL_LAYER_DEVICE_IDENTIFIER_WORD_2_REGISTER: u32 = 0x03;
    /// **Autonegotiation Advertisement** (`PHY_AUTONEG_ADV`, FreeBSD's `e1000_defines.h`).
    #[doc(alias = "autonegotiation advertisement")]
    #[doc(alias = "PHY_AUTONEG_ADV")]
    pub(crate) const PHYSICAL_LAYER_DEVICE_AUTONEGOTIATION_ADVERTISEMENT_REGISTER: u32 = 0x04;
    /// **1000BASE-T Control** (`PHY_1000T_CTRL`, FreeBSD's `e1000_defines.h`).
    #[doc(alias = "1000base-t control")]
    #[doc(alias = "PHY_1000T_CTRL")]
    pub(crate) const PHYSICAL_LAYER_DEVICE_1000BASE_T_CONTROL_REGISTER: u32 = 0x09;
    /// **82577 Configuration** (`I82577_CFG_REG`, FreeBSD's `e1000_phy.h`).
    #[doc(alias = "82577 configuration")]
    #[doc(alias = "I82577_CFG_REG")]
    pub(crate) const I82577_CONFIGURATION_REGISTER: u32 = 22;
    /// **82577 PHY Control 2** (`I82577_PHY_CTRL_2`, FreeBSD's `e1000_phy.h`).
    #[doc(alias = "82577 physical layer device control 2")]
    #[doc(alias = "I82577_PHY_CTRL_2")]
    pub(crate) const I82577_PHYSICAL_LAYER_DEVICE_CONTROL_2_REGISTER: u32 = 18;
    /// **Kumeran Mode Control** (OpenBSD's `HV_KMRN_MODE_CTRL`, with `I2_MODE_CTRL` as its I217
    /// name, `sys/dev/pci/if_em_hw.h`; FreeBSD's `HV_KMRN_MODE_CTRL`, `e1000_ich8lan.h`). Page 769,
    /// register 16. Kumeran is the interconnect between the MAC and this PHY. The register was
    /// first defined for the Hanksville (82577) PHY and still applies on the later I217 through
    /// I219 parts. It is not the `GG82563` PHY's Kumeran Mode Control on page 193
    /// (`GG82563_PHY_KMRN_MODE_CTRL`), which this crate does not address.
    #[doc(alias = "kumeran mode control")]
    #[doc(alias = "HV_KMRN_MODE_CTRL")]
    #[doc(alias = "I2_MODE_CTRL")]
    pub const KUMERAN_MODE_CONTROL_REGISTER: u32 = paged_offset(769, 16);
    /// **Port General Configuration** (`BM_PORT_GEN_CFG`, FreeBSD's `e1000_ich8lan.h`). Page 769,
    /// register 17.
    #[doc(alias = "port general configuration")]
    #[doc(alias = "BM_PORT_GEN_CFG")]
    pub(crate) const BM_PORT_GENERAL_CONFIGURATION_REGISTER: u32 = paged_offset(769, 17);
    /// **System Management Bus Control** (`CV_SMB_CTRL`, FreeBSD's `e1000_ich8lan.h`). Page 769,
    /// register 23.
    #[doc(alias = "system management bus control")]
    #[doc(alias = "CV_SMB_CTRL")]
    pub(crate) const CV_SYSTEM_MANAGEMENT_BUS_CONTROL_REGISTER: u32 = paged_offset(769, 23);
    /// **Power Management Control** (OpenBSD's `HV_PM_CTRL`, `sys/dev/pci/if_em_hw.h`; FreeBSD's
    /// `HV_PM_CTRL`, `e1000_ich8lan.h`). Page 770, register 17: the PHY's power management, with
    /// the K1 enable bit. OpenBSD also names the same offset `IGP3_KMRN_POWER_MNG_CTRL`.
    #[doc(alias = "power management control")]
    #[doc(alias = "HV_PM_CTRL")]
    pub(crate) const POWER_MANAGEMENT_CONTROL_REGISTER: u32 = paged_offset(770, 17);
    /// **Ultra-Low-Power Configuration 1** (`I218_ULP_CONFIG1`, FreeBSD's `e1000_ich8lan.h`). Page
    /// 779, register 16.
    #[doc(alias = "ultra-low-power configuration 1")]
    #[doc(alias = "I218_ULP_CONFIG1")]
    pub(crate) const I218_ULTRA_LOW_POWER_CONFIGURATION1: u32 = paged_offset(779, 16);
    /// **Original Equipment Manufacturer (OEM) Bits** (OpenBSD's `HV_OEM_BITS`, `sys/dev/pci/if_em_hw.h`; FreeBSD's `HV_OEM_BITS`,
    /// `e1000_ich8lan.h`). The bits the original equipment manufacturer's configuration sets: low
    /// power link up, gigabit disable and restart autonegotiation; page 768, register 25.
    #[doc(alias = "oem bits")]
    #[doc(alias = "HV_OEM_BITS")]
    pub(crate) const OEM_BITS_REGISTER: u32 = paged_offset(768, 25);
    /// **System Management Bus Address** (FreeBSD's `HV_SMB_ADDR`, `e1000_ich8lan.h`; OpenBSD does
    /// not name it). Page 768, register 26: the PHY's address on the system management bus, its
    /// valid and packet error checking bits, and the bus frequency.
    #[doc(alias = "system management bus address")]
    #[doc(alias = "HV_SMB_ADDR")]
    pub(crate) const SYSTEM_MANAGEMENT_BUS_ADDRESS_REGISTER: u32 = paged_offset(768, 26);
    /// **Light-Emitting Diode (LED) Configuration** (FreeBSD's `HV_LED_CONFIG`, `e1000_ich8lan.h`; OpenBSD does not name
    /// it). Page 768, register 30: the PHY's LED modes, copied from the MAC's `LEDCTL`.
    #[doc(alias = "led configuration")]
    #[doc(alias = "HV_LED_CONFIG")]
    pub(crate) const LED_CONFIGURATION_REGISTER: u32 = paged_offset(768, 30);
}

/// `PHYSICAL_LAYER_DEVICE_REVISION_MASK`: the identifier without its revision nibble.
pub(crate) const PHYSICAL_LAYER_DEVICE_REVISION_MASK: u32 = 0xffff_fff0;

/// How long to wait for someone else's `SWFLAG` to clear, in ms (`PHY_CFG_TIMEOUT`).
pub const SOFTWARE_FLAG_FREE_MILLISECONDS: u32 = 100;
/// How long to wait for ours to stick, in ms (`SW_FLAG_TIMEOUT`).
pub const SOFTWARE_FLAG_TAKE_MILLISECONDS: u32 = 1000;
/// `EXTCNF_CTRL.SWFLAG`.
pub(crate) const EXTCNF_CTRL_SWFLAG: u32 = 0x0000_0020;
/// `FWSM.RSPCIPHY`: clear while firmware blocks a PHY reset.
pub(crate) const ICH_FWSM_RSPCIPHY: u32 = 0x0000_0040;

/// One MDIO read of `register` at `physical_layer_device_number`, the PHY's position on the management bus,
/// `e1000_read_phy_reg_mdic`. It goes through the `MDIC` register (the management data
/// input/output control register).
#[doc(alias = "mdic")]
#[doc(alias = "e1000_read_phy_reg_mdic")]
pub fn read_management_data_input_output(
    hardware: &mut impl Hardware,
    physical_layer_device_number: u32,
    register: u32,
) -> Result<u16, Error> {
    if register > MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER {
        return Err(Error::OutOfRange);
    }
    hardware.write(
        super::registers::MANAGEMENT_DATA_INTERFACE_CONTROL_REGISTER,
        register << management_data_interface_control::REGISTER_SHIFT
            | physical_layer_device_number
                << management_data_interface_control::PHYSICAL_LAYER_DEVICE_SHIFT
            | management_data_interface_control::OPERATION_READ,
    );
    complete(hardware, register).map(|v| v as u16)
}

/// One MDIO write, `e1000_write_phy_reg_mdic`. It goes through the `MDIC` register (the
/// management data input/output control register).
#[doc(alias = "mdic")]
#[doc(alias = "e1000_write_phy_reg_mdic")]
pub fn write_management_data_input_output(
    hardware: &mut impl Hardware,
    physical_layer_device_number: u32,
    register: u32,
    data: u16,
) -> Result<(), Error> {
    if register > MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER {
        return Err(Error::OutOfRange);
    }
    hardware.write(
        super::registers::MANAGEMENT_DATA_INTERFACE_CONTROL_REGISTER,
        u32::from(data)
            | register << management_data_interface_control::REGISTER_SHIFT
            | physical_layer_device_number
                << management_data_interface_control::PHYSICAL_LAYER_DEVICE_SHIFT
            | management_data_interface_control::OPERATION_WRITE,
    );
    complete(hardware, register).map(|_| ())
}

/// Poll `MDIC` for the transaction's end and judge it, the half both FreeBSD functions share.
fn complete(hardware: &mut impl Hardware, register: u32) -> Result<u32, Error> {
    let mut v = 0;
    for _ in 0..management_data_interface_control::POLLS {
        hardware.delay_microseconds(management_data_interface_control::POLL_MICROSECONDS);
        v = hardware.read(super::registers::MANAGEMENT_DATA_INTERFACE_CONTROL_REGISTER);
        if v & management_data_interface_control::MANAGEMENT_DATA_INPUT_OUTPUT_TRANSACTION_READY
            != 0
        {
            break;
        }
    }
    if v & management_data_interface_control::MANAGEMENT_DATA_INPUT_OUTPUT_TRANSACTION_READY == 0 {
        Err(Error::NotReady)
    } else if v & management_data_interface_control::MANAGEMENT_DATA_INPUT_OUTPUT_TRANSACTION_ERROR
        != 0
    {
        Err(Error::ManagementDataInterface)
    } else if (v & management_data_interface_control::REGISTER_MASK)
        >> management_data_interface_control::REGISTER_SHIFT
        != register
    {
        Err(Error::WrongRegister)
    } else {
        Ok(v)
    }
}

/// **Proof of holding the software/firmware semaphore** over the PHY and some MAC registers.
/// Deliberately not `Clone`: [`SoftwareFlag::release`] consumes it.
#[doc(alias = "swflag")]
#[derive(Debug)]
#[must_use = "a held SWFLAG locks the Management Engine out of the PHY until it is released"]
pub struct SoftwareFlag(());

impl SoftwareFlag {
    /// `e1000_acquire_swflag_ich8lan`: wait for the flag to be free, set it, and wait for it to
    /// read back set. On a refusal it clears the bit it wrote, as FreeBSD does.
    pub fn acquire(hardware: &mut impl Hardware) -> Result<Self, Error> {
        let mut extended_control = 0;
        let mut free = false;
        for _ in 0..SOFTWARE_FLAG_FREE_MILLISECONDS {
            extended_control =
                hardware.read(super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER);
            if extended_control & EXTCNF_CTRL_SWFLAG == 0 {
                free = true;
                break;
            }
            hardware.delay_microseconds(1000);
        }
        if !free {
            return Err(Error::SoftwareFlagHeld);
        }
        extended_control |= EXTCNF_CTRL_SWFLAG;
        hardware.write(
            super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER,
            extended_control,
        );
        for _ in 0..SOFTWARE_FLAG_TAKE_MILLISECONDS {
            extended_control =
                hardware.read(super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER);
            if extended_control & EXTCNF_CTRL_SWFLAG != 0 {
                return Ok(SoftwareFlag(()));
            }
            hardware.delay_microseconds(1000);
        }
        hardware.write(
            super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER,
            extended_control & !EXTCNF_CTRL_SWFLAG,
        );
        Err(Error::SoftwareFlagRefused)
    }

    /// `e1000_release_swflag_ich8lan`. Clears the bit only if it is still set; FreeBSD logs the
    /// other case ("Semaphore unexpectedly released") and so does nothing here.
    pub fn release(self, hardware: &mut impl Hardware) {
        let extended_control =
            hardware.read(super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER);
        if extended_control & EXTCNF_CTRL_SWFLAG != 0 {
            hardware.write(
                super::registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER,
                extended_control & !EXTCNF_CTRL_SWFLAG,
            );
        }
    }

    /// **The flag is given up to a global reset**, which clears `EXTCNF_CTRL`. FreeBSD's
    /// `e1000_reset_hw_ich8lan` takes it before `CTRL.RST` and never releases it; this names that.
    pub(crate) fn lost_to_reset(self) {}
}

/// Which PHY number, the PHY's position on the management bus, a page lives at
/// (`e1000_get_phy_addr_for_hv_page`).
pub(crate) const fn physical_layer_device_number_for(page: u32) -> u32 {
    if page >= FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE {
        1
    } else {
        2
    }
}

/// What one paged access does on the bus: the page-select write, if any, then the register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PagedAccessPlan {
    /// `Some(value)` when the page-select register is written first (at PHY address 1).
    pub select: Option<u16>,
    /// The PHY number of the access itself: its position on the management bus.
    pub physical_layer_device_number: u32,
    /// The five-bit register.
    pub register: u32,
}

/// The decomposition `__e1000_read_phy_reg_hv` and `__e1000_write_phy_reg_hv` share, for the pages
/// this port carries. Pure, so it is host-tested and model-checked.
pub const fn plan_paged_access(offset: u32) -> Result<PagedAccessPlan, Error> {
    let page = offset >> PHYSICAL_LAYER_DEVICE_PAGE_SHIFT;
    let register = offset & MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER;
    if page == BM_WUC_PAGE
        || (page > 0 && page < FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE)
        || page > MAXIMUM_PAGE
    {
        return Err(Error::PageNotPorted);
    }
    let physical_layer_device_number = physical_layer_device_number_for(page);
    // Page 768 is selected as page 0 (FreeBSD: "if (page == HV_INTC_FC_PAGE_START) page = 0").
    let shown = if page == FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE {
        0
    } else {
        page
    };
    let select = if register > MAXIMUM_PHYSICAL_LAYER_DEVICE_MULTI_PAGE_REGISTER {
        Some((shown << PHYSICAL_LAYER_DEVICE_PAGE_SHIFT) as u16)
    } else {
        None
    };
    Ok(PagedAccessPlan {
        select,
        physical_layer_device_number,
        register,
    })
}

/// Read a PCH PHY register by its paged offset ([`paged_offset`]).
pub fn read(hardware: &mut impl Hardware, _held: &SoftwareFlag, offset: u32) -> Result<u16, Error> {
    let p = plan_paged_access(offset)?;
    if let Some(sel) = p.select {
        write_management_data_input_output(
            hardware,
            1,
            IGP01E1000_PHYSICAL_LAYER_DEVICE_PAGE_SELECT,
            sel,
        )?;
    }
    read_management_data_input_output(hardware, p.physical_layer_device_number, p.register)
}

/// Write a PCH PHY register by its paged offset. FreeBSD's 82578 power-down workaround in the
/// write path is not carried: no part this crate claims has that PHY.
pub fn write(
    hardware: &mut impl Hardware,
    _held: &SoftwareFlag,
    offset: u32,
    data: u16,
) -> Result<(), Error> {
    let p = plan_paged_access(offset)?;
    if let Some(sel) = p.select {
        write_management_data_input_output(
            hardware,
            1,
            IGP01E1000_PHYSICAL_LAYER_DEVICE_PAGE_SELECT,
            sel,
        )?;
    }
    write_management_data_input_output(hardware, p.physical_layer_device_number, p.register, data)
}

/// Read-modify-write under a held flag.
pub fn update(
    hardware: &mut impl Hardware,
    held: &SoftwareFlag,
    offset: u32,
    change: impl FnOnce(u16) -> u16,
) -> Result<(), Error> {
    let v = read(hardware, held, offset)?;
    write(hardware, held, offset, change(v))
}

/// `e1000_check_reset_block_ich8lan`: is firmware blocking a PHY reset? Polls `FWSM.RSPCIPHY` up to
/// 31 times 10 ms apart, as FreeBSD does.
pub(crate) fn reset_blocked(hardware: &mut impl Hardware) -> bool {
    for i in 0..=30 {
        if hardware.read(super::registers::FIRMWARE_SEMAPHORE_REGISTER) & ICH_FWSM_RSPCIPHY != 0 {
            return false;
        }
        if i < 30 {
            hardware.delay_microseconds(10_000);
        }
    }
    true
}

// The high word sits at bit 16 and the low one is a u16, so | and ^ compose the identifier
// identically; the compiler checks that here, so the function's
// OR-to-XOR swap is an equivalent mutant.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(((u16::MAX as u32) << 16) & (u16::MAX as u32) == 0);

/// Read the PHY identifier at the 82574L's address, the generic MDIO path with no pages and no
/// semaphore (the 82574L has neither). What QEMU's gates prove of this module.
pub fn read_identifier_at_device_number(
    hardware: &mut impl Hardware,
    physical_layer_device_number: u32,
) -> Result<u32, Error> {
    let high = read_management_data_input_output(
        hardware,
        physical_layer_device_number,
        registers::PHYSICAL_LAYER_DEVICE_IDENTIFIER_WORD_1_REGISTER,
    )?;
    let low = read_management_data_input_output(
        hardware,
        physical_layer_device_number,
        registers::PHYSICAL_LAYER_DEVICE_IDENTIFIER_WORD_2_REGISTER,
    )?;
    Ok((u32::from(high) << 16 | u32::from(low)) & PHYSICAL_LAYER_DEVICE_REVISION_MASK)
}

#[cfg(kani)]
mod proofs {
    use super::*;

    /// Falsification: replayable `falsifications/platform_controller_hub.physical_layer_device.proofs.a_paged_access_selects_the_offsets_own_page_and_register.patch`
    /// **Every paged access this port will make names a PHY number, a position on the management bus, that exists (1 or 2), a
    /// five-bit register that is the offset's own, and a page select that is the offset's page**
    /// (768 shown as 0), written only when the register is above the unpaged range. Pages 1 to
    /// 767, the host wakeup page and every page above [`MAXIMUM_PAGE`] are refused.
    ///
    #[kani::proof]
    fn a_paged_access_selects_the_offsets_own_page_and_register() {
        let off: u32 = kani::any();
        let refused = off >> PHYSICAL_LAYER_DEVICE_PAGE_SHIFT;
        if (refused > 0 && refused < FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE)
            || refused == BM_WUC_PAGE
            || refused > MAXIMUM_PAGE
        {
            assert!(plan_paged_access(off).is_err());
        }
        if let Ok(p) = plan_paged_access(off) {
            let page = off >> PHYSICAL_LAYER_DEVICE_PAGE_SHIFT;
            assert!(p.physical_layer_device_number == 1 || p.physical_layer_device_number == 2);
            assert!(p.register == off & MAXIMUM_PHYSICAL_LAYER_DEVICE_REGISTER_NUMBER);
            assert!(page != BM_WUC_PAGE);
            match p.select {
                Some(sel) => {
                    assert!(p.register > MAXIMUM_PHYSICAL_LAYER_DEVICE_MULTI_PAGE_REGISTER);
                    let shown = if page == FIRST_PAGE_AT_PHYSICAL_LAYER_DEVICE_NUMBER_ONE {
                        0
                    } else {
                        page
                    };
                    assert!(u32::from(sel) >> PHYSICAL_LAYER_DEVICE_PAGE_SHIFT == shown);
                }
                None => assert!(p.register <= MAXIMUM_PHYSICAL_LAYER_DEVICE_MULTI_PAGE_REGISTER),
            }
        }
    }
}
