use dynosaur::dynosaur;

#[dynosaur(pub(crate) DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}
