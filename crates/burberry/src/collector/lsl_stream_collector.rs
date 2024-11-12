use std::{collections::HashMap, sync::Arc, time::Duration};
use async_stream::stream;
use async_trait::async_trait;
use eyre::Result;
use lsl::{StreamInlet, resolve_bypred, Pullable};
use tokio::time::sleep;
use tracing::debug;

use crate::types::{Collector, CollectorStream};

#[derive(Clone, Debug)]
pub struct LSLData {
    pub timestamp: f64,
    pub data: Vec<f64>,
}

// pub struct LSLStreamCollector {
//     inlet: Arc<StreamInlet>,
// }
//
// impl LSLStreamCollector {
//     pub fn new(stream_name: &str, timeout_secs: f64) -> Result<Self> {
//         let inlet = Self::connect_to_stream(stream_name, timeout_secs)?;
//         Ok( Self {
//             inlet,
//         })
//     }
//
//     fn connect_to_stream(stream_name: &str, timeout: f64) -> Result<Arc<StreamInlet>> {
//         debug!("Looking for LSL stream: {}", stream_name);
//         
//         // Create a predicate to find stream by name
//         let pred = format!("name='{}'", stream_name);
//         
//         // Resolve the stream
//         let streams = resolve_bypred(&pred, 1, timeout)?;
//         
//         if streams.is_empty() {
//             return Err(eyre::eyre!("No LSL stream found with name: {}", stream_name));
//         }
//
//         let stream_info = &streams[0];
//         debug!(
//             "Found LSL stream: {} (hostname: {}, source_id: {})",
//             stream_name,
//             stream_info.hostname(),
//             stream_info.source_id()
//         );
//
//         // Create inlet with recovery enabled
//         let inlet = Arc::new(StreamInlet::new(stream_info, 10, 0, true)?);
//         Ok(inlet)
//     }
// }
//
// #[async_trait]
// impl Collector<LSLData> for LSLStreamCollector {
//     fn name(&self) -> &str {
//         "LSLStreamCollector"
//     }
//
//     async fn get_event_stream(&self) -> Result<CollectorStream<'_, LSLData>> {
//         let inlet = &self.inlet;
//
//         // Create an async stream that yields LSLData
//         let stream = stream! {
//             loop {
//                 match inlet.pull_sample(0.0) {
//                     Ok((sample, timestamp)) if sample.len() != 0 => {
//                         // debug!("Received sample {:?} at {}", sample, timestamp);
//                         yield LSLData {
//                             timestamp,
//                             data: sample,
//                         };
//                         // Add a small sleep to control sampling rate
//                         sleep(Duration::from_millis(0)).await;
//                     }
//                     _ => {sleep(Duration::from_millis(0)).await}
//                 }
//             }
//         };
//
//         Ok(Box::pin(stream))
//     }
// }

pub struct LSLStreamCollector2 {
    inlets: HashMap<&'static str, Arc<StreamInlet>>,
}

impl LSLStreamCollector2 {
    pub fn new(stream_names: Vec<&'static str>, timeout_secs: f64) -> Result<Self> {
        let mut inlets = HashMap::new();
        for name in stream_names {
            let inlet = Self::connect_to_stream(name, timeout_secs)?;
            inlets.insert(name, inlet);
        }
        Ok(Self { inlets })
    }

    fn connect_to_stream(stream_name: &str, timeout: f64) -> Result<Arc<StreamInlet>> {
        debug!("Looking for LSL stream: {}", stream_name);
        
        // Create a predicate to find stream by name
        let pred = format!("name='{}'", stream_name);
        
        // Resolve the stream
        let streams = resolve_bypred(&pred, 1, timeout)?;
        
        if streams.is_empty() {
            return Err(eyre::eyre!("No LSL stream found with name: {}", stream_name));
        }

        let stream_info = &streams[0];
        debug!(
            "Found LSL stream: {} (hostname: {}, source_id: {})",
            stream_name,
            stream_info.hostname(),
            stream_info.source_id()
        );

        // Create inlet with recovery enabled
        let inlet = Arc::new(StreamInlet::new(stream_info, 10, 0, true)?);
        Ok(inlet)
    }
}

#[async_trait]
impl Collector<(&'static str, LSLData)> for LSLStreamCollector2 {
    fn name(&self) -> &str {
        "LSLStreamCollector"
    }

    async fn get_event_stream(&self) -> Result<CollectorStream<'_, (&'static str, LSLData)>> {

        // Create an async stream that yields LSLData
        let stream = stream! {
            loop {
                for (name, inlet) in self.inlets.iter() {
                    match inlet.pull_sample(0.0) {
                        Ok((sample, timestamp)) if sample.len() != 0 => {
                            // debug!("Received sample {:?} at {}", sample, timestamp);
                            yield (*name, LSLData {
                                timestamp,
                                data: sample,
                            });
                            // Add a small sleep to control sampling rate
                        }
                        _ => {}
                    }
                    sleep(Duration::from_millis(0)).await;
                };
            }
        };

        Ok(Box::pin(stream))
    }
}

