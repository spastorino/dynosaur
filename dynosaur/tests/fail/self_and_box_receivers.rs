use std::rc::Rc;

#[dynosaur::dynosaur(DynAll = dyn(box) All)]
trait All {
    fn ref_mut(&mut self);
    fn ref_(&self) -> impl Send;
    fn owned(self) -> impl Send;
    fn self_box(self: Box<Self>) -> impl Send;
    fn self_rc(self: Rc<Self>) -> impl Send;
}

fn main() {}
