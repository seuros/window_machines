use magnus::{Error, RClass, Ruby, function, method, prelude::*};

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("WindowMachines")?;
    module.const_set("CAPACITY", crate::CAPACITY)?;
    crate::classes::resolve(ruby, module)?;
    module.define_singleton_method("tumbling", function!(crate::tumbling, 2))?;
    module.define_singleton_method("sliding", function!(crate::sliding, 3))?;
    // The Data classes are defined in Ruby; the core validates them here.
    module.define_singleton_method("__window", function!(crate::validate_window, 2))?;
    module.define_singleton_method("__session", function!(crate::validate_session, 2))?;
    module.define_singleton_method("__inserting", function!(crate::validate_inserting, 1))?;
    let session: RClass = module.const_get("Session")?;
    session.define_method("covers?", method!(crate::session_covers, 1))?;
    crate::watermark::define(ruby, module)?;
    crate::sessions::define(ruby, module)
}
