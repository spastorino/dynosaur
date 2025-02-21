use dynosaur::dynosaur;

#[dynosaur(pub(crate) DynMyTrait)]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}
