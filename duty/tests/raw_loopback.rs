use duty::client::Client;
use duty::error::Error;
use duty::procedure::Procedure;
use duty::{transport, Transport};
use std::net::{TcpListener, TcpStream};
use std::sync::Barrier;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AndProc {
    a: bool,
    b: bool,
}

impl Procedure for AndProc {
    type Response = bool;
    type Request = LogicRequest;

    fn reduce(a: Self::Response, b: Self::Response) -> Self::Response {
        a && b
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct OrProc {
    a: bool,
    b: bool,
}

impl Procedure for OrProc {
    type Response = bool;
    type Request = LogicRequest;

    fn reduce(a: Self::Response, b: Self::Response) -> Self::Response {
        a || b
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum LogicRequest {
    And(AndProc),
    Or(OrProc),
}

impl From<AndProc> for LogicRequest {
    fn from(p: AndProc) -> LogicRequest {
        LogicRequest::And(p)
    }
}

impl From<OrProc> for LogicRequest {
    fn from(p: OrProc) -> LogicRequest {
        LogicRequest::Or(p)
    }
}

#[test]
fn raw_loopback() -> Result<(), Error> {
    const ADDR: &str = "127.0.0.1:34664";

    let start = Barrier::new(2);

    std::thread::scope(|s| {
        s.spawn(|| -> Result<(), Error> {
            let listener = TcpListener::bind(&ADDR).expect("cannot open port");
            start.wait();
            let mut connections = listener.incoming();
            let mut transport = transport::Bincode::new(
                connections
                    .next()
                    .expect("no connections")
                    .expect("no stream"),
            );

            for _ in 0..9 {
                match transport.receive()? {
                    LogicRequest::And(p) => p.a && p.b,
                    LogicRequest::Or(p) => p.a || p.b,
                };
            }
            Ok(())
        });

        start.wait();

        let transport = transport::Bincode::new(TcpStream::connect(&ADDR).expect("cannot connect"));
        let mut client = Client::new(transport);
        assert_eq!(client.call(AndProc { a: true, b: true }).get()?, true);
        // assert_eq!(client.call(AndProc { a: false, b: true }).get()?, false);
        // assert_eq!(client.call(AndProc { a: true, b: false }).get()?, false);
        // assert_eq!(client.call(AndProc { a: false, b: false }).get()?, false);

        // assert_eq!(client.call(OrProc { a: true, b: true }).get()?, true);
        // assert_eq!(client.call(OrProc { a: false, b: true }).get()?, true);
        // assert_eq!(client.call(OrProc { a: true, b: false }).get()?, true);
        // assert_eq!(client.call(OrProc { a: false, b: false }).get()?, false);

        Ok(())
    })
}
