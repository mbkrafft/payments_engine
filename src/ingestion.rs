use std::io::Read;
use std::pin::Pin;

use futures::stream::{self, Stream};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::domain::traits::TransactionStream;
use crate::domain::{Error, Transaction, TransactionKind};

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
    amount: Option<Decimal>,
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
                return Err(Error::Ingestion(format!(
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
                Err(e) => Err(Error::Ingestion(format!(
                    "CSV deserialization error: {}",
                    e
                ))),
            });

        Box::pin(stream::iter(iter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::io::Cursor;

    fn run_stream<R: Read + Send + 'static>(
        rdr: &mut CsvReader<R>,
    ) -> Vec<Result<Transaction, Error>> {
        let mut s = rdr.stream();
        futures::executor::block_on(async move {
            let mut out = Vec::new();
            while let Some(item) = s.next().await {
                out.push(item);
            }
            out
        })
    }

    #[test]
    fn parses_valid_rows_for_all_kinds() {
        let data = b"type, client, tx, amount\n\
deposit, 1, 1, 100.0\n\
withdrawal, 1, 2, 30.5\n\
dispute, 1, 1,\n\
resolve, 1, 1,\n\
chargeback, 1, 1,\n";

        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows = run_stream(&mut rdr);

        assert_eq!(rows.len(), 5);

        match &rows[0] {
            Ok(Transaction {
                kind: TransactionKind::Deposit { amount },
                client_id,
                transaction_id,
            }) => {
                assert_eq!(*client_id, 1);
                assert_eq!(*transaction_id, 1);
                assert!(*amount > rust_decimal::Decimal::ZERO);
            }
            other => panic!("unexpected: {:?}", other),
        }
        match &rows[1] {
            Ok(Transaction {
                kind: TransactionKind::Withdrawal { amount },
                client_id,
                transaction_id,
            }) => {
                assert_eq!(*client_id, 1);
                assert_eq!(*transaction_id, 2);
                assert!(*amount > rust_decimal::Decimal::ZERO);
            }
            other => panic!("unexpected: {:?}", other),
        }
        assert!(matches!(
            rows[2],
            Ok(Transaction {
                kind: TransactionKind::Dispute,
                ..
            })
        ));
        assert!(matches!(
            rows[3],
            Ok(Transaction {
                kind: TransactionKind::Resolve,
                ..
            })
        ));
        assert!(matches!(
            rows[4],
            Ok(Transaction {
                kind: TransactionKind::Chargeback,
                ..
            })
        ));
    }

    #[test]
    fn invalid_type_yields_ingestion_error() {
        let data = b"type, client, tx, amount\nfoo, 1, 1, 10.0\n";
        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows = run_stream(&mut rdr);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], Err(Error::Ingestion(_))));
    }

    #[test]
    fn missing_amount_for_deposit_is_error() {
        let data = b"type, client, tx, amount\ndeposit, 1, 1,\n";
        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows = run_stream(&mut rdr);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], Err(Error::Ingestion(_))));
    }

    #[test]
    fn extra_amount_for_dispute_is_error() {
        let data = b"type, client, tx, amount\ndispute, 1, 1, 2.0\n";
        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows = run_stream(&mut rdr);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], Err(Error::Ingestion(_))));
    }

    #[test]
    fn csv_deserialize_error_surfaces_as_ingestion_error() {
        // invalid client id (non-numeric)
        let data = b"type, client, tx, amount\ndeposit, a, 1, 1.0\n";
        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows = run_stream(&mut rdr);
        assert_eq!(rows.len(), 1);
        assert!(
            matches!(&rows[0], Err(Error::Ingestion(msg)) if msg.contains("CSV deserialization error"))
        );
    }

    #[test]
    fn second_stream_after_consumption_is_empty() {
        let data = b"type, client, tx, amount\ndeposit, 1, 1, 1.0\n";
        let cursor = Cursor::new(&data[..]);
        let mut rdr = CsvReader::new(cursor).expect("csv reader");
        let rows1 = run_stream(&mut rdr);
        assert_eq!(rows1.len(), 1);
        // Now attempt to stream again from the same CsvReader instance
        let rows2 = run_stream(&mut rdr);
        assert!(rows2.is_empty());
    }
}
