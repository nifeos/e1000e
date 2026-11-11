//! **What the driver writes to the controller's control registers**: the bits it sets, and the
//! Receive Control, Transmit Control and Transmit Inter-Packet Gap (`RCTL`, `TCTL` and `TIPG`) values it programs, and the MAC address read back.

/// Device Control (`CTRL`): set link up. OpenBSD's `E1000_CTRL_SLU` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "CTRL_SLU")]
#[doc(alias = "E1000_CTRL_SLU")]
pub const DEVICE_CONTROL_SET_LINK_UP: u32 = 1 << 6;

/// Device Control (`CTRL`): auto-speed detection enable. OpenBSD's `E1000_CTRL_ASDE` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "CTRL_ASDE")]
#[doc(alias = "E1000_CTRL_ASDE")]
pub const DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE: u32 = 1 << 5;

/// Device Control (`CTRL`): global reset, self-clearing. OpenBSD's `E1000_CTRL_RST` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "CTRL_RST")]
#[doc(alias = "E1000_CTRL_RST")]
pub const DEVICE_CONTROL_GLOBAL_RESET: u32 = 1 << 26;

/// Extended Device Control (`CTRL_EXT`): tells management firmware a driver has the device. OpenBSD's `E1000_CTRL_EXT_DRV_LOAD` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "CTRL_EXT_DRV_LOAD")]
#[doc(alias = "E1000_CTRL_EXT_DRV_LOAD")]
pub const EXTENDED_DEVICE_CONTROL_DRIVER_LOAD: u32 = 1 << 28;

/// Device Status (`STATUS`): link up. OpenBSD's `E1000_STATUS_LU` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "STATUS_LU")]
#[doc(alias = "E1000_STATUS_LU")]
pub const DEVICE_STATUS_LINK_UP: u32 = 1 << 1;

/// Receive Filter Control (`RFCTL`): extended receive descriptors. Cleared, so the device writes the legacy format. OpenBSD's `E1000_RFCTL_EXTEN` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "RFCTL_EXTEN")]
#[doc(alias = "E1000_RFCTL_EXTEN")]
pub const RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE: u32 = 1 << 15;

/// Receive Address High (`RAH`): the address in this entry is valid. OpenBSD's `E1000_RAH_AV` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same.
#[doc(alias = "RAH_AV")]
#[doc(alias = "E1000_RAH_AV")]
pub const RECEIVE_MAC_ADDRESS_HIGH_VALID: u32 = 1 << 31;

/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): transmit descriptor written back. OpenBSD's `E1000_ICR_TXDW` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_TXDW")]
#[doc(alias = "E1000_ICR_TXDW")]
pub const INTERRUPT_CAUSE_TRANSMIT_DESCRIPTOR_WRITTEN_BACK: u32 = 1;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): transmit queue empty. OpenBSD's `E1000_ICR_TXQE` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_TXQE")]
#[doc(alias = "E1000_ICR_TXQE")]
pub const INTERRUPT_CAUSE_TRANSMIT_QUEUE_EMPTY: u32 = 1 << 1;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): link status change. OpenBSD's `E1000_ICR_LSC` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_LSC")]
#[doc(alias = "E1000_ICR_LSC")]
pub const INTERRUPT_CAUSE_LINK_STATUS_CHANGE: u32 = 1 << 2;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): receive sequence error. OpenBSD's `E1000_ICR_RXSEQ` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_RXSEQ")]
#[doc(alias = "E1000_ICR_RXSEQ")]
pub const INTERRUPT_CAUSE_RECEIVE_SEQUENCE_ERROR: u32 = 1 << 3;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): receive descriptor minimum threshold reached. OpenBSD's `E1000_ICR_RXDMT0` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_RXDMT0")]
#[doc(alias = "E1000_ICR_RXDMT0")]
pub const INTERRUPT_CAUSE_RECEIVE_DESCRIPTOR_MINIMUM_THRESHOLD: u32 = 1 << 4;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): receive overrun. OpenBSD's `E1000_ICR_RXO` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_RXO")]
#[doc(alias = "E1000_ICR_RXO")]
pub const INTERRUPT_CAUSE_RECEIVE_OVERRUN: u32 = 1 << 6;
/// Interrupt cause (`ICR`, and the same bit in `IMS` and `IMC`): receive timer expired. OpenBSD's `E1000_ICR_RXT0` (`sys/dev/pci/if_em_hw.h`), which FreeBSD's `sys/dev/e1000/e1000_defines.h` and Linux spell the same. Provisional name.
#[doc(alias = "ICR_RXT0")]
#[doc(alias = "E1000_ICR_RXT0")]
pub const INTERRUPT_CAUSE_RECEIVE_TIMER: u32 = 1 << 7;

/// `RCTL`: receive enable (Linux's `regs.h`).
pub(crate) const RECEIVE_CONTROL_ENABLE: u32 = 1 << 1;

/// `RCTL`: accept broadcast, which a Dynamic Host Configuration Protocol (DHCP) offer needs (`E1000_RCTL_BAM`).
const RECEIVE_CONTROL_ACCEPT_BROADCAST: u32 = 1 << 15;

