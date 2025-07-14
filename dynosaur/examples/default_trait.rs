use std::fmt::Display;

#[dynosaur::dynosaur(DynFoo = dyn(box))]
trait Foo {
    fn foo(&self) -> impl Display {
        1
    }
}

impl Foo for i32 {}

fn dyn_dispatch(f: &mut DynFoo<'_>) {
    print!("dyn_dispatch: ");
    let foo = f.foo();
    println!("{foo} ");
}

fn static_dispatch(f: impl Foo) {
    print!("static_dispatch: ");
    let foo = f.foo();
    println!("{foo} ");
}

fn main() {
    dyn_dispatch(&mut DynFoo::new_box(1));
    dyn_dispatch(DynFoo::from_mut(&mut 1));
    static_dispatch(1);
    static_dispatch(DynFoo::new_box(1));
    static_dispatch(DynFoo::from_mut(&mut 1));
}
