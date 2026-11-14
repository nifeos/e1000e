# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## 0.1.0-pre.2

- A frame may carry a VLAN tag: `MAXIMUM_FRAME_BYTES` is 1518. The receive statistics count the
  frames dropped for the caller's room (`dropped_for_lack_of_room`).
- The interrupt vocabulary is named: the `INTERRUPT_CAUSE_*` bits, `INTERRUPT_THROTTLING_REGISTER`
  and `INTERRUPT_SET_MASK_REGISTER`, each with its specification mnemonic as a search alias.
- The receive cursor has a proof: one read, from the consumed slot, within the caller's room.

## 0.1.0-pre.1

- First version.
- A bare-metal smoke test in `tests/qemu-smoke/` runs the data plane under QEMU's `-device e1000e`. `docs/qemu.md` says how to run it and what a pass prints.
- Two cargo-fuzz targets in `fuzz/`: the transmit ring against a device that writes arbitrary status words, and the descriptor ring flush against arbitrary register values.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
