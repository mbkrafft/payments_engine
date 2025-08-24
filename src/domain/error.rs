#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("Ingestion failed with: {0}")]
    Ingestion(String),

    #[error("Engine failed with: {0}")]
    Engine(String),
}
