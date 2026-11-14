use std::cell::RefCell;

use magnus::{Error, RModule, Ruby, Symbol, Value, function, method, prelude::*, typed_data::Obj};
use matryona_machines::ruby::{integer, keep, positive};
use window_machines::{Inserting, Sessions};

use crate::{CAPACITY, advancing, classes, refusal, session, watermark};

#[magnus::wrap(class = "WindowMachines::Sessions", free_immediately, size)]
struct Wrapper(RefCell<Sessions<CAPACITY>>);

impl Wrapper {
    fn new(gap: Value) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Sessions::new(positive(gap, "gap")?))))
    }

    fn gap(&self) -> u64 {
        self.0.borrow().gap().get()
    }

    fn capacity(&self) -> usize {
        self.0.borrow().capacity()
    }

    fn size(&self) -> usize {
        self.0.borrow().len()
    }

    fn empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    /// A copy, as the core answers it: advancing it does not move the tracker.
    fn watermark(&self) -> watermark::Wrapper {
        watermark::Wrapper::holding(self.0.borrow().watermark())
    }

    /// Answers through [`keep`]: a raise while building the answer changes
    /// nothing, a freeze meanwhile raises `FrozenError`, a re-entrant change
    /// `RuntimeError`.
    fn insert(ruby: &Ruby, this: Obj<Self>, timestamp: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let timestamp = integer(timestamp, "timestamp")?;
        let known = classes::classes(ruby)?;
        keep(
            ruby,
            this,
            &this.0,
            |next| next.insert(timestamp),
            |inserted| {
                let (class, held) = match inserted {
                    Ok(Inserting::Opened(held)) => (known.opened, held),
                    Ok(Inserting::Extended(held)) => (known.extended, held),
                    Ok(Inserting::Covered(held)) => (known.covered, held),
                    Err(refused) => return Ok(refusal(ruby, refused)),
                };
                classes::build(ruby, class, (session(ruby, held)?,))
            },
        )
    }

    fn advance_watermark(ruby: &Ruby, this: Obj<Self>, value: Value) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let value = integer(value, "value")?;
        let outcome = this.0.borrow_mut().advance_watermark(value);
        Ok(advancing(ruby, outcome))
    }

    /// Answers through [`keep`]: a raise while building the session leaves
    /// it held, to be taken again; a freeze meanwhile raises `FrozenError`, a
    /// re-entrant change `RuntimeError`.
    fn take_closed(ruby: &Ruby, this: Obj<Self>) -> Result<Option<Value>, Error> {
        classes::classes(ruby)?;
        keep(ruby, this, &this.0, Sessions::take_closed, |closed| {
            closed.map(|closed| session(ruby, closed)).transpose()
        })
    }
}

pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = module.define_class("Sessions", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Wrapper::new, 1))?;
    class.define_method("gap", method!(Wrapper::gap, 0))?;
    class.define_method("capacity", method!(Wrapper::capacity, 0))?;
    class.define_method("size", method!(Wrapper::size, 0))?;
    class.define_method("empty?", method!(Wrapper::empty, 0))?;
    class.define_method("watermark", method!(Wrapper::watermark, 0))?;
    class.define_method("insert", method!(Wrapper::insert, 1))?;
    class.define_method("advance_watermark", method!(Wrapper::advance_watermark, 1))?;
    class.define_method("take_closed", method!(Wrapper::take_closed, 0))
}
