#[dynosaur::dynosaur(DynNextNone = dyn(box) NextNone, bridge(none))]
trait NextNone {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDefault = dyn(box) NextDefault, bridge(blanket))]
trait NextDefault {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDyn = dyn(box) NextDyn, bridge(dyn))]
trait NextDyn {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
