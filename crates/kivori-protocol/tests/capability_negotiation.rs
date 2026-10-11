//! T033 — capability negotiation: the negotiated set is the intersection; a capability only one
//! side advertises is dropped.

use kivori_model::Capabilities;
use kivori_protocol::negotiate;

#[test]
fn negotiated_caps_are_the_intersection() {
    let (minor, caps) = negotiate(
        3,
        1,
        Capabilities::from_bits(0b1011),
        Capabilities::from_bits(0b0110),
    );
    assert_eq!(minor, 1);
    assert_eq!(caps, Capabilities::from_bits(0b0010));
}

#[test]
fn capability_only_one_side_advertises_is_dropped() {
    let (_minor, caps) = negotiate(
        0,
        0,
        Capabilities::from_bits(0b001), // desktop only
        Capabilities::from_bits(0b100), // device only
    );
    assert!(!caps.contains(Capabilities::from_bits(0b001)));
    assert!(!caps.contains(Capabilities::from_bits(0b100)));
    assert!(caps.is_empty());
}

#[test]
fn full_overlap_is_preserved() {
    let both = Capabilities::from_bits(0b1010);
    let (_minor, caps) = negotiate(2, 2, both, both);
    assert_eq!(caps, both);
}

#[test]
fn host_takeovers_needs_both_peers() {
    let both = Capabilities::HOST_TAKEOVERS_V1.union(Capabilities::MASCOT_INTERACTION);
    let (_, caps) = negotiate(5, 5, both, both);
    assert!(caps.contains(Capabilities::HOST_TAKEOVERS_V1));
    // An older device (1.4, no bit 10) never negotiates it, so the desktop must not send the new
    // `Bye` reasons to it.
    let (minor, caps) = negotiate(5, 4, both, Capabilities::MASCOT_INTERACTION);
    assert_eq!(minor, 4);
    assert!(!caps.contains(Capabilities::HOST_TAKEOVERS_V1));
}

#[test]
fn quiet_ack_needs_both_peers() {
    let both = Capabilities::QUIET_ACK_V1.union(Capabilities::MASCOT_INTERACTION);
    let (_, caps) = negotiate(6, 6, both, both);
    assert!(caps.contains(Capabilities::QUIET_ACK_V1));
    // An older device (1.5, no bit 11) never negotiates it, so the desktop must keep sending
    // `Unverified` to it.
    let (minor, caps) = negotiate(6, 5, both, Capabilities::MASCOT_INTERACTION);
    assert_eq!(minor, 5);
    assert!(!caps.contains(Capabilities::QUIET_ACK_V1));
}

#[test]
fn device_internals_needs_both_peers() {
    let both = Capabilities::DEVICE_INTERNALS_V1.union(Capabilities::MASCOT_INTERACTION);
    let (_, caps) = negotiate(6, 6, both, both);
    assert!(caps.contains(Capabilities::DEVICE_INTERNALS_V1));
    // A device without bit 12 never negotiates it, so the desktop must not send the request.
    let (_, caps) = negotiate(6, 6, both, Capabilities::MASCOT_INTERACTION);
    assert!(!caps.contains(Capabilities::DEVICE_INTERNALS_V1));
}

#[test]
fn idle_life_needs_both_peers() {
    let both = Capabilities::IDLE_LIFE_V1.union(Capabilities::MASCOT_INTERACTION);
    let (_, caps) = negotiate(6, 6, both, both);
    assert!(caps.contains(Capabilities::IDLE_LIFE_V1));
    // Older firmware never negotiates bit 13, so the desktop keeps scheduling self-play itself.
    let (_, caps) = negotiate(6, 6, both, Capabilities::MASCOT_INTERACTION);
    assert!(!caps.contains(Capabilities::IDLE_LIFE_V1));
}
