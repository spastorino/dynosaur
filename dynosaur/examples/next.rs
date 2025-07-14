#[dynosaur::dynosaur(DynNext = dyn(box))]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

async fn dyn_dispatch(iter: &mut DynNext<'_, i32>) {
    print!("dyn_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

async fn static_dispatch(mut iter: impl Next<Item = i32>) {
    print!("static_dispatch: ");
    while let Some(item) = iter.next().await {
        print!("{item} ");
    }
    println!();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let v = [1, 2, 3];
    dyn_dispatch(&mut DynNext::new_box(from_iter(v))).await;
    dyn_dispatch(DynNext::from_mut(&mut from_iter(v))).await;
    static_dispatch(from_iter(v)).await;
    static_dispatch(DynNext::new_box(from_iter(v))).await;
    static_dispatch(DynNext::from_mut(&mut from_iter(v))).await;
}

struct ForArray<T: Copy, const N: usize> {
    v: [T; N],
    i: usize,
}

fn from_iter<T: Copy, const N: usize>(v: [T; N]) -> ForArray<T, N> {
    ForArray { v, i: 0 }
}

impl<T: Copy, const N: usize> Next for ForArray<T, N> {
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
