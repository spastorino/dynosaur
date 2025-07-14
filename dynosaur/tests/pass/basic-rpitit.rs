use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    fn foo(&self) -> impl Send;
}

fn main() {}
