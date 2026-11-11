# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## 0.1.0-pre.1

- First version.
- A bare-metal smoke test in `tests/qemu-smoke/` runs the data plane under QEMU's `-device e1000e`. `docs/qemu.md` says how to run it and what a pass prints.
- Two cargo-fuzz targets in `fuzz/`: the transmit ring against a device that writes arbitrary status words, and the descriptor ring flush against arbitrary register values.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
