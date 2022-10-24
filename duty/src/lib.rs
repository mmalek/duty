pub mod error;

pub use crate::error::Error;
pub use duty_attrs::service;

use serde::{de::DeserializeOwned, Serialize};
use std::io::{Read, Write};

pub struct DataStream<S> {
    stream: S,
    size_buf: Vec<u8>,
    data_buf: Vec<u8>,
}

impl<S> DataStream<S>
where
    S: Read,
    for<'a> &'a S: Write,
{
    pub fn new(stream: S) -> DataStream<S> {
        let mut size_buf = Vec::new();
        size_buf.resize(
            bincode::serialized_size(&std::usize::MAX)
                .expect("cannot estimate size of bincode-serialized usize") as usize,
            0,
        );

        DataStream {
            stream,
            size_buf,
            data_buf: Vec::new(),
        }
    }

    pub fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        self.stream
            .read_exact(&mut self.size_buf)
            .map_err(Error::CannotReadMsgRawData)?;
        let size: usize =
            bincode::deserialize(&self.size_buf).map_err(Error::MsgHeaderDeserFailed)?;
        self.data_buf.resize(size, 0);
        self.stream
            .read_exact(&mut self.data_buf)
            .map_err(Error::CannotReadMsgRawData)?;
        bincode::deserialize(&self.data_buf).map_err(Error::MsgBoodyDeserFailed)
    }

    pub fn send<T: Serialize>(&self, data: &T) -> Result<(), Error> {
        let size = bincode::serialized_size(&data)
            .expect("cannot estimate size of bincode-serialized data");
        bincode::serialize_into(&self.stream, &size).map_err(Error::MsgHeaderSerFailed)?;
        bincode::serialize_into(&self.stream, &data).map_err(Error::MsgHeaderSerFailed)
    }

    pub fn send_receive<In: Serialize, Out: DeserializeOwned>(
        &mut self,
        input: &In,
    ) -> Result<Out, Error> {
        self.send(input)?;
        self.receive()
    }
}
