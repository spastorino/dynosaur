#[dynosaur::dynosaur(DynSomeTrait)]
trait SomeTrait {
    async fn multiple_elided_lifetimes(&self, a: &u8, b: &u8);
    async fn multiple_named_lifetimes<'a, 'b>(&self, a: &'a u8, b: &'b u8, c: &u8);
    async fn same_lifetimes_multiple_params<'a>(&self, a1: &'a u8, a2: &'a u8, c: &u8) -> &'a u8;
    async fn multiple_static_and_anonymous(&self, a: &'static u8, b: &'_ u8, c: &'_ u8);
}

impl SomeTrait for () {
    async fn multiple_elided_lifetimes(&self, a: &u8, b: &u8) {}
    async fn multiple_named_lifetimes<'a, 'b>(&self, a: &'a u8, b: &'b u8, c: &u8) {}
    async fn same_lifetimes_multiple_params<'a>(&self, a1: &'a u8, a2: &'a u8, c: &u8) -> &'a u8 {
        a2
    }
    async fn multiple_static_and_anonymous(&self, a: &'static u8, b: &'_ u8, c: &'_ u8) {}
}

fn main() {}
