use crate::ttv_calc::TtvCalc;

pub struct Calculator {
    pub factor: f64,
}

impl TtvCalc for Calculator {
    fn ttv_calc(&self, from: u64, to: u64) -> Vec<f64> {
        (from..to).map(|x| x as f64 * self.factor).collect()
    }
}
