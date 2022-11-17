use crate::error::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Write};

pub trait Transport: Send + 'static {
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error>;

    fn send<T: Serialize>(&mut self, data: &T) -> Result<(), Error>;

    fn send_receive<In: Serialize, Out: DeserializeOwned>(
        &mut self,
        input: &In,
    ) -> Result<Out, Error> {
        self.send(input)?;
        self.receive()
    }
}

pub struct Bincode<S> {
    stream: S,
}

impl<S> Bincode<S> {
    pub fn new(stream: S) -> Bincode<S> {
        Bincode { stream }
    }
}

impl<S: Read + Write + Send + 'static> Transport for Bincode<S> {
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        bincode::deserialize_from(&mut self.stream)
            .map_err(|e| Error::MsgDeserFailed(e.to_string()))
    }

    fn send<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        bincode::serialize_into(&mut self.stream, data)
            .map_err(|e| Error::MsgSerFailed(e.to_string()))
    }
}

pub struct Json<S> {
    stream: S,
}

impl<S> Json<S> {
    pub fn new(stream: S) -> Json<S> {
        Json { stream }
    }
}

impl<S: Read + Write + Send + 'static> Transport for Json<S> {
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        serde_json::from_reader(&mut self.stream).map_err(|e| Error::MsgDeserFailed(e.to_string()))
    }

    fn send<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        serde_json::to_writer(&mut self.stream, data)
            .map_err(|e| Error::MsgSerFailed(e.to_string()))
    }
}
