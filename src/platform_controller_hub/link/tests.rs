//! The link check against the simulator: one case per branch FreeBSD takes on Sunrise Point.

use std::vec::Vec;

use super::super::simulator::{PhysicalLayerDeviceStartState, SimulatedHardware};
use super::*;

const PLATFORM: PlatformLatencyTolerance = PlatformLatencyTolerance {
    // Scale 4, value 3: 3 × 2^20 ns, about 3.1 ms, more than any buffer here needs.
    maximum_snoop: 0x1003,
    maximum_no_snoop: 0,
};
const TRANSMIT_GAP_AS_BRING_UP_LEAVES_IT: u32 = 6 << 20 | 8 << 10 | 0x3ff;
const TRANSMIT_CONTROL_WITH_ANOTHER_COLLISION_DISTANCE: u32 = 0x0f << 12 | 0xa;

/// A device after bring-up whose PHY reports link, with `STATUS` showing `status`, and every
/// register the check changes holding bits it must keep.
fn linked(status: u32) -> SimulatedHardware {
    let mut s = SimulatedHardware::new(PhysicalLayerDeviceStartState::Healthy, true);
    s.link_up = true;
    s.mac.insert(mac::DEVICE_STATUS_REGISTER, status);
    s.mac
        .insert(registers::PACKET_BUFFER_ALLOCATION_REGISTER, 0x0014_001a);
    s.mac.insert(
        mac::TRANSMIT_INTER_PACKET_GAP_REGISTER,
        TRANSMIT_GAP_AS_BRING_UP_LEAVES_IT,
    );
    s.mac.insert(
        mac::TRANSMIT_CONTROL_REGISTER,
        TRANSMIT_CONTROL_WITH_ANOTHER_COLLISION_DISTANCE,
    );
    s.mac.insert(
        registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER,
        0xf0,
    );
    s.mac
        .insert(registers::SIDEBAND_THRESHOLD_REGISTER, 0x0100 | 0x1f);
    let device = &mut s.physical_layer_device_registers;
    device.insert(phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER, 0xf800 | 0x123);
    device.insert(phy::POINTER_GAP_REGISTER, 0x8000 | 0x10 << 2 | 0x3);
    device.insert(phy::LOW_POWER_IDLE_CONTROL_REGISTER, 0x6000 | 0x0abc);
    device.insert(phy::POWER_MANAGEMENT_CONTROL_REGISTER, 0x0001);
    s.kumeran.insert(KUMERAN_K0S_CONTROL, 0x0172);
    s
}

fn writes_to(s: &SimulatedHardware, offset: u32) -> Vec<u16> {
    s.physical_layer_device_writes
        .iter()
        .filter(|w| w.0 == offset)
        .map(|w| w.1)
        .collect()
}

fn is_software_flag_free(s: &SimulatedHardware) -> bool {
    s.mac[&registers::EXTENDED_CONFIGURATION_CONTROL_REGISTER]
        & physical_layer_device::EXTCNF_CTRL_SWFLAG
        == 0
}

/// The gap and receive configuration for every speed and duplex: 10 half alone gets the long gap
/// and the low-latency receive setting, and Sunrise Point's 0xC applies only to full duplex below
/// 1000 Mbit/s.
#[test]
fn each_speed_and_duplex_gets_freebsds_gap() {
    use LinkSpeed::*;
    for (speed, full, want) in [
        (TenMegabitsPerSecond, false, (0xff, 0)),
        (TenMegabitsPerSecond, true, (0x0c, 1)),
        (HundredMegabitsPerSecond, false, (0x08, 1)),
        (HundredMegabitsPerSecond, true, (0x0c, 1)),
        (OneGigabitPerSecond, false, (0x08, 1)),
        (OneGigabitPerSecond, true, (0x08, 1)),
    ] {
        assert_eq!(
            transmit_gap_and_receive_configuration(speed, full),
            want,
            "{speed:?} full duplex {full}"
        );
    }
}

