use std::cell::RefCell;

use magnus::{Error, RModule, Ruby, Symbol, function, method, prelude::*, typed_data::Obj};
use window_machines::Watermark;

use crate::{Interval, Tick, advancing};

#[magnus::wrap(class = "WindowMachines::Watermark", free_immediately, size)]
pub struct Wrapper(RefCell<Watermark>);

impl Wrapper {
    pub const fn holding(watermark: Watermark) -> Self {
        Self(RefCell::new(watermark))
    }

    fn new() -> Self {
        Self::holding(Watermark::default())
    }

    const fn at(value: Tick) -> Self {
        Self::holding(Watermark::at(value.0))
    }

    fn advance(ruby: &Ruby, this: Obj<Self>, value: Tick) -> Result<Symbol, Error> {
        this.check_frozen()?;
        let outcome = this.0.borrow_mut().advance(value.0);
        Ok(advancing(ruby, outcome))
    }

    fn value(&self) -> Option<u64> {
        self.0.borrow().value()
    }

    fn closes(&self, window: Interval, allowed_lateness: Tick) -> bool {
        self.0.borrow().closes(window.0, allowed_lateness.0)
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
