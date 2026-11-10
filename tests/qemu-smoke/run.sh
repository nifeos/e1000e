#!/bin/sh
# Build the smoke test and run it under QEMU's emulated 82574L on the user-mode network, with a
# packet capture of the controller's traffic. Exits 0 on a pass. The run is bounded: a guest that
# hangs is killed after the number of seconds in the first argument (default 60).
set -eu

directory=$(cd "$(dirname "$0")" && pwd)
seconds=${1:-60}
cd "$directory"
cargo build --release
image=target/x86_64-unknown-none/release/e1000e-qemu-smoke
capture=target/e1000e-qemu-smoke.pcap
rm -f "$capture"

qemu-system-x86_64 \
    -machine pc -m 128M -display none -no-reboot \
    -serial stdio -monitor none \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -netdev user,id=network0 \
    -object filter-dump,id=capture0,netdev=network0,file="$capture" \
    -device e1000e,netdev=network0 \
    -kernel "$image" &
qemu=$!
( sleep "$seconds" && kill "$qemu" 2>/dev/null && echo "TIMEOUT after ${seconds}s" >&2 ) &
watchdog=$!
status=0
wait "$qemu" || status=$?
kill "$watchdog" 2>/dev/null || true
wait "$watchdog" 2>/dev/null || true

# isa-debug-exit ends QEMU with status (code * 2 + 1): 33 is the program's pass, 35 its failure.
if [ "$status" -eq 33 ]; then
    echo "capture: $directory/$capture"
    exit 0
fi
echo "QEMU exited with status $status" >&2
exit 1
