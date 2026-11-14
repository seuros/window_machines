use std::cell::RefCell;

use magnus::{Error, RModule, Ruby, Symbol, Value, function, method, prelude::*, typed_data::Obj};
use matryona_machines::ruby::integer;
use window_machines::Watermark;

use crate::{advancing, interval};

#[magnus::wrap(class = "WindowMachines::Watermark", free_immediately, size)]
pub struct Wrapper(RefCell<Watermark>);

impl Wrapper {
    pub const fn holding(watermark: Watermark) -> Self {
        Self(RefCell::new(watermark))
    }

    fn new() -> Self {
        Self::holding(Watermark::default())
    }

    fn at(value: Value) -> Result<Self, Error> {
        Ok(Self::holding(Watermark::at(integer(value, "value")?)))
    }

    fn advance(ruby: &Ruby, this: Obj<Self>, value: Value) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let value = integer(value, "value")?;
        let outcome = this.0.borrow_mut().advance(value);
        Ok(advancing(ruby, outcome))
    }

    fn value(&self) -> Option<u64> {
        self.0.borrow().value()
    }

    fn closes(
        ruby: &Ruby,
        this: &Self,
        window: Value,
        allowed_lateness: Value,
    ) -> Result<bool, Error> {
        let window = interval(ruby, window)?;
        let lateness = integer(allowed_lateness, "allowed_lateness")?;
        Ok(this.0.borrow().closes(window, lateness))
    }
}

pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = module.define_class("Watermark", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Wrapper::new, 0))?;
    class.define_singleton_method("at", function!(Wrapper::at, 1))?;
    class.define_method("advance", method!(Wrapper::advance, 1))?;
    class.define_method("value", method!(Wrapper::value, 0))?;
    class.define_method("closes?", method!(Wrapper::closes, 2))
}
