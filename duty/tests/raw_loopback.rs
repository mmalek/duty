use duty::client::Client;
use duty::error::Error;
use duty::procedure::Procedure;
use duty::stream::MpscStream;
use duty::{transport, Transport};

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
    std::thread::scope(|s| {
        let (client_stream, server_stream) = MpscStream::new_pair();

        s.spawn(|| -> Result<(), Error> {
            let mut transport = transport::Bincode::new(server_stream);

            for _ in 0..9 {
                match transport.receive()? {
                    LogicRequest::And(p) => p.respond(&mut transport, p.a && p.b)?,
                    LogicRequest::Or(p) => transport.send(&p.map(|p| p.a || p.b))?,
                };
            }
            Ok(())
        });

        let transport = transport::Bincode::new(client_stream);
        let mut client = Client::new(transport);

        assert_eq!(client.call(AndProc { a: true, b: true }).get()?, true);
        assert_eq!(client.call(AndProc { a: false, b: true }).get()?, false);
        assert_eq!(client.call(AndProc { a: true, b: false }).get()?, false);
        assert_eq!(client.call(AndProc { a: false, b: false }).get()?, false);

        assert_eq!(client.call(OrProc { a: true, b: true }).get()?, true);
        assert_eq!(client.call(OrProc { a: false, b: true }).get()?, true);
        assert_eq!(client.call(OrProc { a: true, b: false }).get()?, true);
        assert_eq!(client.call(OrProc { a: false, b: false }).get()?, false);

        Ok(())
    })
}
