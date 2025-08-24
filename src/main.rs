mod dlq;
mod domain;
mod engine;
mod ingestion;
mod output_repository;

use std::{env, fs::File, path::Path};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args();

    let file_path = args.nth(1).expect("No command line argument was provided");
    let file_path = Path::new(&file_path);
    let file = File::open(file_path)?;

    let ingestion = ingestion::CsvReader::new(file)?;
    let dlq = dlq::StdErrDLQ::default();
    let output = output_repository::StdOutOutput::new();

    let mut engine = engine::Engine::new(ingestion, output, dlq);

    engine.process().await?;
    engine.flush();

    Ok(())
}
