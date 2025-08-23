mod domain;
mod ingestion;

use std::{env, fs::File, path::Path};

use futures::StreamExt;

use crate::domain::{Error, traits::TransactionStream};

#[tokio::main] // using Tokio runtime for async
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the components
    let mut args = env::args();

    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let file = File::open(file_path)?;

    let ingestion = ingestion::CsvReader::new(file);

    let mut res = ingestion?.stream();
    while let Some(tx) = res.next().await {
        match tx {
            Ok(tx) => println!("{:?}", tx),
            Err(e) => eprintln!("Error processing transaction: {}", e),
        }
    }

    // let dlq = StdErrDLQ::new();
    // let output = CsvOutput::new();

    // // Initialize engine with injected components
    // let mut engine = Engine::new(ingestion, output, dlq);

    // engine.process_transactions().await?;

    // engine.flush();

    Ok(())
}
