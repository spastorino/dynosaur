use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    async fn foo(&mut self)
    where
        Self: Sized;
    async fn bar(&mut self);
}

fn main() {}
