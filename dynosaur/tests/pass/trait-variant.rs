#[trait_variant::make(SendNext: Send)]
#[dynosaur::dynosaur(DynNext = dyn Next, bridge(dyn))]
#[dynosaur::dynosaur(DynSendNext = dyn SendNext, bridge(dyn))]
trait Next {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
