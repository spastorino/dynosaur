use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    type Item;
    async fn foo(&self, x: &i32) -> i32;
}

fn main() {}
