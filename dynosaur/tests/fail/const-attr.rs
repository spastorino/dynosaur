#[dynosaur::dynosaur(DynFoo = dyn(box))]
trait Foo {
    const BAR: i32;

    fn foo(&self) -> impl Send;
}

fn main() {}
