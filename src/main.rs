mod covariances;
mod csp;

use ffsvm::Predict;
use ffsvm::Solution;
use std::iter::zip;
use std::sync::Arc;
use std::time;
use std::time::Duration;
// use iir_filters::filter_design::FilterType;
// use iir_filters::filter_design::butter;
// use iir_filters::sos::zpk2sos;
// use iir_filters::filter::DirectForm2Transposed;
// use iir_filters::filter::Filter;
use butterworth::{Cutoff, Filter};

use ndarray::s;
use ndarray::Array3;
use ndarray::Axis;
use ndarray::{arr1, Array1, Array2};
// use smartcore::linalg::basic::matrix::DenseMatrix;
// use smartcore::svm::svc::{SVC, SVCParameters};
use covariances::Covariances;
use csp::CSP;
use ffsvm::{DenseSVM, Problem};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::time::sleep;

use burberry::{
    collector::{LSLData, LSLStreamCollector2},
    executor::tcp_sender::TCPSender,
    map_collector, map_executor, ActionSubmitter, Engine, Strategy,
};
use tracing::{debug, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone)]
struct DataPoint {
    timestamp: f64,
    raw_data: Vec<f64>,
    filtered_data: Vec<f64>,
}

#[derive(Clone, Debug)]
pub enum ExperimentEvent {
    Rest(f64),
    Animation(f64),
    Else,
}

#[derive(Clone, Debug)]
pub enum LSLDatapoint {
    Event(ExperimentEvent),
    Data(LSLData),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Classification {
    class: i32,
    window_statr_lsl_timestamp: f64,
    window_end_lsl_timestamp: f64,
    classification_time_in_sec: f64,
}

#[derive(Clone, Debug)]
pub enum Action {
    SendClassification(Classification),
}

pub fn parse_data(incoming_data: (&str, LSLData)) -> LSLDatapoint {
    if incoming_data.0 == "NeoRec21-1247" {
        LSLDatapoint::Data(incoming_data.1)
    } else {
        data_to_event(incoming_data.1)
    } //,"NeuroStim-Events"
}

pub struct ProcessLSL {
    state: ExperimentEvent,
    data_history: Vec<DataPoint>,
    filter: Filter,
    num_channels: usize,
    // Window parameters
    window_size: usize,          // M points in window
    current_window_count: usize, // Counter for points after state change
    window_count: usize,

    // ML components
    csp: CSP,
    svc_model: DenseSVM,
}

unsafe impl Send for ProcessLSL {}
unsafe impl Sync for ProcessLSL {}

impl ProcessLSL {
    pub fn new(
        points_before_window: usize,
        window_size: usize,
        csp_path: &Path,
        svc_path: &Path,
    ) -> Self {
        let csp_data = std::fs::read_to_string(csp_path).unwrap();
        let csp = serde_json::from_str(&csp_data).unwrap();

        // Load SVC model
        let svc_data = std::fs::read_to_string(svc_path).unwrap();
        let svc_model = DenseSVM::try_from(svc_data.as_str()).unwrap();

        Self {
            state: ExperimentEvent::Else,
            data_history: Vec::new(),
            filter: Filter::new(4, 500., Cutoff::BandPass(1., 40.)).unwrap(),
            num_channels: 20,
            window_size,
            current_window_count: 0,
            window_count: 1,
            csp,
            svc_model,
        }
    }

    fn filter_data(&mut self, data: &LSLData) {
        let ref_value = data.data[18];

        let filtered: Vec<f64> = data
            .data
            .iter()
            .enumerate()
            .filter_map(|(channel, val)| if channel != 18 { Some(val) } else { None })
            .enumerate()
            .map(|(channel, &value)| (value - ref_value) / 1_000_000.)
            .collect();

        // Store both raw and filtered data
        self.store_data(data.timestamp, data.data.clone(), filtered.clone());
    }

    fn store_data(&mut self, timestamp: f64, raw_data: Vec<f64>, filtered_data: Vec<f64>) {
        self.data_history.push(DataPoint {
            timestamp,
            raw_data,
            filtered_data,
        });

    }

    // Helper method to get recent filtered data
    fn get_recent_filtered_data(&self, samples: usize) -> Vec<&DataPoint> {
        self.data_history.iter().rev().take(samples).collect()
    }

    fn process_window(&self, window: &[DataPoint]) -> Option<i32> {
        // Convert window to matrix
        let data_matrix = self.window_to_matrix(window);
        self.process_matrix(data_matrix)
    }

