use crate::Watermark;
use core::num::NonZeroU64;

/// Boundaries of a session; both endpoints are actual included event timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    /// Earliest event.
    pub first: u64,
    /// Latest event.
    pub last: u64,
}

/// Session event admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Insert {
    /// New or merged boundaries; adapters must merge overlapping payloads too.
    Accepted(Session),
    /// Event is at/below the current watermark.
    Late,
    /// No free slot and no existing session could absorb the event.
    Full,
    /// Inactivity endpoint overflowed.
    Overflow,
}

/// Bounded sessions for one logical event key.
#[derive(Debug)]
pub struct Sessions<const N: usize> {
    entries: [Option<Session>; N],
    gap: NonZeroU64,
    watermark: Watermark,
}
impl<const N: usize> Sessions<N> {
    /// Construct an empty one-key session tracker.
    #[must_use]
    pub fn new(gap: NonZeroU64) -> Self {
        Self {
            entries: [None; N],
            gap,
            watermark: Watermark::default(),
        }
    }

    /// Merge an event atomically, including bridges across multiple sessions.
    #[must_use]
    pub fn insert(&mut self, timestamp: u64) -> Insert {
        if self
            .watermark
            .value()
            .is_some_and(|value| timestamp <= value)
        {
            return Insert::Late;
        }
        if timestamp.checked_add(self.gap.get()).is_none() {
            return Insert::Overflow;
        }
        let mut candidate = Session {
            first: timestamp,
            last: timestamp,
        };
        let mut merged = [false; N];
        // Fixed point handles any slot order and transitive bridges.
        for _ in 0..N {
            let mut changed = false;
            for (index, session) in self.entries.iter().enumerate() {
                let Some(session) = session else {
                    continue;
                };
                if !merged[index]
                    && u128::from(candidate.first)
                        <= u128::from(session.last) + u128::from(self.gap.get())
                    && u128::from(session.first)
                        <= u128::from(candidate.last) + u128::from(self.gap.get())
                {
                    merged[index] = true;
                    candidate.first = candidate.first.min(session.first);
                    candidate.last = candidate.last.max(session.last);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let Some(slot) = merged
            .iter()
            .position(|merged| *merged)
            .or_else(|| self.entries.iter().position(Option::is_none))
        else {
            return Insert::Full;
        };
        for (index, merged) in merged.into_iter().enumerate() {
            if merged {
                self.entries[index] = None;
            }
        }
        self.entries[slot] = Some(candidate);
        Insert::Accepted(candidate)
    }

    /// Advance event-time progress, without silently deleting closed sessions.
    #[must_use]
    pub fn advance_watermark(&mut self, value: u64) -> bool {
        self.watermark.advance(value)
    }

    /// Drain the earliest closed session; payload emission remains the caller's job.
    pub fn take_closed(&mut self) -> Option<Session> {
        let value = self.watermark.value()?;
        let index = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, session)| {
                session
                    .filter(|session| {
                        session
                            .last
                            .checked_add(self.gap.get())
                            .is_some_and(|end| value >= end)
                    })
                    .map(|session| (index, session.first))
            })
            .min_by_key(|&(_, first)| first)
            .map(|(index, _)| index)?;
        self.entries[index].take()
    }
}
