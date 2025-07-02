#[dynosaur::dynosaur(DynRefOnly)]
trait RefOnly {
    fn ref_(&self);
}

#[dynosaur::dynosaur(DynMutAndRef)]
trait MutAndRef {
    fn ref_mut(&mut self);
    fn ref_(&self) -> impl Send;
}

#[dynosaur::dynosaur(DynAll)]
trait All {
    fn ref_mut(&mut self);
    fn ref_(&self) -> impl Send;
    //fn owned(self) -> impl Send;
    //fn self_box(self: Box<Self>) -> impl Send;
}

fn main() {}
