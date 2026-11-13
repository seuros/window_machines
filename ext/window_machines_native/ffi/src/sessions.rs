use std::cell::RefCell;

use magnus::{Error, RModule, Ruby, Symbol, Value, function, method, prelude::*, typed_data::Obj};
use window_machines::{Inserting, Sessions};

use crate::{CAPACITY, Span, Tick, advancing, data, refusal, session, watermark};

#[magnus::wrap(class = "WindowMachines::Sessions", free_immediately, size)]
struct Wrapper(RefCell<Sessions<CAPACITY>>);

impl Wrapper {
    const fn new(gap: Span) -> Self {
        Self(RefCell::new(Sessions::new(gap.0)))
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

    fn insert(ruby: &Ruby, this: Obj<Self>, timestamp: Tick) -> Result<Value, Error> {
        this.check_frozen()?;
        // Resolve the class before inserting, so a failure changes nothing.
        let class = data(ruby, "Session")?;
        // No Ruby allocation while a RefCell guard is alive.
        let outcome = this.0.borrow_mut().insert(timestamp.0);
        let (name, held) = match outcome {
            Ok(Inserting::Opened(held)) => ("Opened", held),
            Ok(Inserting::Extended(held)) => ("Extended", held),
            Ok(Inserting::Covered(held)) => ("Covered", held),
            Err(refused) => return Ok(ruby.to_symbol(refusal(refused)).as_value()),
        };
        data(ruby, name)?.funcall("new", (session(class, held)?,))
    }

    fn advance_watermark(ruby: &Ruby, this: Obj<Self>, value: Tick) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let outcome = this.0.borrow_mut().advance_watermark(value.0);
        Ok(advancing(ruby, outcome))
    }

    fn take_closed(ruby: &Ruby, this: Obj<Self>) -> Result<Option<Value>, Error> {
        this.check_frozen()?;
        // Resolve the class before draining, so a failure loses nothing.
        let class = data(ruby, "Session")?;
        let closed = this.0.borrow_mut().take_closed();
        closed.map(|closed| session(class, closed)).transpose()
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
