//! T031 — sequence policy: duplicate, gap, and wraparound classified as valid decoded frames
//! (docs/architecture.md, wire protocol), not as malformed bytes.

use kivori_protocol::{SeqClass, SequenceTracker};

#[test]
fn first_then_ok_then_gap() {
    let mut t = SequenceTracker::new();
    assert_eq!(t.classify(5), SeqClass::First);
    assert_eq!(t.classify(6), SeqClass::Ok);
    assert_eq!(t.classify(10), SeqClass::Gap(3)); // expected 7, got 10 -> skipped 7,8,9
    assert_eq!(t.last(), Some(10));
}

#[test]
fn duplicate_does_not_advance() {
    let mut t = SequenceTracker::new();
    assert_eq!(t.classify(100), SeqClass::First);
    assert_eq!(t.classify(100), SeqClass::Duplicate);
    assert_eq!(t.classify(100), SeqClass::Duplicate);
    // Resumes from the last accepted value, not from the duplicates.
    assert_eq!(t.classify(101), SeqClass::Ok);
    assert_eq!(t.last(), Some(101));
}

#[test]
fn wraparound_is_ok_not_a_gap() {
    let mut t = SequenceTracker::new();
    assert_eq!(t.classify(0xFFFF), SeqClass::First);
    assert_eq!(t.classify(0x0000), SeqClass::Ok); // u16 wraps 0xFFFF -> 0x0000
    assert_eq!(t.classify(0x0001), SeqClass::Ok);
}

#[test]
fn default_tracker_starts_empty() {
    let t = SequenceTracker::default();
    assert_eq!(t.last(), None);
}
