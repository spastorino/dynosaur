use core::future::Future;
use dynosaur::dynosaur;

trait Foo {}

impl Foo for Box<dyn Foo + '_> {}

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    fn foo(&self, _: impl Foo) -> i32;
    async fn bar(&self, _: impl Foo) -> i32;
    fn baz(&self, _: impl Future<Output = i32>) -> i32;
}

fn main() {}
