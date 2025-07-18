use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    fn foo(&mut self) -> impl ::core::future::Future<Output = ()>
    where
        Self: Sized;
    async fn bar(&mut self);
}

fn main() {}
