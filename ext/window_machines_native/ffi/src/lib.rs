//! Thin event-time boundary adapters; no Ruby payloads are retained.
mod classes;
mod init;
mod sessions;
mod watermark;

use std::{cell::RefCell, num::NonZeroU64};

use classes::classes;
use magnus::{Error, Integer, Ruby, Symbol, TryConvert, Value, value::ReprValue};
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

/// Runs `act` on a copy of the cell's value and keeps the copy only once the
/// answer `act` builds exists, so a raise while building it (a failed
/// allocation, a redefined `new`) changes nothing. No `RefCell` guard is alive
/// while Ruby runs. A build that froze `owner` meanwhile raises
/// `FrozenError`, one that changed the value (a redefined `new` calling back
/// in) `RuntimeError`; neither applies the copy.
fn on_copy<T: Clone + PartialEq, R>(
    ruby: &Ruby,
    owner: impl ReprValue,
    cell: &RefCell<T>,
    act: impl FnOnce(&mut T) -> Result<R, Error>,
) -> Result<R, Error> {
    owner.check_frozen()?;
    let before = cell.borrow().clone();
    let mut after = before.clone();
    let answer = act(&mut after)?;
    owner.check_frozen()?;
    let mut held = cell.borrow_mut();
    if *held != before {
        return Err(Error::new(
            ruby.exception_runtime_error(),
            "the sessions changed while the answer was built; the call was not applied",
        ));
    }
    *held = after;
    Ok(answer)
}

/// A `WindowMachines::Window` (exactly), its members read natively and
/// re-validated through the core: a forged value raises.
#[derive(Clone, Copy)]
struct Interval(Window);

impl TryConvert for Interval {
    fn try_convert(value: Value) -> Result<Self, Error> {
        let ruby = Ruby::get_with(value);
        let (start, end) = classes::pair(&ruby, value, classes(&ruby)?.window)?;
        checked_window(&ruby, start, end).map(Self)
    }
}

/// The core's `Window::new`: what `Window.new` is held to.
fn checked_window(ruby: &Ruby, start: Tick, end: Tick) -> Result<Window, Error> {
    Window::new(start.0, end.0).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "a window's start must be below its end",
        )
    })
}

/// The core's `Session::new`: what `Session.new` is held to.
fn checked_session(ruby: &Ruby, first: Tick, last: Tick) -> Result<Session, Error> {
    Session::new(first.0, last.0).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "a session's first must not be above its last",
        )
    })
}

/// A `WindowMachines::Session` (exactly), its members read natively and
/// re-validated through the core; `TypeError` for anything else.
fn session_from(ruby: &Ruby, value: Value) -> Result<Session, Error> {
    let (first, last) = classes::pair(ruby, value, classes(ruby)?.session)?;
    checked_session(ruby, first, last)
}

/// Private hook behind `Window.new`.
fn validate_window(ruby: &Ruby, start: Tick, end: Tick) -> Result<(), Error> {
    checked_window(ruby, start, end).map(|_| ())
}

/// Private hook behind `Session.new`.
fn validate_session(ruby: &Ruby, first: Tick, last: Tick) -> Result<(), Error> {
    checked_session(ruby, first, last).map(|_| ())
}

/// Private hook behind `Opened.new`, `Extended.new` and `Covered.new`.
fn validate_inserting(ruby: &Ruby, session: Value) -> Result<(), Error> {
    session_from(ruby, session).map(|_| ())
}

/// `Session#covers?`: the core's `Session::covers`.
fn session_covers(ruby: &Ruby, this: Value, timestamp: Tick) -> Result<bool, Error> {
    session_from(ruby, this).map(|session| session.covers(timestamp.0))
}

/// Ruby symbol name of a refusal: the snake case of the core variant.
const fn refusal_name(refused: Refusal) -> &'static str {
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
        Err(refused) => refusal_name(refused),
    })
}

/// The refusal symbol, as an answer.
fn refusal(ruby: &Ruby, refused: Refusal) -> Value {
    ruby.to_symbol(refusal_name(refused)).as_value()
}

fn window(ruby: &Ruby, window: Window) -> Result<Value, Error> {
    let class = classes(ruby)?.window;
    classes::build(ruby, class, (window.start(), window.end()))
}

fn session(ruby: &Ruby, session: Session) -> Result<Value, Error> {
    let class = classes(ruby)?.session;
    classes::build(ruby, class, (session.first(), session.last()))
}

fn tumbling(ruby: &Ruby, timestamp: Tick, width: Span) -> Result<Value, Error> {
    match window_machines::tumbling(timestamp.0, width.0) {
        Ok(found) => window(ruby, found),
        Err(refused) => Ok(refusal(ruby, refused)),
    }
}

fn sliding(ruby: &Ruby, timestamp: Tick, size: Span, slide: Span) -> Result<Value, Error> {
    let mut out = [Window::default(); CAPACITY];
    let written = match window_machines::sliding(timestamp.0, size.0, slide.0, &mut out) {
        Ok(written) => written,
        Err(refused) => return Ok(refusal(ruby, refused)),
    };
    let result = ruby.ary_new_capa(written);
    for found in out.into_iter().take(written) {
        result.push(window(ruby, found)?)?;
    }
    Ok(result.as_value())
}
