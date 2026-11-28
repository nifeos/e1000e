/// **Strap** (`E1000_STRAP`, FreeBSD's `e1000_ich8lan.h`). Read only; FreeBSD writes it to order a
/// config-space read before reset. Bits 5:1 also give the NVM's size
/// ([`super::non_volatile_memory::Geometry`]).
#[doc(alias = "strap")]
#[doc(alias = "E1000_STRAP")]
pub(crate) const STRAP_REGISTER: u64 = 0x0_000c;
/// **Future Extended NVM 6** (`E1000_FEXTNVM6`, FreeBSD's `e1000_regs.h`). Holds the K1-off
/// enable the link-up half copies from [`PCI_EXPRESS_ANALOG_CONFIGURATION_REGISTER`].
#[doc(alias = "future extended non-volatile memory 6")]
#[doc(alias = "E1000_FEXTNVM6")]
pub(crate) const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER: u64 = 0x0_0010;
/// **Management Data Interface (MDI) Control** (`E1000_MDIC`, FreeBSD's `e1000_regs.h`). The MDIO transaction register
/// ([`super::physical_layer_device`]).
#[doc(alias = "management data interface control")]
#[doc(alias = "E1000_MDIC")]
pub(crate) const MANAGEMENT_DATA_INTERFACE_CONTROL_REGISTER: u64 = 0x0_0020;
/// **Future Extended NVM 4** (`E1000_FEXTNVM4`, FreeBSD's `e1000_regs.h`). Its low three bits are
/// the beacon duration.
#[doc(alias = "future extended non-volatile memory 4")]
#[doc(alias = "E1000_FEXTNVM4")]
pub(crate) const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_4_REGISTER: u64 = 0x0_0024;
/// **Future Extended NVM** (`E1000_FEXTNVM`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory")]
#[doc(alias = "E1000_FEXTNVM")]
pub(crate) const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_REGISTER: u64 = 0x0_0028;
/// **Kumeran Control and Status** (`E1000_KMRNCTRLSTA`, FreeBSD's `e1000_regs.h`). The window onto
/// the Kumeran registers of the MAC-PHY interconnect: offset, read enable and data in one word.
#[doc(alias = "kumeran control and status")]
#[doc(alias = "E1000_KMRNCTRLSTA")]
pub(crate) const KUMERAN_CONTROL_AND_STATUS_REGISTER: u64 = 0x0_0034;
/// **Future Extended NVM 3** (`E1000_FEXTNVM3`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory 3")]
#[doc(alias = "E1000_FEXTNVM3")]
pub(crate) const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_3_REGISTER: u64 = 0x0_003c;
/// **Future Extended NVM 7** (`E1000_FEXTNVM7`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "future extended non-volatile memory 7")]
#[doc(alias = "E1000_FEXTNVM7")]
pub(crate) const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_7_REGISTER: u64 = 0x0_00e4;
/// **Sideband Control** (`E1000_SVCR`, FreeBSD's `e1000_regs.h`). Optimized buffer flush/fill
/// (OBFF) enable and its interrupt unmask.
#[doc(alias = "sideband control")]
#[doc(alias = "E1000_SVCR")]
pub(crate) const SIDEBAND_CONTROL_REGISTER: u64 = 0x0_00f0;
/// **Sideband Threshold** (`E1000_SVT`, FreeBSD's `e1000_regs.h`). The optimized buffer flush/fill
/// high water mark, in KiB of receive buffer.
#[doc(alias = "sideband threshold")]
#[doc(alias = "E1000_SVT")]
pub(crate) const SIDEBAND_THRESHOLD_REGISTER: u64 = 0x0_00f4;
/// **Latency Tolerance Reporting Value** (`E1000_LTRV`, FreeBSD's `e1000_ich8lan.h`). The snoop and
/// no-snoop latencies the MAC reports to the platform, and the bit that sends them.
#[doc(alias = "latency tolerance reporting value")]
#[doc(alias = "E1000_LTRV")]
pub(crate) const LATENCY_TOLERANCE_REPORTING_VALUE_REGISTER: u64 = 0x0_00f8;
/// **LED Control** (`E1000_LEDCTL`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "led control")]
#[doc(alias = "E1000_LEDCTL")]
pub(crate) const LED_CONTROL_REGISTER: u64 = 0x0_0e00;
/// **Extended Configuration Control** (`E1000_EXTCNF_CTRL`, FreeBSD's `e1000_regs.h`). The
/// semaphore, the PHY-configuration gate, and the pointer to the NVM's extended configuration
/// region.
#[doc(alias = "extended configuration control")]
#[doc(alias = "E1000_EXTCNF_CTRL")]
pub(crate) const EXTENDED_CONFIGURATION_CONTROL_REGISTER: u64 = 0x0_0f00;
/// **Extended Configuration Size** (`E1000_EXTCNF_SIZE`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "extended configuration size")]
#[doc(alias = "E1000_EXTCNF_SIZE")]
pub(crate) const EXTENDED_CONFIGURATION_SIZE_WORDS_REGISTER: u64 = 0x0_0f08;
/// **PHY Control Register in Control and Status Register (CSR)** (`E1000_PHY_CTRL`, FreeBSD's `e1000_regs.h`). The PHY control
/// the MAC holds: `LPLU` and gigabit disable.
#[doc(alias = "physical layer device control register in control and status registers")]
#[doc(alias = "E1000_PHY_CTRL")]
pub(crate) const PHYSICAL_LAYER_DEVICE_CONTROL_REGISTER: u64 = 0x0_0f10;
/// **PCI Express Analog Configuration** (`E1000_PCIEANACFG`, FreeBSD's `e1000_regs.h`). Read only
/// here: its K1-off bit is copied into [`FUTURE_EXTENDED_NON_VOLATILE_MEMORY_6_REGISTER`].
#[doc(alias = "pci express analog configuration")]
#[doc(alias = "E1000_PCIEANACFG")]
pub(crate) const PCI_EXPRESS_ANALOG_CONFIGURATION_REGISTER: u64 = 0x0_0f18;
/// **`IOSF` Power Control** (`E1000_IOSFPC`, FreeBSD's `e1000_regs.h`). The I/O side-band fabric's
/// power control, which SPT's transmit errata write. FreeBSD's comment says the write avoids corrupted transmit data.
#[doc(alias = "iosf power control")]
#[doc(alias = "E1000_IOSFPC")]
pub(crate) const IOSF_POWER_CONTROL_REGISTER: u64 = 0x0_0f28;
/// **Packet Buffer Allocation** (`E1000_PBA`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "packet buffer allocation")]
#[doc(alias = "E1000_PBA")]
pub(crate) const PACKET_BUFFER_ALLOCATION_REGISTER: u64 = 0x0_1000;
/// **Packet Buffer Error Correcting Code (ECC) Status** (`E1000_PBECCSTS`, FreeBSD's `e1000_regs.h`).
#[doc(alias = "packet buffer ecc status")]
#[doc(alias = "E1000_PBECCSTS")]
pub(crate) const PACKET_BUFFER_ERROR_CORRECTING_CODE_STATUS_REGISTER: u64 = 0x0_100c;
/// **Receive Descriptor Control** (OpenBSD's `E1000_RXDCTL(0)`, `sys/dev/pci/if_em_hw.h`). Queue 0.
#[doc(alias = "receive descriptor control")]
#[doc(alias = "E1000_RXDCTL")]
#[doc(alias = "RXDCTL")]
pub const RECEIVE_DESCRIPTOR_CONTROL_REGISTER: u64 = 0x0_2828;
/// **Analog Front End Band Gap Transmit Reference Data** (`E1000_KABGTXD`, FreeBSD's
/// `e1000_regs.h`).
#[doc(alias = "analog front end band gap transmit reference data")]
#[doc(alias = "E1000_KABGTXD")]
pub(crate) const ANALOG_FRONT_END_BAND_GAP_TRANSMIT_REFERENCE_DATA_REGISTER: u64 = 0x0_3004;
/// **Transmit Descriptor Control** (`E1000_TXDCTL(0)`, FreeBSD's `e1000_regs.h`). Queue 0.
#[doc(alias = "transmit descriptor control")]
pub(crate) const TRANSMIT_DESCRIPTOR_CONTROL_0_REGISTER: u64 = 0x0_3828;
/// **Transmit Arbitration Count** (`E1000_TARC(0)`, FreeBSD's `e1000_regs.h`). Queue 0.
#[doc(alias = "transmit arbitration count")]
pub(crate) const TRANSMIT_ARBITRATION_COUNT_0_REGISTER: u64 = 0x0_3840;
/// **Transmit Descriptor Control** (`E1000_TXDCTL(1)`, FreeBSD's `e1000_regs.h`). Queue 1.
#[doc(alias = "transmit descriptor control")]
pub(crate) const TRANSMIT_DESCRIPTOR_CONTROL_1_REGISTER: u64 = 0x0_3928;
/// **Transmit Arbitration Count** (`E1000_TARC(1)`, FreeBSD's `e1000_regs.h`). Queue 1.
#[doc(alias = "transmit arbitration count")]
pub(crate) const TRANSMIT_ARBITRATION_COUNT_1_REGISTER: u64 = 0x0_3940;
/// **Host to Management Engine** (`E1000_H2ME`, FreeBSD's `e1000_ich8lan.h`). Messages from the driver to the
/// Management Engine.
#[doc(alias = "host to management engine")]
#[doc(alias = "E1000_H2ME")]
pub(crate) const HOST_TO_MANAGEMENT_ENGINE_REGISTER: u64 = 0x0_5b50;
/// **Firmware Semaphore** (`E1000_FWSM`, FreeBSD's `e1000_regs.h`). The firmware semaphore: the
/// Management Engine's status.
#[doc(alias = "firmware semaphore")]
#[doc(alias = "E1000_FWSM")]
pub(crate) const FIRMWARE_SEMAPHORE_REGISTER: u64 = 0x0_5b54;
/// **Future Extended NVM 11** (OpenBSD's `E1000_FEXTNVM11`, `sys/dev/pci/if_em_hw.h`).
#[doc(alias = "future extended non-volatile memory 11")]
#[doc(alias = "E1000_FEXTNVM11")]
#[doc(alias = "FEXTNVM11")]
pub const FUTURE_EXTENDED_NON_VOLATILE_MEMORY_11_REGISTER: u64 = 0x0_5bbc;
