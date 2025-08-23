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
    rust_2024_compatibility,
    rust_2018_idioms,
    rustdoc::broken_intra_doc_links,
    unreachable_pub,
)]

mod any_ref;
pub mod mutex;
pub mod utils;

pub mod collections;

#[cfg(test)]
mod test;
mod arw;

pub use any_ref::{AnyRef, WeakAnyRef};
pub use arw::{Arw, WeakArw};

