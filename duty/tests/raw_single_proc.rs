use duty::dispatcher::Dispatcher;
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
    type Request = Self;

    fn reduce(a: Self::Response, b: Self::Response) -> Self::Response {
        a && b
    }
}

#[test]
fn raw_single_proc() -> Result<(), Error> {
    std::thread::scope(|s| {
        let mut transports = Vec::new();

        for _ in 0..20 {
            let (client_stream, server_stream) = MpscStream::new_pair();

            s.spawn(|| -> Result<(), Error> {
                let mut transport = transport::Bincode::new(server_stream);

                for _ in 0..9 {
                    let p: AndProc = transport.receive()?;
                    p.respond(&mut transport, p.a && p.b)?;
                }
                Ok(())
            });

            transports.push(transport::Bincode::new(client_stream));
        }

        let mut client: Dispatcher<_> = transports.into();

        assert_eq!(client.call(&AndProc { a: true, b: true }).get()?, true);
        assert_eq!(client.call(&AndProc { a: false, b: true }).get()?, false);
        assert_eq!(client.call(&AndProc { a: true, b: false }).get()?, false);
        assert_eq!(client.call(&AndProc { a: false, b: false }).get()?, false);

        Ok(())
    })
}
