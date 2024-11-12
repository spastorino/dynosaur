use std::fmt::Display;

#[dynosaur::dynosaur(DynFoo)]
trait Foo {
    fn foo(&self) -> impl Display {
        1
    }
}

impl Foo for i32 {}

impl<S: Foo + ?Sized> Foo for Box<S> {}

impl<S: Foo + ?Sized> Foo for &mut S {}

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
    dyn_dispatch(&mut DynFoo::boxed(1));
    dyn_dispatch(DynFoo::from_mut(&mut 1));
    static_dispatch(1);
    static_dispatch(DynFoo::boxed(1));
    static_dispatch(DynFoo::from_mut(&mut 1));
}
