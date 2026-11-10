//! **The data plane under QEMU's `-device e1000e`**: a bare-metal consumer of the crate that owns
//! the controller's registers and the direct memory access (DMA) region, performs the bring-up
//! order in `docs/bring-up.md` step by step, then sends one Address Resolution Protocol (ARP)
//! request to QEMU's user-mode network gateway and receives its reply through
//! [`e1000e::DataPlane`]. It prints each step to the serial port and ends QEMU with a pass or
//! fail status. `run.sh` builds and runs it.

#![no_std]
#![no_main]

/// Print to the serial port, like `print!`.
macro_rules! say {
    ($($argument:tt)*) => {{
        use core::fmt::Write as _;
        let _ = write!($crate::machine::Serial, $($argument)*);
    }};
}

mod boot;
mod gateway_lookup;
mod machine;

use e1000e::{DataPlane, DirectMemoryAccessRegion, Tails, layout, registers::*};
use machine::{EXIT_FAIL, EXIT_PASS, PciFunction, Registers, exit_qemu};

/// The region's size, whole pages.
const REGION_BYTES: usize = layout::DIRECT_MEMORY_ACCESS_REGION_PAGES as usize * 4096;

#[repr(C, align(4096))]
struct RegionMemory([u8; REGION_BYTES]);

/// The DMA region. Identity-mapped, so its address is the physical address the device is given.
static mut REGION_MEMORY: RegionMemory = RegionMemory([0; REGION_BYTES]);

/// The DMA region as the data plane reads and writes it: volatile accesses, bounded to the region.
struct Region {
    base: *mut u8,
}

impl Region {
    fn at(&self, offset: u64, length: usize) -> *mut u8 {
        let end = (offset as usize).checked_add(length);
        assert!(
            end.is_some_and(|end| end <= REGION_BYTES),
            "an access outside the region"
        );
        // SAFETY: the assertion keeps `offset .. offset + length` inside the region.
        unsafe { self.base.add(offset as usize) }
    }
}

impl DirectMemoryAccessRegion for Region {
    fn read_u64(&self, offset: u64) -> u64 {
        // SAFETY: `at` bounds the access; every offset the data plane passes is 8-aligned.
        u64::from_le(unsafe { core::ptr::read_volatile(self.at(offset, 8).cast::<u64>()) })
    }
    fn write_u64(&mut self, offset: u64, value: u64) {
        // SAFETY: as in `read_u64`.
        unsafe { core::ptr::write_volatile(self.at(offset, 8).cast::<u64>(), value.to_le()) };
    }
    fn read_bytes(&self, offset: u64, out: &mut [u8]) {
        let source = self.at(offset, out.len());
        for (index, byte) in out.iter_mut().enumerate() {
            // SAFETY: `at` bounds the whole run.
            *byte = unsafe { core::ptr::read_volatile(source.add(index)) };
        }
    }
    fn write_bytes(&mut self, offset: u64, source: &[u8]) {
        let destination = self.at(offset, source.len());
        for (index, byte) in source.iter().enumerate() {
            // SAFETY: `at` bounds the whole run.
            unsafe { core::ptr::write_volatile(destination.add(index), *byte) };
        }
    }
}

/// The two tail registers, written through BAR0.
struct TailRegisters<'a>(&'a Registers);

