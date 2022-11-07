use duty::transport::Bincode;
use std::error::Error;
use std::net::TcpStream;

mod ttv_calc;
use ttv_calc::TtvCalcClient;

fn main() -> Result<(), Box<dyn Error>> {
    let stream = TcpStream::connect("myserver")?;

    let client = TtvCalcClient::new(Bincode::new(stream))?;

    let sum = client.ttv_calc(0, 42)?;

    println!("{:?}", sum);

    Ok(())
}