#[test]
fn a_gigabit_link_is_configured_as_freebsd_configures_it() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    let mut watch = LinkWatch::new(PLATFORM);
    assert_eq!(
        check_for_copper_link(&mut s, &mut watch),
        Ok(LinkReport::Up {
            speed: LinkSpeed::OneGigabitPerSecond,
            is_full_duplex: true
        })
    );
    assert!(!watch.is_check_due());
    assert!(is_software_flag_free(&s));
    assert!(s.elapsed_microseconds >= 50_000);
    assert_eq!(
        s.mac[&mac::TRANSMIT_INTER_PACKET_GAP_REGISTER],
        6 << 20 | 8 << 10 | 0x08
    );
    assert_eq!(s.extended[&I217_RECEIVE_CONFIGURATION], 1);
    assert_eq!(
        s.physical_layer_device_registers[&phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER],
        0xf800 | 0xfa
    );
    assert_eq!(
        s.physical_layer_device_registers[&phy::POWER_MANAGEMENT_CONTROL_REGISTER],
        0x0201
    );
    assert_eq!(
        writes_to(&s, phy::POINTER_GAP_REGISTER),
        [0x8000 | 0x18 << 2 | 0x3]
    );
    assert!(
        s.notes
            .contains(&BringUpObservation::PointerGapRaised { from: 0x10 })
    );
    assert_eq!(
        s.mac[&registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER],
        0xf7
    );
    // 26 KiB of receive buffer drains in 188704 ns at 1000 Mbit/s: scale 2, value 185.
    assert_eq!(
        s.mac[&registers::LATENCY_TOLERANCE_REPORTING_VALUE_REGISTER],
        1 << 31 | 1 << 30 | 0x08b9 << 16 | 1 << 15 | 0x08b9
    );
    assert_eq!(s.mac[&registers::SIDEBAND_THRESHOLD_REGISTER], 0x0100 | 3);
    assert_eq!(s.mac[&registers::SIDEBAND_CONTROL_REGISTER], 0x1001);
    assert!(
        s.notes
            .contains(&BringUpObservation::LatencyToleranceReported {
                encoded: 0x08b9,
                high_water_mark_kibibytes: 3
            })
    );
    assert_eq!(s.kumeran_writes, [(KUMERAN_K0S_CONTROL, 0x0112)]);
    assert_eq!(
        s.physical_layer_device_registers[&phy::LOW_POWER_IDLE_CONTROL_REGISTER],
        0x0abc
    );
    assert_eq!(
        s.extended_reads,
        [I217_ENERGY_EFFICIENT_ETHERNET_PHYSICAL_CODING_SUBLAYER_STATUS]
    );
    assert_eq!(s.mac[&mac::TRANSMIT_CONTROL_REGISTER], 63 << 12 | 0xa);
}

/// Below 1000 Mbit/s the clock gate gets the longer timer, the K1 clock request is left alone and
/// the pointer gap register is written outright.
#[test]
fn a_ten_megabit_half_duplex_link_gets_the_long_gap_and_no_gigabit_settings() {
    let mut s = linked(0);
    let mut watch = LinkWatch::new(PLATFORM);
    assert_eq!(
        check_for_copper_link(&mut s, &mut watch),
        Ok(LinkReport::Up {
            speed: LinkSpeed::TenMegabitsPerSecond,
            is_full_duplex: false
        })
    );
    assert_eq!(
        s.mac[&mac::TRANSMIT_INTER_PACKET_GAP_REGISTER],
        6 << 20 | 8 << 10 | 0xff
    );
    assert_eq!(s.extended[&I217_RECEIVE_CONFIGURATION], 0);
    assert_eq!(
        s.physical_layer_device_registers[&phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER],
        0xf800 | 0x3e8
    );
    assert!(writes_to(&s, phy::POWER_MANAGEMENT_CONTROL_REGISTER).is_empty());
    assert_eq!(writes_to(&s, phy::POINTER_GAP_REGISTER), [0xc023]);
    assert!(is_software_flag_free(&s));
}

/// The pointer gap is only ever raised.
#[test]
fn a_pointer_gap_already_at_its_minimum_is_left_alone() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.physical_layer_device_registers
        .insert(phy::POINTER_GAP_REGISTER, 0x19 << 2);
    assert!(check_for_copper_link(&mut s, &mut LinkWatch::new(PLATFORM)).is_ok());
    assert!(writes_to(&s, phy::POINTER_GAP_REGISTER).is_empty());
    assert!(
        !s.notes
            .iter()
            .any(|n| matches!(n, BringUpObservation::PointerGapRaised { .. }))
    );
}

