use dynosaur::dynosaur;

#[dynosaur(pub(crate) DynMyTrait = dyn(box))]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}
