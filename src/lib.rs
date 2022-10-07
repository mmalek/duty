pub mod error;

use crate::error::Error;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::cell::RefCell;
    use std::marker::PhantomData;
    use std::net::{TcpListener, ToSocketAddrs};
    use std::ops::{Add, Mul};
    use std::sync::Barrier;

    trait CalcService<T: Add + Mul> {
        fn add(&self, a: T, b: T) -> <T as Add>::Output;
        fn mul(&self, a: T, b: T) -> <T as Mul>::Output;
    }

    #[derive(Serialize, Deserialize)]
    enum CalcMessage<T>
    {
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

    struct CallStream<'s, T: Add + Mul> {
        server: &'s CalcServiceServer,
        stream: DataStream,
        phantom: PhantomData<T>,
    }

    impl<'s, T> CallStream<'s, T>
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize,
        <T as Mul>::Output: Serialize,
    {
        fn new(server: &'s CalcServiceServer, stream: TcpStream) -> CallStream<T> {
            CallStream {
                server,
                stream: DataStream::new(stream),
                phantom: PhantomData,
            }
        }

        fn next_call(&mut self) -> Result<(), Error> {
            let message: CalcMessage<T> = self.stream.receive()?;
            self.dispatch(message)
        }

        fn dispatch(&self, message: CalcMessage<T>) -> Result<(), Error> {
            match message {
                CalcMessage::Add { a, b } => {
                    self.stream.send(&CalcService::<T>::add(self.server, a, b))
                }
                CalcMessage::Mul { a, b } => self.stream.send(&self.server.mul(a, b)),
            }
        }
    }

    impl<'s, T> Iterator for CallStream<'s, T>
    where
        T: Add + Mul + Serialize + DeserializeOwned,
        <T as Add>::Output: Serialize,
        <T as Mul>::Output: Serialize,
    {
        type Item = Result<(), Error>;
        fn next(&mut self) -> Option<Self::Item> {
            Some(self.next_call())
        }
    }

    struct CalcServiceServer {
        listener: TcpListener,
    }

    impl CalcServiceServer {
        fn new<A: ToSocketAddrs>(addr: A) -> Result<CalcServiceServer, Error> {
            let listener = TcpListener::bind(addr).map_err(Error::ServerFailedToStart)?;
            Ok(CalcServiceServer { listener })
        }

        fn connections<T>(&self) -> impl Iterator<Item = Result<CallStream<T>, Error>>
        where
            T: Add + Mul + Serialize + DeserializeOwned,
            <T as Add>::Output: Serialize,
            <T as Mul>::Output: Serialize,
        {
            self.listener
                .incoming()
                .map(|stream| stream.map_err(Error::IncomingConnectionError))
                .map(|stream| stream.map(|stream| CallStream::new(self, stream)))
        }
    }

    impl<T: Add + Mul> CalcService<T> for CalcServiceServer {
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
                .borrow()
                .send(&CalcMessage::Add { a, b })
                .expect("Sending message error");
            self.stream
                .borrow_mut()
                .receive()
                .expect("Receiving message error")
        }

        fn mul(&self, a: T, b: T) -> <T as Mul>::Output {
            self.stream
                .borrow()
                .send(&CalcMessage::Mul { a, b })
                .expect("Sending message error");
            self.stream
                .borrow_mut()
                .receive()
                .expect("Receiving message error")
        }
    }

    #[test]
    fn loopback() -> Result<(), Error> {
        const ADDR: &str = "127.0.0.1:34563";

        let start = Barrier::new(2);

        std::thread::scope(|s| {
            s.spawn(|| {
                let service = CalcServiceServer::new(&ADDR)?;
                start.wait();
                let mut connections = service.connections();
                let requests: CallStream<i32> = connections.next().unwrap()?;
                requests.take(5).collect::<Result<(), Error>>()
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
