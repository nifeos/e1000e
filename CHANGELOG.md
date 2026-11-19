# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## 0.1.0-pre.3

- `Tails` lets an implementation hold a receive tail write back and store a later value in its
  place. Under QEMU with the Kernel Virtual Machine (KVM), one `RDT` store per burst of 15
  frames drains the burst 15 to 19 times faster than one store per frame; `docs/qemu.md` has the measurement and the smoke test repeats it.
- A third cargo-fuzz target, `e1000e_receive_ring`, drives `DataPlane::receive` against a device
  that fills descriptors in order and one that writes any status word anywhere.
- The frame-error, end-of-packet and refused-page assertions have their own Kani harnesses and
  falsification patches.

## 0.1.0-pre.2

- Breaking rename: the management data input output names spell it `io`. The two transaction bits
  are `MANAGEMENT_DATA_IO_TRANSACTION_READY` and `MANAGEMENT_DATA_IO_TRANSACTION_ERROR`, and the
  two accessors are `read_management_data_io` and `write_management_data_io`. The old spellings
  have no alias.
- A frame may carry a virtual local area network (VLAN) tag, so `MAXIMUM_FRAME_BYTES` is 1518. The
  receive statistics count the frames dropped for the caller's room (`dropped_for_lack_of_room`).
- The crate names the interrupt vocabulary: the `INTERRUPT_CAUSE_*` bits, `INTERRUPT_THROTTLING_REGISTER`
  and `INTERRUPT_SET_MASK_REGISTER`, each with its specification mnemonic as a search alias.
- The receive cursor has a proof: one read, from the consumed slot, within the caller's room.

## 0.1.0-pre.1

- First version.
- A bare-metal smoke test in `tests/qemu-smoke/` runs the data plane under QEMU's `-device e1000e`. `docs/qemu.md` says how to run it and what a pass prints.
- Two cargo-fuzz targets in `fuzz/`: the transmit ring against a device that writes arbitrary status words, and the descriptor ring flush against arbitrary register values.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
