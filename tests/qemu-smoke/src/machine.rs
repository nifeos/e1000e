//! **The machine around the controller**: input-output ports, the first serial port, the
//! Peripheral Component Interconnect (PCI) configuration space, QEMU's exit device and the
//! controller's memory-mapped registers.

use core::fmt;

fn port_write_u8(port: u16, value: u8) {
    // SAFETY: an `out` touches only the named port; every port this program writes is one QEMU's
    // `pc` machine defines (the serial port, the PCI address and data ports, the exit device).
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nostack)) };
}

fn port_read_u8(port: u16) -> u8 {
    let value: u8;
    // SAFETY: as in `port_write_u8`; reading the serial line status has no side effect.
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nostack)) };
    value
}

fn port_write_u16(port: u16, value: u16) {
    // SAFETY: as in `port_write_u8`.
    unsafe { core::arch::asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack)) };
}

fn port_write_u32(port: u16, value: u32) {
    // SAFETY: as in `port_write_u8`.
    unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack)) };
}

fn port_read_u32(port: u16) -> u32 {
    let value: u32;
    // SAFETY: as in `port_write_u8`; a configuration space read has no side effect.
    unsafe { core::arch::asm!("in eax, dx", in("dx") port, out("eax") value, options(nostack)) };
    value
}

/// The first serial port, which `-serial stdio` connects to the terminal.
pub struct Serial;

const SERIAL_PORT: u16 = 0x3f8;
const SERIAL_LINE_STATUS: u16 = SERIAL_PORT + 5;
const SERIAL_TRANSMIT_HOLDING_EMPTY: u8 = 1 << 5;

impl fmt::Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' {
                serial_write_byte(b'\r');
            }
            serial_write_byte(byte);
        }
        Ok(())
    }
}

fn serial_write_byte(byte: u8) {
    while port_read_u8(SERIAL_LINE_STATUS) & SERIAL_TRANSMIT_HOLDING_EMPTY == 0 {
        core::hint::spin_loop();
    }
    port_write_u8(SERIAL_PORT, byte);
}

/// QEMU's `isa-debug-exit` device, at the port `run.sh` gives it. Writing `code` ends QEMU with
/// exit status `code * 2 + 1`.
pub fn exit_qemu(code: u8) -> ! {
    port_write_u8(0xf4, code);
    loop {
        // SAFETY: `hlt` with interrupts disabled stops this processor; the write above has
        // already ended the machine.
        unsafe { core::arch::asm!("hlt", options(nomem, nostack)) };
    }
}

/// The status `run.sh` reads as a pass: QEMU exits with 33.
pub const EXIT_PASS: u8 = 0x10;
/// The status `run.sh` reads as a failure: QEMU exits with 35.
pub const EXIT_FAIL: u8 = 0x11;

const CONFIGURATION_SELECT_PORT: u16 = 0xcf8;
const CONFIGURATION_DATA_PORT: u16 = 0xcfc;

/// One PCI function on bus 0, addressed by device and function number.
#[derive(Clone, Copy)]
pub struct PciFunction {
    /// The device number, 0 to 31.
    pub device: u8,
    /// The function number, 0 to 7.
    pub function: u8,
}

impl PciFunction {
    fn select(self, offset: u8) {
        let selector = 1 << 31
            | u32::from(self.device) << 11
            | u32::from(self.function) << 8
            | u32::from(offset & 0xfc);
        port_write_u32(CONFIGURATION_SELECT_PORT, selector);
    }

    /// The configuration space doubleword at `offset`, which is a multiple of four.
    pub fn read_u32(self, offset: u8) -> u32 {
        self.select(offset);
        port_read_u32(CONFIGURATION_DATA_PORT)
    }

    /// Write the 16-bit configuration register at `offset`, which is even. A 16-bit write leaves
    /// the status register beside the command register alone, whose bits clear when written as one.
    pub fn write_u16(self, offset: u8, value: u16) {
        self.select(offset);
        port_write_u16(CONFIGURATION_DATA_PORT + u16::from(offset & 2), value);
    }
}

/// Find the first function on bus 0 whose vendor and device identifiers satisfy `wanted`.
pub fn find_pci_function(wanted: impl Fn(u16, u16) -> bool) -> Option<(PciFunction, u16)> {
    for device in 0..32 {
        for function in 0..8 {
            let candidate = PciFunction { device, function };
            let identifiers = candidate.read_u32(0);
            let vendor = identifiers as u16;
            let device_identifier = (identifiers >> 16) as u16;
            if vendor != 0xffff && wanted(vendor, device_identifier) {
                return Some((candidate, device_identifier));
            }
        }
    }
    None
}

/// The controller's base address register 0 (BAR0), identity-mapped and uncached by the boot stub.
pub struct Registers {
    base: usize,
}

impl Registers {
    /// The registers at physical address `base`, which `boot.rs` maps.
    pub fn new(base: u64) -> Registers {
        assert!(
            (3 << 30..4 << 30).contains(&base),
            "BAR0 is outside the uncached gibibyte"
        );
        Registers {
            base: base as usize,
        }
    }

    /// One volatile 32-bit load of the register at `offset`.
    pub fn read(&self, offset: u64) -> u32 {
        assert!(offset < 128 * 1024 && offset.is_multiple_of(4));
        // SAFETY: BAR0 is 128 KiB of the controller's registers, identity-mapped by the boot
        // stub; the assertion keeps the access inside it and aligned.
        unsafe { core::ptr::read_volatile((self.base + offset as usize) as *const u32) }
    }

    /// One volatile 32-bit store to the register at `offset`.
    pub fn write(&self, offset: u64, value: u32) {
        assert!(offset < 128 * 1024 && offset.is_multiple_of(4));
        // SAFETY: as in `read`.
        unsafe { core::ptr::write_volatile((self.base + offset as usize) as *mut u32, value) };
    }
}
