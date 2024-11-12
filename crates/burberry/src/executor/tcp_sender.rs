use crate::Executor;
use eyre::Result;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tracing::debug;
use std::fmt::Debug;

pub struct TCPSender {
    addr: String,
}

impl TCPSender {
    pub fn new(addr: impl Into<String>) -> Self {
        Self {
            addr: addr.into(),
        }
    }
}

#[async_trait::async_trait]
impl<A> Executor<A> for TCPSender 
where 
    A: Send + Sync + Serialize + Debug + 'static,
{
    fn name(&self) -> &str {
        "TCPSender"
    }

    async fn execute(&self, action: A) -> Result<()> {
        // Serialize the action to JSON
        debug!("Preparing to send data {:?}", action);
        let serialized = serde_json::to_vec(&action)?;
        
        // Connect to the TCP endpoint
        let mut stream = TcpStream::connect(&self.addr).await?;
        debug!("Connected to stream");
        
        // Send the serialized data
        stream.write_all(&serialized).await?;
        debug!("Sent data");
        stream.flush().await?;
        
        Ok(())
    }
}
