use dynosaur::dynosaur;

trait Foo {}

impl Foo for Box<dyn Foo + '_> {}

#[dynosaur(DynMyTrait)]
trait MyTrait {
    fn foo(&self, _: impl Foo) -> i32;
    async fn bar(&self, _: impl Foo) -> i32;
}

fn main() {}
