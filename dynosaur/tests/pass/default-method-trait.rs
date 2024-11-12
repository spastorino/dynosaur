use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    fn foo(&mut self) -> impl Send {
        10
    }
}

fn main() {}
