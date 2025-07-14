use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn)]
trait MyTrait {
    async fn foo(&self) -> i32;
}

#[dynosaur(DynMyTrait2 = dyn MyTrait2)]
trait MyTrait2 {
    async fn foo(&self) -> i32;
}

fn main() {}
