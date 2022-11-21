use serde::{de::DeserializeOwned, Serialize};

pub trait Procedure: Clone + Sized {
    type Response: Serialize + DeserializeOwned + Send + 'static;
    type Request: Serialize + DeserializeOwned + From<Self> + Send + 'static;

    fn reduce(a: Self::Response, b: Self::Response) -> Self::Response;
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::Deserialize;

    #[derive(Clone, Serialize, Deserialize)]
    struct SumProcedure {
        a: Vec<f64>,
    }

    impl Procedure for SumProcedure {
        type Response = Vec<f64>;
        type Request = Self;

        fn reduce(mut a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
            a.extend(b);
            a
        }
    }

    #[derive(Clone, Serialize, Deserialize)]
    struct ProductProcedure {
        a: Vec<f64>,
    }

    impl Procedure for ProductProcedure {
        type Response = Vec<f64>;
        type Request = MathRequest;

        fn reduce(mut a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
            a.extend(b);
            a
        }
    }

    #[derive(Serialize, Deserialize)]
    enum MathRequest {
        SumProcedure(SumProcedure),
        ProductProcedure(ProductProcedure),
    }

    impl From<SumProcedure> for MathRequest {
        fn from(r: SumProcedure) -> Self {
            MathRequest::SumProcedure(r)
        }
    }

    impl From<ProductProcedure> for MathRequest {
        fn from(r: ProductProcedure) -> Self {
            MathRequest::ProductProcedure(r)
        }
    }
}
