use std::future::Future;

#[dynosaur::dynosaur(DynFoo)]
trait Foo: Send {
    fn foo(&self) -> impl Future<Output = i32> + Send;
}

struct FooImpl;

impl Foo for FooImpl {
    async fn foo(&self) -> i32 {
        1
    }
}

fn main() {}
