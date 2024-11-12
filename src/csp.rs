use ndarray::{s, Array1, Array2, Array3, ArrayView, ArrayView2, Axis};
use serde::{Deserialize, Serialize};

// Main CSP struct
#[derive(Serialize, Deserialize)]
pub struct CSP {
    // n_filters: usize,
    // metric: String,
    // log: bool,
    // ajd_method: String,
    // filters: Array2<f64>,
    filters: [[f64; 20]; 10],
    // patterns_: Array2<f64>,
}

impl CSP {
    pub fn new(
        // n_filters: usize,
        // metric: &str,
        // log: bool,
        // ajd_method: &str,
        filters: [[f64;20];10]
        // patterns: Array2<f64>,
    ) -> Self {
        CSP {
            // n_filters,
            // metric: metric.to_string(),
            // log,
            // ajd_method: ajd_method.to_string(),
            filters: filters,
            // patterns_: patterns,
        }
    }

    pub fn transform(&self, x: &Array2<f64>) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        let filters: ArrayView2<f64> = ArrayView2::from_shape((10, 20), self.filters.as_flattened()).unwrap();

        let n_channels = filters.shape()[0];

        let mut x_filt = Array2::zeros((n_channels, x.shape()[1]));

        x_filt = filters.dot(x).dot(&filters.t());

        let out: Vec<f64> = x_filt.diag().into_iter().map(|x| f64::ln(*x)).collect();
        Ok(out)
    }

    pub fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let csp: CSP = serde_json::from_str(json_str)?;
        Ok(csp)
    }

    pub fn to_json(&self) -> Result<String, Box<dyn std::error::Error>> {
        let json = serde_json::to_string(self)?;
        Ok(json)
    }

    pub fn to_json_pretty(&self) -> Result<String, Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(json)
    }
}


