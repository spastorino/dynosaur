use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    type Item;
    async fn foo(&self) -> Self::Item;
}

fn main() {}
