use duty::error::Error;
use duty::service;

#[test]
fn loopback_specific() -> Result<(), Error> {
    #[service]
    trait LogicService {
        fn and(&self, a: bool, b: bool) -> bool;
        fn or(&self, a: bool, b: bool) -> bool;
    }

    let _ = LogicServiceClient::new("");

    Ok(())
}
