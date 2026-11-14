//! The gem's `Data` classes, resolved once when the extension loads.
//!
//! Sealing in Ruby stops accidents, not reflection: a subclass constant is
//! bound before `inherited` raises, `Data#initialize` can be bound directly
//! and `Marshal.load` skips `initialize`. So the binding is the authority: a
//! value it reads must be an instance of exactly the sealed class, its members
//! are read natively (no method is dispatched) and the caller re-validates
//! them through the core constructor.
use magnus::{Error, RModule, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Class, Registry, exact_fields};

/// `WindowMachines::Window`, `Session`, `Opened`, `Extended` and `Covered`.
pub struct Classes {
    pub window: Class,
    pub session: Class,
    pub opened: Class,
    pub extended: Class,
    pub covered: Class,
}

static CLASSES: Registry<Classes> = Registry::new("window_machines");

/// Resolve and pin the classes Ruby defined before loading the extension.
pub fn resolve(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = |name| Class::resolve(ruby, module, name);
    let classes = Classes {
        window: class("Window")?,
        session: class("Session")?,
        opened: class("Opened")?,
        extended: class("Extended")?,
        covered: class("Covered")?,
    };
    CLASSES.set(ruby, classes).map(drop)
}

/// The classes resolved at init.
pub fn classes(ruby: &Ruby) -> Result<&'static Classes, Error> {
    CLASSES.get(ruby)
}

/// A new value of `class`, through its own `new` (so through its check).
pub fn build<A: magnus::ArgList>(ruby: &Ruby, class: Class, args: A) -> Result<Value, Error> {
    class.get(ruby).funcall("new", args)
}

/// The two members of `value`, read from the struct (no reader runs), when
/// `value` is an instance of exactly `class` (`instance_of?`, not `is_a?`);
/// `TypeError` with `message` otherwise.
pub fn pair(
    ruby: &Ruby,
    value: Value,
    class: Class,
    message: &'static str,
) -> Result<[Value; 2], Error> {
    exact_fields::<2>(ruby, value, class.get(ruby), message)
}
