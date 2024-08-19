use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    async fn foo(&self);
}

fn main() {}
