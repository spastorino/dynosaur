#[dynosaur::dynosaur(DynSomeTrait)]
trait SomeTrait {
    fn get_iter(&mut self) -> impl Iterator<Item = u8> + '_;
}

struct MyImpl([u8; 4]);

impl SomeTrait for MyImpl {
    fn get_iter(&mut self) -> impl Iterator<Item = u8> + '_ {
        return self.0.into_iter();
    }
}

fn main() {
    let mut st = DynSomeTrait::new_box(MyImpl([3, 2, 4, 1]));
    for x in st.get_iter() {
        println!("{}", x);
    }
}
