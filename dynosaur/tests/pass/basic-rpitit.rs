use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    fn foo(&self) -> impl Send;
}

fn main() {}
