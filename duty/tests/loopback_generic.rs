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
    fn mul(&mut self, a: M, b: M) -> <M as Mul>::Output;

    fn magic_number() -> A;
}

struct CalculatorServer;

impl Calculator<i32, u32> for CalculatorServer {
    fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    fn mul(&mut self, a: u32, b: u32) -> u32 {
        a * b
    }

    fn magic_number() -> i32 {
        42
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

            let mut server = CalculatorServer;
            for _ in 0..6 {
                server.handle_next_request(&mut stream)?;
            }
            Ok(())
        });

        start.wait();

        let client = CalculatorClient::<_, i32, u32>::new(
            TcpStream::connect(&ADDR).expect("cannot connect"),
        )?;
        assert_eq!(client.add(2, 3)?, 5);
        assert_eq!(client.add(38, 78)?, 116);
        assert_eq!(client.mul(42, 5)?, 210);
        assert_eq!(client.add(115, -42)?, 73);
        assert_eq!(client.add(987, 13)?, 1000);
        assert_eq!(client.magic_number()?, 42);

        Ok(())
    })
}
