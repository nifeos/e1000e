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

- The last run did not exercise the Kani install commands, because Kani 0.68.0 was already
  installed. The next runner should start without it.
- The ring paragraph tells a reader to implement two traits and drive a `DataPlane`, with no code
  to follow.

Last run: 2026-11-09
