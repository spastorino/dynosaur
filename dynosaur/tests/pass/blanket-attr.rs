#[dynosaur::dynosaur(DynNextNone, blanket = none)]
trait NextNone {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDefault, blanket = default)]
trait NextDefault {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}

#[dynosaur::dynosaur(DynNextDyn, blanket = dyn)]
trait NextDyn {
    type Item;
    async fn next(&mut self) -> Option<Self::Item>;
}
