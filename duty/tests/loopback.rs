use duty::error::Error;
use duty::stream::MpscStream;
use duty::{service, transport};

#[service]
trait LogicService {
    fn and(&self, a: bool, b: bool) -> bool;
    fn or(&self, a: bool, b: bool) -> bool;
    fn magic_const() -> bool;
}

struct LogicServiceServer;

impl LogicService for LogicServiceServer {
    fn and(&self, a: bool, b: bool) -> bool {
        a && b
    }

    fn or(&self, a: bool, b: bool) -> bool {
        a || b
    }

    fn magic_const() -> bool {
        true
    }
}

#[test]
fn loopback() -> Result<(), Error> {
    std::thread::scope(|s| {
        let (client_stream, server_stream) = MpscStream::new_pair();

        s.spawn(|| -> Result<(), Error> {
            let mut transport = transport::Bincode::new(server_stream);

            let server = LogicServiceServer;
            for _ in 0..9 {
                server.handle_next_request(&mut transport)?;
            }
            Ok(())
        });

        let transport = transport::Bincode::new(client_stream);
        let client = LogicServiceClient::new(transport)?;

        assert_eq!(client.and(true, true)?, true);
        assert_eq!(client.and(false, true)?, false);
        assert_eq!(client.and(true, false)?, false);
        assert_eq!(client.and(false, false)?, false);

        assert_eq!(client.or(true, true)?, true);
        assert_eq!(client.or(false, true)?, true);
        assert_eq!(client.or(true, false)?, true);
        assert_eq!(client.or(false, false)?, false);

        assert_eq!(client.magic_const()?, true);

        Ok(())
    })
}
