#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Generic {0}")]
    Generic(String), // TODO: REMOVE THIS BEFORE COMITTING

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("Ingestion failed with: {0}")]
    IngestionError(String),

    #[error("Ingestion failed with: {0}")]
    OutputError(String),

    #[error("Engine failed with: {0}")]
    EngineError(String),
}
