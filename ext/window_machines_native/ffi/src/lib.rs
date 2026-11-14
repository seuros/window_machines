//! Thin event-time boundary adapters; no Ruby payloads are retained.
mod classes;
mod init;
mod sessions;
mod watermark;

use classes::classes;
use magnus::{Error, Ruby, Symbol, Value, value::ReprValue};
use matryona_machines::ruby::{Symbols, integer, positive, symbol};
use window_machines::{Advancing, Refusal, Session, Window};

/// Sessions one tracker holds, and windows one `sliding` call can answer.
const CAPACITY: usize = 32;

/// A `WindowMachines::Window` (exactly), its members read natively and
/// re-validated through the core: a forged value raises.
fn interval(ruby: &Ruby, value: Value) -> Result<Window, Error> {
    let window = classes(ruby)?.window;
    let [start, end] = classes::pair(ruby, value, window, "expected a WindowMachines::Window")?;
    checked_window(ruby, start, end)
}

/// The core's `Window::new`: what `Window.new` is held to.
fn checked_window(ruby: &Ruby, start: Value, end: Value) -> Result<Window, Error> {
    let (start, end) = (integer(start, "start")?, integer(end, "end")?);
    Window::new(start, end).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "a window's start must be below its end",
        )
    })
}

/// The core's `Session::new`: what `Session.new` is held to.
fn checked_session(ruby: &Ruby, first: Value, last: Value) -> Result<Session, Error> {
    let (first, last) = (integer(first, "first")?, integer(last, "last")?);
    Session::new(first, last).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "a session's first must not be above its last",
        )
    })
}

/// A `WindowMachines::Session` (exactly), its members read natively and
/// re-validated through the core; `TypeError` for anything else.
fn session_from(ruby: &Ruby, value: Value) -> Result<Session, Error> {
    let session = classes(ruby)?.session;
    let [first, last] = classes::pair(ruby, value, session, "expected a WindowMachines::Session")?;
    checked_session(ruby, first, last)
}

/// Private hook behind `Window.new`.
fn validate_window(ruby: &Ruby, start: Value, end: Value) -> Result<(), Error> {
    checked_window(ruby, start, end).map(|_| ())
}

/// Private hook behind `Session.new`.
fn validate_session(ruby: &Ruby, first: Value, last: Value) -> Result<(), Error> {
    checked_session(ruby, first, last).map(|_| ())
}

/// Private hook behind `Opened.new`, `Extended.new` and `Covered.new`.
fn validate_inserting(ruby: &Ruby, session: Value) -> Result<(), Error> {
    session_from(ruby, session).map(|_| ())
}

/// `Session#covers?`: the core's `Session::covers`.
fn session_covers(ruby: &Ruby, this: Value, timestamp: Value) -> Result<bool, Error> {
    let timestamp = integer(timestamp, "timestamp")?;
    session_from(ruby, this).map(|session| session.covers(timestamp))
}

matryona_machines::symbols! {
    /// The core's refusals, each the snake case of its variant.
    Refusals for Refusal as "a refusal" {
        Overflow => "overflow",
        Undersized => "undersized",
        Regressed => "regressed",
        Late => "late",
        Full => "full",
    }
}

matryona_machines::symbols! {
    /// An honoured watermark advance.
    Advancings for Advancing as "an advance" {
        Advanced => "advanced",
        Repeated => "repeated",
    }
}

fn advancing(ruby: &Ruby, outcome: Result<Advancing, Refusal>) -> Symbol {
    symbol(ruby, outcome.map_or_else(Refusals::name, Advancings::name))
}

/// The refusal symbol, as an answer.
fn refusal(ruby: &Ruby, refused: Refusal) -> Value {
    Refusals::symbol(ruby, refused).as_value()
}

fn window(ruby: &Ruby, window: Window) -> Result<Value, Error> {
    let class = classes(ruby)?.window;
    classes::build(ruby, class, (window.start(), window.end()))
}

fn session(ruby: &Ruby, session: Session) -> Result<Value, Error> {
    let class = classes(ruby)?.session;
    classes::build(ruby, class, (session.first(), session.last()))
}

fn tumbling(ruby: &Ruby, timestamp: Value, width: Value) -> Result<Value, Error> {
    let (timestamp, width) = (integer(timestamp, "timestamp")?, positive(width, "width")?);
    match window_machines::tumbling(timestamp, width) {
        Ok(found) => window(ruby, found),
        Err(refused) => Ok(refusal(ruby, refused)),
    }
}

fn sliding(ruby: &Ruby, timestamp: Value, size: Value, slide: Value) -> Result<Value, Error> {
    let timestamp = integer(timestamp, "timestamp")?;
    let (size, slide) = (positive(size, "size")?, positive(slide, "slide")?);
    let mut out = [Window::default(); CAPACITY];
    let written = match window_machines::sliding(timestamp, size, slide, &mut out) {
        Ok(written) => written,
        Err(refused) => return Ok(refusal(ruby, refused)),
    };
    let result = ruby.ary_new_capa(written);
    for found in out.into_iter().take(written) {
        result.push(window(ruby, found)?)?;
    }
    Ok(result.as_value())
}
