//! Random operation streams checked against a naive model.
use super::*;
use matryona_machines::testing::XorShift64;
use std::vec::Vec;

type Spans = Vec<(u128, u128)>;

/// Naive model: an unordered list of spans compared with wide arithmetic,
/// rebuilt from scratch on every insert, so it shares no slot or merge logic.
struct Model {
    spans: Spans,
    gap: u128,
    watermark: Option<u64>,
}
impl Model {
    fn insert(&mut self, capacity: usize, timestamp: u64) -> Result<Inserting, Refusal> {
        let at = u128::from(timestamp);
        if let Some(&span) = self.spans.iter().find(|span| span.0 <= at && at <= span.1) {
            return Ok(Inserting::Covered(narrow(span)));
        }
        if self.watermark.is_some_and(|mark| timestamp <= mark) {
            return Err(Refusal::Late);
        }
        if at + self.gap > u128::from(u64::MAX) {
            return Err(Refusal::Overflow);
        }
        let (near, far): (Spans, Spans) = self
            .spans
            .iter()
            .copied()
            .partition(|span| span.0 <= at + self.gap && at <= span.1 + self.gap);
        if near.is_empty() && far.len() == capacity {
            return Err(Refusal::Full);
        }
        let first = near.iter().map(|span| span.0).fold(at, u128::min);
        let last = near.iter().map(|span| span.1).fold(at, u128::max);
        let opened = near.is_empty();
        self.spans = far;
        self.spans.push((first, last));
        let session = narrow((first, last));
        Ok(if opened {
            Inserting::Opened(session)
        } else {
            Inserting::Extended(session)
        })
    }

    fn take_closed(&mut self) -> Option<Session> {
        let mark = u128::from(self.watermark?);
        let index = (0..self.spans.len())
            .filter(|index| self.spans[*index].1 + self.gap <= mark)
            .min_by_key(|index| self.spans[*index].0)?;
        Some(narrow(self.spans.swap_remove(index)))
    }

    fn sorted(&self) -> Vec<Session> {
        let mut spans = self.spans.clone();
        spans.sort_unstable();
        spans.into_iter().map(narrow).collect()
    }
}

fn narrow(span: (u128, u128)) -> Session {
    Session::new(
        u64::try_from(span.0).expect("fits"),
        u64::try_from(span.1).expect("fits"),
    )
    .expect("first <= last")
}

/// Every held session, earliest first, read from a copy pushed to the end of
/// event time. Every session closes there: `last + gap` never overflows.
fn held<const N: usize>(sessions: &Sessions<N>) -> Vec<Session> {
    let mut copy = sessions.clone();
    assert!(copy.advance_watermark(u64::MAX).is_ok());
    core::iter::from_fn(|| copy.take_closed()).collect()
}

/// How a call left the tracker, checked against a clone taken before it.
enum Answer {
    Changed,
    Repeated,
    Refused,
    Unchanged,
}

fn run_model(seed: u64, gap: u64, base: u64, spread: u64) {
    const N: usize = 5;
    let mut draws = XorShift64::new(seed);
    let mut sessions = Sessions::<N>::new(nz(gap));
    let mut model = Model {
        spans: Vec::new(),
        gap: u128::from(gap),
        watermark: None,
    };
    let (mut refused, mut repeated) = (0, 0);
    for _ in 0..4_000 {
        let before = sessions.clone();
        let near = model.watermark.unwrap_or(base);
        let timestamp = base
            .max(near.saturating_sub(spread / 2))
            .saturating_add(draws.below(spread));
        let answer = match draws.below(10) {
            0..=5 => {
                let expected = model.insert(N, timestamp);
                assert_eq!(sessions.insert(timestamp), expected);
                match expected {
                    Ok(Inserting::Opened(_) | Inserting::Extended(_)) => Answer::Changed,
                    Ok(Inserting::Covered(_)) => Answer::Repeated,
                    Err(Refusal::Late | Refusal::Overflow | Refusal::Full) => Answer::Refused,
                    Err(Refusal::Undersized | Refusal::Regressed) => {
                        panic!("insert never answers {expected:?}");
                    }
                }
            }
            6 | 7 => {
                let expected = match model.watermark {
                    Some(mark) if timestamp < mark => Err(Refusal::Regressed),
                    Some(mark) if timestamp == mark => Ok(Advancing::Repeated),
                    Some(_) | None => Ok(Advancing::Advanced),
                };
                assert_eq!(sessions.advance_watermark(timestamp), expected);
                match expected {
                    Ok(Advancing::Advanced) => {
                        model.watermark = Some(timestamp);
                        Answer::Changed
                    }
                    Ok(Advancing::Repeated) => Answer::Repeated,
                    Err(Refusal::Regressed) => Answer::Refused,
                    Err(
                        Refusal::Overflow | Refusal::Undersized | Refusal::Late | Refusal::Full,
                    ) => panic!("advance_watermark never answers {expected:?}"),
                }
            }
            _ => {
                let expected = model.take_closed();
                assert_eq!(sessions.take_closed(), expected);
                if expected.is_some() {
                    Answer::Changed
                } else {
                    Answer::Unchanged
                }
            }
        };
        match answer {
            Answer::Changed => assert_ne!(sessions, before),
            Answer::Repeated => {
                repeated += 1;
                assert_eq!(sessions, before);
            }
            Answer::Refused => {
                refused += 1;
                assert_eq!(sessions, before);
            }
            Answer::Unchanged => assert_eq!(sessions, before),
        }
        assert_eq!(sessions.watermark().value(), model.watermark);
        assert_eq!(sessions.len(), model.spans.len());
        assert_eq!(held(&sessions), model.sorted());
    }
    assert!(refused > 0 && repeated > 0, "seed {seed} exercised nothing");
}

#[test]
fn random_streams_match_the_model() {
    for seed in 1..=40 {
        run_model(seed, 1 + seed % 7, 0, 60);
        run_model(seed ^ 0xABCD, 3, 1_000, 25);
    }
}

#[test]
fn random_streams_match_the_model_at_the_end_of_event_time() {
    for seed in 1..=20 {
        run_model(seed, 1 + seed % 5, u64::MAX - 40, 41);
    }
}
