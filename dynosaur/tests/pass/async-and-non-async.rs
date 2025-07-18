use dynosaur::dynosaur;

#[dynosaur(DynJobQueue = dyn(box) JobQueue)]
pub trait JobQueue {
    fn len(&self) -> usize;
    async fn dispatch(&self);
}

fn main() {}