    fn process_matrix(&self, data_matrix: Array2<f64>) -> Option<i32> {
        let covariances = Covariances::new(false).transform(data_matrix)?;

        // Apply CSP
        let csp_features = self.csp.transform(&covariances).ok()?;

        let mut problem = Problem::from(&self.svc_model);
        let features = problem.features();
        for (index, csp_feature) in zip((0..), csp_features) {
            features[index] = csp_feature as f32
        }

        self.svc_model.predict_value(&mut problem).ok()?;

        if let Solution::Label(class) = problem.solution() {
            Some(class)
        } else {
            None
        }
    }

    fn window_to_matrix(&self, window: &[DataPoint]) -> Array2<f64> {
        let n_samples = window.len();
        let n_channels = self.num_channels;

        let mut data = Array2::zeros((n_channels, n_samples));

        for (i, point) in window.iter().enumerate() {
            for (j, &value) in point.filtered_data.iter().enumerate() {
                data[[j, i]] = value;
            }
        }
        self.filter_matrix(&mut data);

        data
    }

    fn filter_matrix(&self, window: &mut Array2<f64>) {
        for mut i in window.axis_iter_mut(Axis(0)) {
            let row_vec = i.to_vec();
            let transformed = self.filter.bidirectional(&row_vec).unwrap();
            i.as_slice_mut().unwrap().copy_from_slice(&transformed);
        }
    }
}

#[burberry::async_trait]
impl Strategy<LSLDatapoint, Action> for ProcessLSL {
    fn name(&self) -> &str {
        "ProcessLSL"
    }

    async fn sync_state(
        &mut self,
        _submitter: Arc<dyn ActionSubmitter<Action>>,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn process_event(
        &mut self,
        event: LSLDatapoint,
        submitter: Arc<dyn ActionSubmitter<Action>>,
    ) {
        match event {
            LSLDatapoint::Event(event) => {
                self.state = event.clone();
                self.current_window_count = 0;
                self.window_count = 1;

                match event {
                    ExperimentEvent::Rest(_) | ExperimentEvent::Animation(_) => {
                        debug!("State changed to {:?}, starting window collection", event);
                    }
                    _ => {}
                }
            }
            LSLDatapoint::Data(datapoint) => {
                self.filter_data(&datapoint);
                sleep(Duration::from_millis(0)).await;

                self.current_window_count += 1;

                // Check if we've reached window collection point
                if self.current_window_count % 250 == 0 {
                    let window_start =
                        self.data_history.len().saturating_sub(self.window_size);
                    let window = &self.data_history[window_start..];

                    if window.len() == self.window_size {
                        let current_time = time::SystemTime::now();
                        if let Some(classification) = self.process_window(window) {
                            let new_current_time = time::SystemTime::now();
                            debug!("Classification result: {}", classification);
                            submitter.submit(Action::SendClassification(Classification {
                                class: classification,
                                window_statr_lsl_timestamp: window[0].timestamp,
                                window_end_lsl_timestamp: window.last().unwrap().timestamp,
                                classification_time_in_sec: new_current_time
                                    .duration_since(current_time)
                                    .unwrap()
                                    .as_secs_f64(),
                            }));
                        }
                        self.window_count += 1;
                    }
                }
            }
        }
    }
}

pub fn data_to_event(lsl_data: LSLData) -> LSLDatapoint {
    let event_code = lsl_data.data.into_iter().next().unwrap();
    let event = match event_code as u8 {
        0 => ExperimentEvent::Rest(lsl_data.timestamp),
        1 => ExperimentEvent::Animation(lsl_data.timestamp),
        _ => ExperimentEvent::Else,
    };
    debug!("Received event {:?}", event);
    LSLDatapoint::Event(event)
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::DEBUG.into())
        .from_env()
        .unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let mut engine = Engine::<LSLDatapoint, Action>::default();

    engine.add_collector(map_collector!(
        LSLStreamCollector2::new(vec!["NeoRec21-1247"], 60.)
            .expect("Expected to create LSL collector"),
        parse_data
    ));

    // engine.add_collector(map_collector!(
    //     LSLStreamCollector2::new("NeuroStim-Events", 60.)
    //         .expect("Expected to create Event collector"),
    //     data_to_event
    // ));

    let mut processor = ProcessLSL::new(
        250,
        500,
        Path::new("csp_filters.json"),
        Path::new("svc.model"),
    );

    let executor = TCPSender::new("192.168.1.107:50032");

    engine.add_strategy(Box::new(processor));
    engine.add_executor(map_executor!(executor, Action::SendClassification));

    engine.run_and_join().await.unwrap();
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;
    use ndarray::arr2;
    use serde::{Deserialize, Serialize};

    // Main CSP struct
    #[derive(Serialize, Deserialize)]
    struct Data {
        X: Vec<Vec<Vec<f64>>>,
        X_unfiltered: Vec<Vec<Vec<f64>>>,
        y_true: Vec<i32>,
        y_pred: Vec<i32>,
    }

