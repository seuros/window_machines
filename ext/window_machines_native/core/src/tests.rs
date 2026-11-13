#![allow(clippy::expect_used)]
extern crate std;
use super::*;

mod model;

fn nz(x: u64) -> NonZeroU64 {
    NonZeroU64::new(x).expect("positive")
}
const fn window(start: u64, end: u64) -> Window {
    Window { start, end }
}
const fn session(first: u64, last: u64) -> Session {
    Session { first, last }
}
/// A buffer no real window equals, to prove what a call left untouched.
const MARK: Window = window(77, 7);

#[test]
fn tumbling_boundaries_are_half_open_and_overflow_is_refused() {
    assert_eq!(tumbling(10, nz(5)), Ok(window(10, 15)));
    assert_eq!(tumbling(14, nz(5)), Ok(window(10, 15)));
    assert_eq!(tumbling(15, nz(5)), Ok(window(15, 20)));
    assert_eq!(tumbling(u64::MAX, nz(2)), Err(Refusal::Overflow));
    assert_eq!(tumbling(u64::MAX, nz(1)), Err(Refusal::Overflow));
    assert_eq!(
        tumbling(u64::MAX - 1, nz(1)),
        Ok(window(u64::MAX - 1, u64::MAX))
    );
}

#[test]
fn sliding_writes_the_first_n_newest_first_and_leaves_the_rest() {
    let mut out = [MARK; 4];
    assert_eq!(sliding(10, nz(10), nz(5), &mut out), Ok(2));
    assert_eq!(out, [window(10, 20), window(5, 15), MARK, MARK]);
    // Windows never start below zero.
    let mut out = [MARK; 4];
    assert_eq!(sliding(0, nz(10), nz(5), &mut out), Ok(1));
    assert_eq!(out, [window(0, 10), MARK, MARK, MARK]);
    let mut out = [MARK; 4];
    assert_eq!(sliding(7, nz(20), nz(5), &mut out), Ok(2));
    assert_eq!(out[..2], [window(5, 25), window(0, 20)]);
    // A slide wider than the size leaves holes: nothing contains 12.
    let mut out = [MARK; 2];
    assert_eq!(sliding(12, nz(1), nz(5), &mut out), Ok(0));
    assert_eq!(out, [MARK; 2]);
    assert_eq!(sliding(12, nz(1), nz(5), &mut []), Ok(0));
    // An exactly sized buffer is enough.
    let mut out = [MARK; 2];
    assert_eq!(sliding(10, nz(10), nz(5), &mut out), Ok(2));
}

#[test]
fn sliding_refusals_write_nothing() {
    let mut out = [MARK; 1];
    assert_eq!(
        sliding(10, nz(10), nz(5), &mut out),
        Err(Refusal::Undersized)
    );
    assert_eq!(out, [MARK]);
    assert_eq!(sliding(0, nz(1), nz(1), &mut []), Err(Refusal::Undersized));
    let mut out = [MARK; 3];
    assert_eq!(
        sliding(u64::MAX, nz(2), nz(1), &mut out),
        Err(Refusal::Overflow)
    );
    assert_eq!(out, [MARK; 3]);
    assert_eq!(
        sliding(u64::MAX - 1, nz(u64::MAX), nz(1), &mut out),
        Err(Refusal::Overflow)
    );
    // More windows than any buffer could hold is still just undersized.
    assert_eq!(
        sliding(u64::MAX / 2, nz(u64::MAX / 2), nz(1), &mut out),
        Err(Refusal::Undersized)
    );
    assert_eq!(out, [MARK; 3]);
}

#[test]
fn sliding_agrees_with_a_brute_force_scan() {
    for size in 1..=9 {
        for slide in 1..=9 {
            for timestamp in 0..40 {
                let mut expected = std::vec::Vec::new();
                for start in (0..=timestamp).rev().filter(|start| start % slide == 0) {
                    if start + size > timestamp {
                        expected.push(window(start, start + size));
                    }
                }
                let mut out = [MARK; 9];
                let written = sliding(timestamp, nz(size), nz(slide), &mut out);
                assert_eq!(written, Ok(expected.len()));
                assert_eq!(out[..expected.len()], expected[..]);
                assert!(out[expected.len()..].iter().all(|slot| *slot == MARK));
                if let Some(short) = expected.len().checked_sub(1) {
                    let mut out = [MARK; 9];
                    assert_eq!(
                        sliding(timestamp, nz(size), nz(slide), &mut out[..short]),
                        Err(Refusal::Undersized)
                    );
                    assert_eq!(out, [MARK; 9]);
                }
            }
        }
    }
}

