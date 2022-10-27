pub mod error;

pub use crate::error::Error;
pub use duty_attrs::service;

use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Write};

pub struct DataStream<S> {
    stream: S,
}

impl<S> DataStream<S>
where
    S: Read + Write,
{
    pub fn new(stream: S) -> DataStream<S> {
        let mut size_buf = Vec::new();
        size_buf.resize(
            bincode::serialized_size(&std::usize::MAX)
                .expect("cannot estimate size of bincode-serialized usize") as usize,
            0,
        );

        DataStream { stream }
    }

    pub fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        bincode::deserialize_from(&mut self.stream)
            .map_err(Error::MsgBoodyDeserFailed)
    }

    pub fn send<T: Serialize>(&mut self, data: &T) -> Result<(), Error> {
        bincode::serialize_into(&mut self.stream, &data)
            .map_err(Error::MsgHeaderSerFailed)
    }

    pub fn send_receive<In: Serialize, Out: DeserializeOwned>(
        &mut self,
        input: &In,
    ) -> Result<Out, Error> {
        self.send(input)?;
        self.receive()
    }
}
