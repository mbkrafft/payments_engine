use futures::Stream;

use crate::domain::{Account, Error, Transaction};

pub trait TransactionStream {
    type TxStream: Stream<Item = Result<Transaction, Error>> + Send + Unpin + 'static;
    fn stream(&mut self) -> Self::TxStream;
}

pub trait DeadLetterQueue {
    fn report(&self, error: &Error);
}

pub trait OutputRepository {
    fn get_or_create_account(&mut self, client_id: &u16) -> &mut Account;
    fn flush(&mut self);

    fn report_transaction(
        &mut self,
        transaction_id: &u32,
        transaction: &Transaction,
    ) -> Result<(), Error>;

    fn get_transaction(&mut self, transaction_id: u32) -> Option<&Transaction>;

    fn mark_transaction_disputed(&mut self, transaction_id: u32);

    fn mark_transaction_resolved(&mut self, transaction_id: u32);

    fn has_dispute(&self, transaction_id: u32) -> bool;
}
