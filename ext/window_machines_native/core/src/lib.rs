#![no_std]
#![doc = include_str!("../README.md")]
use core::num::NonZeroU64;

mod refusal;
mod sessions;

pub use refusal::Refusal;
pub use sessions::{Inserting, Session, Sessions};

/// Half-open event-time interval `[start, end)`.
///
/// The `Default` value is an output-buffer placeholder (`[Window::default(); N]`).
/// It is not a sentinel: read only the entries a call reports.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// Inclusive start.
    pub start: u64,
    /// Exclusive end.
    pub end: u64,
}

/// Zero-aligned tumbling window containing this event.
///
/// `timestamp` and `width` are event time in one caller-defined unit.
///
/// # Errors
/// [`Refusal::Overflow`] when the window's end would pass `u64::MAX`.
pub const fn tumbling(timestamp: u64, width: NonZeroU64) -> Result<Window, Refusal> {
    let start = timestamp - timestamp % width.get();
    match start.checked_add(width.get()) {
        Some(end) => Ok(Window { start, end }),
        None => Err(Refusal::Overflow),
    }
}

/// Write every zero-aligned sliding window containing this event into `out`,
/// newest start first, and return how many were written.
///
/// Windows are `size` wide and start every `slide`. The first `n` entries of
/// `out` are written and the rest left untouched; `Ok(0)` means the timestamp
/// falls in a hole between windows (`slide` wider than `size`).
///
/// # Errors
/// [`Refusal::Overflow`] when the newest window's end would pass `u64::MAX`,
/// [`Refusal::Undersized`] when `out` is shorter than the windows to write.
/// `out` is untouched by a refusal.
pub fn sliding(
    timestamp: u64,
    size: NonZeroU64,
    slide: NonZeroU64,
    out: &mut [Window],
) -> Result<usize, Refusal> {
    let (size, slide) = (size.get(), slide.get());
    let offset = timestamp % slide;
    let newest = timestamp - offset;
    if size <= offset {
        return Ok(0);
    }
    if newest.checked_add(size).is_none() {
        return Err(Refusal::Overflow);
    }
    // Window `k` starts `k` slides before the newest one: it must still reach
    // past the timestamp and must not start below zero.
    let older = ((size - offset - 1) / slide).min(newest / slide);
    let count = usize::try_from(older)
        .ok()
        .and_then(|older| older.checked_add(1))
        .filter(|count| *count <= out.len())
        .ok_or(Refusal::Undersized)?;
    let mut start = newest;
    for slot in &mut out[..count] {
        *slot = Window {
            start,
            end: start + size,
        };
        start = start.saturating_sub(slide);
    }
    Ok(count)
}

/// What an honoured watermark call did. Neither is a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advancing {
    /// The watermark moved forward to the value given, or took its first.
    Advanced,
    /// The watermark already stood exactly there: an idempotent repeat that
    /// changed nothing.
    Repeated,
}

/// Monotonic event-time progress, not a wall clock or consensus watermark.
///
/// The `Default` value has observed nothing yet.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Watermark {
    value: Option<u64>,
}
impl Watermark {
    /// A watermark that already stands at `value`, for progress restored from
    /// saved state or read from another owner.
    #[must_use]
    pub const fn at(value: u64) -> Self {
        Self { value: Some(value) }
    }

    /// Advance or repeat progress.
    ///
    /// [`Advancing::Advanced`] when the watermark moved forward or took its
    /// first value. Repeating the value it already holds is the effect asked
    /// for already holding: [`Advancing::Repeated`], and nothing changes.
    ///
    /// # Errors
    /// [`Refusal::Regressed`] when `value` is behind the watermark. Nothing
    /// changes.
    pub const fn advance(&mut self, value: u64) -> Result<Advancing, Refusal> {
        match self.value {
            Some(held) if value < held => Err(Refusal::Regressed),
            Some(held) if value == held => Ok(Advancing::Repeated),
            Some(_) | None => {
                self.value = Some(value);
                Ok(Advancing::Advanced)
            }
        }
    }

    /// Last observation, or `None` before the first.
    #[must_use]
    pub const fn value(self) -> Option<u64> {
        self.value
    }

    /// Whether event-time progress has reached this window's end plus the
    /// allowed lateness.
    ///
    /// Only `window.end` is read. An `end + allowed_lateness` past `u64::MAX`
    /// is a point no watermark can reach, so the answer is exactly `false`.
    #[must_use]
    pub const fn closes(self, window: Window, allowed_lateness: u64) -> bool {
        match (window.end.checked_add(allowed_lateness), self.value) {
            (Some(end), Some(value)) => value >= end,
            (None, Some(_) | None) | (Some(_), None) => false,
        }
    }
}
#[cfg(test)]
mod tests;