    #[derive(Serialize, Deserialize)]
    struct FilteredData {
        X: Vec<Vec<f64>>
    }

    fn array_to_window(arr: Vec<Vec<f64>>) -> Vec<DataPoint> {
        let mut result = Vec::with_capacity(500);
        for i in 0..500 {
            let mut filtered_data = Vec::with_capacity(20);
            for channel in 0..20 {
                filtered_data.push(arr[channel][i]);
            }
            result.push(DataPoint {
                timestamp: 0.0,
                raw_data: vec![],
                filtered_data,
            });
        }
        result
    }

    fn array_to_ndarray(arr: Vec<Vec<f64>>) -> Array2<f64> {
        arr2(
            arr.into_iter()
                .map(|v| {
                    let mut res = [0.; 500];
                    res.clone_from_slice(v.as_slice());
                    res
                })
                .collect::<Vec<[f64; 500]>>()
                .as_slice(),
        )
    }

    #[test]
    fn process_window_should_work() {
        let mut processor = ProcessLSL::new(
            250,
            500,
            Path::new("csp_filters.json"),
            Path::new("svc.model"),
        );

        let data_encoded = std::fs::read_to_string(Path::new("test_data.json")).unwrap();
        let data: Data = serde_json::from_str(&data_encoded).unwrap();
        println!("parsed data");
        let mut preds = Vec::new();
        let mut preds_foreign_filter = Vec::new();
        for (index, x_i) in data.X_unfiltered.iter().enumerate() {
            let window = array_to_window(x_i.clone());
            let prediction = processor.process_window(&window).unwrap();
            preds.push(prediction);
        }
        // let mut filtered_data = FilteredData {X: vec![]};
        for (index, x_i) in data.X.iter().enumerate() {
            let window = array_to_ndarray(x_i.clone());
            // let mut filtered_window = window.clone();
            // processor.filter_matrix(&mut filtered_window);
            // filtered_data.X.push(filtered_window.flatten().to_vec());
            let prediction = processor.process_matrix(window).unwrap();
            preds_foreign_filter.push(prediction);
        }
        // let data_str = serde_json::to_string(&filtered_data).unwrap();
        // std::fs::write(Path::new("rust_filtered_data.json"), data_str);
        


        println!(
            "our filter {:#?}",
            zip(preds.iter(), data.y_pred.iter())
                .map(|(our, python)| (our == python) as i32)
                .sum::<i32>()
        );
        println!(
            "python filter {:#?}",
            zip(preds_foreign_filter.iter(), data.y_pred.iter())
                .map(|(our, python)| (our == python) as i32)
                .sum::<i32>()
        );
        assert!(false);
    }

use npy::NpyData;
use ndarray::arr3;

    #[test]
    fn abiba() {
        let mut processor = ProcessLSL::new(
            250,
            500,
            Path::new("csp_filters.json"),
            Path::new("svc.model"),
        );

        // let data_encoded = std::fs::read_to_string(Path::new("test_data.json")).unwrap();
        // let data: Data = serde_json::from_str(&data_encoded).unwrap();
        let mut buf = Vec::new();
        std::fs::File::open("X_unfiltered.npy").unwrap().read_to_end(&mut buf).unwrap();
        let data: Vec<f64> = NpyData::from_bytes(&buf).unwrap().to_vec();
        let arr = Array3::from_shape_vec((8640, 20, 500), data);
        println!("parsed data");
        let mut buf = Vec::new();

        // let mut filtered_data = FilteredData {X: vec![]};
        for x_i in arr.iter() {
            // let mut window = array_to_ndarray(x_i.clone());
            let mut window = x_i.to_owned();
            processor.filter_matrix(&mut window);
            buf.extend_from_slice(window.flatten().as_slice());
            // let prediction = processor.process_matrix(window).unwrap();
            // preds_foreign_filter.push(prediction);
        }
        let data = NpyData::from(&buf);
        npy::to_file("X_rust_filtered.npy", data).unwrap();

        // let data_str = serde_json::to_string(&filtered_data).unwrap();
        // std::fs::write(Path::new("rust_filtered_data.json"), data_str);
        


        // println!(
        //     "our filter {:#?}",
        //     zip(preds.iter(), data.y_pred.iter())
        //         .map(|(our, python)| (our == python) as i32)
        //         .sum::<i32>()
        // );
        // println!(
        //     "python filter {:#?}",
        //     zip(preds_foreign_filter.iter(), data.y_pred.iter())
        //         .map(|(our, python)| (our == python) as i32)
        //         .sum::<i32>()
        // );
        assert!(false);
    }
}
