use duty::transport::Bincode;
use std::error::Error;
use std::net::TcpListener;

mod ttv_calc;
use ttv_calc::TtvCalc;

mod ttv_calculator;
use ttv_calculator::Calculator;

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("0.0.0.0")?;

    let calculator = Calculator { factor: 1.5 };

    for connection in listener.incoming() {
        let mut transport = Bincode::new(connection?);

        while let Ok(()) = calculator.handle_next_request(&mut transport) {}
    }

    Ok(())
}
