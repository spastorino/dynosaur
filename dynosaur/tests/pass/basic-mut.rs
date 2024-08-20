use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    async fn foo(&mut self);
}

fn main() {}
