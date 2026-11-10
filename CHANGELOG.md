# Changelog

Dates are UTC. One line per release, what changed and why, so a reader of the diff has the
sentence beside it.

## Unreleased

- First version.
- Two cargo-fuzz targets in `fuzz/`: the transmit ring against a device that writes arbitrary status words, and the descriptor ring flush against arbitrary register values.

## BUGS

- The template's lag check guards the CI pin, not this file; a change that matters to a user of
  the crate belongs in a release line here.
