#[dynosaur::dynosaur(DynSomeTrait = dyn(box) SomeTrait)]
trait SomeTrait<'a, 'b> {
    async fn lotsa_lifetimes<'d, 'e, 'f>(&self, a: &'d u32, b: &'e u32, c: &'f u32) -> &'a u32
    where
        'b: 'a;
}

struct MyData<'a, 'b, 'c>(&'a u32, &'b u32, &'c u32)
where
    'b: 'a;

impl<'a, 'b, 'c> SomeTrait<'a, 'b> for MyData<'a, 'b, 'c> {
    async fn lotsa_lifetimes<'d, 'e, 'f>(&self, a: &'d u32, b: &'e u32, c: &'f u32) -> &'a u32
    where
        'b: 'a,
    {
        self.0
    }
}

fn main() {}
