#[duty::service]
pub trait TtvCalc {
    fn ttv_calc(&self, from: u64, to: u64) -> Vec<f64>;
}
