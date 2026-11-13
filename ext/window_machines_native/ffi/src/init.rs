use magnus::{Error, Ruby, function, prelude::*};

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("WindowMachines")?;
    module.const_set("CAPACITY", crate::CAPACITY)?;
    module.define_singleton_method("tumbling", function!(crate::tumbling, 2))?;
    module.define_singleton_method("sliding", function!(crate::sliding, 3))?;
    crate::watermark::define(ruby, module)?;
    crate::sessions::define(ruby, module)
}
