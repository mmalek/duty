use duty::transport::Bincode;
use readwrite::ReadWrite;
use std::error::Error;

mod ttv_calc;
use ttv_calc::TtvCalc;

mod ttv_calculator;
use ttv_calculator::Calculator;

fn main() -> Result<(), Box<dyn Error>> {
    let mut transport = Bincode::new(ReadWrite::new(std::io::stdin(), std::io::stdout()));

    let calculator = Calculator { factor: 1.5 };
    calculator.handle_next_request(&mut transport)?;

    Ok(())
}