/// With no link the platform hears that nothing is required, the MAC-side K1 and K0s settings are
/// still made, nothing speed-dependent is touched, and the next call checks again.
#[test]
fn no_link_reports_no_latency_requirement_and_stays_due() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.link_up = false;
    let mut watch = LinkWatch::new(PLATFORM);
    assert_eq!(
        check_for_copper_link(&mut s, &mut watch),
        Ok(LinkReport::Down)
    );
    assert!(watch.is_check_due());
    assert_eq!(
        s.mac[&registers::LATENCY_TOLERANCE_REPORTING_VALUE_REGISTER],
        LATENCY_TOLERANCE_SEND
    );
    assert_eq!(s.mac[&registers::SIDEBAND_THRESHOLD_REGISTER], 0x0100);
    assert_eq!(s.kumeran_writes, [(KUMERAN_K0S_CONTROL, 0x0112)]);
    assert_eq!(
        s.mac[&registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER],
        0xf7
    );
    assert_eq!(
        s.mac[&mac::TRANSMIT_INTER_PACKET_GAP_REGISTER],
        TRANSMIT_GAP_AS_BRING_UP_LEAVES_IT
    );
    assert!(s.physical_layer_device_writes.is_empty());
    assert_eq!(
        s.mac[&mac::TRANSMIT_CONTROL_REGISTER],
        TRANSMIT_CONTROL_WITH_ANOTHER_COLLISION_DISTANCE
    );
    assert!(is_software_flag_free(&s));
}

/// A link that dropped and came back still latches low in `PHY_STATUS`: one read would call it
/// down.
#[test]
fn a_latched_loss_of_link_is_read_past() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.link_latched_low = true;
    assert!(matches!(
        check_for_copper_link(&mut s, &mut LinkWatch::new(PLATFORM)),
        Ok(LinkReport::Up { .. })
    ));
}

/// A check not due touches nothing, not even the 50 ms wait; the next due one leaves set what the
/// first set.
#[test]
fn a_check_not_due_does_not_reach_the_hardware() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    let mut watch = LinkWatch::new(PLATFORM);
    assert!(check_for_copper_link(&mut s, &mut watch).is_ok());
    let (reads, writes, elapsed) = (s.reads.len(), s.writes.len(), s.elapsed_microseconds);
    assert_eq!(
        check_for_copper_link(&mut s, &mut watch),
        Ok(LinkReport::NotChecked)
    );
    assert_eq!(
        (s.reads.len(), s.writes.len(), s.elapsed_microseconds),
        (reads, writes, elapsed)
    );
    watch.link_status_changed();
    assert!(check_for_copper_link(&mut s, &mut watch).is_ok());
    assert!(s.writes.len() > writes);
    assert_eq!(
        s.physical_layer_device_registers[&phy::POWER_MANAGEMENT_CONTROL_REGISTER],
        0x0201
    );
    assert_eq!(s.mac[&registers::SIDEBAND_CONTROL_REGISTER], 0x1001);
}

/// Sunrise Point's K1-off workaround: `FEXTNVM6` takes `PCIEANACFG`'s bit either way and keeps the
/// rest.
#[test]
fn the_k1_off_enable_follows_the_analog_configuration() {
    for analog in [FEXTNVM6_K1_OFF_ENABLE, 0] {
        let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
        s.mac.insert(
            registers::PCI_EXPRESS_ANALOG_CONFIGURATION_REGISTER,
            analog | 0x5,
        );
        s.mac.insert(
            registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER,
            (!analog & FEXTNVM6_K1_OFF_ENABLE) | 0x0300,
        );
        assert!(check_for_copper_link(&mut s, &mut LinkWatch::new(PLATFORM)).is_ok());
        assert_eq!(
            s.mac[&registers::FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER],
            analog | 0x0300
        );
    }
}

/// A latency the registers cannot hold fails the check before the K1 and K0s settings, as in
/// FreeBSD, and leaves it due.
#[test]
fn a_latency_too_long_to_encode_fails_the_check_and_leaves_it_due() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.mac
        .insert(registers::PACKET_BUFFER_ALLOCATION_REGISTER, 0xffff);
    let mut watch = LinkWatch::new(PLATFORM);
    assert_eq!(
        check_for_copper_link(&mut s, &mut watch),
        Err(Error::LatencyToleranceOutOfRange)
    );
    assert!(watch.is_check_due());
    assert!(s.kumeran_writes.is_empty());
    assert!(is_software_flag_free(&s));
}

/// A clock gate read FreeBSD does not check: here it skips the write and is noted, and the link
/// still comes up.
#[test]
fn a_failed_clock_gate_read_writes_nothing_and_is_noted() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.failing_offsets
        .push(phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER);
    assert!(matches!(
        check_for_copper_link(&mut s, &mut LinkWatch::new(PLATFORM)),
        Ok(LinkReport::Up { .. })
    ));
    assert!(writes_to(&s, phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER).is_empty());
    assert!(s.notes.iter().any(|n| matches!(
        n,
        BringUpObservation::LinkPhysicalLayerDeviceAccessFailed { offset, .. }
            if *offset == phy::PHASE_LOCKED_LOOP_CLOCK_GATE_REGISTER
    )));
    assert!(is_software_flag_free(&s));
}

