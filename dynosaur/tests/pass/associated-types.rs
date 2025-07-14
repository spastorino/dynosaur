trait Baz {}

#[dynosaur::dynosaur(DynFoo = dyn(box) Foo)]
trait Foo {
    type Bar: Baz;

    async fn foo(&self) -> Self::Bar;
}
