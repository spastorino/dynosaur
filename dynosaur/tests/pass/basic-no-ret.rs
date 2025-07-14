use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    async fn foo(&self);
}

fn main() {}
