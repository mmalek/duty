pub mod error;

pub use crate::error::Error;
pub use duty_attrs::service;

use serde::{de::DeserializeOwned, Serialize};
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};

pub struct DataStream {
    stream: TcpStream,
    size_buf: Vec<u8>,
    data_buf: Vec<u8>,
}

impl DataStream {
    pub fn new(stream: TcpStream) -> DataStream {
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

    pub fn connect<T: ToSocketAddrs>(addr: T) -> Result<DataStream, Error> {
        TcpStream::connect(addr)
            .map_err(Error::OutgoingConnectionError)
            .map(DataStream::new)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::net::{TcpListener, ToSocketAddrs};
    use std::ops::{Add, Mul};
    use std::sync::Barrier;

    trait CalcService<T>
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize,
        <T as Mul>::Output: Serialize,
    {
        fn add(&self, a: T, b: T) -> <T as Add>::Output;
        fn mul(&self, a: T, b: T) -> <T as Mul>::Output;

        fn handle_next_request(&self, stream: &mut DataStream) -> Result<(), Error> {
            let request: CalcMessage<T> = stream.receive()?;
            match request {
                CalcMessage::Add { a, b } => stream.send(&self.add(a, b)),
                CalcMessage::Mul { a, b } => stream.send(&self.mul(a, b)),
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    enum CalcMessage<T> {
        Add { a: T, b: T },
        Mul { a: T, b: T },
    }

    impl<T> CalcMessage<T>
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize,
        <T as Mul>::Output: Serialize,
    {
    }

    struct CalcServiceServer;

    impl<T> CalcService<T> for CalcServiceServer
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize,
        <T as Mul>::Output: Serialize,
    {
        fn add(&self, a: T, b: T) -> <T as Add>::Output {
            a + b
        }

        fn mul(&self, a: T, b: T) -> <T as Mul>::Output {
            a * b
        }
    }

    struct CalcServiceClient {
        stream: RefCell<DataStream>,
    }

    impl CalcServiceClient {
        fn new<A: ToSocketAddrs>(addr: A) -> Result<CalcServiceClient, Error> {
            let stream = RefCell::new(DataStream::connect(addr)?);
            Ok(CalcServiceClient { stream })
        }
    }

    impl<T> CalcService<T> for CalcServiceClient
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize + DeserializeOwned,
        <T as Mul>::Output: Serialize + DeserializeOwned,
    {
        fn add(&self, a: T, b: T) -> <T as Add>::Output {
            self.stream
                .borrow_mut()
                .send_receive(&CalcMessage::Add { a, b })
                .expect("Communication error")
        }

        fn mul(&self, a: T, b: T) -> <T as Mul>::Output {
            self.stream
                .borrow_mut()
                .send_receive(&CalcMessage::Mul { a, b })
                .expect("Communication error")
        }
    }

    #[test]
    fn loopback_generic() -> Result<(), Error> {
        const ADDR: &str = "127.0.0.1:34563";

        let start = Barrier::new(2);

        std::thread::scope(|s| {
            s.spawn(|| -> Result<(), Error> {
                let listener = TcpListener::bind(&ADDR).expect("cannot open port");
                start.wait();
                let mut connections = listener.incoming();
                let mut stream = DataStream::new(
                    connections
                        .next()
                        .expect("no connections")
                        .expect("no stream"),
                );

                let server = CalcServiceServer;
                for _ in 0..5 {
                    CalcService::<i32>::handle_next_request(&server, &mut stream)?;
                }
                Ok(())
            });

            start.wait();

            let service = CalcServiceClient::new(&ADDR)?;
            assert_eq!(service.add(2, 3), 5);
            assert_eq!(service.add(38, 78), 116);
            assert_eq!(service.mul(42, 5), 210);
            assert_eq!(service.add(115, -42), 73);
            assert_eq!(service.add(987, 13), 1000);

            Ok(())
        })
    }
}
