// use nalgebra::ComplexField;
use ndarray::{s, Array1, Array2, Array3, ArrayView2, Axis, NewAxis};
use ndarray_stats::CorrelationExt;
use ndarray::{aview2, arr2};

pub struct Covariances {
    assume_centered: bool,
}

impl Covariances {
    pub fn new(assume_centered: bool) -> Self {
        Covariances {
            assume_centered,
        }
    }

    /// Transform multi-channel time series into covariance matrices
    pub fn transform(&self, x: Array2<f64>) -> Option<Array2<f64>> {
        let (n_channels, n_times) = x.dim();
        let mut covmats = Array2::zeros((n_channels, n_channels));

        covmats
            .assign(&compute_scm(x.into(), self.assume_centered)?);

        Some(covmats)
    }
}

/// Compute Sample Covariance Matrix
fn compute_scm(x: Array2<f64>, _assume_centered: bool) -> Option<Array2<f64>> {
    let (n_channels, n_times) = x.dim();
    
    // if !assume_centered {
        // Center the data
        // Ok(x.dot(&x.mapv(|v| v.conjugate()).t()) / n_times as f64)
        x.cov(0.).ok()
        // let means = x.mean_axis(Axis(1)).unwrap();
        // let x_centered = &x - &means.slice(s![.., NewAxis]);
        // Ok((x_centered.dot(&x_centered.t())) / (n_times as f64))
    // } else {
    //     Ok((x.dot(&x.t())) / (n_times as f64))
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array;
    use ndarray_stats::CorrelationExt;

    #[test]
    fn test_scm() {
        // Create sample data: 2 channels, 3 time points
        let x = arr2(&[
            [0.375, 0.951, 0.732],
            [0.599, 0.156, 0.156]
        ]);
        println!("{:?}", x.t().cov(1.));
        
        let result_true = Array2::from_shape_vec((2, 2), vec![
            0.02877489, 0.01797633,
            0.01797633, 0.01290067,
        ]).unwrap();

        let cov = Covariances::new(false);
        let result = cov.transform(x).unwrap();
        println!("{:?}", result);
        
        // assert!(false);
        // Verify shape
        // assert_eq!(result.shape(), &[1, 2, 2]);
        //
        // 
        // // Verify symmetry
        // assert!((result[[0, 0, 1]] - result[[0, 1, 0]]).abs() < 1e-10);
    }
}
