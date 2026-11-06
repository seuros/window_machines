#![no_std]
#![doc = include_str!("../README.md")]
use core::num::NonZeroU64;

mod sessions;
pub use sessions::{Insert, Session, Sessions};

/// Half-open event-time interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// Inclusive start.
    pub start: u64,
    /// Exclusive end.
    pub end: u64,
}

/// Zero-aligned tumbling window containing this event.
#[must_use]
pub const fn tumbling(timestamp: u64, width: NonZeroU64) -> Option<Window> {
    let start = timestamp - timestamp % width.get();
    match start.checked_add(width.get()) {
        Some(end) => Some(Window { start, end }),
        None => None,
    }
}

/// All nonnegative zero-aligned sliding windows containing this event.
#[must_use]
pub fn sliding<const N: usize>(
    timestamp: u64,
    size: NonZeroU64,
    slide: NonZeroU64,
) -> Option<[Option<Window>; N]> {
    let mut result = [None; N];
    let mut start = timestamp - timestamp % slide.get();
    let mut count = 0;
    loop {
        let end = start.checked_add(size.get())?;
        if end <= timestamp {
            break;
        }
        if count == N {
            return None;
        }
        result[count] = Some(Window { start, end });
        count += 1;
        let Some(previous) = start.checked_sub(slide.get()) else {
            break;
        };
        start = previous;
    }
    Some(result)
}

/// Monotonic event-time progress, not a wall clock or consensus watermark.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Watermark {
    value: Option<u64>,
}
impl Watermark {
    /// Advance or repeat progress; reject backwards observations.
    #[must_use]
    pub fn advance(&mut self, value: u64) -> bool {
        if self.value.is_some_and(|old| value < old) {
            return false;
        }
        self.value = Some(value);
        true
    }
    /// Last observation, or `None` before initialization.
    #[must_use]
    pub const fn value(self) -> Option<u64> {
        self.value
    }
    /// Whether event-time progress closes this interval plus allowed lateness.
    #[must_use]
    pub fn closes(self, window: Window, allowed_lateness: u64) -> bool {
        window
            .end
            .checked_add(allowed_lateness)
            .is_some_and(|end| self.value.is_some_and(|value| value >= end))
    }
}
#[cfg(test)]
mod tests;
