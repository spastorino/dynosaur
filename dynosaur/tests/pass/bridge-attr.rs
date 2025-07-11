#[dynosaur::dynosaur(DynNextNone, bridge(none))]
trait NextNone {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDefault, bridge(static))]
trait NextDefault {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDyn, bridge(dyn))]
trait NextDyn {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
