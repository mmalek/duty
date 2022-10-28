use crate::error::Error;
use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Write};

pub trait Transport {
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

impl<S> Bincode<S>
where
    S: Read + Write,
{
    pub fn new(stream: S) -> Bincode<S> {
        Bincode { stream }
    }
}

impl<S: Read + Write> Transport for Bincode<S> {
    fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        let mut deserializer =
            bincode::Deserializer::with_reader(&mut self.stream, bincode::DefaultOptions::new());
        serde::Deserialize::deserialize(&mut deserializer).map_err(Error::MsgBoodyDeserFailed)
    }

    fn send<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        let mut serializer =
            bincode::Serializer::new(&mut self.stream, bincode::DefaultOptions::new());
        serde::Serialize::serialize(&data, &mut serializer).map_err(Error::MsgHeaderSerFailed)
    }
}