#[test]
fn watermark_advances_repeats_and_refuses_regression() {
    let mut watermark = Watermark::default();
    assert_eq!(watermark.value(), None);
    assert!(!watermark.closes(window(0, 10), 0));
    assert_eq!(watermark.advance(10), Ok(Advancing::Advanced));
    assert!(watermark.closes(window(0, 10), 0));
    assert!(!watermark.closes(window(0, 10), 1));

    let before = watermark;
    assert_eq!(watermark.advance(10), Ok(Advancing::Repeated));
    assert_eq!(watermark, before);
    assert_eq!(watermark.advance(9), Err(Refusal::Regressed));
    assert_eq!(watermark, before);
    assert_eq!(watermark.value(), Some(10));

    assert_eq!(watermark.advance(u64::MAX), Ok(Advancing::Advanced));
    assert_eq!(watermark.advance(u64::MAX), Ok(Advancing::Repeated));
    // A closing point past the end of event time is never reached.
    assert!(!watermark.closes(window(0, 10), u64::MAX));
    assert!(watermark.closes(window(0, u64::MAX), 0));
    // Only the end is read; a malformed window is the caller's.
    assert!(watermark.closes(window(9, 3), 0));
}

#[test]
fn a_watermark_built_at_a_value_equals_one_advanced_to_it() {
    let mut advanced = Watermark::default();
    assert_eq!(advanced.advance(0), Ok(Advancing::Advanced));
    assert_eq!(Watermark::at(0), advanced);
    assert_ne!(Watermark::at(0), Watermark::default());
    let mut restored = Watermark::at(8);
    assert_eq!(restored.advance(8), Ok(Advancing::Repeated));
    assert_eq!(restored.advance(7), Err(Refusal::Regressed));
    assert_eq!(restored, Watermark::at(8));
}

#[test]
fn out_of_order_bridge_merges_full_session_tables_without_losing_data() {
    let mut sessions = Sessions::<2>::new(nz(5));
    assert_eq!((sessions.gap(), sessions.capacity()), (nz(5), 2));
    assert!(sessions.is_empty());
    assert_eq!(sessions.insert(0), Ok(Inserting::Opened(session(0, 0))));
    assert_eq!(sessions.insert(10), Ok(Inserting::Opened(session(10, 10))));
    let before = sessions.clone();
    assert_eq!(sessions.insert(30), Err(Refusal::Full));
    assert_eq!(sessions, before);
    assert_eq!(sessions.len(), 2);
    // 5 is within the gap of both: one session, one slot freed.
    assert_eq!(sessions.insert(5), Ok(Inserting::Extended(session(0, 10))));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions.insert(15), Ok(Inserting::Extended(session(0, 15))));
    assert_eq!(sessions.advance_watermark(20), Ok(Advancing::Advanced));
    assert_eq!(sessions.watermark(), Watermark::at(20));

    let before = sessions.clone();
    assert_eq!(sessions.insert(20), Err(Refusal::Late));
    assert_eq!(sessions.insert(16), Err(Refusal::Late));
    assert_eq!(sessions.insert(u64::MAX), Err(Refusal::Overflow));
    assert_eq!(sessions.insert(u64::MAX - 4), Err(Refusal::Overflow));
    assert_eq!(sessions.advance_watermark(19), Err(Refusal::Regressed));
    assert_eq!(sessions, before);
    assert_eq!(sessions.advance_watermark(20), Ok(Advancing::Repeated));
    assert_eq!(sessions, before);

    assert_eq!(sessions.take_closed(), Some(session(0, 15)));
    assert_eq!(sessions.take_closed(), None);
    assert!(sessions.is_empty());
    assert_eq!(
        sessions.insert(u64::MAX - 5),
        Ok(Inserting::Opened(session(u64::MAX - 5, u64::MAX - 5)))
    );
}

