# The data plane under QEMU

`tests/qemu-smoke/` is a small bare-metal `x86_64-unknown-none` program that drives QEMU's emulated
82574L (`-device e1000e`) with this crate. It owns the controller's memory-mapped registers and the
direct memory access (DMA) region, performs the 82574L list in [bring-up.md](bring-up.md) step by
step, and calls `DataPlane::start`. It then sends one Address Resolution Protocol (ARP) request to
the gateway of QEMU's user-mode network through `DataPlane::transmit`, and takes the gateway's
reply through `DataPlane::receive`, and last measures what the receive tail write costs (see
[The receive tail write](#the-receive-tail-write)). It is not part of CI: a person runs it by hand.

## What it needs

- The target: `rustup target add x86_64-unknown-none`.
- `qemu-system-x86_64` with the user-mode network. QEMU reads the PVH (para-virtualized hardware)
  entry note from version 4.0 on; the output below is from QEMU 10.2.
- Optionally `tcpdump`, to read the capture.

## Running it

```sh
tests/qemu-smoke/run.sh
```

The script builds the program in release mode against the committed `Cargo.lock` (`--locked`, so the
build never rewrites it) and runs QEMU with a watchdog that kills the run after 60 seconds (the
first argument changes that). It exits 0 on a pass and 1 otherwise. The
QEMU command line it runs is:

```sh
qemu-system-x86_64 \
    -machine pc -m 128M -display none -no-reboot \
    -serial stdio -monitor none \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -netdev user,id=network0 \
    -object filter-dump,id=capture0,netdev=network0,file=target/e1000e-qemu-smoke.pcap \
    -device e1000e,netdev=network0 \
    -kernel target/x86_64-unknown-none/release/e1000e-qemu-smoke
```

QEMU loads the program with `-kernel` through the PVH entry note that `src/boot.rs` carries, so no
bootloader runs. The firmware assigns base address register 0 (BAR0) before the program starts, and
the program finds the controller on bus 0 with `is_supported_device`. The `isa-debug-exit` device
lets the program end QEMU: exit status 33 is a pass and 35 a failure. `-machine q35` passes the same
way.

## What a pass prints

```text
e1000e QEMU smoke test
found 8086:10d3 82574L (QEMU's e1000e) at 00:03.0
BAR0 at 0xfeb80000
step 1: memory space and bus mastering enabled
step 2: interrupts masked
step 3: reset complete
step 4: driver load set
step 5: link up
step 6: station address 52:54:00:12:34:56
step 7: multicast table cleared
step 8: receive ring at 0x11a000, 256 bytes
step 9: legacy receive descriptors
step 10: transmit ring at 0x11b000, 256 bytes
step 11: receive and transmit enabled
step 12: DataPlane::start posted 15 receive buffers
sent: ARP request who-has 10.0.2.2 tell 10.0.2.15, 42 bytes
transmit descriptor 0: descriptor done
received: 64 bytes, ether type 0806
received: ARP reply 10.0.2.2 is-at 52:55:0a:00:02:02
counters: 1 good frames transmitted, 1 good frames received
measurement: 32 bursts of 15 frames each way, median ticks per burst
measurement: one tail write per frame: 125550 ticks, 8370 per frame, 480 tail writes
measurement: one tail write per burst: 17730 ticks, 1182 per frame, 32 tail writes
measurement: one receive tail store: 6881 ticks
PASS
capture: <repository>/tests/qemu-smoke/target/e1000e-qemu-smoke.pcap
```

The step numbers are the steps of the 82574L list. The measurement's tick counts vary from run to
run and from host to host. The addresses depend on the QEMU version and
the machine type; the station address is QEMU's default. The last line before `PASS` reads the
good-frame counters, so the device itself confirms that a frame went out and one came in. A
failure prints `FAIL:` and the reason, such as `FAIL: the link never came up`, and the program
bounds every wait, so a stuck device fails rather than hangs.

## Reading the capture

`filter-dump` records every frame between the controller and the user-mode network:

```sh
tcpdump -nr tests/qemu-smoke/target/e1000e-qemu-smoke.pcap -e
```

A pass shows two frames, the broadcast request from `52:54:00:12:34:56` and the gateway's reply
from `52:55:0a:00:02:02`.

## The receive tail write

`DataPlane::receive` passes one receive tail value per descriptor it hands back, and `Tails`
allows an implementation to keep the newest value back and store it once. The program's last step,
`tests/qemu-smoke/src/receive_tail_cost.rs`, measures what that changes. Each round sends 15 ARP
requests, waits until the gateway's 15 replies fill the receive ring, and times draining the ring
with the time-stamp counter. Rounds alternate between a `Tails` that stores every value and one
that stores only the last, after the burst. A final step times 1000 stores of the value `RDT`
already holds.

Six runs with the Kernel Virtual Machine (KVM) and seven with QEMU's Tiny Code Generator
(TCG) instead, with QEMU 10.2.1 on a 2.1 gigahertz Intel Xeon Platinum 8160 host, printed medians
in these ranges, in time-stamp counter ticks at 2.1 gigahertz:

| | with KVM (`-accel kvm -cpu host`) | with TCG |
|---|---|---|
| burst of 15, one store per frame | 1,207,542 to 1,509,500 (576 to 720 microseconds) | 67,060 to 136,148 (32 to 65 microseconds) |
| burst of 15, one store per burst | 78,094 to 81,366 (37 to 39 microseconds) | 16,224 to 21,300 (8 to 10 microseconds) |
| one `RDT` store | 81,201 to 89,043 (39 to 43 microseconds) | 5,879 to 8,562 (3 to 4 microseconds) |

Nearly all of a burst's time is its tail stores: under KVM each store leaves the guest for QEMU's
device model. One store per burst drains it 15 to 19 times faster with KVM and 3 to 7 times faster
without. These are the costs of QEMU's model, not of an 82574L, where a register write is a posted bus
write; no one has measured real hardware.

To repeat the measurement with KVM, run the command line above with `-accel kvm -cpu host` added.
