use std::io::Read;
use std::pin::Pin;

use futures::stream::{self, Stream};
use serde::Deserialize;

use crate::domain::traits::TransactionStream;
use crate::domain::{Error, Money, Transaction, TransactionKind};

pub struct CsvReader<R: Read> {
    reader: Option<csv::Reader<R>>,
}

impl<R: Read> CsvReader<R> {
    pub fn new(reader: R) -> Result<Self, Error> {
        let rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(reader);

        Ok(Self { reader: Some(rdr) })
    }
}

/// Internal shape used only for CSV deserialization.
#[derive(Debug, Deserialize)]
struct CsvRow {
    #[serde(rename = "type")]
    kind: String,
    client: u16,
    tx: u32,
    amount: Option<Money>,
}

impl TryFrom<CsvRow> for Transaction {
    type Error = Error;

    fn try_from(row: CsvRow) -> Result<Self, Self::Error> {
        let kind = match (row.kind.trim().to_ascii_lowercase().as_str(), row.amount) {
            ("deposit", Some(amount)) => TransactionKind::Deposit { amount },
            ("withdrawal", Some(amount)) => TransactionKind::Withdrawal { amount },
            ("dispute", None) => TransactionKind::Dispute,
            ("resolve", None) => TransactionKind::Resolve,
            ("chargeback", None) => TransactionKind::Chargeback,
            (other, _) => {
                return Err(Error::IngestionError(format!(
                    "Invalid transaction type: {}",
                    other
                )));
            }
        };

        Ok(Transaction {
            kind,
            client_id: row.client,
            transaction_id: row.tx,
        })
    }
}

impl<R: Read + Send + 'static> TransactionStream for CsvReader<R> {
    type TxStream = Pin<Box<dyn Stream<Item = Result<Transaction, Error>> + Send>>;

    fn stream(&mut self) -> Self::TxStream {
        // Take ownership of the reader so the iterator we build owns all data and is 'static.
        let reader = match self.reader.take() {
            Some(r) => r,
            None => {
                // Already consumed; return an empty stream.
                return Box::pin(stream::iter(Vec::<Result<Transaction, Error>>::new()));
            }
        };

        // into_deserialize consumes the reader and returnes an owning iterator
        let iter = reader
            .into_deserialize::<CsvRow>()
            .map(|row_res| match row_res {
                Ok(row) => Transaction::try_from(row),
                Err(e) => Err(Error::IngestionError(format!(
                    "CSV deserialization error: {}",
                    e
                ))),
            });

        Box::pin(stream::iter(iter))
    }
}
