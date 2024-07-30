use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    type Item;
    async fn foo<T>(&self, x: &T) -> i32;
}

fn main() {}
