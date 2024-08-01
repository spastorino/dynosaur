use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait<T> {
    const N: i32;
    type Item;
    async fn foo(&self, x: &T) -> i32;
}

fn main() {}
