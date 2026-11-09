#![no_std]
//! **The Intel `e1000e` family, as pure logic**: the 82574L and the Sunrise Point I219.
//!
//! Everything both halves of the driver compute, and nothing either touches. A kernel half resets
//! the controller, reads its MAC address and programs the ring bases; a network-stack half fills
//! descriptors and moves the tails. A ring's base and tail share a page ([`registers`]), so the
//! input-output memory management unit (IOMMU) is the whole direct memory access (DMA) confinement;
//! the process holds only [`RECEIVE_QUEUE_PAGE`] and [`TRANSMIT_QUEUE_PAGE`]. The register facts
//! come from Linux's `regs.h`, `defines.h` and `hw.h` for the 82574L and from FreeBSD's Intel
//! shared code for the I219 ([`platform_controller_hub`]).

//! # BUGS
//!
//! - **The I219's bring-up is ported in [`platform_controller_hub`] but not its link-up half**, which [`platform_controller_hub`]'s own
//!   BUGS records.
//! - **Legacy descriptors on the I219 are an assumption**, believed from the 8254x lineage and not
//!   read anywhere.
//! - **One receive and one transmit queue, polled.** No interrupt, no checksum offload, no
//!   segmentation offload, no jumbo frames. Each is a later decision rather than an omission.

#[cfg(test)]
extern crate std;

// The README's usage example runs as a doctest, so it cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[doc(alias = "pch")]
pub mod platform_controller_hub;

/// Register offsets in base address register 0 (BAR0). Read from Linux's `regs.h`; queue 0 only.
pub mod registers;

/// Offsets inside the DMA region, which the kernel half allocates contiguous and confines whole.
pub mod layout;

#[cfg(test)]
mod tests;

mod control_registers;
mod data_plane;
mod descriptor;
mod supported_devices;

pub use control_registers::{
    DEVICE_CONTROL_AUTO_SPEED_DETECTION_ENABLE, DEVICE_CONTROL_GLOBAL_RESET,
    DEVICE_CONTROL_SET_LINK_UP, DEVICE_STATUS_LINK_UP, EXTENDED_DEVICE_CONTROL_DRIVER_LOAD,
    RECEIVE_FILTER_CONTROL_EXTENDED_DESCRIPTORS_ENABLE, RECEIVE_MAC_ADDRESS_HIGH_VALID,
    receive_control_register_value, station_mac_address_from_receive_words,
    transmit_control_register_value, transmit_inter_packet_gap_register_value,
};
pub use data_plane::{DataPlane, DirectMemoryAccessRegion, Tails, TransmitError};
use descriptor::*;
pub use descriptor::{
    BUFFER_BYTES, MAXIMUM_FRAME_BYTES, RECEIVE_ENTRIES, RECEIVE_FRAME_ERROR_MASK,
    RECEIVE_QUEUE_PAGE, RECEIVE_STATUS_DESCRIPTOR_DONE, RECEIVE_STATUS_END_OF_PACKET,
    ReceiveVerdict, TAIL_IN_PAGE, TRANSMIT_COMMAND, TRANSMIT_ENTRIES, TRANSMIT_QUEUE_PAGE,
    TRANSMIT_STATUS_DESCRIPTOR_DONE, receive_verdict, transmit_descriptor_command_word,
};
/// The register access the I219 sequences and the 82574L's PHY identifier read both take.
pub use platform_controller_hub::Hardware;
pub use supported_devices::{DEVICE_IDS, VENDOR_INTEL, is_supported_device, model_name};

#[cfg(kani)]
mod proofs;
#[cfg(kani)]
mod ring_proofs;
