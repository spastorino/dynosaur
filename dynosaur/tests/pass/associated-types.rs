trait Baz {}

#[dynosaur::dynosaur(DynFoo)]
trait Foo {
    type Bar: Baz;

    async fn foo(&self) -> Self::Bar;
}
