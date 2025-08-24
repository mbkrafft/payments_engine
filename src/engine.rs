use crate::domain::{
    Error, Transaction, TransactionKind,
    traits::{DeadLetterQueue, OutputRepository, TransactionStream},
};

use futures::StreamExt;

#[derive(Debug)]
pub struct Engine<I, O, D>
where
    I: TransactionStream,
    O: OutputRepository,
    D: DeadLetterQueue,
{
    ingestion: I,
    output_repository: O,
    dlq: D,
}

impl<I, O, D> Engine<I, O, D>
where
    I: TransactionStream,
    O: OutputRepository,
    D: DeadLetterQueue,
{
    pub fn new(ingestion: I, output_repository: O, dlq: D) -> Self {
        Self {
            ingestion,
            output_repository,
            dlq,
        }
    }

    pub async fn process(&mut self) -> Result<(), Error> {
        let mut res = self.ingestion.stream();

        while let Some(tx) = res.next().await {
            match tx {
                Ok(tx) => match self.apply_transaction(tx) {
                    Ok(()) => {}
                    Err(e) => self.dlq.report(&e),
                },
                Err(e) => self.dlq.report(&e),
            }
        }

        Ok(())
    }

    fn apply_transaction(&mut self, tx: Transaction) -> Result<(), Error> {
        {
            let account = self.output_repository.get_or_create_account(&tx.client_id);

            if account.locked {
                return Err(Error::Engine(
                    tx.client_id.to_string() + " account is locked",
                ));
            }
        }

        match tx.kind {
            TransactionKind::Deposit { amount } => self.deposit(&tx, amount),
            TransactionKind::Withdrawal { amount } => self.withraw(&tx, amount),
            TransactionKind::Dispute => self.dispute(&tx),
            TransactionKind::Resolve => self.resolve(&tx),
            TransactionKind::Chargeback => self.chargeback(tx),
        }
    }

    fn chargeback(&mut self, tx: Transaction) -> Result<(), Error> {
        let chargeback_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;
        if chargeback_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match resolve client ID".to_string(),
            ));
        }
        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            chargeback_tx.kind
        {
            // (Only if orig_tx was under dispute)
            let acct = self.output_repository.get_or_create_account(&tx.client_id);
            acct.available += amount;
            acct.held -= amount;
        }
        Ok(())
    }

    fn resolve(&mut self, tx: &Transaction) -> Result<(), Error> {
        let resolved_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;
        if resolved_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match resolve client ID".to_string(),
            ));
        }
        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            resolved_tx.kind
        {
            // (Only if orig_tx was under dispute)
            let acct = self.output_repository.get_or_create_account(&tx.client_id);
            acct.available += amount;
            acct.held -= amount;
        }
        Ok(())
    }

    fn dispute(&mut self, tx: &Transaction) -> Result<(), Error> {
        let disputed_tx = self
            .output_repository
            .get_transaction(tx.transaction_id)
            .ok_or_else(|| Error::Engine("Referenced transaction not found".to_string()))?;
        if disputed_tx.client_id != tx.client_id {
            return Err(Error::Engine(
                "Transaction client ID does not match dispute client ID".to_string(),
            ));
        }
        if let TransactionKind::Deposit { amount } | TransactionKind::Withdrawal { amount } =
            disputed_tx.kind
        {
            let acct = self.output_repository.get_or_create_account(&tx.client_id);
            acct.available -= amount;
            acct.held += amount;
        }
        Ok(())
    }

    fn withraw(&mut self, tx: &Transaction, amount: rust_decimal::Decimal) -> Result<(), Error> {
        match self
            .output_repository
            .report_transaction(&tx.transaction_id, tx)
        {
            Ok(_) => {
                let account = self.output_repository.get_or_create_account(&tx.client_id);

                if account.available < amount {
                    return Err(Error::Engine(
                        format!("Insufficient funds for client {}", tx.client_id).to_owned(),
                    ));
                }

                account.available -= amount;
                account.sync_total();

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn deposit(&mut self, tx: &Transaction, amount: rust_decimal::Decimal) -> Result<(), Error> {
        match self
            .output_repository
            .report_transaction(&tx.transaction_id, tx)
        {
            Ok(_) => {
                let account = self.output_repository.get_or_create_account(&tx.client_id);
                account.available += amount;
                account.sync_total();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn flush(&mut self) {
        self.output_repository.flush();
    }
}
