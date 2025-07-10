#[trait_variant::make(SendNext: Send)]
#[dynosaur::dynosaur(DynNext = dyn Next, blanket = dyn)]
#[dynosaur::dynosaur(DynSendNext = dyn SendNext, blanket = dyn)]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
