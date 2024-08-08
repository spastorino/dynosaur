[![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GHA Status]][GitHub Actions] ![License]

The core goal is to make `async fn` in trait and other RPITIT usable
with dynamic dispatch via a proc macro, in a way similar to how it works
with async_trait, but while continuing to allow static dispatch.

The core idea of how it works is described here:
https://rust-lang.github.io/async-fundamentals-initiative/explainer/async_fn_in_dyn_trait/hardcoding_box.html

Given a trait like:

```rust,ignore
use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    type Item;
    async fn foo(&self) -> Self::Item;
}
```

Instead of using a type like `Box<dyn MyTrait>` which wouldn't compile
today, we would have the proc macro create a type `DynMyTrait` with this
interface:

```rust,ignore
impl<'a, Item> DynMyTrait<'a, Item> {
    fn new<T>(value: T) -> DynMyTrait<'a, Item>
    where
        T: MyTrait<Item = Item> + 'a,
        Item: 'a,
    {
        unimplemented!()
    }
}

impl<'a, Item> MyTrait for DynMyTrait<'a, Item> {
    type Item = Item;
    fn foo(&self) -> impl std::future::Future<Output = Self::Item> { /* return Pin<Box<dyn Future<Output = Item> here */ }
}
```

The struct itself would look something like this. A full explanation is
in the links and in the exposition below.

```rust
struct DynMyTrait<'a, Item> {
    ptr: *mut (dyn ErasedMyTrait<Item = Item> + 'a),
}

trait ErasedMyTrait {
    type Item;
    fn foo<'a>(&'a self) -> ::core::pin::Pin<Box<dyn ::core::future::Future<Output = Self::Item> + 'a>>;
}
```

A full example including macro output is here:
https://github.com/nikomatsakis/dyner/blob/main/src/async_iter.rs.
Notes:

* The `ErasedAsyncIter` trait is used to have the compiler generate a
  vtable for each concrete type that is used to create a `DynAsyncIter`.
  It is not part of the public interface. `*mut dyn ErasedAsyncIter` is
  a fat pointer.
* Note the use of `Ref` and `RefMut` wrapper types (which would go in
  some support library) so that we can also have
  `DynAsyncIter::from_ref` and `DynAsyncIter::from_ref_mut`. These
  wrappers are slightly verbose, but due to their deref impls, can be
  reborrowed to create `&DynMyTrait` and `&mut DynMyTrait` respectively.
* This code uses GATs instead of RPITIT in the original trait, since
  those weren't stable yet, but the same ideas should apply.
* This code makes use of a union to do bit-twiddling on the data
  pointer, as a way of marking whether the underlying value is owned.
  This is not well-defined ~and should _not_ be necessary, because we
  can instead have `Ref<DynAsyncIter>`'s field be
  `ManuallyDrop<DynAsyncIter>`~ EDIT: Actually we probably do need
  something like this if we have `RefMut` which gives out `&mut
  DynAsyncIter`; we just need to make it well-defined by using std
  pointer APIs.

#### License and usage notes

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

[GitHub Actions]: https://github.com/spastorino/impl-trait-utils/actions
[GHA Status]: https://github.com/spastorino/impl-trait-utils/actions/workflows/rust.yml/badge.svg
[crates.io]: https://crates.io/crates/trait-variant
[Latest Version]: https://img.shields.io/crates/v/dynosaur.svg
[Documentation]: https://img.shields.io/docsrs/dynosaur
[docs.rs]: https://docs.rs/dynosaur
[License]: https://img.shields.io/crates/l/dynosaur.svg
