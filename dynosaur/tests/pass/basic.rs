use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    async fn foo(&self) -> i32;
}

fn main() {}