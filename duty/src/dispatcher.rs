use crate::client::{CallHandle, Client};
use crate::error::Error;
use crate::procedure::Procedure;
use crate::transport::Transport;

pub struct Dispatcher<T: Transport> {
    clients: Vec<Client<T>>,
}

impl<T: Transport> Dispatcher<T> {
    pub fn dispatch<P: Procedure>(&mut self, proc: &P) -> DispatchHandle<P> {
        let call_handlers = self
            .clients
            .iter_mut()
            .map(|c| c.call(proc.clone()))
            .collect();

        DispatchHandle { call_handlers }
    }
}

pub struct DispatchHandle<P: Procedure> {
    call_handlers: Vec<CallHandle<P::Response>>,
}

impl<P: Procedure> DispatchHandle<P> {
    pub fn is_finished(&self) -> bool {
        self.call_handlers.iter().all(|h| h.is_finished())
    }

    pub fn get(self) -> Result<P::Response, Error> {
        self.call_handlers
            .into_iter()
            .map(CallHandle::get)
            .reduce(|a, b| Ok(P::reduce(a?, b?)))
            .expect("Call handler list is empty")
    }

    pub fn cancel(self) {
        self.call_handlers.into_iter().for_each(CallHandle::cancel)
    }
}
