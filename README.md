[![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GHA Status]][GitHub Actions] ![License]

dynosaur lets you use dynamic dispatch on traits with with `async fn` and
methods returning `impl Trait`.

```rust,ignore
#[dynosaur::dynosaur(DynNext)]
trait Next {
    type Item;
    async fn next(&self) -> Self::Item;
}
```

The macro above generates a type called `DynNext` which can be used like this:

```rust,ignore
async fn dyn_dispatch(iter: &mut DynNext<'_, i32>) {
    while let Some(item) = iter.next().await {
        println!("- {item}");
    }
}

let a = [1, 2, 3];
dyn_dispatch(DynNext::from_mut(&mut a.into_iter())).await;
```

The general rule is that anywhere you would write `dyn Trait` (which would
result in a compiler error), you instead write `DynTrait`.

Methods returning `impl Trait` box their return types when dispatched
dynamically, but not when dispatched statically.

#### License and usage notes

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

[GitHub Actions]: https://github.com/spastorino/dynosaur/actions
[GHA Status]: https://github.com/spastorino/dynosaur/actions/workflows/rust.yml/badge.svg
[crates.io]: https://crates.io/crates/dynosaur
[Latest Version]: https://img.shields.io/crates/v/dynosaur.svg
[Documentation]: https://img.shields.io/docsrs/dynosaur
[docs.rs]: https://docs.rs/dynosaur
[License]: https://img.shields.io/crates/l/dynosaur.svg