/// EEE that cannot be turned off is noted and the link reported up, with FreeBSD's early return
/// past the collision distance.
#[test]
fn energy_efficient_ethernet_left_on_is_noted_and_the_link_is_up() {
    let mut s = linked(STATUS_SPEED_1000 | STATUS_FULL_DUPLEX);
    s.failing_offsets.push(phy::LOW_POWER_IDLE_CONTROL_REGISTER);
    let mut watch = LinkWatch::new(PLATFORM);
    assert!(matches!(
        check_for_copper_link(&mut s, &mut watch),
        Ok(LinkReport::Up { .. })
    ));
    assert!(!watch.is_check_due());
    assert!(
        s.notes
            .iter()
            .any(|n| matches!(n, BringUpObservation::EnergyEfficientEthernetNotDisabled(_)))
    );
    assert_eq!(
        s.mac[&mac::TRANSMIT_CONTROL_REGISTER],
        TRANSMIT_CONTROL_WITH_ANOTHER_COLLISION_DISTANCE
    );
    assert!(is_software_flag_free(&s));
}

/// `e1000_platform_pm_pch_lpt`'s arithmetic, worked by hand from FreeBSD's code.
#[test]
fn the_latency_tolerance_is_freebsds() {
    use LinkSpeed::*;
    // 26 KiB less two 1518-byte frames is 188704 ns at 1000 Mbit/s and ten times that per step
    // down; each leaves 3 KiB above the drain. At 10 Mbit/s that is more than `PLATFORM`
    // allows.
    assert_eq!(
        latency_tolerance(26, OneGigabitPerSecond, PLATFORM),
        Ok((2 << 10 | 185, 3))
    );
    assert_eq!(
        latency_tolerance(26, HundredMegabitsPerSecond, PLATFORM),
        Ok((3 << 10 | 58, 3))
    );
    let unlimited = PlatformLatencyTolerance {
        maximum_snoop: 0x1fff,
        maximum_no_snoop: 0,
    };
    assert_eq!(
        latency_tolerance(26, TenMegabitsPerSecond, unlimited),
        Ok((3 << 10 | 576, 3))
    );
    // The larger platform limit wins, and a lower one caps the latency and the drain: 100 × 2^10
    // ns is 102400 ns, 12 KiB at 1000 Mbit/s.
    let low = PlatformLatencyTolerance {
        maximum_snoop: 2 << 10 | 50,
        maximum_no_snoop: 2 << 10 | 100,
    };
    assert_eq!(
        latency_tolerance(26, OneGigabitPerSecond, low),
        Ok((2 << 10 | 100, 14))
    );
    // A platform that tolerates nothing gets zero, and so does a buffer under two frames; zero
    // leaves the high water mark at zero, not at the buffer.
    let none = PlatformLatencyTolerance {
        maximum_snoop: 0,
        maximum_no_snoop: 0,
    };
    assert_eq!(latency_tolerance(26, OneGigabitPerSecond, none), Ok((0, 0)));
    assert_eq!(
        latency_tolerance(2, OneGigabitPerSecond, PLATFORM),
        Ok((0, 0))
    );
    // Two encodings of one latency: the platform's is kept only when it is strictly shorter, so
    // here the high water mark comes from the buffer's own drain, not the platform's.
    let equal = PlatformLatencyTolerance {
        maximum_snoop: 0x0fe9,
        maximum_no_snoop: 0,
    };
    assert_eq!(
        latency_tolerance(43, TenMegabitsPerSecond, equal),
        Ok((0x0fe9, 3))
    );
    // Scale 5 is the largest that encodes, even where the platform's limit then replaces it; a
    // latency needing scale 6 is refused before the limit is looked at.
    let capped = |encoded| PlatformLatencyTolerance {
        maximum_snoop: encoded,
        maximum_no_snoop: 0,
    };
    assert_eq!(
        latency_tolerance(1320, TenMegabitsPerSecond, capped(4 << 10 | 992)),
        Ok((4 << 10 | 992, 20))
    );
    assert_eq!(
        latency_tolerance(42970, TenMegabitsPerSecond, capped(6 << 10 | 32)),
        Err(Error::LatencyToleranceOutOfRange)
    );
    // Above about 125 KiB the drain, counted in kilobytes, outruns the buffer, counted in KiB.
    assert_eq!(
        latency_tolerance(1000, OneGigabitPerSecond, PLATFORM),
        Err(Error::LatencyToleranceOutOfRange)
    );
}
