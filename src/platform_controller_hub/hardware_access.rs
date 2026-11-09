//! **What the PCH sequences need from the hardware**: the register access, delays and flushes a
//! sequence is written against, so the simulator can stand in for the device.

use super::*;

/// **What the I219 sequences need from whoever holds BAR0.** A kernel implements it over its
/// mapping of the registers; the host tests implement it over a simulated device. The 82574L
/// path needs it too, for [`physical_layer_device::read_identifier_at_device_number`].
///
/// Every register access here has a side effect the compiler cannot see: a read can clear bits or
/// end a cycle, and a write starts one. So each method is one volatile access of exactly its width
/// (`core::ptr::read_volatile` or `write_volatile` on a mapping the processor treats as device
/// memory), never elided, merged, split or reordered against another access to BAR0.
pub trait Hardware {
    /// Read the 32-bit register at `offset` in BAR0, as one volatile 32-bit load.
    fn read(&mut self, offset: u64) -> u32;
    /// Write `value` to the 32-bit register at `offset` in BAR0, as one volatile 32-bit store.
    fn write(&mut self, offset: u64, value: u32);
    /// Read the 16-bit flash status word (`HSFSTS`, the low half of the flash register at
    /// [`non_volatile_memory::HARDWARE_SEQUENCING_FLASH_STATUS_REGISTER`]) at `offset` in BAR0, as
    /// one volatile 16-bit load. It is the only 16-bit access the crate makes. FreeBSD reads the
    /// word this way with `E1000_READ_FLASH_REG16` (`sys/dev/e1000/e1000_osdep.h`); OpenBSD's
    /// `E1000_READ_ICH_FLASH_REG16` (`sys/dev/pci/if_em_osdep.h`) is the same read, though
    /// OpenBSD reads the whole 32-bit register on Sunrise Point and keeps the low half.
    fn read_flash_status(&mut self, offset: u64) -> u16;
    /// Wait at least `microseconds` microseconds.
    fn delay_microseconds(&mut self, microseconds: u64);
    /// The function's PCI vendor id, read from configuration space. The read is the point: it is
    /// the delay `e1000_reset_hw_ich8lan` needs around the global reset.
    fn pci_vendor_id(&mut self) -> u16;
    /// Something the bench boot should show. Never needed for correctness.
    fn note(&mut self, note: sequence::BringUpObservation);
}

pub(super) fn delay_milliseconds(hardware: &mut impl Hardware, milliseconds: u64) {
    hardware.delay_microseconds(milliseconds * 1000);
}

/// A posted-write flush (`E1000_WRITE_FLUSH`): any read of BAR0 will do; FreeBSD reads `STATUS`.
pub(super) fn flush_writes(hardware: &mut impl Hardware) {
    let _ = hardware.read(crate::registers::DEVICE_STATUS_REGISTER);
}

/// Is `device` a PCH part (an I219) rather than the 82574L? Every I219 id this crate claims is in
/// FreeBSD's `e1000_pch_spt` class (`e1000_api.c`, `e1000_set_mac_type`), which is the class all
/// three ported sequences are written for.
pub fn is_platform_controller_hub_device(device: u16) -> bool {
    crate::is_supported_device(crate::VENDOR_INTEL, device) && device != 0x10d3
}