/// `RCTL`: strip the cyclic redundancy check (CRC) (`E1000_RCTL_SECRC`).
const RECEIVE_CONTROL_STRIP_CRC: u32 = 1 << 26;

// The three share no bit, so | and ^ compose the word identically; the compiler checks that here,
// so receive_control_register_value's OR-to-XOR swaps are equivalent mutants.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    RECEIVE_CONTROL_ENABLE ^ RECEIVE_CONTROL_ACCEPT_BROADCAST ^ RECEIVE_CONTROL_STRIP_CRC
        == RECEIVE_CONTROL_ENABLE | RECEIVE_CONTROL_ACCEPT_BROADCAST | RECEIVE_CONTROL_STRIP_CRC
);

/// **Receive Control** (`E1000_RCTL`, Linux's `regs.h`) as the kernel half programs it: enabled,
/// broadcast accepted (DHCP's offer is broadcast), 2048-byte buffers (size field zero), the CRC
/// stripped. **Not** promiscuous: unicast is filtered to receive-address entry 0, which the process
/// cannot write.
#[doc(alias = "receive control")]
#[doc(alias = "rctl")]
#[doc(alias = "E1000_RCTL")]
pub const fn receive_control_register_value() -> u32 {
    RECEIVE_CONTROL_ENABLE | RECEIVE_CONTROL_ACCEPT_BROADCAST | RECEIVE_CONTROL_STRIP_CRC
}

/// `TCTL`: transmit enable (Linux's `regs.h`).
pub(crate) const TRANSMIT_CONTROL_ENABLE: u32 = 1 << 1;

/// `TCTL`: pad short frames (`E1000_TCTL_PSP`).
const TRANSMIT_CONTROL_PAD_SHORT_PACKETS: u32 = 1 << 3;

// The four operands share no bit, so | and ^ compose the word identically; the compiler checks
// that here, so transmit_control_register_value's OR-to-XOR swaps are equivalent mutants.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(
    TRANSMIT_CONTROL_ENABLE ^ TRANSMIT_CONTROL_PAD_SHORT_PACKETS ^ (15 << 4) ^ (63 << 12)
        == TRANSMIT_CONTROL_ENABLE | TRANSMIT_CONTROL_PAD_SHORT_PACKETS | (15 << 4) | (63 << 12)
);

/// **Transmit Control** (`E1000_TCTL`, Linux's `regs.h`) as the kernel half programs it: enabled, short
/// frames padded, collision threshold 15 and distance 63 (Linux's `E1000_COLLISION_THRESHOLD` and
/// `E1000_COLLISION_DISTANCE`, `defines.h`).
#[doc(alias = "transmit control")]
#[doc(alias = "tctl")]
#[doc(alias = "E1000_TCTL")]
pub const fn transmit_control_register_value() -> u32 {
    TRANSMIT_CONTROL_ENABLE | TRANSMIT_CONTROL_PAD_SHORT_PACKETS | 15 << 4 | 63 << 12
}

// The three field values share no bit, so | and ^ compose the word identically; the compiler
// checks that here, so transmit_inter_packet_gap_register_value's OR-to-XOR swaps are equivalent mutants.
#[allow(clippy::eq_op, clippy::assertions_on_constants)] // the lint folds the constant comparison; it is the argument
const _: () = assert!(8 ^ (8 << 10) ^ (6 << 20) == 8 | (8 << 10) | (6 << 20));

/// **Transmit Inter-Packet Gap** (`E1000_TIPG`, Linux's `regs.h`) for copper: `IPGT` 8, `IPGR1` 8,
/// `IPGR2` 6 (Linux's `DEFAULT_82543_TIPG_*`).
#[doc(alias = "transmit inter-packet gap")]
#[doc(alias = "tipg")]
#[doc(alias = "E1000_TIPG")]
pub const fn transmit_inter_packet_gap_register_value() -> u32 {
    8 | 8 << 10 | 6 << 20
}

/// The station address (the MAC address) in receive-address entry 0, or `None` when the entry is
/// not marked valid, which on a real part means the NVM load did not happen and on QEMU does not
/// occur.
#[doc(alias = "mac")]
#[doc(alias = "mac_address")]
pub fn station_mac_address_from_receive_words(
    receive_mac_address_low: u32,
    receive_mac_address_high: u32,
) -> Option<[u8; 6]> {
    if receive_mac_address_high & RECEIVE_MAC_ADDRESS_HIGH_VALID == 0 {
        return None;
    }
    let low_bytes = receive_mac_address_low.to_le_bytes();
    let high_bytes = receive_mac_address_high.to_le_bytes();
    let station_mac_address = [
        low_bytes[0],
        low_bytes[1],
        low_bytes[2],
        low_bytes[3],
        high_bytes[0],
        high_bytes[1],
    ];
    // All zeros, or a group address, is not an address a station can own.
    (station_mac_address != [0; 6] && station_mac_address[0] & 1 == 0)
        .then_some(station_mac_address)
}
