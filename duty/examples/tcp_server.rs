use duty::transport::Bincode;
use std::error::Error;
use std::net::TcpListener;

mod ttv_calc;
use ttv_calc::TtvCalc;

mod ttv_calculator;
use ttv_calculator::Calculator;

struct TtvCalcWorker {
    factor: f64,
}

impl TtvCalc for TtvCalcWorker {
    fn ttv_calc(&self, from: u64, to: u64) -> Vec<f64> {
        (from..to).map(|x| x as f64 * self.factor).collect()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("0.0.0.0")?;

    let calculator = Calculator { factor: 1.5 };

    for connection in listener.incoming() {
        let mut transport = Bincode::new(connection?);

        while let Ok(()) = calculator.handle_next_request(&mut transport) {}
    }

    Ok(())
}
