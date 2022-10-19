use duty::error::Error;
use duty::{service, DataStream};
use serde::{de::DeserializeOwned, Serialize};
use std::net::{TcpListener, TcpStream};
use std::ops::{Add, Mul};
use std::sync::Barrier;

#[service]
trait Calculator<T>
where
    T: Add + Mul + Serialize + DeserializeOwned,
    <T as Add>::Output: Serialize + DeserializeOwned,
    <T as Mul>::Output: Serialize + DeserializeOwned,
{
    fn add(&self, a: T, b: T) -> <T as Add>::Output;
    fn mul(&self, a: T, b: T) -> <T as Mul>::Output;
}

struct CalculatorServer;

impl<T> Calculator<T> for CalculatorServer
where
    T: Add + Mul + Serialize + DeserializeOwned,
    <T as Add>::Output: Serialize + DeserializeOwned,
    <T as Mul>::Output: Serialize + DeserializeOwned,
{
    fn add(&self, a: T, b: T) -> <T as Add>::Output {
        a + b
    }

    fn mul(&self, a: T, b: T) -> <T as Mul>::Output {
        a * b
    }
}

#[test]
fn loopback_specific() -> Result<(), Error> {
    const ADDR: &str = "127.0.0.1:34564";

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

            let server = CalculatorServer;
            for _ in 0..5 {
                Calculator::<i32>::handle_next_request(&server, &mut stream)?;
            }
            Ok(())
        });

        start.wait();

        let client = CalculatorClient::new(TcpStream::connect(&ADDR).expect("cannot connect"))?;
        assert_eq!(client.add(2, 3), 5);
        assert_eq!(client.add(38, 78), 116);
        assert_eq!(client.mul(42, 5), 210);
        assert_eq!(client.add(115, -42), 73);
        assert_eq!(client.add(987, 13), 1000);

        Ok(())
    })
}
