#[trait_variant::make(SendNext: Send)]
#[dynosaur::dynosaur(DynNext = dyn Next)]
#[dynosaur::dynosaur(DynSendNext = dyn SendNext)]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

async fn dyn_dispatch(iter: &mut DynSendNext<'_, i32>) {
    print!("dyn_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

async fn dyn_dispatch_local(iter: &mut DynNext<'_, i32>) {
    print!("dyn_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

async fn static_dispatch(mut iter: impl SendNext<Item = i32>) {
    print!("static_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

async fn static_dispatch_local(mut iter: impl Next<Item = i32>) {
    print!("static_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let v = [1, 2, 3];
    dyn_dispatch(&mut DynSendNext::boxed(from_iter(v))).await;
    dyn_dispatch(&mut DynSendNext::boxed(from_iter(v))).await;
    dyn_dispatch_local(DynNext::from_mut(&mut from_iter(v))).await;
    dyn_dispatch_local(DynNext::from_mut(&mut from_iter(v))).await;
    static_dispatch(from_iter(v)).await;
    static_dispatch_local(from_iter(v)).await;
    static_dispatch(DynSendNext::boxed(from_iter(v))).await;
    static_dispatch(DynSendNext::from_mut(&mut from_iter(v))).await;
    static_dispatch_local(DynNext::boxed(from_iter(v))).await;
    static_dispatch_local(DynNext::from_mut(&mut from_iter(v))).await;
}

impl<S: SendNext + ?Sized> SendNext for Box<S> {
    type Item = S::Item;
    fn next(&mut self) -> impl core::future::Future<Output = Option<Self::Item>> {
        S::next(&mut *self)
    }
}

impl<S: SendNext + ?Sized> SendNext for &mut S {
    type Item = S::Item;
    fn next(&mut self) -> impl core::future::Future<Output = Option<Self::Item>> {
        S::next(&mut *self)
    }
}

impl<Item> Next for Box<DynNext<'_, Item>> {
    type Item = Item;
    fn next(&mut self) -> impl core::future::Future<Output = Option<Self::Item>> {
        DynNext::next(&mut *self)
    }
}

impl<Item> Next for &mut DynNext<'_, Item> {
    type Item = Item;
    fn next(&mut self) -> impl core::future::Future<Output = Option<Self::Item>> {
        DynNext::next(&mut *self)
    }
}

struct ForArray<T: Copy, const N: usize> {
    v: [T; N],
    i: usize,
}

fn from_iter<T: Copy, const N: usize>(v: [T; N]) -> ForArray<T, N> {
    ForArray { v, i: 0 }
}

impl<T: Copy + Send, const N: usize> SendNext for ForArray<T, N> {
    type Item = T;

    async fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.v.len() {
            return None;
        }
        stuff().await;
        let item = self.v[self.i];
        self.i += 1;
        Some(item)
    }
}

async fn stuff() {}
