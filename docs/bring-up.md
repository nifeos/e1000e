# Bring-up order

This crate makes no register access of its own. A driver performs the bring-up below with its own
volatile accesses to base address register 0 (BAR0), and calls the crate where the list names it.
[`DataPlane::start`](../src/data_plane.rs) assumes everything above it in either list has happened:
both rings programmed, both heads and both tails at zero, receive and transmit enabled.

## 82574L (QEMU's `-device e1000e`)

The order follows FreeBSD's `em_reset`, `em_initialize_receive_unit` and
`em_initialize_transmit_unit`, reduced to what a polled driver with legacy descriptors needs.

1. Enable bus mastering in the function's Peripheral Component Interconnect (PCI) command register. QEMU receives nothing without it.
2. Mask every interrupt: write all ones to `INTERRUPT_CLEAR_MASK_REGISTER` (`IMC`), then read
   `INTERRUPT_CAUSE_READ_REGISTER` (`ICR`) to clear what is pending.
3. Reset: set `DEVICE_CONTROL_GLOBAL_RESET` in `DEVICE_CONTROL_REGISTER` (`CTRL`), wait for the
   bit to read as clear, then mask the interrupts and read `ICR` again. FreeBSD's
   `e1000_reset_hw_82571` does the same, with a 10-millisecond wait before the reset.
4. Tell management firmware a driver has the device: set `EXTENDED_DEVICE_CONTROL_DRIVER_LOAD` in
   `EXTENDED_DEVICE_CONTROL_REGISTER`.
5. Link: set `DEVICE_CONTROL_SET_LINK_UP` and `DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE` in
   `CTRL`. `DEVICE_STATUS_LINK_UP` in `DEVICE_STATUS_REGISTER` says when the link is up.
6. The station address: read `RECEIVE_MAC_ADDRESS_0_LOW_REGISTER` and
   `RECEIVE_MAC_ADDRESS_0_HIGH_REGISTER` and pass both to `station_mac_address_from_receive_words`.
7. Clear the multicast table: write zero to each of the `MULTICAST_TABLE_ARRAY_ENTRIES` 32-bit
   entries from `MULTICAST_TABLE_ARRAY_REGISTER` on.
8. The receive ring: write the region's physical base plus `layout::RECEIVE_RING_OFFSET_BYTES` to
   `RECEIVE_DESCRIPTOR_BASE_PHYSICAL_ADDRESS_LOW_REGISTER` and `..._HIGH_REGISTER`,
   `RECEIVE_ENTRIES * 16` to `RECEIVE_DESCRIPTOR_LENGTH_BYTES_REGISTER`, and zero to
   `RECEIVE_DESCRIPTOR_HEAD_REGISTER` and `RECEIVE_DESCRIPTOR_TAIL_REGISTER`.
9. Legacy receive descriptors: clear `RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE` in
   `RECEIVE_FILTER_CONTROL_REGISTER`. The crate parses only the legacy format.
10. The transmit ring: the same four writes with the `TRANSMIT_` registers and
    `layout::TRANSMIT_RING_OFFSET_BYTES`.
11. Enable: write `receive_control_register_value()` to `RECEIVE_CONTROL_REGISTER`,
    `transmit_control_register_value()` to `TRANSMIT_CONTROL_REGISTER` and
    `transmit_inter_packet_gap_register_value()` to `TRANSMIT_INTER_PACKET_GAP_REGISTER`.
12. `DataPlane::start`, which posts every receive buffer and moves the receive tail.

[qemu.md](qemu.md) runs this list under QEMU, one line per step, from `tests/qemu-smoke/`.

A split driver does steps 1 to 11 in its kernel half and maps only `RECEIVE_QUEUE_PAGE` and
`TRANSMIT_QUEUE_PAGE` into the stack half, which calls step 12 and everything after it.

## I219 (Sunrise Point)

The I219 needs FreeBSD's three passes from `platform_controller_hub::sequence` around the same
steps. Its module documentation names the FreeBSD function each one follows and every place the
port departs from it.

1. At attach: `sequence::initialize_physical_layer_device_workarounds`.
2. Before any reset: `flush::empty_descriptor_rings`, which needs the transmit ring already at its
   base and of its length if the device asks for a flush.
3. `sequence::global_reset`, which also writes the packet buffer allocation.
4. Steps 2, 4, 6 and 7 of the 82574L list.
5. `sequence::initialize_hardware`, which also sets up the copper link.
6. Steps 8 to 11 of the 82574L list, then `sequence::transmit_errata` once transmit is on.
7. `DataPlane::start`.

## Counting frames

Four counters in `registers` say what crossed the wire, independent of the rings:
`GOOD_PACKETS_RECEIVED_COUNT_REGISTER`, `GOOD_PACKETS_TRANSMITTED_COUNT_REGISTER`,
`CYCLIC_REDUNDANCY_CHECK_ERROR_COUNT_REGISTER` and `MISSED_PACKET_COUNT_REGISTER`. Each is 32 bits
and clears when it is read, so a driver adds every read to a running total of its own.
`sequence::initialize_hardware` does not clear them, so read each once and discard it before step
11 of the 82574L list. They sit in BAR0's page at `0x4000`, outside `RECEIVE_QUEUE_PAGE` and
`TRANSMIT_QUEUE_PAGE`, so in a split driver it is the kernel half that reads them.
