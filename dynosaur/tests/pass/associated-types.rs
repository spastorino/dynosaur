trait Baz {}

#[dynosaur::dynosaur(DynFoo = dyn(box))]
trait Foo {
    type Bar: Baz;

    async fn foo(&self) -> Self::Bar;
}
