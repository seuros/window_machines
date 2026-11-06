use std::cell::RefCell;

use magnus::{Error, RArray, RModule, Ruby, function, method, prelude::*, typed_data::Obj};
use window_machines::{Insert, Sessions};

use crate::{Tick, positive};

#[magnus::wrap(class = "WindowMachines::Sessions", free_immediately, size)]
struct Wrapper(RefCell<Sessions<32>>);

impl Wrapper {
    fn new(ruby: &Ruby, gap: Tick) -> Result<Self, Error> {
        Ok(Self(RefCell::new(Sessions::new(positive(ruby, gap)?))))
    }

    fn insert(ruby: &Ruby, this: Obj<Self>, time: Tick) -> Result<RArray, Error> {
        this.check_frozen()?;
        let result = this.0.borrow_mut().insert(time.0);
        // No Ruby allocation while a RefCell guard is alive.
        match result {
            Insert::Accepted(session) => Ok(ruby.ary_from_iter([session.first, session.last])),
            Insert::Late => Err(Error::new(
                ruby.exception_arg_error(),
                "event at/below watermark",
            )),
            Insert::Full => Err(Error::new(
                ruby.exception_range_error(),
                "32 session capacity reached",
            )),
            _ => Err(Error::new(
                ruby.exception_range_error(),
                "session endpoint overflow",
            )),
        }
    }

    fn advance(this: Obj<Self>, time: Tick) -> Result<bool, Error> {
        this.check_frozen()?;
        Ok(this.0.borrow_mut().advance_watermark(time.0))
    }

    fn take_closed(this: Obj<Self>) -> Result<Option<(u64, u64)>, Error> {
        this.check_frozen()?;
        let session = this.0.borrow_mut().take_closed();
        Ok(session.map(|s| (s.first, s.last)))
    }
}

pub fn define(ruby: &Ruby, module: RModule) -> Result<(), Error> {
    let class = module.define_class("Sessions", ruby.class_object())?;
    class.define_singleton_method("new", function!(Wrapper::new, 1))?;
    class.define_method("insert", method!(Wrapper::insert, 1))?;
    class.define_method("advance_watermark", method!(Wrapper::advance, 1))?;
    class.define_method("take_closed", method!(Wrapper::take_closed, 0))
}
