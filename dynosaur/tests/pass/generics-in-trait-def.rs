use std::future::Future;

#[dynosaur::dynosaur(DynService)]
pub trait Service<Request> {
    type Response;
    type Error;

    fn call(&self, req: Request) -> impl Future<Output = Result<Self::Response, Self::Error>>;
}

fn main() {}
