use duty::error::Error;
use duty::{service, DataStream};
use serde::{de::DeserializeOwned, Serialize};
use std::net::{TcpListener, TcpStream};
use std::ops::{Add, Mul};
use std::sync::Barrier;

#[service]
pub trait Calculator<A, M>
where
    A: Add + Serialize + DeserializeOwned,
    M: Mul + Serialize + DeserializeOwned,
    <A as Add>::Output: Serialize + DeserializeOwned,
    <M as Mul>::Output: Serialize + DeserializeOwned,
{
    fn add(&self, a: A, b: A) -> <A as Add>::Output;
    fn mul(&self, a: M, b: M) -> <M as Mul>::Output;
}

struct CalculatorServer;

impl<A, M> Calculator<A, M> for CalculatorServer
where
    A: Add + Serialize + DeserializeOwned,
    M: Mul + Serialize + DeserializeOwned,
    <A as Add>::Output: Serialize + DeserializeOwned,
    <M as Mul>::Output: Serialize + DeserializeOwned,
{
    fn add(&self, a: A, b: A) -> <A as Add>::Output {
        a + b
    }

    fn mul(&self, a: M, b: M) -> <M as Mul>::Output {
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
                Calculator::<i32, u32>::handle_next_request(&server, &mut stream)?;
            }
            Ok(())
        });

        start.wait();

        let client = CalculatorClient::new(TcpStream::connect(&ADDR).expect("cannot connect"))?;
        assert_eq!(client.add::<i32, u32>(2, 3)?, 5);
        assert_eq!(client.add::<i32, u32>(38, 78)?, 116);
        assert_eq!(client.mul::<i32, u32>(42, 5)?, 210);
        assert_eq!(client.add::<i32, u32>(115, -42)?, 73);
        assert_eq!(client.add::<i32, u32>(987, 13)?, 1000);

        Ok(())
    })
}
