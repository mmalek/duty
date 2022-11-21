use crate::error::Error;
use crate::procedure::Procedure;
use crate::transport::Transport;
use serde::{de::DeserializeOwned, Serialize};

pub struct Server<T, R> {
    transport: T,
    _marker: std::marker::PhantomData<R>,
}

impl<T, R> Server<T, R>
where
    T: Transport,
    R: Serialize + DeserializeOwned + Send + 'static,
{
    pub fn new(transport: T) -> Server<T, R> {
        Server {
            transport,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn next<'s>(&'s mut self) -> Result<(R, RequestHandle<'s, T>), Error> {
        let request = self.transport.receive()?;
        let handle = RequestHandle {
            transport: &mut self.transport,
        };
        Ok((request, handle))
    }
}

pub struct RequestHandle<'s, T> {
    transport: &'s mut T,
}

impl<'s, T> RequestHandle<'s, T>
where
    T: Transport,
{
    pub fn is_canceled(&self) -> bool {
        todo!();
    }

    pub fn respond<Proc: Procedure>(
        self,
        _proc: &Proc,
        response: &Proc::Response,
    ) -> Result<(), Error> {
        self.transport.send(response)
    }
}
