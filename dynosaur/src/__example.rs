//! Example generated code.
//!
//! [`Next`] is defined as:
//! ```
//! # use dynosaur::dynosaur;
//! #[dynosaur(pub DynNext = dyn(box) Next)]
//! pub trait Next {
//!     type Item;
//!     async fn next(&self) -> Option<Self::Item>;
//! }
//! # fn main() {}
//! ```
//!
//! which generates the struct [`DynNext`].

#![allow(async_fn_in_trait)]

use crate::dynosaur;

/// Example trait
#[dynosaur(pub DynNext = dyn(box) Next)]
pub trait Next {
    type Item;
    async fn next(&self) -> Option<Self::Item>;
}
