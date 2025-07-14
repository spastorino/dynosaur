#[dynosaur::dynosaur(DynNextNone = dyn(box), bridge(none))]
trait NextNone {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDefault = dyn(box), bridge(blanket))]
trait NextDefault {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDyn = dyn(box), bridge(dyn))]
trait NextDyn {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
