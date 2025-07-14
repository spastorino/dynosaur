use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    async fn foo(&mut self);
}

fn main() {}
