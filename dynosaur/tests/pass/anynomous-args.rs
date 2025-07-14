#[dynosaur::dynosaur(DynSomeTrait = dyn(box) SomeTrait)]
trait SomeTrait {
    async fn multiple_elided_lifetimes(&self, _: &u8, _: &u8);
}
impl SomeTrait for () {
    async fn multiple_elided_lifetimes(&self, _: &u8, _: &u8) {}
}

fn main() {}
