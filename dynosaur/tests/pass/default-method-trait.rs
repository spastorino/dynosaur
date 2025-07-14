use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box) MyTrait)]
trait MyTrait {
    fn foo(&mut self) -> impl Send {
        10
    }
}

fn main() {}
