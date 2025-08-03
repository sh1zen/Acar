#![doc = include_str!("../README.md")]
//#![warn(missing_docs)]

mod any_ref;
mod mutex;
mod utils;

mod tests;

pub use any_ref::{AnyRef, Downcast, WeakAnyRef};
pub use mutex::{Mutex, WatchGuard};