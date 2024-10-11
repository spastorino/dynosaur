use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use dynosaur::dynosaur;

#[dynosaur(DynMyTrait)]
trait MyTrait {
    async fn foo(self: Box<Self>);
    async fn bar(self: Rc<Self>);
    async fn baz(self: Arc<Self>);
    async fn zoo(self: Pin<Box<Self>>);
}

fn main() {}
