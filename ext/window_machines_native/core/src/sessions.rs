use crate::{Advancing, Refusal, Watermark};
use core::num::NonZeroU64;

/// Boundaries of a session; both endpoints are actual included event timestamps.
///
/// The `Default` value is a placeholder, not a sentinel.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Session {
    /// Earliest event.
    pub first: u64,
    /// Latest event.
    pub last: u64,
}
impl Session {
    /// Whether the timestamp lies between the two endpoints, inclusive.
    #[must_use]
    pub const fn covers(self, timestamp: u64) -> bool {
        self.first <= timestamp && timestamp <= self.last
    }
}

/// What an honoured [`Sessions::insert`] did, with the session the event is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inserting {
    /// The event joined no held session: a new one-event session took a slot.
    Opened(Session),
    /// The event moved boundaries: it grew one held session, or bridged
    /// several into the one returned. Adapters must merge every payload whose
    /// old session lies inside the returned boundaries.
    Extended(Session),
    /// A held session already covered the timestamp: nothing changed.
    ///
    /// The core keeps boundaries, not events, so this proves the timestamp
    /// is inside the session and not that the same event was seen before;
    /// telling a replay from a second event at that time is the caller's job.
    Covered(Session),
}
impl Inserting {
    /// The session the event belongs to, whichever way it got there.
    #[must_use]
    pub const fn session(self) -> Session {
        match self {
            Self::Opened(session) | Self::Extended(session) | Self::Covered(session) => session,
        }
    }
}

/// Bounded sessions for one logical event key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sessions<const N: usize> {
    entries: [Option<Session>; N],
    gap: NonZeroU64,
    watermark: Watermark,
}
impl<const N: usize> Sessions<N> {
    /// Construct an empty one-key session tracker.
    ///
    /// `gap` is the longest silence, in event-time units, two neighbouring
    /// events of one session may have between them.
    #[must_use]
    pub const fn new(gap: NonZeroU64) -> Self {
        Self {
            entries: [None; N],
            gap,
            watermark: Watermark { value: None },
        }
    }

    /// The inactivity gap fixed at construction.
    #[must_use]
    pub const fn gap(&self) -> NonZeroU64 {
        self.gap
    }

    /// Sessions that can be held at once.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Sessions currently held, closed-but-undrained ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    /// Whether no session is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Event-time progress of this tracker, a copy.
    #[must_use]
    pub const fn watermark(&self) -> Watermark {
        self.watermark
    }

    /// Merge an event atomically, including bridges across multiple sessions.
    ///
    /// A timestamp a held session already covers is the effect asked for
    /// already holding: [`Inserting::Covered`], whatever the watermark says,
    /// and nothing changes. That holds only while the session is held: once
    /// [`take_closed`](Self::take_closed) drained it no history remains and
    /// the same timestamp is [`Refusal::Late`].
    ///
    /// # Errors
    /// [`Refusal::Late`] at or below the watermark with no held session
    /// covering the timestamp, [`Refusal::Overflow`] when `timestamp + gap`
    /// would pass `u64::MAX`, [`Refusal::Full`] without a free slot or a
    /// session to join. Nothing changes.
    pub fn insert(&mut self, timestamp: u64) -> Result<Inserting, Refusal> {
        if let Some(session) = self
            .entries
            .iter()
            .flatten()
            .find(|session| session.covers(timestamp))
        {
            return Ok(Inserting::Covered(*session));
        }
        if self
            .watermark
            .value()
            .is_some_and(|value| timestamp <= value)
        {
            return Err(Refusal::Late);
        }
        if timestamp.checked_add(self.gap.get()).is_none() {
            return Err(Refusal::Overflow);
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
        let joined = merged.iter().position(|merged| *merged);
        let Some(slot) = joined.or_else(|| self.entries.iter().position(Option::is_none)) else {
            return Err(Refusal::Full);
        };
        for (index, merged) in merged.into_iter().enumerate() {
            if merged {
                self.entries[index] = None;
            }
        }
        self.entries[slot] = Some(candidate);
        Ok(if joined.is_some() {
            Inserting::Extended(candidate)
        } else {
            Inserting::Opened(candidate)
        })
    }

    /// Advance or repeat event-time progress, as [`Watermark::advance`] does.
    ///
    /// Advancing never removes sessions; drain them with
    /// [`take_closed`](Self::take_closed).
    ///
    /// # Errors
    /// [`Refusal::Regressed`] when `value` is behind the watermark. Nothing
    /// changes.
    pub const fn advance_watermark(&mut self, value: u64) -> Result<Advancing, Refusal> {
        self.watermark.advance(value)
    }

    /// Drain the earliest closed session; payload emission remains the caller's job.
    ///
    /// A session is closed once the watermark has reached `last + gap`.
    /// `None` means nothing is closed yet, not a refusal.
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
