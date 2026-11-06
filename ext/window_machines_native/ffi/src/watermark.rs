use std::cell::RefCell;

use magnus::{Error, RModule, Ruby, function, method, prelude::*, typed_data::Obj};
use window_machines::{Watermark, Window};

use crate::Tick;

#[magnus::wrap(class = "WindowMachines::Watermark", free_immediately, size)]
struct Wrapper(RefCell<Watermark>);

impl Wrapper {
    fn new() -> Self {
        Self(RefCell::new(Watermark::default()))
    }

    fn advance(this: Obj<Self>, time: Tick) -> Result<bool, Error> {
        this.check_frozen()?;
        Ok(this.0.borrow_mut().advance(time.0))
    }

    fn value(&self) -> Option<u64> {
        self.0.borrow().value()
    }

    fn closes(
        ruby: &Ruby,
        this: &Self,
        start: Tick,
        end: Tick,
        lateness: Tick,
    ) -> Result<bool, Error> {
        if start.0 >= end.0 || end.0.checked_add(lateness.0).is_none() {
            return Err(Error::new(
                ruby.exception_arg_error(),
                "invalid interval/lateness",
            ));
        }
        Ok(this.0.borrow().closes(
            Window {
                start: start.0,
                end: end.0,
            },
            lateness.0,
        ))
    }
}

pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = module.define_class("Watermark", ruby.class_object())?;
    class.define_singleton_method("new", function!(Wrapper::new, 0))?;
    class.define_method("advance", method!(Wrapper::advance, 1))?;
    class.define_method("value", method!(Wrapper::value, 0))?;
    class.define_method("closes?", method!(Wrapper::closes, 3))
}
