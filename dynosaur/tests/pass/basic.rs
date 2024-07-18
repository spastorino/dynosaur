use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    type Item;
    async fn foo(&self) -> Self::Item;
}

fn main() {}
