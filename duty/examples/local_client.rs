use duty::transport::Bincode;
use readwrite::ReadWrite;
use std::error::Error;
use std::process::{Command, Stdio};

mod ttv_calc;
use ttv_calc::TtvCalcClient;

fn main() -> Result<(), Box<dyn Error>> {
    let mut child = Command::new("local_worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let stream = ReadWrite::new(
        child.stdout.take().expect("Missing stdout"),
        child.stdin.take().expect("Missing stdin"),
    );

    let client = TtvCalcClient::new(Bincode::new(stream))?;

    let sum = client.ttv_calc(0, 42)?;

    println!("{:?}", sum);

    Ok(())
}