#[test]
fn a_covered_timestamp_is_honoured_while_its_session_is_held() {
    let mut sessions = Sessions::<2>::new(nz(5));
    for timestamp in [0, 5, 10] {
        assert!(sessions.insert(timestamp).is_ok());
    }
    let held = session(0, 10);
    let before = sessions.clone();
    // Endpoints and interior alike: the boundaries already say so.
    for timestamp in [0, 5, 7, 10] {
        assert_eq!(sessions.insert(timestamp), Ok(Inserting::Covered(held)));
        assert_eq!(sessions, before);
    }
    assert_eq!(Inserting::Covered(held).session(), held);

    // Below the watermark the session still covers it, closed or not.
    assert_eq!(sessions.advance_watermark(7), Ok(Advancing::Advanced));
    assert_eq!(sessions.insert(5), Ok(Inserting::Covered(held)));
    assert_eq!(sessions.advance_watermark(15), Ok(Advancing::Advanced));
    let before = sessions.clone();
    assert_eq!(sessions.insert(5), Ok(Inserting::Covered(held)));
    assert_eq!(sessions.insert(10), Ok(Inserting::Covered(held)));
    // Outside the boundaries a late event would have to move them: refused.
    assert_eq!(sessions.insert(11), Err(Refusal::Late));
    assert_eq!(sessions, before);

    // Drained: no history is kept, so the same replay is now late.
    assert_eq!(sessions.take_closed(), Some(held));
    let before = sessions.clone();
    assert_eq!(sessions.insert(5), Err(Refusal::Late));
    assert_eq!(sessions, before);
}

#[test]
fn a_full_table_still_honours_covered_and_joining_events() {
    let mut sessions = Sessions::<2>::new(nz(1));
    assert_eq!(sessions.insert(0), Ok(Inserting::Opened(session(0, 0))));
    assert_eq!(sessions.insert(10), Ok(Inserting::Opened(session(10, 10))));
    assert_eq!(
        sessions.insert(11),
        Ok(Inserting::Extended(session(10, 11)))
    );
    assert_eq!(sessions.insert(10), Ok(Inserting::Covered(session(10, 11))));
    assert_eq!(sessions.insert(20), Err(Refusal::Full));
    assert_eq!(Sessions::<0>::new(nz(1)).insert(3), Err(Refusal::Full));
}

#[test]
fn closed_sessions_drain_earliest_first_and_only_when_asked() {
    let mut sessions = Sessions::<3>::new(nz(2));
    for timestamp in [20, 0, 10] {
        assert!(sessions.insert(timestamp).is_ok());
    }
    assert_eq!(sessions.take_closed(), None);
    assert_eq!(sessions.advance_watermark(11), Ok(Advancing::Advanced));
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions.take_closed(), Some(session(0, 0)));
    assert_eq!(sessions.take_closed(), None);
    assert_eq!(sessions.advance_watermark(22), Ok(Advancing::Advanced));
    assert_eq!(sessions.take_closed(), Some(session(10, 10)));
    assert_eq!(sessions.take_closed(), Some(session(20, 20)));
    assert_eq!(sessions.take_closed(), None);
}

#[test]
fn refusals_display_and_defaults_are_placeholders() {
    use std::string::ToString;
    let all = [
        Refusal::Overflow,
        Refusal::Undersized,
        Refusal::Regressed,
        Refusal::Late,
        Refusal::Full,
    ];
    for (index, refusal) in all.iter().enumerate() {
        let error: &dyn core::error::Error = refusal;
        assert_ne!(error.to_string(), "");
        assert!(all[..index].iter().all(|other| other != refusal));
    }
    assert_eq!(Window::default(), window(0, 0));
    assert_eq!(Session::default(), session(0, 0));
    assert!(session(3, 5).covers(3) && session(3, 5).covers(5));
    assert!(!session(3, 5).covers(2) && !session(3, 5).covers(6));
}
