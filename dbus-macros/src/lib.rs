//! This crate implements macros to make using [`dbus`](https://docs.rs/dbus) less verbose.

use attribute_derive::FromAttr;
use manyhow::manyhow;
use signature::Signature;

mod derive;
mod signature;

#[manyhow(proc_macro_derive(Arg, attributes(dbus)))]
pub use derive::arg;
// ArgAll
// Append
// AppendAll
// Get
// ReadAll
// RefArg
// DictKey

#[derive(FromAttr)]
#[attribute(ident = dbus)]
struct Dbus {
    signature: Option<Signature>,
    as_struct: bool,
}
