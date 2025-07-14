use dynosaur::dynosaur;

#[dynosaur(DynMyTrait = dyn(box))]
trait MyTrait {
    fn foo(&mut self) -> impl Send {
        10
    }
}

fn main() {}
