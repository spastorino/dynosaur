[![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GHA Status]][GitHub Actions] ![License]

Currently Rust does not support dynamic dispatch on traits that use `async fn` or methods returning `impl Trait`. Dynosaur is a proc macro that allows dynamic dispatch on these traits but uses static dispatch otherwise. It requires at least Rust 1.75.

This removes the need for the use of the `async_trait` proc macro, giving users the performance benefits of static dispatch without giving up the flexibility of dynamic dispatch.

```rust,ignore
#[dynosaur::dynosaur(DynNext = dyn(box))]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
```

The macro above generates a type called `DynNext` which can be used like this:

```rust,ignore
async fn dyn_dispatch(iter: &mut DynNext<'_, i32>) {
    while let Some(item) = iter.next().await {
        println!("- {item}");
    }
}

let v = [1, 2, 3];
dyn_dispatch(&mut DynNext::boxed(my_next_iter)).await;
dyn_dispatch(DynNext::from_mut(&mut my_next_iter)).await;
```

Where `my_next_iter` is a value of a type that you would create that implements `Next`. For example:

```rust,ignore
struct MyNextIter<T: Copy> {
    v: Vec<T>,
    i: usize,
}

impl<T: Copy> Next for MyNextIter<T> {
    type Item = T;

    async fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.v.len() {
            return None;
        }
        do_some_async_work().await;
        let item = self.v[self.i];
        self.i += 1;
        Some(item)
    }
}

async fn do_some_async_work() {
     // do something :)
}
```

The general rule is that anywhere you would write `dyn Trait` (which would result in a compiler error), you instead write `DynTrait`. Methods using `impl Trait` box their return types when dispatched dynamically, but not when dispatched statically.

You can find more details in the [API docs][docs.rs].

For more examples, take a look at [`dynosaur/examples`](dynosaur/examples) and [`dynosaur/tests/pass`](dynosaur/tests/pass). In tests you would find `.rs` files with what the user would write and `.stdout` files with what dynosaur generates.

## What will it take to implement this support in Rust?

There are many design questions to be answered before building this support into the language. You can find more background here: <https://smallcultfollowing.com/babysteps/blog/2025/03/25/dyn-you-have-idea-for-dyn/>.

## How does this crate solve this issue?

Given a trait `MyTrait`, this crate generates a struct called `DynMyTrait` that implements `MyTrait` by delegating to the actual impls on the concrete type and wrapping the result in a box. For more details on what is exactly generated you may want to check `.stdout` files in [`dynosaur/tests/pass`](dynosaur/tests/pass).

#### License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.

[GitHub Actions]: https://github.com/spastorino/dynosaur/actions
[GHA Status]: https://github.com/spastorino/dynosaur/actions/workflows/ci.yaml/badge.svg
[crates.io]: https://crates.io/crates/dynosaur
[Latest Version]: https://img.shields.io/crates/v/dynosaur.svg
[Documentation]: https://img.shields.io/docsrs/dynosaur
[docs.rs]: https://docs.rs/dynosaur
[License]: https://img.shields.io/crates/l/dynosaur.svg
