use futures::Stream;

use crate::domain::{Account, Error, Transaction};

pub trait TransactionStream {
    type TxStream: Stream<Item = Result<Transaction, Error>> + Send + Unpin + 'static;
    fn stream(&mut self) -> Self::TxStream;
}

pub trait DeadLetterQueue {
    fn report(&self, transaction: &Transaction, error: &Error);
}

pub trait OutputClient {
    fn record_update(&mut self, account_id: u16, account: &Account);
    fn flush(&mut self) -> Result<(), Error>;
}