impl Tails for TailRegisters<'_> {
    fn set_receive_tail(&mut self, value: u32) {
        self.0.write(RECEIVE_DESCRIPTOR_TAIL_REGISTER, value);
    }
    fn set_transmit_tail(&mut self, value: u32) {
        self.0.write(TRANSMIT_DESCRIPTOR_TAIL_REGISTER, value);
    }
    fn barrier(&mut self) {
        // x86_64 keeps stores in order and loads in order, as `Tails::barrier` states.
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

/// How many register reads a wait makes before it gives up.
const WAIT_READS: u32 = 10_000_000;

fn fail(reason: &str) -> ! {
    say!("FAIL: {reason}\n");
    exit_qemu(EXIT_FAIL)
}

/// Read `offset` until `done` holds of the value, or fail after [`WAIT_READS`] reads.
fn wait_for(registers: &Registers, offset: u64, done: impl Fn(u32) -> bool, what: &str) -> u32 {
    for _ in 0..WAIT_READS {
        let value = registers.read(offset);
        if done(value) {
            return value;
        }
        core::hint::spin_loop();
    }
    fail(what)
}

#[unsafe(no_mangle)]
extern "C" fn smoke_main() -> ! {
    say!("e1000e QEMU smoke test\n");
    let (function, device_identifier) = machine::find_pci_function(e1000e::is_supported_device)
        .unwrap_or_else(|| {
            fail("no supported controller on PCI bus 0");
        });
    let model = e1000e::model_name(device_identifier).unwrap_or("?");
    say!(
        "found 8086:{device_identifier:04x} {model} at 00:{:02x}.{}\n",
        function.device,
        function.function
    );
    let registers = Registers::new(memory_window_physical_address(function));
    let station = bring_up(function, &registers);
    let region_physical_address = (&raw mut REGION_MEMORY) as u64;
    let mut region = Region {
        base: (&raw mut REGION_MEMORY).cast::<u8>(),
    };
    program_rings(&registers, region_physical_address);
    let mut tails = TailRegisters(&registers);
    let mut data_plane = DataPlane::new(region_physical_address);
    data_plane.start(&mut region, &mut tails);
    say!(
        "step 12: DataPlane::start posted {} receive buffers\n",
        e1000e::RECEIVE_ENTRIES - 1
    );
    gateway_lookup::exchange(&mut data_plane, &mut region, &mut tails, station);
    let transmitted = registers.read(GOOD_PACKETS_TRANSMITTED_COUNT_REGISTER);
    let received = registers.read(GOOD_PACKETS_RECEIVED_COUNT_REGISTER);
    say!("counters: {transmitted} good frames transmitted, {received} good frames received\n");
    if transmitted == 0 || received == 0 {
        fail("the frame counters did not move");
    }
    say!("PASS\n");
    exit_qemu(EXIT_PASS)
}

/// BAR0's physical address, as the firmware assigned it.
fn memory_window_physical_address(function: PciFunction) -> u64 {
    let low = function.read_u32(0x10);
    if low & 1 != 0 {
        fail("BAR0 is an input-output port range, not memory");
    }
    let high = if (low >> 1) & 3 == 2 {
        function.read_u32(0x14)
    } else {
        0
    };
    let base = u64::from(high) << 32 | u64::from(low & !0xf);
    if base == 0 {
        fail("the firmware assigned BAR0 no address");
    }
    say!("BAR0 at {base:#x}\n");
    base
}

/// Steps 1 to 7 of the 82574L list in `docs/bring-up.md`. Returns the station address.
fn bring_up(function: PciFunction, registers: &Registers) -> [u8; 6] {
    let command = function.read_u32(0x04) as u16;
    function.write_u16(0x04, command | 1 << 1 | 1 << 2);
    say!("step 1: memory space and bus mastering enabled\n");
    registers.write(INTERRUPT_CLEAR_MASK_REGISTER, u32::MAX);
    registers.read(INTERRUPT_CAUSE_READ_REGISTER);
    say!("step 2: interrupts masked\n");
    let control = registers.read(DEVICE_CONTROL_REGISTER);
    registers.write(
        DEVICE_CONTROL_REGISTER,
        control | e1000e::DEVICE_CONTROL_GLOBAL_RESET,
    );
    let reset_clear = |value: u32| value & e1000e::DEVICE_CONTROL_GLOBAL_RESET == 0;
    wait_for(
        registers,
        DEVICE_CONTROL_REGISTER,
        reset_clear,
        "the reset bit never cleared",
    );
    registers.write(INTERRUPT_CLEAR_MASK_REGISTER, u32::MAX);
    registers.read(INTERRUPT_CAUSE_READ_REGISTER);
    say!("step 3: reset complete\n");
    let extended = registers.read(EXTENDED_DEVICE_CONTROL_REGISTER);
    registers.write(
        EXTENDED_DEVICE_CONTROL_REGISTER,
        extended | e1000e::EXTENDED_DEVICE_CONTROL_DRIVER_LOAD,
    );
    say!("step 4: driver load set\n");
    let control = registers.read(DEVICE_CONTROL_REGISTER);
    let link_bits =
        e1000e::DEVICE_CONTROL_SET_LINK_UP | e1000e::DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE;
    registers.write(DEVICE_CONTROL_REGISTER, control | link_bits);
    let link_up = |value: u32| value & e1000e::DEVICE_STATUS_LINK_UP != 0;
    wait_for(
        registers,
        DEVICE_STATUS_REGISTER,
        link_up,
        "the link never came up",
    );
    say!("step 5: link up\n");
    let station = e1000e::station_mac_address_from_receive_words(
        registers.read(RECEIVE_MAC_ADDRESS_0_LOW_REGISTER),
        registers.read(RECEIVE_MAC_ADDRESS_0_HIGH_REGISTER),
    )
    .unwrap_or_else(|| fail("receive-address entry 0 holds no valid station address"));
    say!("step 6: station address ");
    gateway_lookup::say_mac_address(station);
    say!("\n");
    for entry in 0..MULTICAST_TABLE_ARRAY_ENTRIES {
        registers.write(MULTICAST_TABLE_ARRAY_REGISTER + entry * 4, 0);
    }
    say!("step 7: multicast table cleared\n");
    station
}

/// Steps 8 to 11 of the 82574L list: both rings, legacy descriptors, the counters read once, and
/// receive and transmit enabled.
fn program_rings(registers: &Registers, region_physical_address: u64) {
    let receive_ring = region_physical_address + layout::RECEIVE_RING_OFFSET_BYTES;
    registers.write(
        RECEIVE_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        receive_ring as u32,
    );
    registers.write(
        RECEIVE_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (receive_ring >> 32) as u32,
    );
    let receive_bytes = u32::from(e1000e::RECEIVE_ENTRIES) * 16;
    registers.write(RECEIVE_DESCRIPTOR_LENGTH_BYTES_REGISTER, receive_bytes);
    registers.write(RECEIVE_DESCRIPTOR_HEAD_REGISTER, 0);
    registers.write(RECEIVE_DESCRIPTOR_TAIL_REGISTER, 0);
    say!("step 8: receive ring at {receive_ring:#x}, {receive_bytes} bytes\n");
    let filter = registers.read(RECEIVE_FILTER_CONTROL_REGISTER);
    registers.write(
        RECEIVE_FILTER_CONTROL_REGISTER,
        filter & !e1000e::RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE,
    );
    say!("step 9: legacy receive descriptors\n");
    let transmit_ring = region_physical_address + layout::TRANSMIT_RING_OFFSET_BYTES;
    registers.write(
        TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER,
        transmit_ring as u32,
    );
    registers.write(
        TRANSMIT_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_HIGH_REGISTER,
        (transmit_ring >> 32) as u32,
    );
    let transmit_bytes = u32::from(e1000e::TRANSMIT_ENTRIES) * 16;
    registers.write(TRANSMIT_DESCRIPTOR_LENGTH_BYTES_REGISTER, transmit_bytes);
    registers.write(TRANSMIT_DESCRIPTOR_HEAD_REGISTER, 0);
    registers.write(TRANSMIT_DESCRIPTOR_TAIL_REGISTER, 0);
    say!("step 10: transmit ring at {transmit_ring:#x}, {transmit_bytes} bytes\n");
    for counter in [
        GOOD_PACKETS_RECEIVED_COUNT_REGISTER,
        GOOD_PACKETS_TRANSMITTED_COUNT_REGISTER,
        CYCLIC_REDUNDANCY_CHECK_ERROR_COUNT_REGISTER,
        MISSED_PACKET_COUNT_REGISTER,
    ] {
        registers.read(counter);
    }
    registers.write(
        RECEIVE_CONTROL_REGISTER,
        e1000e::receive_control_register_value(),
    );
    registers.write(
        TRANSMIT_CONTROL_REGISTER,
        e1000e::transmit_control_register_value(),
    );
    registers.write(
        TRANSMIT_INTER_PACKET_GAP_REGISTER,
        e1000e::transmit_inter_packet_gap_register_value(),
    );
    say!("step 11: receive and transmit enabled\n");
}

#[panic_handler]
fn panic(information: &core::panic::PanicInfo) -> ! {
    say!("FAIL: {information}\n");
    exit_qemu(EXIT_FAIL)
}
