//! Thin event-time boundary adapters; no Ruby payloads are retained.
mod init;
mod sessions;
mod watermark;

use std::num::NonZeroU64;

use magnus::{Error, Integer, RArray, Ruby, TryConvert, Value};

#[derive(Clone, Copy)]
struct Tick(u64);

impl TryConvert for Tick {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "time must be an Integer in 0..2**64",
            ));
        }
        u64::try_convert(value).map(Self)
    }
}

fn positive(ruby: &Ruby, value: Tick) -> Result<NonZeroU64, Error> {
    NonZeroU64::new(value.0)
        .ok_or_else(|| Error::new(ruby.exception_arg_error(), "width/gap must be positive"))
}

fn tumbling(ruby: &Ruby, time: Tick, width: Tick) -> Result<(u64, u64), Error> {
    let window = window_machines::tumbling(time.0, positive(ruby, width)?)
        .ok_or_else(|| Error::new(ruby.exception_range_error(), "window endpoint overflow"))?;
    Ok((window.start, window.end))
}

fn sliding(ruby: &Ruby, time: Tick, width: Tick, slide: Tick) -> Result<RArray, Error> {
    let windows =
        window_machines::sliding::<32>(time.0, positive(ruby, width)?, positive(ruby, slide)?)
            .ok_or_else(|| {
                Error::new(
                    ruby.exception_range_error(),
                    "endpoint overflow or more than 32 sliding windows",
                )
            })?;
    Ok(ruby.ary_from_iter(windows.into_iter().flatten().map(|w| (w.start, w.end))))
}
