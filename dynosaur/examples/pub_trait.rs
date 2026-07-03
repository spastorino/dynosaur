//! Example demonstrating using dynosaur in a library crate with `#![missing_docs]`.

// Deny missing docs to prove that documentation is generated for DynMyTrait.
#![deny(missing_docs)]

use std::future::Future;

use dynosaur::dynosaur;

/// A simple trait to test lint compatibility.
#[dynosaur(pub DynMyTrait = dyn(box) MyTrait)]
pub trait MyTrait {
    /// A simple method to test lint compatability.
    fn foo(&self) -> impl Future<Output = i32>;
}

fn main() {}
