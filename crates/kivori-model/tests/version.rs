//! T019 — protocol version compatibility matrix and capability negotiation (FR-003).

use kivori_model::capabilities::Capabilities;
use kivori_model::version::ProtocolVersion;

#[test]
fn matching_major_is_compatible() {
    let device = ProtocolVersion::new(1, 3);
    assert!(device.is_compatible_with(&[1]));
    assert!(device.is_compatible_with(&[1, 2]));
}

#[test]
fn mismatched_major_is_incompatible() {
    assert!(!ProtocolVersion::new(2, 0).is_compatible_with(&[1]));
    assert!(!ProtocolVersion::new(1, 0).is_compatible_with(&[2, 3]));
    assert!(!ProtocolVersion::new(1, 0).is_compatible_with(&[]));
}

#[test]
fn minor_negotiation_takes_the_lower() {
    let a = ProtocolVersion::new(1, 5);
    let b = ProtocolVersion::new(1, 2);
    assert_eq!(ProtocolVersion::negotiate_minor(a, b), 2);
    assert_eq!(ProtocolVersion::negotiate_minor(b, a), 2);
    assert_eq!(ProtocolVersion::negotiate_minor(a, a), 5);
}

#[test]
fn major_zero_is_an_ordinary_major() {
    // The protocol restarted at 0.1 before the first release (ADR-0014): 0 is the current major,
    // and boards flashed before the reset (major 1) are a different, incompatible major.
    assert!(ProtocolVersion::new(0, 1).is_compatible_with(&[0]));
    assert!(!ProtocolVersion::new(0, 1).is_compatible_with(&[1]));
    assert!(!ProtocolVersion::new(1, 6).is_compatible_with(&[0]));
}

#[test]
fn capability_intersection_is_the_negotiated_set() {
    let desktop = Capabilities::from_bits(0b1011);
    let device = Capabilities::from_bits(0b0110);
    let negotiated = desktop.intersection(device);
    assert_eq!(negotiated.bits(), 0b0010);
    // A feature only either side has is not in the negotiated set.
    assert!(desktop.contains(Capabilities::from_bits(0b0001)));
    assert!(!negotiated.contains(Capabilities::from_bits(0b0001)));
}

#[test]
fn capability_union_contains_and_empty() {
    let a = Capabilities::from_bits(0b0001);
    let b = Capabilities::from_bits(0b0100);
    let u = a.union(b);
    assert_eq!(u.bits(), 0b0101);
    assert!(u.contains(a));
    assert!(u.contains(b));
    assert!(Capabilities::NONE.is_empty());
    assert_eq!(Capabilities::default(), Capabilities::NONE);
}
