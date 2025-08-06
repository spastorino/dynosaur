// This file should match the code in "README.md"

#[dynosaur::dynosaur(DynNext = dyn(box) Next)]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

async fn dyn_dispatch(iter: &mut DynNext<'_, i32>) {
    while let Some(item) = iter.next().await {
        println!("- {item}");
    }
}

async fn _main() {
    let mut my_next_iter = MyNextIter::new(vec![1, 2, 3]);
    dyn_dispatch(&mut DynNext::new_box(my_next_iter.clone())).await;
    dyn_dispatch(DynNext::from_mut(&mut my_next_iter)).await;
}

#[derive(Clone)]
struct MyNextIter<T: Copy> {
    v: Vec<T>,
    i: usize,
}

impl<T: Copy> MyNextIter<T> {
    fn new(v: Vec<T>) -> Self {
        Self {
            v,
            i: 0,
        }
    }
}

impl<T: Copy> Next for MyNextIter<T> {
    type Item = T;

    async fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.v.len() {
            return None;
        }
        do_some_async_work().await;
        let item = self.v[self.i];
        self.i += 1;
        Some(item)
    }
}

async fn do_some_async_work() {
     // do something :)
}

fn main() {}
