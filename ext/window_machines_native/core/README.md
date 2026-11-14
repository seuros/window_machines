# window-machines

> **AMS Akidau**, NCC-4219, *Akidau*-class survey vessel.

Engine-room flight manual: divide observation time into bounded survey windows.

Allocation-free `no_std` window boundaries and one-key bounded session tracking.

## Time

Every timestamp, watermark and span between them (window `width`, `size`,
`slide`, session `gap`, `allowed_lateness`) is a `u64` of **event time** in one
caller-selected unit and origin. Event time is not a monotonic clock, so none of
it is a `core::time::Duration`; the core reads no clock at all.

## Windows

Tumbling and sliding windows are aligned to zero and half-open `[start, end)`.
`tumbling(timestamp, width)` returns `Result<Window, Refusal>`.
`sliding(timestamp, size, slide, out: &mut [Window])` writes the windows
containing the timestamp into the first `n` entries of `out`, newest start
first, leaves the rest untouched and returns `Ok(n)`; `Ok(0)` is a timestamp in
a hole between windows. `Window` implements `Default` so a buffer is
`[Window::default(); N]`; the default is a placeholder, not a sentinel.

## Values

`Window` and `Session` keep their fields private and are valid or do not
exist. `Window::new(start, end)` is `None` unless `start < end` (an interval
holding no timestamp is not a window); `Session::new(first, last)` is `None`
unless `first <= last`. Readers: `start()`, `end()`, `first()`, `last()`. The
only value built otherwise is the `Default` placeholder of an output buffer.

## Watermarks

`Watermark::default()` has observed nothing; `Watermark::at(value)` already
stands at a value (restored progress). `advance(value)` returns
`Result<Advancing, Refusal>`: `Advancing::Advanced` when it moved forward or
took its first value, `Advancing::Repeated` when it already held exactly that
value. `closes(window, allowed_lateness)` is a pure query: true once the
watermark reaches `window.end() + allowed_lateness`. It reads only `end`, and a
sum past `u64::MAX` is a point no watermark reaches, so it is `false`.

## Sessions

`Sessions<N>` merges events whose inactivity gaps are **at most** the configured
gap, including out-of-order bridging events. `insert(timestamp)` returns
`Result<Inserting, Refusal>`; every `Inserting` variant carries the `Session`
the event is in:

- `Opened`: joined no held session; a new one took a slot.
- `Extended`: grew one held session or bridged several into one.
- `Covered`: a held session already covered the timestamp; nothing changed.

Session allowed lateness is zero. Advancing the watermark does not discard
sessions: drain them with `take_closed()`, earliest first, `None` when nothing
is closed (a drain, not a refusal). A session is closed once the watermark
reaches `last + gap`. Readers: `gap`, `capacity`, `len`, `is_empty`,
`watermark` (a copy).

## Contract

- **One `Refusal` enum, and a refusal changes nothing.** `out` included.

  | Call | Refusals |
  | --- | --- |
  | `tumbling` | `Overflow` |
  | `sliding` | `Overflow`, `Undersized` |
  | `Watermark::advance`, `Sessions::advance_watermark` | `Regressed` |
  | `Sessions::insert` | `Late`, `Overflow`, `Full` |

  `Overflow`: a window end, or `timestamp + gap`, would pass `u64::MAX`.
  `Undersized`: `out` is shorter than the windows to write; nothing is
  truncated. `Regressed`: the value is behind the watermark. `Late`: the
  timestamp is at or below the watermark and no held session covers it.
  `Full`: no free slot and no held session to join; nothing is evicted.
- **Repeats are honoured when provable.** A watermark repeated at its own value
  is `Advancing::Repeated`. A timestamp inside a held session's boundaries is
  `Inserting::Covered`, at or below the watermark too: the effect asked for
  already holds and the boundaries prove it. The core stores boundaries, not
  events, so `Covered` does not prove the *same event* was seen; deduplicating
  events is the caller's job. Once `take_closed` drained a session no history is
  kept, and a replay of its timestamps is `Late`.
- **Exhaustive.** No public enum is `#[non_exhaustive]`; a new variant is a
  breaking change.
- **No clock is recorded.** Every value here is event time; there is no `now`
  and no monotonic clock. `Watermark::advance` records the watermark it was
  given (progress, not a clock) when it answers `Advanced`, and nothing on
  `Repeated` or a refusal.
- **No unreachable arms.** The core has no `panic!`, `assert!`,
  `unreachable!` or `expect` outside tests.
- **No issued ids, no scopes.** Nothing here hands out handles. To resume after
  a restart, rebuild a watermark with `Watermark::at` and re-insert sessions.

Callers retain aggregation payloads and apply the same merges; the core stores
boundaries only. Multi-key indexing, retractions, and distributed watermark
coordination are not implemented.

```rust
use core::num::NonZeroU64;
use window_machines::{
    Advancing, Inserting, Refusal, Session, Sessions, Watermark, Window, sliding, tumbling,
};

// Hand-built values go through the validating constructors.
let win = |start, end| Window::new(start, end).ok_or(Refusal::Overflow);
let span = |first, last| Session::new(first, last).ok_or(Refusal::Overflow);
let ten = NonZeroU64::new(10).ok_or(Refusal::Overflow)?;
let five = NonZeroU64::new(5).ok_or(Refusal::Overflow)?;

assert_eq!(tumbling(12, ten)?, win(10, 20)?);
let mut out = [Window::default(); 4];
let written = sliding(12, ten, five, &mut out)?;
assert_eq!(out[..written], [win(10, 20)?, win(5, 15)?]);

let mut watermark = Watermark::default();
assert_eq!(watermark.advance(22), Ok(Advancing::Advanced));
assert_eq!(watermark.advance(22), Ok(Advancing::Repeated));
assert_eq!(watermark.advance(9), Err(Refusal::Regressed));
assert!(watermark.closes(out[0], 2));

let mut sessions = Sessions::<4>::new(five);
assert_eq!(sessions.insert(10)?, Inserting::Opened(span(10, 10)?));
assert_eq!(sessions.insert(15)?, Inserting::Extended(span(10, 15)?));
assert_eq!(sessions.insert(12)?, Inserting::Covered(span(10, 15)?));
assert_eq!(sessions.advance_watermark(20), Ok(Advancing::Advanced));
assert_eq!(sessions.insert(12)?, Inserting::Covered(span(10, 15)?));
assert_eq!(sessions.insert(16), Err(Refusal::Late));
assert_eq!(sessions.take_closed(), Some(span(10, 15)?));
assert_eq!(sessions.insert(12), Err(Refusal::Late));
# Ok::<(), Refusal>(())
```

The Ruby gem mirrors `Window` and `Session` as `Data` values of the same names
and fields, validated at construction by `Window::new` and `Session::new`, the `Inserting` variants as `Opened`, `Extended` and `Covered`
(`Data` with a `session`), `Advancing` and every `Refusal` as symbols.
