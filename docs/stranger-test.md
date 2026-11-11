# The stranger cadence

The release's whole claim is that a stranger, given this repository and nothing else, can build
the crate and run its proofs from the README alone. A real agent runs that check on a
cadence, and the weekly `stranger cadence` job fails when the
last run is past 28 days.

What a run is:

1. Clone the repository fresh.
2. Follow the README and nothing else: build, test, and run the proofs.
3. Note every place the README lied, omitted, or assumed. Those are findings against the README,
   not against the stranger.
4. Record the date on the `Last run:` line at the end, and keep the findings still open above it.

## Open findings

- The README did not say that the full `cargo kani` run takes about nine minutes. It did not say
  that setup downloads a pinned nightly toolchain as well as the model checker. It did not say that
  Kani prints an unstable-feature warning and an unsupported-constructs note on a run that
  succeeds. The README's proofs section now says all three.
- The ring paragraph is prose with no code beside it; the example and `docs/bring-up.md` carry the
  detail, and the cold runner found both from the README. The runner did not open `docs/bring-up.md`
  or `docs/qemu.md`, so a cold reader has not yet checked those two.
- The runner did not replay a falsification patch or the QEMU smoke test.

## Cold run on 2026-11-11

A fresh agent with no source context took only the repository URL and the steps above, and
started from a clean clone of `ef01cef` (`0.1.0-pre.1`). It ran the Kani install into an empty tool
home: `cargo install --locked kani-verifier --version 0.68.0` and `cargo kani setup` both passed.
`cargo build` (0.5 s), `cargo test` (76 unit tests and 2 doctests), `cargo run --example host_ring`
and `cargo kani` (all six harnesses, about 551 s) all passed from the README alone. This run closes
the previous run's findings about the Kani install path and the missing ring example.

Last run: 2026-11-11
