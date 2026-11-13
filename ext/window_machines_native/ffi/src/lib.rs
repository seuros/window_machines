//! Thin event-time boundary adapters; no Ruby payloads are retained.
mod init;
mod sessions;
mod watermark;

use std::num::NonZeroU64;

use magnus::{Error, Integer, RClass, RModule, Ruby, Symbol, TryConvert, Value, prelude::*};
use window_machines::{Advancing, Refusal, Session, Window};

/// Sessions one tracker holds, and windows one `sliding` call can answer.
const CAPACITY: usize = 32;

/// An event-time timestamp or span: an Integer in `0..2**64`.
#[derive(Clone, Copy)]
struct Tick(u64);

impl TryConvert for Tick {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "event time must be an Integer in 0..2**64",
            ));
        }
        u64::try_convert(value).map(Self)
    }
}

/// A width, size, slide or gap: a `Tick` above zero.
#[derive(Clone, Copy)]
struct Span(NonZeroU64);

impl TryConvert for Span {
    fn try_convert(value: Value) -> Result<Self, Error> {
        NonZeroU64::new(Tick::try_convert(value)?.0)
            .map(Self)
            .ok_or_else(|| {
                Error::new(
                    Ruby::get_with(value).exception_range_error(),
                    "width, size, slide and gap must be positive",
                )
            })
    }
}

fn data(ruby: &Ruby, name: &str) -> Result<Value, Error> {
    let module: RModule = ruby.class_object().const_get("WindowMachines")?;
    module.const_get(name)
}

/// A `WindowMachines::Window`, read field by field so both are validated.
#[derive(Clone, Copy)]
struct Interval(Window);

impl TryConvert for Interval {
    fn try_convert(value: Value) -> Result<Self, Error> {
        let ruby = Ruby::get_with(value);
        let class = RClass::try_convert(data(&ruby, "Window")?)?;
        if !value.is_kind_of(class) {
            return Err(Error::new(
                ruby.exception_type_error(),
                "expected a WindowMachines::Window",
            ));
        }
        let field = |name: &str| -> Result<u64, Error> {
            let raw: Value = value.funcall(name, ())?;
            Tick::try_convert(raw).map(|tick| tick.0)
        };
        Ok(Self(Window {
            start: field("start")?,
            end: field("end")?,
        }))
    }
}

/// Ruby symbol name of a refusal: the snake case of the core variant.
const fn refusal(refused: Refusal) -> &'static str {
    match refused {
        Refusal::Overflow => "overflow",
        Refusal::Undersized => "undersized",
        Refusal::Regressed => "regressed",
        Refusal::Late => "late",
        Refusal::Full => "full",
    }
}

fn advancing(ruby: &Ruby, outcome: Result<Advancing, Refusal>) -> Symbol {
    ruby.to_symbol(match outcome {
        Ok(Advancing::Advanced) => "advanced",
        Ok(Advancing::Repeated) => "repeated",
        Err(refused) => refusal(refused),
    })
}

fn window(class: Value, window: Window) -> Result<Value, Error> {
    class.funcall("new", (window.start, window.end))
}

fn session(class: Value, session: Session) -> Result<Value, Error> {
    class.funcall("new", (session.first, session.last))
}

fn tumbling(ruby: &Ruby, timestamp: Tick, width: Span) -> Result<Value, Error> {
    match window_machines::tumbling(timestamp.0, width.0) {
        Ok(found) => window(data(ruby, "Window")?, found),
        Err(refused) => Ok(ruby.to_symbol(refusal(refused)).as_value()),
    }
}

fn sliding(ruby: &Ruby, timestamp: Tick, size: Span, slide: Span) -> Result<Value, Error> {
    let mut out = [Window::default(); CAPACITY];
    let written = match window_machines::sliding(timestamp.0, size.0, slide.0, &mut out) {
        Ok(written) => written,
        Err(refused) => return Ok(ruby.to_symbol(refusal(refused)).as_value()),
    };
    let class = data(ruby, "Window")?;
    let result = ruby.ary_new_capa(written);
    for found in out.into_iter().take(written) {
        result.push(window(class, found)?)?;
    }
    Ok(result.as_value())
}
