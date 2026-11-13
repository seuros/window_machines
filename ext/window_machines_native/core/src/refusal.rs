//! The one refusal type of the crate.

use core::fmt;

/// Why a call was not honoured. A refused call changed nothing.
///
/// [`tumbling`](crate::tumbling) answers [`Overflow`](Self::Overflow);
/// [`sliding`](crate::sliding) answers [`Overflow`](Self::Overflow) or
/// [`Undersized`](Self::Undersized);
/// [`Watermark::advance`](crate::Watermark::advance) and
/// [`Sessions::advance_watermark`](crate::Sessions::advance_watermark) answer
/// [`Regressed`](Self::Regressed);
/// [`Sessions::insert`](crate::Sessions::insert) answers [`Late`](Self::Late),
/// [`Overflow`](Self::Overflow) or [`Full`](Self::Full).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A window end, or a session's closing point `timestamp + gap`, would
    /// pass `u64::MAX`.
    Overflow,
    /// The output buffer is shorter than the windows containing the
    /// timestamp; nothing was written rather than a truncated list.
    Undersized,
    /// The value is behind the watermark; a watermark never moves back.
    Regressed,
    /// The timestamp is at or below the watermark and no held session covers
    /// it.
    Late,
    /// No free slot and no held session the event could join.
    Full,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Overflow => "endpoint would pass the end of event time",
            Self::Undersized => "output buffer is shorter than the windows to write",
            Self::Regressed => "value is behind the watermark",
            Self::Late => "timestamp is at or below the watermark",
            Self::Full => "no free session slot",
        })
    }
}

impl core::error::Error for Refusal {}
