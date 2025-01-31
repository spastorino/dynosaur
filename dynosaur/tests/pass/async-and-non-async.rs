use dynosaur::dynosaur;

#[dynosaur(DynJobQueue)]
pub trait JobQueue {
    fn len(&self) -> usize;
    async fn dispatch(&self);
}

fn main() {}
