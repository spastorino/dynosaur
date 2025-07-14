use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}
