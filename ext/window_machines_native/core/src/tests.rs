#![allow(clippy::expect_used)]
use super::*;
fn nz(x: u64) -> NonZeroU64 {
    NonZeroU64::new(x).expect("positive")
}

#[test]
fn tumbling_and_sliding_boundaries_are_half_open() {
    assert_eq!(tumbling(10, nz(5)), Some(Window { start: 10, end: 15 }));
    assert_eq!(
        sliding::<3>(10, nz(10), nz(5)),
        Some([
            Some(Window { start: 10, end: 20 }),
            Some(Window { start: 5, end: 15 }),
            None
        ])
    );
    assert_eq!(sliding::<1>(10, nz(10), nz(5)), None);
    assert_eq!(
        sliding::<1>(0, nz(10), nz(5)),
        Some([Some(Window { start: 0, end: 10 })])
    );
    assert_eq!(tumbling(u64::MAX, nz(2)), None);
    assert_eq!(sliding::<0>(0, nz(1), nz(1)), None);
}

#[test]
fn watermarks_are_monotonic_and_allow_lateness() {
    let mut watermark = Watermark::default();
    let window = Window { start: 0, end: 10 };
    assert!(!watermark.closes(window, 0));
    assert!(watermark.advance(10));
    assert!(watermark.closes(window, 0));
    assert!(!watermark.closes(window, 1));
    assert!(!watermark.advance(9));
    assert!(!watermark.closes(window, u64::MAX));
}

#[test]
fn out_of_order_bridge_merges_full_session_tables_without_losing_data() {
    let mut sessions = Sessions::<2>::new(nz(5));
    assert_eq!(
        sessions.insert(0),
        Insert::Accepted(Session { first: 0, last: 0 })
    );
    assert_eq!(
        sessions.insert(10),
        Insert::Accepted(Session {
            first: 10,
            last: 10
        })
    );
    assert_eq!(sessions.insert(30), Insert::Full);
    assert_eq!(
        sessions.insert(5),
        Insert::Accepted(Session { first: 0, last: 10 })
    );
    assert!(sessions.advance_watermark(15));
    assert_eq!(sessions.insert(15), Insert::Late);
    assert_eq!(sessions.take_closed(), Some(Session { first: 0, last: 10 }));
    assert_eq!(sessions.take_closed(), None);
    assert!(!sessions.advance_watermark(14));
    assert_eq!(sessions.insert(u64::MAX), Insert::Overflow);
}
