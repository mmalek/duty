use duty::error::Error;
use duty::stream::MpscStream;
use duty::{service, transport};
use serde::{de::DeserializeOwned, Serialize};
use std::ops::{Add, Mul};

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
fn loopback_generic() -> Result<(), Error> {
    std::thread::scope(|s| {
        let (client_stream, server_stream) = MpscStream::new_pair();

        s.spawn(|| -> Result<(), Error> {
            let mut transport = transport::Bincode::new(server_stream);

            let mut server = CalculatorServer;
            for _ in 0..6 {
                server.handle_next_request(&mut transport)?;
            }
            Ok(())
        });

        let transport = transport::Bincode::new(client_stream);
        let client = CalculatorClient::<_, i32, u32>::new(transport)?;

        assert_eq!(client.add(2, 3)?, 5);
        assert_eq!(client.add(38, 78)?, 116);
        assert_eq!(client.mul(42, 5)?, 210);
        assert_eq!(client.add(115, -42)?, 73);
        assert_eq!(client.add(987, 13)?, 1000);
        assert_eq!(client.magic_number()?, 42);

        Ok(())
    })
}
