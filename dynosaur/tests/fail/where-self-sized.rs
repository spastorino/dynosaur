use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    async fn foo(&mut self)
    where
        Self: Sized;
    async fn bar(&mut self);
}

fn main() {}
