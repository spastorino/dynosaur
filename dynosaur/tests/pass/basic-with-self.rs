use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    type Item;
    async fn foo(&self) -> Self::Item;
}

fn main() {}
