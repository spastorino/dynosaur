#[trait_variant::make(SendNext: Send)]
#[dynosaur::dynosaur(DynNext = dyn Next)]
#[dynosaur::dynosaur(DynSendNext = dyn SendNext)]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
