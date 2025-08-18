#![doc = include_str!("../README.md")]
#![allow(dead_code)]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(
    missing_debug_implementations,
    rust_2024_compatibility,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    unreachable_pub,
)]

mod any_ref;
mod mutex;
mod utils;

mod collections;
mod tests;

pub use any_ref::{AnyRef, Downcast, WeakAnyRef};
pub use collections::AtomicVec;
pub(crate) use mutex::Backoff;
pub use mutex::{Mutex, WatchGuardMut, WatchGuardRef, WatchGuard};
pub use utils::{create_raw_pointer, dealloc_layout, dealloc_raw_pointer};
