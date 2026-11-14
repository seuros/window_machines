extern crate std;
use super::*;
use matryona_machines::testing::{refuses, unchanged};

mod model;

fn nz(x: u64) -> NonZeroU64 {
    NonZeroU64::new(x).expect("positive")
}
const fn window(start: u64, end: u64) -> Window {
    Window::new(start, end).expect("start < end")
}
const fn session(first: u64, last: u64) -> Session {
    Session::new(first, last).expect("first <= last")
}
/// A tracker with `gap` that honoured an insert of every timestamp given.
fn filled<const N: usize>(gap: u64, timestamps: &[u64]) -> Sessions<N> {
    let mut sessions = Sessions::new(nz(gap));
    for &timestamp in timestamps {
        assert!(sessions.insert(timestamp).is_ok());
    }
    sessions
}
/// A buffer no real window equals (`Window::new` refuses it), to prove what a
/// call left untouched.
const MARK: Window = Window { start: 77, end: 7 };

#[test]
fn values_are_valid_or_do_not_exist() {
    assert_eq!(Window::new(20, 10), None);
    assert_eq!(Window::new(10, 10), None);
    let found = window(10, 20);
    assert_eq!((found.start(), found.end()), (10, 20));
    assert_eq!(Window::new(0, u64::MAX).map(Window::end), Some(u64::MAX));
    assert_eq!(Session::new(5, 3), None);
    let held = session(3, 5);
    assert_eq!((held.first(), held.last()), (3, 5));
    assert_eq!(Session::new(7, 7).map(Session::first), Some(7));
    assert!(held.covers(3) && held.covers(5));
    assert!(!held.covers(2) && !held.covers(6));
}

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
    // (timestamp, size, slide, buffer length, windows written)
    let cases: [(u64, u64, u64, usize, &[Window]); 5] = [
        (10, 10, 5, 4, &[window(10, 20), window(5, 15)]),
        // An exactly sized buffer is enough.
        (10, 10, 5, 2, &[window(10, 20), window(5, 15)]),
        // Windows never start below zero.
        (0, 10, 5, 4, &[window(0, 10)]),
        (7, 20, 5, 4, &[window(5, 25), window(0, 20)]),
        // A slide wider than the size leaves holes: nothing contains 12.
        (12, 1, 5, 0, &[]),
    ];
    for (timestamp, size, slide, len, expected) in cases {
        let mut out = [MARK; 4];
        let written = sliding(timestamp, nz(size), nz(slide), &mut out[..len]);
        assert_eq!(written, Ok(expected.len()));
        assert_eq!(out[..expected.len()], *expected);
        assert!(out[expected.len()..].iter().all(|slot| *slot == MARK));
    }
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

    assert_eq!(
        unchanged(&mut watermark, |w| w.advance(10)),
        Ok(Advancing::Repeated)
    );
    refuses(&mut watermark, Refusal::Regressed, |w| w.advance(9));
    assert_eq!(watermark.value(), Some(10));

    assert_eq!(watermark.advance(u64::MAX), Ok(Advancing::Advanced));
    assert_eq!(watermark.advance(u64::MAX), Ok(Advancing::Repeated));
    // A closing point past the end of event time is never reached.
    assert!(!watermark.closes(window(0, 10), u64::MAX));
    assert!(watermark.closes(window(0, u64::MAX), 0));
    // Only the end is read, the placeholder's too.
    assert!(watermark.closes(Window::default(), 0));

    // Built at a value: equal to one advanced to it, and held to it.
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
    refuses(&mut sessions, Refusal::Full, |s| s.insert(30));
    assert_eq!(sessions.len(), 2);
    // 5 is within the gap of both: one session, one slot freed.
    assert_eq!(sessions.insert(5), Ok(Inserting::Extended(session(0, 10))));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions.insert(15), Ok(Inserting::Extended(session(0, 15))));
    assert_eq!(sessions.advance_watermark(20), Ok(Advancing::Advanced));
    assert_eq!(sessions.watermark(), Watermark::at(20));

    refuses(&mut sessions, Refusal::Late, |s| s.insert(20));
    refuses(&mut sessions, Refusal::Late, |s| s.insert(16));
    refuses(&mut sessions, Refusal::Overflow, |s| s.insert(u64::MAX));
    refuses(&mut sessions, Refusal::Overflow, |s| s.insert(u64::MAX - 4));
    refuses(&mut sessions, Refusal::Regressed, |s| {
        s.advance_watermark(19)
    });
    assert_eq!(
        unchanged(&mut sessions, |s| s.advance_watermark(20)),
        Ok(Advancing::Repeated)
    );

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
    let mut sessions = filled::<2>(5, &[0, 5, 10]);
    let held = session(0, 10);
    // Endpoints and interior alike: the boundaries already say so.
    for timestamp in [0, 5, 7, 10] {
        assert_eq!(
            unchanged(&mut sessions, |s| s.insert(timestamp)),
            Ok(Inserting::Covered(held))
        );
    }
    assert_eq!(Inserting::Covered(held).session(), held);

    // Below the watermark the session still covers it, closed or not.
    assert_eq!(sessions.advance_watermark(7), Ok(Advancing::Advanced));
    assert_eq!(sessions.insert(5), Ok(Inserting::Covered(held)));
    assert_eq!(sessions.advance_watermark(15), Ok(Advancing::Advanced));
    for timestamp in [5, 10] {
        assert_eq!(
            unchanged(&mut sessions, |s| s.insert(timestamp)),
            Ok(Inserting::Covered(held))
        );
    }
    // Outside the boundaries a late event would have to move them: refused.
    refuses(&mut sessions, Refusal::Late, |s| s.insert(11));

    // Drained: no history is kept, so the same replay is now late.
    assert_eq!(sessions.take_closed(), Some(held));
    refuses(&mut sessions, Refusal::Late, |s| s.insert(5));
}

#[test]
fn a_full_table_still_honours_covered_and_joining_events() {
    let mut sessions = filled::<2>(1, &[0, 10]);
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
    let mut sessions = filled::<3>(2, &[20, 0, 10]);
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
fn refusals_display_distinctly() {
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
}
