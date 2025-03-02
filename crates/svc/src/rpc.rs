use tokio::sync::oneshot::{self, Receiver, Sender};

#[derive(Debug)]
pub struct RpcRequest<Q, A> {
    data: Q,
    rsp: Sender<A>,
}

impl<Q, A> RpcRequest<Q, A> {
    pub fn new(data: Q) -> (Self, Receiver<A>) {
        let (tx, rx) = oneshot::channel();
        let req = Self { data, rsp: tx };
        (req, rx)
    }

    pub fn respond(self, mut func: impl FnMut(Q) -> A) {
        let res = func(self.data);
        let _ = self.rsp.send(res);
    }
}
