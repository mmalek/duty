use crate::error::Error;
use crate::procedure::Procedure;
use crate::transport::Transport;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct Client<T> {
    transport: Arc<Mutex<T>>,
}

impl<T: Transport> Client<T> {
    pub fn new(transport: T) -> Client<T> {
        Client {
            transport: Arc::new(Mutex::new(transport)),
        }
    }

    pub fn call<P: Procedure>(&mut self, proc: P) -> CallHandle<P::Response> {
        let request: P::Request = proc.into();

        let transport = self.transport.clone();
        let join_handle = std::thread::spawn(move || {
            transport
                .lock()
                .expect("Mutex is poisoned")
                .send_receive(&request)
        });

        CallHandle { join_handle }
    }
}

pub struct CallHandle<R> {
    join_handle: JoinHandle<Result<R, Error>>,
}

impl<R> CallHandle<R> {
    pub fn is_finished(&self) -> bool {
        self.join_handle.is_finished()
    }

    pub fn get(self) -> Result<R, Error> {
        self.join_handle.join().expect("Thread panicked")
    }

    pub fn cancel(self) {
        todo!();
    }
}

#[cfg(test)]
mod test {}
