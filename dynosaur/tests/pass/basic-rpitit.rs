use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    fn foo(&self) -> impl Send;
}

fn main() {}
