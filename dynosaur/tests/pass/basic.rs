use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}
