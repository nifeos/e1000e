//! **One Address Resolution Protocol (ARP) exchange** with QEMU's user-mode network: a broadcast
//! request for the gateway's hardware address, sent through [`DataPlane::transmit`], and the
//! gateway's unicast reply, taken through [`DataPlane::receive`].

use e1000e::{DataPlane, DirectMemoryAccessRegion, MAXIMUM_FRAME_BYTES, Tails, layout};

/// The address QEMU's user-mode network gives the guest by default.
const GUEST_IP_ADDRESS: [u8; 4] = [10, 0, 2, 15];
/// The user-mode network's gateway, which answers ARP for itself.
const GATEWAY_IP_ADDRESS: [u8; 4] = [10, 0, 2, 2];
const ETHER_TYPE_ARP: [u8; 2] = [0x08, 0x06];
const OPERATION_REQUEST: u8 = 1;
const OPERATION_REPLY: u8 = 2;
/// How many times the receive ring is polled before the exchange fails.
const RECEIVE_POLLS: u32 = 50_000_000;

/// Print a hardware address as six colon-separated hexadecimal octets.
pub fn say_mac_address(mac_address: [u8; 6]) {
    let [a, b, c, d, e, f] = mac_address;
    say!("{a:02x}:{b:02x}:{c:02x}:{d:02x}:{e:02x}:{f:02x}");
}

/// The 42-byte request: Ethernet header, then the ARP body for Ethernet and Internet Protocol
/// version 4 (IPv4).
fn request(station: [u8; 6]) -> [u8; 42] {
    let mut frame = [0; 42];
    frame[0..6].copy_from_slice(&[0xff; 6]);
    frame[6..12].copy_from_slice(&station);
    frame[12..14].copy_from_slice(&ETHER_TYPE_ARP);
    // Hardware type 1 (Ethernet), protocol type 0x0800 (IPv4), lengths 6 and 4.
    frame[14..22].copy_from_slice(&[0, 1, 0x08, 0x00, 6, 4, 0, OPERATION_REQUEST]);
    frame[22..28].copy_from_slice(&station);
    frame[28..32].copy_from_slice(&GUEST_IP_ADDRESS);
    frame[38..42].copy_from_slice(&GATEWAY_IP_ADDRESS);
    frame
}

/// The gateway's hardware address, when `frame` is its ARP reply to this station.
fn reply_sender(frame: &[u8], station: [u8; 6]) -> Option<[u8; 6]> {
    let is_reply = frame.len() >= 42
        && frame[0..6] == station
        && frame[12..14] == ETHER_TYPE_ARP
        && frame[20..22] == [0, OPERATION_REPLY]
        && frame[28..32] == GATEWAY_IP_ADDRESS
        && frame[38..42] == GUEST_IP_ADDRESS;
    is_reply.then(|| frame[22..28].try_into().expect("six bytes"))
}

/// Send the request, wait for its descriptor to be done, then receive until the reply arrives.
pub fn exchange(
    data_plane: &mut DataPlane,
    region: &mut impl DirectMemoryAccessRegion,
    tails: &mut impl Tails,
    station: [u8; 6],
) {
    let frame = request(station);
    if let Err(error) = data_plane.transmit(region, tails, &frame) {
        crate::fail(match error {
            e1000e::TransmitError::Full => "transmit: ring full",
            _ => "transmit: bad length",
        });
    }
    say!(
        "sent: ARP request who-has 10.0.2.2 tell 10.0.2.15, {} bytes\n",
        frame.len()
    );
    let status_offset = layout::transmit_descriptor(0) + 8;
    let done = (0..RECEIVE_POLLS).any(|_| {
        (region.read_u64(status_offset) >> 32) as u8 & e1000e::TRANSMIT_STATUS_DESCRIPTOR_DONE != 0
    });
    if !done {
        crate::fail("the device never marked the transmit descriptor done");
    }
    say!("transmit descriptor 0: descriptor done\n");
    let mut buffer = [0; MAXIMUM_FRAME_BYTES];
    for _ in 0..RECEIVE_POLLS {
        let Some(length) = data_plane.receive(region, tails, &mut buffer) else {
            core::hint::spin_loop();
            continue;
        };
        let received = &buffer[..length];
        say!(
            "received: {length} bytes, ether type {:02x}{:02x}\n",
            received[12],
            received[13]
        );
        if let Some(gateway) = reply_sender(received, station) {
            say!("received: ARP reply 10.0.2.2 is-at ");
            say_mac_address(gateway);
            say!("\n");
            return;
        }
    }
    crate::fail("no ARP reply from the gateway");
}
