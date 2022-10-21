use duty::error::Error;
use duty::{service, DataStream};
use std::net::{TcpListener, TcpStream};
use std::sync::Barrier;

#[service]
trait LogicService {
    fn and(&self, a: bool, b: bool) -> bool;
    fn or(&self, a: bool, b: bool) -> bool;
}

struct LogicServiceServer;

impl LogicService for LogicServiceServer {
    fn and(&self, a: bool, b: bool) -> bool {
        a && b
    }

    fn or(&self, a: bool, b: bool) -> bool {
        a || b
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

            let server = LogicServiceServer;
            for _ in 0..8 {
                server.handle_next_request(&mut stream)?;
            }
            Ok(())
        });

        start.wait();

        let client = LogicServiceClient::new(TcpStream::connect(&ADDR).expect("cannot connect"))?;
        assert_eq!(client.and(true, true)?, true);
        assert_eq!(client.and(false, true)?, false);
        assert_eq!(client.and(true, false)?, false);
        assert_eq!(client.and(false, false)?, false);

        assert_eq!(client.or(true, true)?, true);
        assert_eq!(client.or(false, true)?, true);
        assert_eq!(client.or(true, false)?, true);
        assert_eq!(client.or(false, false)?, false);

        Ok(())
    })
}
